use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileCoord;

use crate::iso::world_pos_to_tile_coord;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::hud::HoveredTileCoord;
use crate::ui::save_window::SaveWindowState;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::toolbar::minimap::minimap_contains_cursor;
use crate::ui::toolbar::minimap::{MinimapCell, MinimapRoot};

/// Actualiza la tesela bajo el cursor (preview, órdenes). No modifica la selección por clic.
pub(crate) fn update_cursor_tile(
    save_window: Option<Res<SaveWindowState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &Transform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    sim: Res<SimWorld>,
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
    let cam_global = GlobalTransform::from(*cam_tf);
    let Ok(world_pos) = camera.viewport_to_world_2d(&cam_global, cursor_pos) else {
        return;
    };
    hovered.pos =
        world_pos_to_tile_coord(world_pos, &sim.state.map).map(|(tx, ty)| TileCoord::new(tx, ty));
}
