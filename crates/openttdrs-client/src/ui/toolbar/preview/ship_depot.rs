//! Fantasma de depósito naval: las dos secciones y las capas BUILD nativas.

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, ship_depot_footprint};

use crate::iso::{iso, overlay_pos, remap_tile_offset, tile_slope_and_min_z};
use crate::render::viewport_sort::{ParentSpriteBounds, tile_seq_parent_bounds};
use crate::render::{
    CompanyColoredSprites, ViewportSortableParent, sprite_from_company_or_asset,
    viewport_insertion_key, viewport_source_depth,
};
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
    let (extent_x, extent_y) = crate::sprites::ship_depot_seq_extent(axis_y);
    let tint = super::plan::preview_tint(valid);

    for (coord, part_south) in [(origin, origin_is_south), (other, !origin_is_south)] {
        let Some(_) = map.get(coord) else {
            continue;
        };
        let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
        let ref_pos = iso(coord.x, coord.y);
        for (layer_i, layer) in ship_depot_layers(axis_y, part_south).iter().enumerate() {
            let local = remap_tile_offset(layer.dx, layer.dy, 0.0) * 0.5;
            let mut pos = overlay_pos(
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
            let source_depth = viewport_source_depth(pos.z, coord.x as u32, map.dimensions().0);
            pos.z = source_depth;
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
                ViewportSortableParent {
                    sprite_id: 4070 + layer.sprite_index as u32,
                    bounds: ship_depot_parent_bounds(
                        coord,
                        base_z,
                        layer.dx as i32,
                        layer.dy as i32,
                        extent_x,
                        extent_y,
                    ),
                    insertion_key: viewport_insertion_key(
                        coord.x as u32,
                        coord.y as u32,
                        (layer_i as u8).saturating_add(1),
                    ),
                    source_depth,
                },
            ));
        }
    }
}

/// Caja `TILE_SEQ_LINE` que comparte el ghost con el parent del mapa.
fn ship_depot_parent_bounds(
    coord: TileCoord,
    base_z: u8,
    dx: i32,
    dy: i32,
    extent_x: i32,
    extent_y: i32,
) -> ParentSpriteBounds {
    tile_seq_parent_bounds(coord.x, coord.y, base_z, dx, dy, 0, extent_x, extent_y, 20)
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

    #[test]
    fn preview_parent_bounds_match_runtime_tile_seq_line() {
        let coord = TileCoord::new(1, 1);
        let layer = ship_depot_layers(false, false)[0];
        assert_eq!(
            ship_depot_parent_bounds(coord, 0, layer.dx as i32, layer.dy as i32, 16, 1),
            ParentSpriteBounds::new(16, 31, 0, 31, 31, 19)
        );

        let layer = ship_depot_layers(true, true)[1];
        assert_eq!(
            ship_depot_parent_bounds(coord, 3, layer.dx as i32, layer.dy as i32, 1, 16),
            ParentSpriteBounds::new(31, 16, 24, 31, 31, 43)
        );
    }
}
