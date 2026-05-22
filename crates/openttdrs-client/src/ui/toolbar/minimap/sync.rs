use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Map, TileCoord, TileKind};

use crate::iso::{ISO_HW, ISO_QH, world_pos_to_tile_coord};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;

use super::palette::minimap_color;
use super::{
    MINIMAP_CELL, MINIMAP_COLS, MINIMAP_PAD, MINIMAP_ROWS, MinimapCell, MinimapRoot,
    MinimapViewport,
};

pub(crate) fn sync_minimap(
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    mut root_q: Query<&mut Visibility, With<MinimapRoot>>,
    mut cells: Query<(&MinimapCell, &mut BackgroundColor)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    mut viewport_q: Query<&mut Node, With<MinimapViewport>>,
) {
    if let Ok(mut vis) = root_q.single_mut() {
        *vis = if hud.minimap_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !hud.minimap_visible {
        return;
    }
    let (mw, mh) = sim.state.map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    for (cell, mut bg) in &mut cells {
        let x = (MINIMAP_COLS.saturating_sub(1).saturating_sub(cell.col)) * mw / MINIMAP_COLS;
        let y = cell.row * mh / MINIMAP_ROWS;
        let c = TileCoord::new(x as i32, y as i32);
        *bg = BackgroundColor(minimap_color(
            sim.state.map.get_kind(c).unwrap_or(TileKind::Void),
        ));
    }

    update_minimap_viewport(&sim.state.map, &windows, &cam_q, &mut viewport_q);
}

fn update_minimap_viewport(
    map: &Map,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_q: &Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    viewport_q: &mut Query<&mut Node, With<MinimapViewport>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((cam_tf, projection)) = cam_q.single() else {
        return;
    };
    let Projection::Orthographic(proj) = projection else {
        return;
    };
    let (mw, mh) = map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    let center_world = Vec2::new(cam_tf.translation.x, cam_tf.translation.y);
    let Some((cx, cy)) = world_pos_to_tile_coord(center_world, map) else {
        return;
    };

    let half_w = window.width() * proj.scale * 0.5;
    let half_h = window.height() * proj.scale * 0.5;
    let half_tiles = (0.5 * (half_w / ISO_HW + half_h / ISO_QH)).max(2.0);
    let half_tiles_x = half_tiles.clamp(2.0, mw as f32);
    let half_tiles_y = half_tiles.clamp(2.0, mh as f32);

    let min_x = (cx as f32 - half_tiles_x).clamp(0.0, mw.saturating_sub(1) as f32);
    let max_x = (cx as f32 + half_tiles_x).clamp(0.0, mw.saturating_sub(1) as f32);
    let min_y = (cy as f32 - half_tiles_y).clamp(0.0, mh.saturating_sub(1) as f32);
    let max_y = (cy as f32 + half_tiles_y).clamp(0.0, mh.saturating_sub(1) as f32);
    let left_min = MINIMAP_PAD
        + ((mw as f32 - 1.0 - max_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL);
    let left_max = MINIMAP_PAD
        + ((mw as f32 - 1.0 - min_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL);
    let top_min = MINIMAP_PAD + (min_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL);
    let top_max = MINIMAP_PAD + (max_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL);
    let width = (left_max - left_min).max(3.0);
    let height = (top_max - top_min).max(3.0);
    let Ok(mut node) = viewport_q.single_mut() else {
        return;
    };
    node.left = Val::Px(left_min);
    node.top = Val::Px(top_min);
    node.width = Val::Px(width);
    node.height = Val::Px(height);
}
