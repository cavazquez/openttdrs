//! Fantasma de colocación de waypoint ferroviario (postes reales, no `tile_select`).

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, TileKind};

use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, iso, tile_pos_half, tile_slope_and_min_z};
use crate::render::TileAtlas;
use crate::sprites::{
    RAIL_TB_X, RAIL_TB_Y, RAIL_WAYPOINT_SPRITE_TINT, rail_station_ground_track_sprite,
    rail_station_sprite_meta, rail_waypoint_draw_layers, rail_waypoint_sprite_center,
};

use super::BuildGhostPreview;

/// Eje del waypoint en `m5` bajo (bit 0 = Y), o `None` si la vía no es recta.
fn waypoint_m5_on_tile(map: &Map, coord: TileCoord) -> Option<u8> {
    let tile = map.get(coord)?;
    match tile.kind {
        TileKind::Rail => match tile.m5 & 0x3F {
            RAIL_TB_X => Some(0),
            RAIL_TB_Y => Some(1),
            _ => None,
        },
        TileKind::Station => Some(tile.m5 & 0x0F),
        _ => None,
    }
}

pub(crate) fn spawn_rail_waypoint_preview(
    commands: &mut Commands,
    atlas: Option<&TileAtlas>,
    map: &Map,
    coord: TileCoord,
    valid: bool,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let Some(m5) = waypoint_m5_on_tile(map, coord) else {
        return;
    };
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let tint = if valid {
        RAIL_WAYPOINT_SPRITE_TINT.with_alpha(0.65)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.65)
    };

    let track_sid = rail_station_ground_track_sprite(m5, tileh);
    if let Some(img) = atlas.try_get(&format!("rail_{track_sid}.png")) {
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(Color::srgba(0.88, 0.9, 0.98, 0.45)),
            Transform::from_translation(tile_pos_half(coord.x, coord.y, base_z, 0.02, half_h)),
        ));
    }

    let origin = iso(coord.x, coord.y);
    for layer in rail_waypoint_draw_layers(m5) {
        let Some(img) = atlas.try_get(&format!("rail_{}.png", layer.sprite_id)) else {
            continue;
        };
        let Some((w, h, nfo_xrel, nfo_yrel)) = rail_station_sprite_meta(layer.sprite_id) else {
            continue;
        };
        let pos3 = rail_waypoint_sprite_center(
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
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(pos3),
        ));
    }
}
