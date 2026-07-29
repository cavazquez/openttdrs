//! Stub Industry Production graph (`WC_INDUSTRY_PRODUCTION`) — hija de Industry (#269).

use bevy::prelude::*;
use openttdrs_core::cargo_display_name;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CREAM,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct IndustryProductionWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct IndustryProductionBodyText;

pub(crate) fn setup_industry_production_window(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::IndustryProduction,
        "Producción industria",
        TITLE_CREAM,
        Vec2::new(100.0, 200.0),
        260.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            IndustryProductionBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_industry_production_window(
    state: Res<IndustryProductionWindowState>,
    panel: Res<IndustryPanelState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<IndustryProductionBodyText>>,
    mut body_q: Query<
        &mut Text,
        (
            With<IndustryProductionBodyText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::IndustryProduction)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let Some(focus) = panel.focus_tile else {
        if let Ok(mut body) = body_q.single_mut() {
            **body = "Sin industria seleccionada.".into();
        }
        return;
    };
    let Some(ind) = sim
        .state
        .industries
        .iter()
        .find(|i| i.tiles.contains(&focus) || i.pos == focus)
    else {
        if let Ok(mut body) = body_q.single_mut() {
            **body = "Industria no encontrada.".into();
        }
        return;
    };

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(tt, _)| tt.0 == FloatingWindowId::IndustryProduction)
    {
        **title = format!("Producción — {:?}", ind.kind);
    }

    let cargos: Vec<&str> = ind
        .produced_cargos()
        .iter()
        .map(|c| cargo_display_name(*c))
        .collect();
    let cargo_line = if cargos.is_empty() {
        "(sin cargo producido)".into()
    } else {
        cargos.join(", ")
    };
    if let Ok(mut body) = body_q.single_mut() {
        **body = format!(
            "Nivel prod: {}\nRate ciclo: {}\nProducido total: {}\nCargas: {cargo_line}\n\
             Stub — gráfico mensual 15.3 residual (#269).",
            ind.prod_level,
            ind.produce_amount(),
            ind.produced_total,
        );
    }
}

pub(crate) fn industry_production_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<IndustryProductionWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::IndustryProduction
            || msg.0.class == FloatingWindowId::Industry
        {
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
    fn on_closed_clears_production_state() {
        let mut world = World::new();
        world.init_resource::<IndustryProductionWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.resource_mut::<IndustryProductionWindowState>().open = true;
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::IndustryProduction),
        ));
        world
            .run_system_once(industry_production_window_on_closed)
            .unwrap();
        assert!(!world.resource::<IndustryProductionWindowState>().open);
    }

    #[test]
    fn parent_industry_closed_closes_production() {
        let mut world = World::new();
        world.init_resource::<IndustryProductionWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.resource_mut::<IndustryProductionWindowState>().open = true;
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::Industry),
        ));
        world
            .run_system_once(industry_production_window_on_closed)
            .unwrap();
        assert!(!world.resource::<IndustryProductionWindowState>().open);
    }
}
