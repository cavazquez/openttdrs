//! Fantasma de depósito naval: las dos secciones y las capas BUILD nativas.

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, ship_depot_footprint};

use crate::iso::{iso, overlay_pos, remap_tile_offset, tile_slope_and_min_z};
use crate::render::{CompanyColoredSprites, sprite_from_company_or_asset};
use crate::sprites::{SHIP_DEPOT_PATHS, ship_depot_layers};

use super::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

/// Spawn del depósito completo. `origin` conserva la tesela que recibe el
/// comando; la segunda sección se calcula con la misma función que usa el
/// núcleo al validar y materializar la huella.
pub(crate) fn spawn_ship_depot_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    company: Option<&CompanyColoredSprites>,
    map: &Map,
    origin: TileCoord,
    dir: u8,
    valid: bool,
) {
    let [origin, other] = ship_depot_footprint(origin, dir);
    let axis_y = dir & 0x01 != 0;
    let origin_is_south = matches!(dir & 0x03, 1 | 2);
    let tint = super::plan::preview_tint(valid);

    for (coord, part_south) in [(origin, origin_is_south), (other, !origin_is_south)] {
        let Some(_) = map.get(coord) else {
            continue;
        };
        let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
        let ref_pos = iso(coord.x, coord.y);
        for (layer_i, layer) in ship_depot_layers(axis_y, part_south).iter().enumerate() {
            let local = remap_tile_offset(layer.dx, layer.dy, 0.0) * 0.5;
            let pos = overlay_pos(
                ref_pos + local,
                layer.xrel,
                layer.yrel,
                layer.width,
                layer.height,
                base_z,
                PREVIEW_Z_BASE + 0.04 + layer_i as f32 * 0.0005,
                coord.x,
                coord.y,
            );
            let sprite = sprite_from_company_or_asset(
                company,
                asset_server,
                SHIP_DEPOT_PATHS[layer.sprite_index],
                tint,
            );
            commands.spawn((
                BuildGhostPreview,
                sprite,
                Transform::from_translation(pos).with_scale(Vec3::splat(PREVIEW_SCALE)),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_maps_origin_to_native_part_and_axis() {
        let origin = TileCoord::new(8, 8);
        for (dir, expected_other, expected_axis_y, expected_origin_south) in [
            (0, TileCoord::new(9, 8), false, false),
            (1, TileCoord::new(8, 7), true, true),
            (2, TileCoord::new(7, 8), false, true),
            (3, TileCoord::new(8, 9), true, false),
        ] {
            let [actual_origin, actual_other] = ship_depot_footprint(origin, dir);
            assert_eq!(actual_origin, origin);
            assert_eq!(actual_other, expected_other);
            assert_eq!(dir & 1 != 0, expected_axis_y);
            assert_eq!(matches!(dir, 1 | 2), expected_origin_south);
        }
    }

    #[test]
    fn layer_count_covers_both_sections_for_each_direction() {
        for dir in 0..4 {
            let axis_y = dir & 1 != 0;
            let origin_south = matches!(dir, 1 | 2);
            let count = ship_depot_layers(axis_y, origin_south).len()
                + ship_depot_layers(axis_y, !origin_south).len();
            assert_eq!(count, 3);
        }
    }
}
