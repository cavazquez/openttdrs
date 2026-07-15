//! Ventana Link Graph (flujos estación→estación observados).

use bevy::prelude::*;
use openttdrs_core::{ALL_CARGO_TYPES, CargoType};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct LinkGraphWindowState {
    pub(crate) open: bool,
    /// Filtro de cargo (`None` = todos). Cicla con el botón Filtrar.
    pub(crate) cargo_filter: Option<CargoType>,
}

#[derive(Component)]
pub(crate) struct LinkGraphBodyText;

#[derive(Component)]
pub(crate) struct LinkGraphFilterButton;

pub(crate) fn setup_link_graph_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::LinkGraphLegend,
        "Link Graph",
        TITLE_BROWN,
        Vec2::new(480.0, 320.0),
        440.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn((
                Button,
                LinkGraphFilterButton,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.35, 0.3, 0.22)),
                BuildMenuUi,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Filtro: todos"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                    BuildMenuUi,
                ));
            });
        panel.spawn((
            LinkGraphBodyText,
            Text::new(link_graph_empty_text()),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
    });
}

fn link_graph_empty_text() -> &'static str {
    "Sin flujos observados aún.\n\n\
     Se registran al cargar en una estación y\n\
     descargar/transferir en otra (mismo cargo).\n\n\
     Modo de distribución: Ajustes → Distribución de carga…"
}

fn cargo_label(c: CargoType) -> &'static str {
    match c {
        CargoType::Passengers => "Pax",
        CargoType::Mail => "Mail",
        CargoType::Goods => "Goods",
        CargoType::Coal => "Coal",
        CargoType::Wood => "Wood",
        CargoType::Oil => "Oil",
        CargoType::Livestock => "Live",
        CargoType::Grain => "Grain",
        CargoType::IronOre => "Ore",
        CargoType::Steel => "Steel",
        CargoType::Valuables => "Val",
    }
}

fn filter_label(filter: Option<CargoType>) -> String {
    match filter {
        None => "Filtro: todos".into(),
        Some(c) => format!("Filtro: {}", cargo_label(c)),
    }
}

fn next_cargo_filter(current: Option<CargoType>) -> Option<CargoType> {
    match current {
        None => Some(ALL_CARGO_TYPES[0]),
        Some(c) => {
            let idx = ALL_CARGO_TYPES.iter().position(|x| *x == c)?;
            ALL_CARGO_TYPES.get(idx + 1).copied()
        }
    }
}

fn format_link_graph_body(sim: &SimWorld, filter: Option<CargoType>) -> String {
    let edges = sim.state.link_graph.top_edges_filtered(filter, 24);
    if edges.is_empty() {
        return link_graph_empty_text().to_string();
    }
    let mut lines = vec![
        "Flujos observados (mes / total)".to_string(),
        "Leyenda: intensidad ≈ units_month".to_string(),
        String::new(),
    ];
    for (key, sample) in edges {
        lines.push(format!(
            "({},{})→({},{}) {}  {}/{}",
            key.from.x,
            key.from.y,
            key.to.x,
            key.to.y,
            cargo_label(key.cargo),
            sample.units_month,
            sample.units_total,
        ));
    }
    lines.join("\n")
}

pub(crate) fn open_link_graph_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut link: ResMut<LinkGraphWindowState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::LinkGraph {
            link.open = true;
        }
    }
}

pub(crate) fn handle_link_graph_filter_button(
    mut q: Query<(&Interaction, &Children), (Changed<Interaction>, With<LinkGraphFilterButton>)>,
    mut text_q: Query<&mut Text>,
    mut state: ResMut<LinkGraphWindowState>,
) {
    for (interaction, children) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.cargo_filter = next_cargo_filter(state.cargo_filter);
        let label = filter_label(state.cargo_filter);
        for child in children {
            if let Ok(mut text) = text_q.get_mut(*child) {
                **text = label.clone();
            }
        }
    }
}

pub(crate) fn sync_link_graph_window(
    state: Res<LinkGraphWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut body_q: Query<&mut Text, With<LinkGraphBodyText>>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::LinkGraphLegend)
    else {
        return;
    };
    *vis = if state.open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !state.open {
        return;
    }
    if let Ok(mut text) = body_q.single_mut() {
        **text = format_link_graph_body(&sim, state.cargo_filter);
    }
}

pub(crate) fn link_graph_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<LinkGraphWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::LinkGraphLegend {
            state.open = false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn route_opens_link_graph() {
        let mut world = World::new();
        world.init_resource::<LinkGraphWindowState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::LinkGraph));
        world.run_system_once(open_link_graph_from_routes).unwrap();
        assert!(world.resource::<LinkGraphWindowState>().open);
    }

    #[test]
    fn filter_cycles_through_cargos() {
        let mut f = None;
        // None → 11 cargos → None
        for _ in 0..=ALL_CARGO_TYPES.len() {
            f = next_cargo_filter(f);
        }
        assert_eq!(f, None);
    }
}
