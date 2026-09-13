//! Fantasma de depósito naval: las dos secciones y las capas BUILD nativas.

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, WaterClass, ship_depot_footprint, water_class};

use crate::iso::{
    full_tile_sprite_pos, ground_draw_z, iso, overlay_pos, remap_tile_offset, tile_slope_and_min_z,
};
use crate::render::viewport_sort::{ParentSpriteBounds, tile_seq_parent_bounds};
use crate::render::{
    CompanyColoredSprites, NewGrfAction5SpriteCache, ViewportSortableParent, WorldAssets,
    canal_dike_slots, river_slope_sprite_index, sprite_from_atlas_or_company_colour,
    sprite_from_company_or_asset, viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{
    SHIP_DEPOT_PATHS, WATER_CANAL_DIKE_SPRITE_META, WATER_RIVER_SLOPE_SPRITE_META,
    ship_depot_layers,
};

use super::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_WATER_LAYER: f32 = PREVIEW_Z_BASE - 0.030;
const PREVIEW_DIKE_LAYER: f32 = PREVIEW_WATER_LAYER + 0.010;
const PREVIEW_SCALE: f32 = 1.002;
const ACTION5_CANALS_DIKES_OFFSET: usize = 52;
const ACTION5_TYPE_CANALS: u8 = openttdrs_core::ACTION5_TYPE_CANALS;
const RIVER_SLOPE_PATHS: [&str; 4] = [
    "assets/opengfx/tiles/water_river_slope_y_up.png",
    "assets/opengfx/tiles/water_river_slope_x_down.png",
    "assets/opengfx/tiles/water_river_slope_x_up.png",
    "assets/opengfx/tiles/water_river_slope_y_down.png",
];

/// Spawn del depósito completo. `origin` conserva la tesela que recibe el
/// comando; la segunda sección se calcula con la misma función que usa el
/// núcleo al validar y materializar la huella.
#[allow(clippy::too_many_arguments)] // parámetros ECS/assets de spawn
pub(crate) fn spawn_ship_depot_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    company: Option<&CompanyColoredSprites>,
    world_assets: Option<&WorldAssets>,
    map: &Map,
    origin: TileCoord,
    dir: u8,
    valid: bool,
    canal_action5: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: &mut NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
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
        let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
        let river_slope = (map.get(coord).and_then(water_class) == Some(WaterClass::River))
            .then(|| river_slope_sprite_index(tileh))
            .flatten();
        let (water, water_position) = if let Some(index) = river_slope {
            let action5 = canal_action5
                .get(index)
                .and_then(Option::as_ref)
                .and_then(|decoded| {
                    let sprite = action5_sprites.sprite_colored(
                        ACTION5_TYPE_CANALS,
                        index,
                        canal_action5,
                        tint,
                        images,
                    )?;
                    Some((
                        sprite,
                        f32::from(decoded.width),
                        f32::from(decoded.height),
                        f32::from(decoded.x_offs),
                        f32::from(decoded.y_offs),
                    ))
                });
            let (water, width, height, xrel, yrel) = action5.unwrap_or_else(|| {
                let (width, height, xrel, yrel) = WATER_RIVER_SLOPE_SPRITE_META[index];
                let water = world_assets.map_or_else(
                    || Sprite {
                        image: asset_server.load::<Image>(RIVER_SLOPE_PATHS[index]),
                        color: tint,
                        ..default()
                    },
                    |assets| assets.river_slopes[index].sprite_colored(tint),
                );
                (
                    water,
                    f32::from(width),
                    f32::from(height),
                    f32::from(xrel),
                    f32::from(yrel),
                )
            });
            let mut position = overlay_pos(
                iso(coord.x, coord.y),
                xrel,
                yrel,
                width,
                height,
                base_z,
                PREVIEW_WATER_LAYER,
                coord.x,
                coord.y,
            );
            position.z = ground_draw_z(coord.x, coord.y, PREVIEW_WATER_LAYER);
            (water, position)
        } else {
            let water = world_assets.map_or_else(
                || Sprite {
                    image: asset_server.load::<Image>("assets/opengfx/tiles/water.png"),
                    color: tint,
                    ..default()
                },
                |assets| assets.water.sprite_colored(tint),
            );
            (
                water,
                full_tile_sprite_pos(coord.x, coord.y, base_z, PREVIEW_WATER_LAYER),
            )
        };
        commands.spawn((
            BuildGhostPreview,
            water,
            Transform::from_translation(water_position).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
        spawn_ship_depot_canal_dikes(
            commands,
            asset_server,
            world_assets,
            map,
            coord,
            base_z,
            tint,
            canal_action5,
            action5_sprites,
            images,
        );
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
            let sprite = world_assets.map_or_else(
                || {
                    sprite_from_company_or_asset(
                        company,
                        asset_server,
                        SHIP_DEPOT_PATHS[layer.sprite_index],
                        tint,
                    )
                },
                |assets| {
                    // El ghost y `DrawWaterDepot` deben consumir la misma
                    // entrada del atlas. El helper conserva además el
                    // recolor de la compañía activa antes del alpha del
                    // preview.
                    sprite_from_atlas_or_company_colour(
                        company,
                        None,
                        &assets.ship_depot[layer.sprite_index],
                        SHIP_DEPOT_PATHS[layer.sprite_index],
                        tint,
                    )
                },
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

/// Dibuja los bordes vanilla que `DrawWaterEdges(true, 0, tile)` agrega a un
/// depósito sobre canal. El selector comparte la conectividad del renderer:
/// un segundo depósito adyacente no vuelve a pintar el dique interno.
#[allow(clippy::too_many_arguments)] // parámetros ECS/assets de spawn
fn spawn_ship_depot_canal_dikes(
    commands: &mut Commands,
    asset_server: &AssetServer,
    world_assets: Option<&WorldAssets>,
    map: &Map,
    coord: TileCoord,
    base_z: u8,
    tint: Color,
    canal_action5: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: &mut NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
) {
    if map.get(coord).and_then(water_class) != Some(WaterClass::Canal) {
        return;
    }
    let origin = iso(coord.x, coord.y);
    for (slot, selected) in canal_dike_slots(map, coord).into_iter().enumerate() {
        if !selected {
            continue;
        }
        let action5_slot = ACTION5_CANALS_DIKES_OFFSET.saturating_add(slot);
        let action5 = canal_action5
            .get(action5_slot)
            .and_then(Option::as_ref)
            .and_then(|decoded| {
                let sprite = action5_sprites.sprite_colored(
                    ACTION5_TYPE_CANALS,
                    action5_slot,
                    canal_action5,
                    tint,
                    images,
                )?;
                Some((
                    sprite,
                    f32::from(decoded.width),
                    f32::from(decoded.height),
                    f32::from(decoded.x_offs),
                    f32::from(decoded.y_offs),
                ))
            });
        let (sprite, width, height, xrel, yrel) = action5.unwrap_or_else(|| {
            let (width, height, xrel, yrel) = WATER_CANAL_DIKE_SPRITE_META[slot];
            let sprite = world_assets.map_or_else(
                || Sprite {
                    image: asset_server.load::<Image>(format!(
                        "assets/opengfx/tiles/water_canal_dike_{slot:02}.png"
                    )),
                    color: tint,
                    ..default()
                },
                |assets| assets.canal_dikes[slot].sprite_colored(tint),
            );
            (
                sprite,
                f32::from(width),
                f32::from(height),
                f32::from(xrel),
                f32::from(yrel),
            )
        });
        let mut position = overlay_pos(
            origin,
            xrel,
            yrel,
            width,
            height,
            base_z,
            PREVIEW_DIKE_LAYER + slot as f32 * 0.0001,
            coord.x,
            coord.y,
        );
        position.z = ground_draw_z(coord.x, coord.y, PREVIEW_DIKE_LAYER + slot as f32 * 0.0001);
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
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
