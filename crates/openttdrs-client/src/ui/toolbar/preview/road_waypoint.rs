//! Fantasma de colocación de waypoint road (carretera recta).

use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::iso::{TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};

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
    let handle = asset_server.load::<Image>(format!("assets/opengfx/tiles/road_flat_{idx:02}.png"));
    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: handle,
            color: tint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(coord.x, coord.y, base_z, 2.4, TILE_HALF_H)),
    ));
}
