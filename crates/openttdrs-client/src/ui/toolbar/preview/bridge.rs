//! Fantasma de puente: eje según el tramo y sprite distinto en rampas vs vano.

use bevy::prelude::*;
use openttdrs_core::{BridgeType, calc_bridge_piece};

use crate::iso::{HEIGHT_PX, iso, remap_tile_offset, tile_slope_and_min_z};
use crate::sprites::{BridgeDeckSpriteIds, bridge_deck_sprite_ids, bridge_sprite_meta};
use crate::ui::toolbar::BuildMenuAction;

use super::BuildGhostPreview;

const DECK_LAYER: f32 = 0.04;
const FRONT_LAYER: f32 = 0.045;

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

/// Desplazamiento del sprite front (misma lógica que `spawn_bridge_deck`).
fn bridge_front_shift(axis: usize) -> Vec2 {
    if axis == 0 {
        remap_tile_offset(0.0, 12.0, 0.0) * 0.5
    } else {
        remap_tile_offset(12.0, 0.0, 0.0) * 0.5
    }
}

/// Posición en pantalla con offsets NFO, como `spawn_layer` en `bridge_draw.rs`.
fn bridge_ghost_translation(
    px: i32,
    py: i32,
    base_z: u8,
    sprite_id: u32,
    shift: Vec2,
    layer: f32,
) -> Vec3 {
    let (w, h, xrel, yrel) = bridge_sprite_meta(sprite_id).unwrap_or((64.0, 32.0, -32.0, -16.0));
    let iso_pos = iso(px, py);
    let z_px = f32::from(base_z) * HEIGHT_PX;
    Vec3::new(
        iso_pos.x + shift.x + xrel + w / 2.0,
        iso_pos.y + shift.y - yrel - h / 2.0 + z_px,
        (px + py) as f32 * 0.01 + f32::from(base_z) * 0.001 + layer,
    )
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
    let axis = usize::from(axis_y);
    let total = tiles.len();
    let bridge_type = BridgeType::Wooden;
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
        let (_, base_z) = tile_slope_and_min_z(map, px as u32, py as u32);
        let north_len = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let south_len = u32::try_from(total.saturating_sub(index)).unwrap_or(u32::MAX);
        let piece = calc_bridge_piece(north_len, south_len);
        let is_middle = total > 2 && index > 0 && index + 1 < total;
        let ids = bridge_deck_sprite_ids(bridge_type, piece);
        let (sprite_id, shift, layer) = if is_middle {
            (ids.front[axis], bridge_front_shift(axis), FRONT_LAYER)
        } else {
            (ids.rear(is_rail, axis), Vec2::ZERO, DECK_LAYER)
        };
        let path = format!(
            "assets/opengfx/tiles/{}",
            BridgeDeckSpriteIds::atlas_name(sprite_id)
        );
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: asset_server.load(path),
                color: tint,
                ..default()
            },
            Transform::from_translation(bridge_ghost_translation(
                px, py, base_z, sprite_id, shift, layer,
            ))
            .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::bridge_span_axis_y;

    #[test]
    fn bridge_axis_y_when_span_runs_north_south() {
        let tiles = vec![(3, 2), (3, 3), (3, 4), (3, 5)];
        assert!(bridge_span_axis_y(&tiles));
    }

    #[test]
    fn bridge_axis_x_when_span_runs_east_west() {
        let tiles = vec![(2, 4), (3, 4), (4, 4), (5, 4)];
        assert!(!bridge_span_axis_y(&tiles));
    }
}
