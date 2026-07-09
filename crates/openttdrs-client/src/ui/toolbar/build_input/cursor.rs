use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileCoord;

use crate::iso::{world_pos_to_rail_signal_pick, world_pos_to_tile_coord, world_pos_to_tile_fract};
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::hud::HoveredTileCoord;
use crate::ui::save_window::SaveWindowState;
use crate::ui::toolbar::minimap::minimap_contains_cursor;
use crate::ui::toolbar::minimap::{MinimapCell, MinimapRoot};
use crate::ui::toolbar::{BuildMenuAction, BuildMenuUi, UiToolState};

/// Actualiza la tesela bajo el cursor (preview, órdenes). No modifica la selección por clic.
pub(crate) fn update_cursor_tile(
    save_window: Option<Res<SaveWindowState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    sim: Res<SimWorld>,
    tool_state: Res<UiToolState>,
    mut hovered: ResMut<HoveredTileCoord>,
    toolbar_pointer: Query<
        &Interaction,
        (
            With<BuildMenuUi>,
            Without<MinimapRoot>,
            Without<MinimapCell>,
        ),
    >,
) {
    hovered.pos = None;
    hovered.fract_x = 128;
    hovered.fract_y = 128;
    if save_window.is_some_and(|w| w.open) {
        return;
    }
    if toolbar_pointer.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    if minimap_contains_cursor(cursor_pos, window) {
        return;
    }
    let Ok((camera, cam_tf)) = cam_q.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_tf, cursor_pos) else {
        return;
    };

    if tool_state.active_tool == Some(BuildMenuAction::RailSignals) {
        if let Some((tx, ty, fx, fy)) = world_pos_to_rail_signal_pick(world_pos, &sim.state.map) {
            hovered.pos = Some(TileCoord::new(tx, ty));
            hovered.fract_x = fx;
            hovered.fract_y = fy;
        }
        return;
    }

    if tool_state.active_tool == Some(BuildMenuAction::Clear)
        && let Some((tx, ty, fx, fy)) = world_pos_to_rail_signal_pick(world_pos, &sim.state.map)
    {
        let c = TileCoord::new(tx, ty);
        if let Some(tile) = sim.state.map.get(c)
            && tile.kind == openttdrs_core::TileKind::Rail
            && openttdrs_core::rail_signals::rail_tile_is_signals(tile.m5)
        {
            hovered.pos = Some(c);
            hovered.fract_x = fx;
            hovered.fract_y = fy;
            return;
        }
    }

    hovered.pos =
        world_pos_to_tile_coord(world_pos, &sim.state.map).map(|(tx, ty)| TileCoord::new(tx, ty));
    if let Some(pos) = hovered.pos {
        let (fx, fy) = world_pos_to_tile_fract(world_pos, &sim.state.map, pos.x, pos.y);
        hovered.fract_x = fx;
        hovered.fract_y = fy;
    }
}
