//! Fantasma de puente: eje según el tramo y sprite distinto en rampas vs vano.

use bevy::prelude::*;

use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::ui::toolbar::BuildMenuAction;

use super::BuildGhostPreview;

/// Eje Y del puente (vía vertical en mapa) a partir del tramo de teselas.
#[must_use]
pub fn bridge_span_axis_y(tiles: &[(i32, i32)]) -> bool {
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    if sx == ex && sy == ey {
        return false;
    }
    (ex - sx).abs() < (ey - sy).abs()
}

/// Ruta del PNG de preview (solo madera por ahora; tipo en `m6` al construir).
#[must_use]
pub fn bridge_preview_sprite(
    is_rail: bool,
    axis_y: bool,
    index: usize,
    total: usize,
) -> &'static str {
    let is_middle = total > 2 && index > 0 && index + 1 < total;
    match (is_rail, axis_y, is_middle) {
        (true, true, true) => "assets/opengfx/tiles/bridge_wood_y_front.png",
        (true, false, true) => "assets/opengfx/tiles/bridge_wood_x_front.png",
        (true, true, false) => "assets/opengfx/tiles/bridge_wood_rail_y.png",
        (true, false, false) => "assets/opengfx/tiles/bridge_wood_rail_x.png",
        (false, true, true) => "assets/opengfx/tiles/bridge_wood_y_front.png",
        (false, false, true) => "assets/opengfx/tiles/bridge_wood_x_front.png",
        (false, true, false) => "assets/opengfx/tiles/bridge_wood_road_y.png",
        (false, false, false) => "assets/opengfx/tiles/bridge_wood_road_x.png",
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_bridge_span_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
    map: &openttdrs_core::Map,
    valid: bool,
) {
    let is_rail = action == BuildMenuAction::RailBridge;
    let axis_y = bridge_span_axis_y(tiles);
    let total = tiles.len();
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.58)
    } else {
        Color::srgba(1.0, 0.28, 0.22, 0.58)
    };
    for (index, &(px, py)) in tiles.iter().enumerate() {
        let coord = openttdrs_core::TileCoord::new(px, py);
        if map.get(coord).is_none() {
            continue;
        }
        let (tileh, base_z) = tile_slope_and_min_z(map, px as u32, py as u32);
        let half_h = if tileh == 0 {
            TILE_HALF_H
        } else {
            SLOPE_HALF_H[tileh as usize]
        };
        let path = bridge_preview_sprite(is_rail, axis_y, index, total);
        let layer = if total > 2 && index > 0 && index + 1 < total {
            0.045
        } else {
            0.04
        };
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: asset_server.load::<Image>(path),
                color: tint,
                ..default()
            },
            Transform::from_translation(tile_pos_half(px, py, base_z, layer, half_h))
                .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));
    }
}
