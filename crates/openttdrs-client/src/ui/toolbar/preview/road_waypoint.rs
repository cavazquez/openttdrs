//! Fantasma de colocación de waypoint road (carretera recta).

use bevy::prelude::*;
use openttdrs_core::{prelude::*, tram_road_type_from_tile};

use crate::iso::{
    TILE_HALF_H, full_tile_sprite_pos_half, iso, remap_tile_offset, road_stop_build_sprite_center,
    tile_slope_and_min_z,
};
use crate::render::viewport_sort::{
    ParentSprite, ParentSpriteBounds, depths_in_viewport_sort_order, tile_seq_parent_bounds,
};
use crate::render::{
    CatenarySpriteAnchor, NewGrfAction5SpriteCache, NewGrfRoadSpriteCache, TileAtlas,
    ViewportSortableChild, ViewportSortableParent, WorldAssets, catenary_sprite_anchor,
    catenary_sprite_center, catenary_sprite_horizontal_crop, forced_leveled_foundation_decision_at,
    road_newgrf_view_index, specific_sprite_for_tile, viewport_insertion_key,
    viewport_source_depth,
};
use crate::sprites::{
    ROAD_WAYPOINT_SPRITE_PATHS, catenary_hidden, catenary_sprite_color, foundation_asset_path,
    foundation_gfx_for_tileh, road_catenary_sprite_ids, road_stop_seq_gfx,
    road_waypoint_build_layers, road_waypoint_sprite_index, tramway_sprite_atlas_key,
};

use super::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 2.4;
const FOUNDATION_LAYER: f32 = PREVIEW_Z_BASE - 0.006;
const TRAM_LAYER: f32 = PREVIEW_Z_BASE + 0.025;
const PREVIEW_SCALE: f32 = 1.002;

fn road_waypoint_flat_index(map: &Map, coord: TileCoord) -> Option<usize> {
    let tile = map.get(coord)?;
    let bits = match tile.kind {
        TileKind::Road => tile.m5 & 0x0F,
        TileKind::Station => tile.m3 & 0x0F,
        _ => return None,
    };
    match bits {
        0x0A => Some(10), // road_flat_10 ≈ eje X
        0x05 => Some(5),  // road_flat_05 ≈ eje Y
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_road_waypoint_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    world_assets: Option<&WorldAssets>,
    map: &Map,
    coord: TileCoord,
    valid: bool,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: &mut NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    road_sprites: &mut NewGrfRoadSpriteCache,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
) {
    let Some(idx) = road_waypoint_flat_index(map, coord) else {
        return;
    };
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.65)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.65)
    };
    let foundation = spawn_road_waypoint_foundation(
        commands,
        asset_server,
        atlas,
        world_assets,
        map,
        coord,
        tileh,
        base_z,
        tint,
        foundation_newgrf,
        action5_sprites,
        images,
    );
    let surface_base_z = foundation.surface_base_z;
    let ground = world_assets
        .and_then(|assets| assets.road_flat.get(idx))
        .map(|image| image.sprite_colored(tint))
        .or_else(|| {
            atlas
                .and_then(|atlas| atlas.try_get(&format!("road_flat_{idx:02}.png")))
                .map(|image| image.sprite_colored(tint))
        })
        .unwrap_or_else(|| Sprite {
            image: asset_server
                .load::<Image>(format!("assets/opengfx/tiles/road_flat_{idx:02}.png")),
            color: tint,
            ..default()
        });
    spawn_road_waypoint_surface(
        commands,
        ground,
        full_tile_sprite_pos_half(
            coord.x,
            coord.y,
            surface_base_z,
            PREVIEW_Z_BASE,
            TILE_HALF_H,
        ),
        coord,
        map.dimensions().0,
        foundation.child_parent,
    );

    if map
        .get(coord)
        .is_some_and(|tile| tram_road_type_from_tile(&tile).is_some())
    {
        let tram_overlay = world_assets
            .and_then(|assets| assets.tram_flat.get(idx))
            .map(|image| image.sprite_colored(tint))
            .or_else(|| {
                atlas
                    .and_then(|atlas| atlas.try_get(&format!("tram_flat_{idx:02}.png")))
                    .map(|image| image.sprite_colored(tint))
            })
            .unwrap_or_else(|| Sprite {
                image: asset_server
                    .load::<Image>(format!("assets/opengfx/tiles/tram_flat_{idx:02}.png")),
                color: tint,
                ..default()
            });
        spawn_road_waypoint_surface(
            commands,
            tram_overlay,
            full_tile_sprite_pos_half(coord.x, coord.y, surface_base_z, TRAM_LAYER, TILE_HALF_H),
            coord,
            map.dimensions().0,
            foundation.child_parent,
        );
    }

    let Some(tile) = map.get(coord) else {
        return;
    };
    let waypoint_bits = road_waypoint_catenary_bits(tile);
    let road_def =
        openttdrs_core::road_type_def(road_catalog, openttdrs_core::road_type_from_tile(&tile))
            .filter(|def| def.has_catenary());
    let tram_def = tram_road_type_from_tile(&tile)
        .or_else(|| {
            (openttdrs_core::tram_track_bits(&tile) != 0).then_some(openttdrs_core::RoadType::TRAM)
        })
        .and_then(|road_type| openttdrs_core::road_type_def(road_catalog, road_type))
        .filter(|def| def.has_catenary());
    spawn_road_waypoint_catenary(
        commands,
        asset_server,
        world_assets,
        coord,
        surface_base_z,
        tint,
        waypoint_bits,
        road_def,
        tram_def,
        map,
        tile,
        road_catalog,
        climate,
        newgrf_stack,
        road_sprites,
        images,
        catenary_newgrf,
    );

    let layers = road_waypoint_build_layers(u8::from(idx == 5));
    let child_centers = foundation
        .child_parent
        .map(|_| road_waypoint_sorted_layer_centers(coord, surface_base_z, layers));
    for (layer_index, layer) in layers.iter().enumerate() {
        let Some(sprite_index) = road_waypoint_sprite_index(layer.sprite_id) else {
            continue;
        };
        let sprite = world_assets
            .and_then(|assets| assets.road_waypoint.get(sprite_index))
            .map(|image| image.sprite_colored(tint))
            .or_else(|| {
                atlas
                    .and_then(|atlas| atlas.try_get(ROAD_WAYPOINT_SPRITE_PATHS[sprite_index]))
                    .map(|image| image.sprite_colored(tint))
            })
            .unwrap_or_else(|| Sprite {
                image: asset_server.load::<Image>(ROAD_WAYPOINT_SPRITE_PATHS[sprite_index]),
                color: tint,
                ..default()
            });
        let position = child_centers
            .as_ref()
            .and_then(|centers| centers.get(layer_index))
            .copied()
            .unwrap_or_else(|| {
                road_stop_build_sprite_center(
                    iso(coord.x, coord.y),
                    coord.x,
                    coord.y,
                    surface_base_z,
                    layer.z,
                    road_stop_seq_gfx(layer),
                    layer.w,
                    layer.h,
                )
            });
        let source_depth = viewport_source_depth(position.z, coord.x as u32, map.dimensions().0);
        let mut entity = commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(Vec3::new(position.x, position.y, source_depth))
                .with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
        if let Some(parent) = foundation.child_parent {
            entity.insert(ViewportSortableChild {
                parent,
                source_depth,
            });
        } else {
            entity.insert(road_waypoint_parent(
                layer,
                layer_index,
                coord,
                surface_base_z,
                source_depth,
            ));
        }
    }
}

/// `DrawTile_Station` publica la catenaria de un waypoint con los roadbits del
/// eje, no con el índice de las capas BUILD. Una estación importada conserva
/// el eje en `m5`; los fixtures antiguos pueden necesitar el nibble de `m3`.
#[must_use]
fn road_waypoint_catenary_bits(tile: Tile) -> u8 {
    match tile.kind {
        TileKind::Road => tile.m5 & 0x0F,
        TileKind::Station => match tile.m5 {
            openttdrs_core::RSV_DRIVE_THROUGH_X => 0x0A,
            openttdrs_core::RSV_DRIVE_THROUGH_Y => 0x05,
            _ => match tile.m3 & 0x0F {
                0x05 | 0x0A => tile.m3 & 0x0F,
                _ => 0,
            },
        },
        _ => 0,
    }
}

/// Preview de las tres columnas recortadas y el frente que emite
/// `DrawRoadTypeCatenary`. El waypoint queda nivelado antes de entrar aquí,
/// por eso se consulta la fila plana aunque el terreno original sea pendiente.
#[allow(clippy::too_many_arguments)]
fn spawn_road_waypoint_catenary(
    commands: &mut Commands,
    asset_server: &AssetServer,
    world_assets: Option<&WorldAssets>,
    coord: TileCoord,
    surface_base_z: u8,
    tint: Color,
    road_bits: u8,
    road_def: Option<&openttdrs_core::RoadTypeDef>,
    tram_def: Option<&openttdrs_core::RoadTypeDef>,
    map: &Map,
    tile: Tile,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    road_sprites: &mut NewGrfRoadSpriteCache,
    images: &mut Assets<Image>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
) {
    if catenary_hidden() || road_bits == 0 {
        return;
    }
    let Some((back_id, front_id)) = road_catenary_sprite_ids(0, road_bits) else {
        return;
    };
    let tint = tint.with_alpha(tint.alpha() * catenary_sprite_color().alpha());
    let view_idx = road_newgrf_view_index(0, road_bits);
    for def in [road_def, tram_def] {
        let Some(def) = def else {
            continue;
        };
        let custom_back = def
            .has_newgrf_specific_group(5)
            .then(|| {
                preview_custom_road_catenary_sprite(
                    def,
                    5,
                    view_idx,
                    map,
                    coord,
                    tile,
                    road_catalog,
                    climate,
                    newgrf_stack,
                    road_sprites,
                    images,
                    tint,
                )
            })
            .flatten();
        let custom_front = def
            .has_newgrf_specific_group(4)
            .then(|| {
                preview_custom_road_catenary_sprite(
                    def,
                    4,
                    view_idx,
                    map,
                    coord,
                    tile,
                    road_catalog,
                    climate,
                    newgrf_stack,
                    road_sprites,
                    images,
                    tint,
                )
            })
            .flatten();
        let custom_any = custom_back.is_some() || custom_front.is_some();
        let back = if custom_any {
            custom_back
        } else {
            preview_road_catenary_sprite(asset_server, world_assets, back_id, tint, catenary_newgrf)
        };
        let Some((back, back_anchor)) = back else {
            continue;
        };
        for (index, (left, right)) in [
            (None, Some(-12.0)),
            (Some(-12.0), Some(12.0)),
            (Some(12.0), None),
        ]
        .into_iter()
        .enumerate()
        {
            let Some((sprite, x_shift)) =
                catenary_sprite_horizontal_crop(back.clone(), back_anchor, left, right)
            else {
                continue;
            };
            let mut position = catenary_sprite_center(
                coord.x,
                coord.y,
                surface_base_z,
                PREVIEW_Z_BASE + 0.034 + index as f32 * 0.0001,
                0.0,
                0.0,
                0.0,
                back_anchor,
            );
            position.x += x_shift;
            commands.spawn((
                BuildGhostPreview,
                sprite,
                Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
            ));
        }

        let front = if custom_any {
            custom_front
        } else {
            preview_road_catenary_sprite(
                asset_server,
                world_assets,
                front_id,
                tint,
                catenary_newgrf,
            )
        };
        let Some((sprite, anchor)) = front else {
            continue;
        };
        let position = catenary_sprite_center(
            coord.x,
            coord.y,
            surface_base_z,
            PREVIEW_Z_BASE + 0.04,
            0.0,
            0.0,
            0.0,
            anchor,
        );
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn preview_custom_road_catenary_sprite(
    def: &openttdrs_core::RoadTypeDef,
    selector: u8,
    view_idx: usize,
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    road_sprites: &mut NewGrfRoadSpriteCache,
    images: &mut Assets<Image>,
    tint: Color,
) -> Option<(Sprite, CatenarySpriteAnchor)> {
    let mut road_sprites = Some(road_sprites);
    let mut images = Some(images);
    let (mut sprite, view) = specific_sprite_for_tile(
        def,
        map,
        selector,
        view_idx,
        coord,
        tile,
        climate,
        road_catalog,
        newgrf_stack,
        None,
        &mut road_sprites,
        &mut images,
    )?;
    sprite.color = tint;
    Some((sprite, CatenarySpriteAnchor::from_decoded(&view)))
}

fn preview_road_catenary_sprite(
    asset_server: &AssetServer,
    world_assets: Option<&WorldAssets>,
    sprite_id: u32,
    tint: Color,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
) -> Option<(Sprite, CatenarySpriteAnchor)> {
    let anchor = catenary_sprite_anchor(sprite_id, catenary_newgrf)?;
    let image = world_assets
        .and_then(|assets| assets.rail.get(&sprite_id))
        .map(|asset| asset.sprite_colored(tint))
        .or_else(|| {
            tramway_sprite_atlas_key(sprite_id).map(|path| Sprite {
                image: asset_server.load::<Image>(format!("assets/opengfx/tiles/{path}")),
                color: tint,
                ..default()
            })
        })?;
    Some((image, anchor))
}

#[derive(Clone, Copy, Debug)]
struct RoadWaypointFoundation {
    surface_base_z: u8,
    child_parent: Option<Entity>,
}

/// Materializa el mismo `DrawFoundation(Leveled)` que usa el renderer del
/// waypoint. El parent queda disponible para el suelo, tranvía y postes:
/// todos son `AddChildSpriteScreen` en una pendiente.
#[allow(clippy::too_many_arguments)]
fn spawn_road_waypoint_foundation(
    commands: &mut Commands,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    world_assets: Option<&WorldAssets>,
    map: &Map,
    coord: TileCoord,
    tileh: u8,
    base_z: u8,
    tint: Color,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: &mut NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
) -> RoadWaypointFoundation {
    if tileh == 0 {
        return RoadWaypointFoundation {
            surface_base_z: base_z,
            child_parent: None,
        };
    }

    let decision =
        forced_leveled_foundation_decision_at(map, coord, map.dimensions(), tileh, base_z);
    let plan = openttdrs_core::foundation_draw_plan(
        tileh,
        openttdrs_core::FOUNDATION_LEVELED,
        decision.sprite_block,
    );
    let foundation_tint = tint.with_alpha(tint.alpha() * 0.84);
    let mut child_parent = None;
    for (ordinal, draw) in plan.sprites.into_iter().flatten().enumerate() {
        let Some((sprite, xrel, yrel, width, height)) = road_waypoint_foundation_sprite(
            asset_server,
            atlas,
            world_assets,
            draw,
            foundation_tint,
            foundation_newgrf,
            action5_sprites,
            images,
        ) else {
            continue;
        };
        let mut position = crate::iso::overlay_pos(
            iso(coord.x, coord.y),
            xrel,
            yrel,
            width,
            height,
            base_z.saturating_add(draw.z_delta),
            FOUNDATION_LAYER + ordinal as f32 * 0.0005,
            coord.x,
            coord.y,
        );
        let bounds_offset = remap_tile_offset(
            f32::from(draw.bounds.ox),
            f32::from(draw.bounds.oy),
            f32::from(draw.bounds.oz),
        ) * 0.5;
        position += Vec3::new(bounds_offset.x, bounds_offset.y, 0.0);
        let source_depth = viewport_source_depth(position.z, coord.x as u32, map.dimensions().0);
        position.z = source_depth;
        let parent = commands
            .spawn((
                BuildGhostPreview,
                sprite,
                Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
                road_waypoint_foundation_parent(coord, base_z, draw, source_depth, ordinal),
            ))
            .id();
        child_parent = Some(parent);
    }
    RoadWaypointFoundation {
        surface_base_z: decision.surface_base_z,
        child_parent,
    }
}

#[allow(clippy::too_many_arguments)]
fn road_waypoint_foundation_sprite(
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    world_assets: Option<&WorldAssets>,
    draw: openttdrs_core::RailFoundationSpriteDraw,
    tint: Color,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: &mut NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
) -> Option<(Sprite, f32, f32, f32, f32)> {
    if let Some(tileh) = draw
        .sprite_id
        .checked_sub(openttdrs_core::FOUNDATION_ORIGINAL_SPRITE_BASE)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|tileh| (1..=14).contains(tileh))
    {
        let gfx = foundation_gfx_for_tileh(tileh)?;
        let path = foundation_asset_path(tileh)?;
        let image = world_assets
            .and_then(|assets| assets.foundations.get(usize::from(tileh - 1)))
            .map(|image| image.sprite_colored(tint))
            .or_else(|| {
                atlas
                    .and_then(|atlas| atlas.try_get(&format!("foundation_{tileh:02}.png")))
                    .map(|image| image.sprite_colored(tint))
            })
            .unwrap_or_else(|| Sprite {
                image: asset_server.load::<Image>(path),
                color: tint,
                ..default()
            });
        return Some((image, gfx.xrel, gfx.yrel, gfx.w, gfx.h));
    }

    let slot = openttdrs_core::foundation_action5_slot_for_sprite_id(draw.sprite_id)?;
    let decoded = foundation_newgrf.get(slot).and_then(Option::as_ref)?;
    let slot_u16 = u16::try_from(slot).ok()?;
    let image = action5_sprites.handle_for(
        openttdrs_core::ACTION5_TYPE_FOUNDATIONS,
        slot_u16,
        decoded,
        images,
    );
    Some((
        Sprite {
            image,
            color: tint,
            ..default()
        },
        f32::from(decoded.x_offs),
        f32::from(decoded.y_offs),
        f32::from(decoded.width),
        f32::from(decoded.height),
    ))
}

fn road_waypoint_foundation_parent(
    coord: TileCoord,
    base_z: u8,
    draw: openttdrs_core::RailFoundationSpriteDraw,
    source_depth: f32,
    ordinal: usize,
) -> ViewportSortableParent {
    let x = coord.x * 16 + i32::from(draw.bounds.ox);
    let y = coord.y * 16 + i32::from(draw.bounds.oy);
    let z = i32::from(base_z) * i32::from(openttdrs_core::TILE_PIXEL_HEIGHT)
        + i32::from(draw.z_delta) * i32::from(openttdrs_core::TILE_PIXEL_HEIGHT)
        + i32::from(draw.bounds.oz);
    ViewportSortableParent {
        sprite_id: draw.sprite_id,
        bounds: ParentSpriteBounds::new(
            x,
            y,
            z,
            x + i32::from(draw.bounds.ex) - 1,
            y + i32::from(draw.bounds.ey) - 1,
            z + i32::from(draw.bounds.ez) - 1,
        ),
        insertion_key: viewport_insertion_key(
            coord.x as u32,
            coord.y as u32,
            u8::try_from(ordinal).unwrap_or(u8::MAX),
        ),
        source_depth,
    }
}

fn spawn_road_waypoint_surface(
    commands: &mut Commands,
    sprite: Sprite,
    mut position: Vec3,
    coord: TileCoord,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
) {
    if let Some(parent) = foundation_child_parent {
        let source_depth = viewport_source_depth(position.z, coord.x as u32, map_width);
        position.z = source_depth;
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
            ViewportSortableChild {
                parent,
                source_depth,
            },
        ));
    } else {
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}

fn road_waypoint_sorted_layer_centers(
    coord: TileCoord,
    base_z: u8,
    layers: &[crate::sprites::RoadStopLayerGfx],
) -> Vec<Vec3> {
    let origin = iso(coord.x, coord.y);
    let mut centers: Vec<_> = layers
        .iter()
        .map(|layer| {
            road_stop_build_sprite_center(
                origin,
                coord.x,
                coord.y,
                base_z,
                layer.z,
                road_stop_seq_gfx(layer),
                layer.w,
                layer.h,
            )
        })
        .collect();
    let parents: Vec<_> = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            ParentSprite::sprite(
                index as u64,
                layer.sprite_id,
                tile_seq_parent_bounds(
                    coord.x,
                    coord.y,
                    base_z,
                    layer.dx as i32,
                    layer.dy as i32,
                    layer.dz as i32,
                    layer.bounds.0,
                    layer.bounds.1,
                    layer.bounds.2,
                ),
            )
        })
        .collect();
    let depths: Vec<_> = centers.iter().map(|center| center.z).collect();
    for (center, depth) in centers
        .iter_mut()
        .zip(depths_in_viewport_sort_order(&parents, &depths))
    {
        center.z = depth;
    }
    centers
}

/// Cuerpo `TILE_SEQ_LINE` de un poste de waypoint vial.
///
/// El runtime reserva los ordinales 2 y 3 cuando el waypoint no materializa
/// catenaria. La preview aún no dibuja ese stream; mantener el mismo bloque
/// evita que sus postes queden por delante de una catenaria vecina.
fn road_waypoint_parent(
    layer: &crate::sprites::RoadStopLayerGfx,
    layer_index: usize,
    coord: TileCoord,
    base_z: u8,
    source_depth: f32,
) -> ViewportSortableParent {
    let (ex, ey, ez) = layer.bounds;
    ViewportSortableParent {
        sprite_id: layer.sprite_id,
        bounds: tile_seq_parent_bounds(
            coord.x,
            coord.y,
            base_z,
            layer.dx as i32,
            layer.dy as i32,
            layer.dz as i32,
            ex,
            ey,
            ez,
        ),
        insertion_key: viewport_insertion_key(
            coord.x as u32,
            coord.y as u32,
            2u8.saturating_add(u8::try_from(layer_index).unwrap_or(u8::MAX)),
        ),
        source_depth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::viewport_insertion_key;
    use crate::render::viewport_sort::ParentSpriteBounds;

    #[test]
    fn preview_road_waypoint_x_parents_match_runtime_prisms() {
        let layer = road_waypoint_build_layers(0)[1];
        let parent = road_waypoint_parent(&layer, 1, TileCoord::new(1, 1), 0, 0.05);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(16, 29, 0, 31, 31, 15)
        );
        assert_eq!(parent.insertion_key, viewport_insertion_key(1, 1, 3));
    }

    #[test]
    fn preview_road_waypoint_y_parent_uses_northwest_anchor() {
        let layer = road_waypoint_build_layers(1)[0];
        let parent = road_waypoint_parent(&layer, 0, TileCoord::new(1, 1), 0, 0.05);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(29, 16, 0, 31, 31, 15)
        );
        assert_eq!(parent.insertion_key, viewport_insertion_key(1, 1, 2));
    }

    #[test]
    fn preview_road_waypoint_foundation_parent_matches_runtime_bounds() {
        let draw =
            openttdrs_core::foundation_draw_plan(0x03, openttdrs_core::FOUNDATION_LEVELED, 0)
                .sprites[0]
                .expect("leveled foundation draw");
        let parent = road_waypoint_foundation_parent(TileCoord::new(2, 3), 4, draw, 0.05, 0);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(32, 48, 32, 47, 63, 38)
        );
        assert_eq!(parent.insertion_key, viewport_insertion_key(2, 3, 0));
    }

    #[test]
    fn preview_road_waypoint_steep_foundation_raises_two_levels() {
        let map = Map::new_flat(4, 4, 0);
        let decision = forced_leveled_foundation_decision_at(
            &map,
            TileCoord::new(1, 1),
            map.dimensions(),
            0x1B,
            3,
        );
        assert_eq!(decision.surface_tileh, 0);
        assert_eq!(decision.surface_base_z, 5);
    }

    #[test]
    fn road_waypoint_catenary_uses_station_axis_bits() {
        let x = Tile {
            height: 0,
            kind: TileKind::Station,
            mapt: 0,
            m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        let y = Tile {
            height: 0,
            kind: TileKind::Station,
            mapt: 0,
            m5: openttdrs_core::RSV_DRIVE_THROUGH_Y,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        assert_eq!(road_waypoint_catenary_bits(x), 0x0A);
        assert_eq!(road_waypoint_catenary_bits(y), 0x05);
        assert_eq!(
            road_catenary_sprite_ids(0, road_waypoint_catenary_bits(x)),
            Some((6071, 6043))
        );
        assert_eq!(
            road_catenary_sprite_ids(0, road_waypoint_catenary_bits(y)),
            Some((6070, 6042))
        );
    }
}
