//! Inspector de tile seleccionado (UI-8). Dump estructurado sin Action0–14.

use bevy::math::Isometry2d;
use bevy::prelude::*;
use openttdrs_core::TileCoord;

use crate::config;
use crate::iso::{gizmo_diamond, iso};
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::SelectedTileInfo;
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct TileInspectorWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct TileInspectorBodyText;

pub(crate) fn setup_tile_inspector_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::TileInspector,
        "Inspector de tile",
        TITLE_BROWN,
        Vec2::new(40.0, 120.0),
        380.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Selecciona un tile (clic) · F2 abre/cierra · gizmos marcan bounds"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            TileInspectorBodyText,
            Text::new("(sin selección)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            BuildMenuUi,
        ));
    });
}

pub(crate) fn sync_tile_inspector_window(
    state: Res<TileInspectorWindowState>,
    selected: Res<SelectedTileInfo>,
    sim: Option<Res<SimWorld>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut body_q: Query<&mut Text, With<TileInspectorBodyText>>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::TileInspector {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        return;
    }
    let dump = match (selected.pos, sim.as_deref()) {
        (Some(pos), Some(sim)) => format_tile_inspect(&sim.state, pos),
        (None, _) => "(sin selección — clic en el mapa)".into(),
        (Some(pos), None) => format!("({},{}) — sin SimWorld", pos.x, pos.y),
    };
    for mut text in &mut body_q {
        **text = dump.clone();
    }
}

pub(crate) fn handle_tile_inspector_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<TileInspectorWindowState>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
) {
    if console.is_some_and(|c| crate::ui::dev_console::dev_console_captures_keyboard(&c)) {
        return;
    }
    if keyboard.just_pressed(KeyCode::F2) {
        state.open = !state.open;
    }
}

pub(crate) fn tile_inspector_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<TileInspectorWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::TileInspector {
            state.open = false;
        }
    }
}

/// Sprite aligner lite: diamante + coords en el tile seleccionado (con gizmos ON).
pub(crate) fn draw_selected_tile_bounds(
    selected: Res<SelectedTileInfo>,
    prefs: Res<ClientPreferences>,
    mut gizmos: Gizmos,
) {
    if !(prefs.show_debug_gizmos || config::env_flag("OPENTTDRS_GIZMOS")) {
        return;
    }
    let Some(pos) = selected.pos else {
        return;
    };
    let center = iso(pos.x, pos.y);
    gizmo_diamond(&mut gizmos, center, 34.0, 16.0, Color::srgb(1.0, 0.35, 0.2));
    gizmo_diamond(&mut gizmos, center, 18.0, 8.0, Color::srgb(1.0, 0.85, 0.2));
    let label = format!("sel ({},{})", pos.x, pos.y);
    gizmos.text_2d(
        Isometry2d::from_translation(center + Vec2::new(0.0, 22.0)),
        &label,
        11.0,
        Vec2::ZERO,
        Color::srgb(1.0, 0.9, 0.6),
    );
}

/// Dump estructurado reutilizable (consola / ventana).
#[must_use]
pub(crate) fn format_tile_inspect(state: &openttdrs_core::GameState, pos: TileCoord) -> String {
    let Some(tile) = state.map.get(pos) else {
        return format!("({},{}) fuera de mapa", pos.x, pos.y);
    };
    let mut lines = vec![
        format!("pos ({}, {})", pos.x, pos.y),
        format!("kind {:?}", tile.kind),
        format!(
            "height {}  slope/m1 {:02X}  mapt {:02X}",
            tile.height, tile.m1, tile.mapt
        ),
        format!(
            "m2={:02X} m3={:02X} m3hi={:02X} m5={:02X} m7={:02X}",
            tile.m2, tile.m3, tile.m3hi, tile.m5, tile.m7
        ),
    ];
    if let Some(st) = state.stations.iter().find(|s| s.pos == pos) {
        let name = st.name.as_deref().unwrap_or("(sin nombre)");
        lines.push(format!("estación {:?} «{name}»", st.stop_kind));
    }
    if let Some(ind) = state.industries.iter().find(|i| i.pos == pos) {
        lines.push(format!(
            "industria {:?} stock {}/{}",
            ind.kind, ind.stock, ind.capacity
        ));
    }
    if let Some(veh) = state.vehicles.iter().find(|v| v.pos == pos) {
        lines.push(format!(
            "vehículo #{} {:?} dir {:?}",
            veh.id, veh.kind, veh.direction
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::prelude::*;

    #[test]
    fn format_tile_inspect_reports_kind() {
        let mut state = GameState::new(8, 8);
        let pos = TileCoord { x: 2, y: 3 };
        state.map.set_kind(pos, TileKind::Road).unwrap();
        state.map.set_height(pos, 1).unwrap();
        let text = format_tile_inspect(&state, pos);
        assert!(text.contains("Road"));
        assert!(text.contains("(2, 3)"));
    }
}
