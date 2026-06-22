//! Fantasma de colocación de señal: un solo sprite de señal, no realce de vía entera.

use bevy::prelude::*;
use openttdrs_core::{
    Map, TileCoord, TileKind,
    rail_signals::{signal_facing_for_orientation, signal_placement_for_facing},
};

use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::render::TileAtlas;
use crate::sprites::{RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_SIGNALS, collect_signal_sprite_ids};

use super::BuildGhostPreview;

fn straight_trackbits(map: &Map, coord: TileCoord) -> Option<u8> {
    let tile = map.get(coord)?;
    if tile.kind != TileKind::Rail {
        return None;
    }
    match tile.m5 & 0x3F {
        RAIL_TB_X | RAIL_TB_Y => Some(tile.m5 & 0x3F),
        _ => None,
    }
}

pub(crate) fn spawn_rail_signal_preview(
    commands: &mut Commands,
    atlas: Option<&TileAtlas>,
    map: &Map,
    coord: TileCoord,
    orientation: u8,
    valid: bool,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let Some(tb) = straight_trackbits(map, coord) else {
        return;
    };
    let face = signal_facing_for_orientation(tb, orientation);
    let Some(placement) = signal_placement_for_facing(tb, face) else {
        return;
    };
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let m5 = tb | (RAIL_TILE_SIGNALS << 6);
    let sig_ids = collect_signal_sprite_ids(placement.m2, placement.m3, placement.m3hi, m5);
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.75)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.75)
    };
    for (i, sid) in sig_ids.iter().copied().enumerate() {
        let Some(img) = atlas.try_get(&format!("rail_{sid}.png")) else {
            continue;
        };
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(tile_pos_half(
                coord.x,
                coord.y,
                base_z,
                0.04 + i as f32 * 0.001,
                half_h,
            )),
        ));
    }
}
