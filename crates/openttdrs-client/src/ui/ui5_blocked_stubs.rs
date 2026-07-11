//! Stub UI-5 para Link Graph (bloqueado por backend CargoDist).
//! SignList pasó a `sign_list_window` (UI-6b).

use bevy::prelude::*;

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
}

#[derive(Component)]
pub(crate) struct LinkGraphBodyText;

pub(crate) fn setup_link_graph_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::LinkGraphLegend,
        "Link Graph",
        TITLE_BROWN,
        Vec2::new(440.0, 180.0),
        380.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            LinkGraphBodyText,
            Text::new(link_graph_stub_text()),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
    });
}

fn link_graph_stub_text() -> &'static str {
    "Leyenda Link Graph / CargoDist\n\n\
     Bloqueado: no hay módulo linkgraph ni flujos\n\
     estación→estación en la simulación.\n\n\
     Disponible hoy: gráficos económicos\n\
     (Economía → Ingresos / Beneficio / Valor)."
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

pub(crate) fn sync_link_graph_window(
    state: Res<LinkGraphWindowState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
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
}
