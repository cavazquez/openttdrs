//! Fantasma de colocación de señal: un solo sprite de señal, no realce de vía entera.

use bevy::prelude::*;
use openttdrs_core::{
    Map, TileCoord, TileKind,
    rail_signals::{
        calendar_year_at_tick, default_signal_variant, resolve_signal_track,
        signal_facing_for_orientation, signal_placement_for_track,
    },
    tick::GameTick,
};

use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::render::TileAtlas;
use crate::sprites::{
    RAIL_TILE_SIGNALS, collect_signal_sprite_draws, rail_signal_subtile_offset, signal_draw_pos,
};

use super::BuildGhostPreview;

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_signal_preview(
    commands: &mut Commands,
    atlas: Option<&TileAtlas>,
    map: &Map,
    coord: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
    valid: bool,
    tick: GameTick,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let Some(tile) = map.get(coord).filter(|t| t.kind == TileKind::Rail) else {
        return;
    };
    let tb = tile.m5 & 0x3F;
    let Some(track) = resolve_signal_track(tb, fract_x, fract_y) else {
        return;
    };
    let face = signal_facing_for_orientation(track, orientation);
    let variant = default_signal_variant(calendar_year_at_tick(tick));
    let Some(placement) = signal_placement_for_track(track, face, variant) else {
        return;
    };
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let m5 = track.track_bit() | (RAIL_TILE_SIGNALS << 6);
    let sig_draws = collect_signal_sprite_draws(placement.m2, placement.m3, placement.m3hi, m5);
    let preview_pos = signal_draw_pos(track as u8, placement.sig_bit);
    let track_offset = rail_signal_subtile_offset(preview_pos);
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.75)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.75)
    };
    for (i, draw) in sig_draws.iter().copied().enumerate() {
        let Some(img) = atlas.try_get(&format!("rail_{}.png", draw.sprite_id)) else {
            continue;
        };
        let base = tile_pos_half(coord.x, coord.y, base_z, 0.04 + i as f32 * 0.001, half_h);
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(base + Vec3::new(track_offset.x, track_offset.y, 0.0)),
        ));
    }
}
