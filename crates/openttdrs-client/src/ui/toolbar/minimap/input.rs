use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileCoord;

use crate::iso::tile_pos;
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;
use crate::ui::toolbar::BuildMenuUi;

use super::{MINIMAP_BOTTOM, MINIMAP_CELL, MINIMAP_COLS, MINIMAP_PAD, MINIMAP_RIGHT, MINIMAP_ROWS};

pub(crate) fn handle_minimap_click(
    mouse: Res<ButtonInput<MouseButton>>,
    hud: Res<SimHudControls>,
    windows: Query<&Window, With<PrimaryWindow>>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
) {
    if !hud.minimap_visible || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if menu_pointer.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some((tile_x, tile_y)) = cursor_to_minimap_tile(cursor, window, sim.state.map.dimensions())
    else {
        return;
    };
    let coord = TileCoord::new(tile_x, tile_y);
    let height = sim.state.map.get(coord).map_or(0, |tile| tile.height);
    let pos = tile_pos(tile_x, tile_y, height, 0.0);
    let Ok(mut tf) = cam_q.single_mut() else {
        return;
    };
    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
}

fn cursor_to_minimap_tile(
    cursor: Vec2,
    window: &Window,
    dimensions: (u32, u32),
) -> Option<(i32, i32)> {
    let (mw, mh) = dimensions;
    if mw == 0 || mh == 0 {
        return None;
    }
    let total_w = MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
    let left = window.width() - MINIMAP_RIGHT - total_w;
    let total_h = MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
    let top = window.height() - MINIMAP_BOTTOM - total_h;
    let local_x = cursor.x - left - MINIMAP_PAD;
    let local_y_from_top = cursor.y - top - MINIMAP_PAD;
    if local_x < 0.0
        || local_y_from_top < 0.0
        || local_x >= MINIMAP_COLS as f32 * MINIMAP_CELL
        || local_y_from_top >= MINIMAP_ROWS as f32 * MINIMAP_CELL
    {
        return None;
    }
    let col = (local_x / MINIMAP_CELL).floor() as u32;
    let row = (local_y_from_top / MINIMAP_CELL).floor() as u32;
    let x = ((MINIMAP_COLS.saturating_sub(1).saturating_sub(col)) * mw / MINIMAP_COLS) as i32;
    let y = (row * mh / MINIMAP_ROWS) as i32;
    Some((x, y))
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::super::{
        MINIMAP_BOTTOM, MINIMAP_CELL, MINIMAP_COLS, MINIMAP_PAD, MINIMAP_RIGHT, MINIMAP_ROWS,
    };
    use super::cursor_to_minimap_tile;

    #[test]
    fn cursor_to_minimap_tile_top_left_maps_to_small_coords() {
        let window = Window {
            resolution: (200_u32, 150_u32).into(),
            ..default()
        };
        let total_w = MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
        let left = window.width() - MINIMAP_RIGHT - total_w;
        let total_h = MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
        let top = window.height() - MINIMAP_BOTTOM - total_h;
        let cursor = Vec2::new(left + MINIMAP_PAD + 1.0, top + MINIMAP_PAD + 1.0);
        assert!(cursor_to_minimap_tile(cursor, &window, (64, 40)).is_some());
    }
}
