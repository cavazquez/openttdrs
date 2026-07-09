//! Fantasma de colocación de waypoint ferroviario (postes reales, no `tile_select`).

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, TileKind};

use crate::iso::{TILE_HALF_H, iso, tile_pos_half, tile_slope_and_min_z};
use crate::render::{CompanyColoredSprites, TileAtlas};
use crate::sprites::{
    RAIL_TB_X, RAIL_TB_Y, rail_station_ground_track_sprite, rail_station_sprite_meta,
    rail_waypoint_draw_layers, rail_waypoint_sprite_center,
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
    company: Option<&CompanyColoredSprites>,
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
    let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.65)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.65)
    };

    let origin = iso(coord.x, coord.y);
    let track_sid = rail_station_ground_track_sprite(m5, 0);
    if let Some(img) = atlas.try_get(&format!("rail_{track_sid}.png")) {
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(tile_pos_half(coord.x, coord.y, base_z, 2.4, TILE_HALF_H)),
        ));
    }
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
        let sprite = company
            .and_then(|c| c.rail_handle(layer.sprite_id))
            .map(|handle| Sprite {
                image: handle.clone(),
                color: tint,
                ..default()
            })
            .unwrap_or_else(|| img.sprite_colored(tint));
        commands.spawn((BuildGhostPreview, sprite, Transform::from_translation(pos3)));
    }
}
