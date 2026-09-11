//! Fantasma de muelle: las piezas de tierra y agua de `MakeDock`.

use bevy::prelude::*;
use openttdrs_core::station::dock_water_tile;
use openttdrs_core::{Map, TileCoord};

use crate::iso::{iso, overlay_pos, remap_tile_offset, tile_slope_and_min_z};
use crate::render::viewport_sort::{ParentSpriteBounds, tile_seq_parent_bounds};
use crate::render::{
    CompanyColoredSprites, ViewportSortableParent, sprite_from_company_or_asset,
    viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::dock_tile_layer;

use super::BuildGhostPreview;
use super::plan::preview_tint;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

/// Dibuja el ghost nativo completo: `StationGfx 0..3` sobre tierra y `4..5`
/// sobre el agua contigua. La tercera tesela, de aproximación, no se pinta.
#[allow(clippy::too_many_arguments)] // parámetros ECS/assets de spawn
pub(crate) fn spawn_dock_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    company: Option<&CompanyColoredSprites>,
    map: &Map,
    origin: TileCoord,
    water: TileCoord,
    dir: u8,
    valid: bool,
) {
    let dir = dir & 0x03;
    debug_assert_eq!(water, dock_water_tile(origin, dir));
    let parts = [(origin, dir), (water, 4 + (dir & 1))];
    let tint = preview_tint(valid);
    let map_width = map.dimensions().0;

    for (part_index, (coord, gfx)) in parts.into_iter().enumerate() {
        let Some(_) = map.get(coord) else {
            continue;
        };
        let layer = dock_tile_layer(gfx);
        let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
        let ref_pos = iso(coord.x, coord.y);
        let local = remap_tile_offset(layer.dx, layer.dy, layer.dz) * 0.5;
        let mut pos = overlay_pos(
            ref_pos + local,
            layer.x_offs,
            layer.y_offs,
            layer.w,
            layer.h,
            base_z,
            PREVIEW_Z_BASE + 0.04 + part_index as f32 * 0.0005,
            coord.x,
            coord.y,
        );
        let source_depth = viewport_source_depth(pos.z, coord.x as u32, map_width);
        pos.z = source_depth;
        let sprite = sprite_from_company_or_asset(company, asset_server, layer.path, tint);
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(pos).with_scale(Vec3::splat(PREVIEW_SCALE)),
            ViewportSortableParent {
                sprite_id: layer.sprite_id,
                bounds: dock_parent_bounds(coord, base_z, layer),
                insertion_key: viewport_insertion_key(
                    coord.x as u32,
                    coord.y as u32,
                    u8::try_from(part_index + 1).unwrap_or(u8::MAX),
                ),
                source_depth,
            },
        ));
    }
}

fn dock_parent_bounds(
    coord: TileCoord,
    base_z: u8,
    layer: crate::sprites::DockTileLayer,
) -> ParentSpriteBounds {
    tile_seq_parent_bounds(
        coord.x,
        coord.y,
        base_z,
        layer.dx as i32,
        layer.dy as i32,
        layer.dz as i32,
        layer.sx,
        layer.sy,
        layer.sz,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_use_land_gfx_and_axis_water_gfx() {
        let origin = TileCoord::new(8, 8);
        for (dir, expected_water, expected_gfx) in [
            (0, TileCoord::new(7, 8), 4),
            (1, TileCoord::new(8, 9), 5),
            (2, TileCoord::new(9, 8), 4),
            (3, TileCoord::new(8, 7), 5),
        ] {
            let water = dock_water_tile(origin, dir);
            assert_eq!(water, expected_water);
            assert_eq!(4 + (dir & 1), expected_gfx);
            assert!(dock_tile_layer(dir).sprite_id >= 2727);
            assert_eq!(
                dock_tile_layer(expected_gfx).sprite_id,
                2731 + (dir & 1) as u32
            );
        }
    }

    #[test]
    fn parent_bounds_follow_the_runtime_station_layer() {
        let coord = TileCoord::new(2, 3);
        let layer = dock_tile_layer(5);
        assert_eq!(
            dock_parent_bounds(coord, 2, layer),
            ParentSpriteBounds::new(36, 48, 16, 43, 63, 23)
        );
    }
}
