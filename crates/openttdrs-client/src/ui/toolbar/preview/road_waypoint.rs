//! Fantasma de colocación de waypoint road (carretera recta).

use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::iso::{TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::iso::{iso, road_stop_build_sprite_center};
use crate::render::viewport_sort::tile_seq_parent_bounds;
use crate::render::{
    TileAtlas, ViewportSortableParent, WorldAssets, viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{
    ROAD_WAYPOINT_SPRITE_PATHS, road_stop_seq_gfx, road_waypoint_build_layers,
    road_waypoint_sprite_index,
};

use super::BuildGhostPreview;

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

pub(crate) fn spawn_road_waypoint_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    world_assets: Option<&WorldAssets>,
    map: &Map,
    coord: TileCoord,
    valid: bool,
) {
    let Some(idx) = road_waypoint_flat_index(map, coord) else {
        return;
    };
    let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.65)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.65)
    };
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
    commands.spawn((
        BuildGhostPreview,
        ground,
        Transform::from_translation(tile_pos_half(coord.x, coord.y, base_z, 2.4, TILE_HALF_H)),
    ));

    for (layer_index, layer) in road_waypoint_build_layers(u8::from(idx == 5))
        .iter()
        .enumerate()
    {
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
        let position = road_stop_build_sprite_center(
            iso(coord.x, coord.y),
            coord.x,
            coord.y,
            base_z,
            layer.z,
            road_stop_seq_gfx(layer),
            layer.w,
            layer.h,
        );
        let source_depth = viewport_source_depth(position.z, coord.x as u32, map.dimensions().0);
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
            road_waypoint_parent(layer, layer_index, coord, base_z, source_depth),
        ));
    }
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
}
