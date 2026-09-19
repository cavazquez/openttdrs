//! Fantasma de colocación de waypoint ferroviario (postes reales, no `tile_select`).

use bevy::prelude::*;
use openttdrs_core::{RailType, prelude::*};

use crate::iso::{TILE_HALF_H, iso, tile_pos_half, tile_slope_and_min_z};
use crate::render::viewport_sort::tile_seq_parent_bounds;
use crate::render::{
    CompanyColoredSprites, TileAtlas, ViewportSortableChild, ViewportSortableParent, WorldAssets,
    viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{
    RAIL_TB_X, RAIL_TB_Y, rail_station_ground_track_sprite_for_type,
    rail_waypoint_child_parent_slot, rail_waypoint_draw_layers, rail_waypoint_layer_bounds,
    rail_waypoint_layer_meta, rail_waypoint_parent_slot, rail_waypoint_sprite_center,
};

use super::BuildGhostPreview;

/// Eje del waypoint en `m5` bajo (bit 0 = Y), o `None` si la vía no es recta.
fn waypoint_m5_on_tile(map: &Map, coord: TileCoord) -> Option<(u8, RailType)> {
    let tile = map.get(coord)?;
    match tile.kind {
        TileKind::Rail => match tile.m5 & 0x3F {
            RAIL_TB_X => Some((0, openttdrs_core::rail_type_from_tile(tile))),
            RAIL_TB_Y => Some((1, openttdrs_core::rail_type_from_tile(tile))),
            _ => None,
        },
        TileKind::Station => Some((tile.m5 & 0x0F, openttdrs_core::rail_type_from_tile(tile))),
        _ => None,
    }
}

pub(crate) fn spawn_rail_waypoint_preview(
    commands: &mut Commands,
    atlas: Option<&TileAtlas>,
    world_assets: Option<&WorldAssets>,
    company: Option<&CompanyColoredSprites>,
    map: &Map,
    coord: TileCoord,
    valid: bool,
) {
    if atlas.is_none() && world_assets.is_none() {
        return;
    }
    let Some((m5, rail_type)) = waypoint_m5_on_tile(map, coord) else {
        return;
    };
    let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.65)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.65)
    };

    let origin = iso(coord.x, coord.y);
    let track_sid = rail_station_ground_track_sprite_for_type(m5, 0, rail_type);
    let track_sprite = world_assets
        .and_then(|assets| assets.rail.get(&track_sid))
        .cloned()
        .map(|img| img.sprite_colored(tint))
        .or_else(|| {
            atlas
                .and_then(|atlas| atlas.try_get(&format!("rail_{track_sid}.png")))
                .map(|img| img.sprite_colored(tint))
        });
    if let Some(track_sprite) = track_sprite {
        commands.spawn((
            BuildGhostPreview,
            track_sprite,
            Transform::from_translation(tile_pos_half(coord.x, coord.y, base_z, 2.4, TILE_HALF_H)),
        ));
    }
    let mut waypoint_parents = [None; 2];
    for (layer_index, layer) in rail_waypoint_draw_layers(m5).iter().enumerate() {
        let img = world_assets
            .and_then(|assets| assets.rail.get(&layer.sprite_id))
            .cloned()
            .or_else(|| {
                atlas.and_then(|atlas| atlas.try_get(&format!("rail_{}.png", layer.sprite_id)))
            });
        let Some(img) = img else {
            continue;
        };
        let Some((w, h, nfo_xrel, nfo_yrel)) = rail_waypoint_layer_meta(layer.sprite_id) else {
            continue;
        };
        let mut pos3 = rail_waypoint_sprite_center(
            origin,
            coord.x,
            coord.y,
            base_z,
            layer.z + 2.5,
            layer,
            nfo_xrel,
            nfo_yrel,
            w,
            h,
        );
        let sprite = company
            .and_then(|c| c.rail_handle(layer.sprite_id))
            .map(|handle| Sprite {
                image: handle.clone(),
                color: tint,
                ..default()
            })
            .unwrap_or_else(|| img.sprite_colored(tint));
        let source_depth = viewport_source_depth(pos3.z, coord.x as u32, map.dimensions().0);
        pos3.z = source_depth;
        let mut preview =
            commands.spawn((BuildGhostPreview, sprite, Transform::from_translation(pos3)));
        if let Some(parent) = rail_waypoint_parent(layer, layer_index, coord, base_z, source_depth)
        {
            let entity = preview.id();
            if let Some(slot) = rail_waypoint_parent_slot(layer.sprite_id) {
                waypoint_parents[slot] = Some(entity);
            }
            preview.insert(parent);
        } else if let Some(slot) = rail_waypoint_child_parent_slot(layer.sprite_id)
            && let Some(parent) = waypoint_parents[slot]
        {
            preview.insert(ViewportSortableChild {
                parent,
                source_depth,
            });
        }
    }
}

fn rail_waypoint_parent(
    layer: &crate::sprites::RailStationLayer,
    layer_index: usize,
    coord: TileCoord,
    base_z: u8,
    source_depth: f32,
) -> Option<ViewportSortableParent> {
    let (ex, ey, ez) = rail_waypoint_layer_bounds(layer.sprite_id)?;
    Some(ViewportSortableParent {
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
            16u8.saturating_add(u8::try_from(layer_index).unwrap_or(u8::MAX)),
        ),
        source_depth,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::render::viewport_insertion_key;
    use crate::render::viewport_sort::ParentSpriteBounds;

    #[test]
    fn preview_waypoint_parent_uses_runtime_body_prism() {
        let layer = rail_waypoint_draw_layers(0)[0];
        let parent = rail_waypoint_parent(&layer, 0, TileCoord::new(2, 3), 4, 0.05)
            .expect("waypoint body bounds");
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(32, 48, 32, 47, 50, 47)
        );
        assert_eq!(parent.insertion_key, viewport_insertion_key(2, 3, 16));
    }

    #[test]
    fn preview_waypoint_parents_keep_runtime_layer_ordinals() {
        let layer = rail_waypoint_draw_layers(0)[1];
        let parent = rail_waypoint_parent(&layer, 1, TileCoord::new(2, 3), 4, 0.05)
            .expect("waypoint body bounds");
        assert_eq!(parent.insertion_key, viewport_insertion_key(2, 3, 17));
    }

    #[test]
    fn waypoint_canopies_remain_children_of_their_body() {
        assert_eq!(rail_waypoint_parent_slot(4974), Some(0));
        assert_eq!(rail_waypoint_parent_slot(4975), Some(1));
        assert_eq!(rail_waypoint_child_parent_slot(4978), Some(0));
        assert_eq!(rail_waypoint_child_parent_slot(4979), Some(1));
        assert_eq!(rail_waypoint_layer_bounds(4978), None);
    }
}
