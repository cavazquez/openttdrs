//! Fantasma de colocación de señal: vía existente + sprite de señal encima.

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
    RAIL_TILE_SIGNALS, collect_rail_ghost_sprites, collect_signal_sprite_draws,
    rail_ghost_overlay_offset, signal_draw_pos, signal_screen_position,
};

use super::BuildGhostPreview;

/// Sprite eléctrico verde (OpenGFX) usado si el atlas no resuelve la entrada.
const FALLBACK_SIGNAL_SPRITE_ID: u32 = 1278;

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_signal_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    map: &Map,
    coord: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
    valid: bool,
    tick: GameTick,
) {
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
    let preview_tex = sig_draws
        .first()
        .map(|d| d.sprite_id)
        .unwrap_or(FALLBACK_SIGNAL_SPRITE_ID);
    let preview_pos = signal_draw_pos(track as u8, placement.sig_bit);
    let signal_xy =
        signal_screen_position(coord.x, coord.y, preview_pos, preview_tex, half_h, base_z);
    let signal_tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.75)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.75)
    };
    let rail_tint = if valid {
        Color::srgba(1.0, 1.02, 1.05, 0.38)
    } else {
        Color::srgba(1.0, 0.4, 0.35, 0.45)
    };
    let rail_center = tile_pos_half(coord.x, coord.y, base_z, 0.02, half_h);
    let signal_base = Vec3::new(
        signal_xy.x,
        signal_xy.y,
        tile_pos_half(coord.x, coord.y, base_z, 0.04, half_h).z,
    );

    if let Some(atlas) = atlas {
        let mut rail_ids = Vec::new();
        collect_rail_ghost_sprites(track.track_bit(), tileh, &mut rail_ids);
        for (i, sid) in rail_ids.iter().copied().enumerate() {
            let offset = rail_ghost_overlay_offset(sid);
            let img = atlas.get(&format!("rail_{sid}.png"));
            commands.spawn((
                BuildGhostPreview,
                img.sprite_colored(rail_tint),
                Transform::from_translation(
                    rail_center + Vec3::new(offset.x, offset.y, i as f32 * 0.001),
                ),
            ));
        }
    }

    let mut spawned = 0usize;
    if let Some(atlas) = atlas {
        for (i, draw) in sig_draws.iter().copied().enumerate() {
            let Some(img) = atlas.try_get(&format!("rail_{}.png", draw.sprite_id)) else {
                continue;
            };
            commands.spawn((
                BuildGhostPreview,
                img.sprite_colored(signal_tint),
                Transform::from_translation(
                    signal_base + Vec3::new(0.0, 0.0, 0.01 + i as f32 * 0.001),
                ),
            ));
            spawned += 1;
        }
    }

    if spawned == 0 {
        let sprite_id = sig_draws
            .first()
            .map(|d| d.sprite_id)
            .unwrap_or(FALLBACK_SIGNAL_SPRITE_ID);
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: asset_server.load(format!("assets/opengfx/tiles/rail_{sprite_id}.png")),
                color: signal_tint,
                ..default()
            },
            Transform::from_translation(signal_base),
        ));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use openttdrs_core::{Map, rail_signals::RAIL_TILE_NORMAL};

    const RAIL_TB_X: u8 = 0x01;

    fn flat_rail_x_map() -> Map {
        let mut map = Map::new_flat(5, 5, 0);
        let c = TileCoord::new(2, 2);
        map.set_kind(c, TileKind::Rail).expect("kind");
        let mut t = map.get(c).expect("tile");
        t.m5 = RAIL_TB_X | (RAIL_TILE_NORMAL << 6);
        map.set_tile(c, t).expect("tile");
        map
    }

    fn signal_draws_for_face(map: &Map, face: u8) -> Vec<crate::sprites::SignalSpriteDraw> {
        let tb = map.get(TileCoord::new(2, 2)).expect("tile").m5 & 0x3F;
        let track = resolve_signal_track(tb, 128, 128).expect("track");
        let variant = default_signal_variant(1950);
        let placement = signal_placement_for_track(track, face, variant).expect("placement");
        let m5 = track.track_bit() | (RAIL_TILE_SIGNALS << 6);
        collect_signal_sprite_draws(placement.m2, placement.m3, placement.m3hi, m5)
    }

    #[test]
    fn preview_resolves_signal_sprite_for_diagonal_track() {
        let map = flat_rail_x_map();
        let draws = signal_draws_for_face(&map, 0);
        assert!(
            !draws.is_empty(),
            "preview debe resolver al menos un sprite de señal"
        );
        assert_ne!(
            draws[0].sprite_id, 704,
            "sprite de señal no debe ser el cursor de demolición (704)"
        );
        assert_ne!(
            draws[0].sprite_id, 1565,
            "sprite de señal no debe coincidir con ui_demolish (1565)"
        );
        assert_eq!(
            draws[0].sprite_id, 1278,
            "NE eléctrica (1950+) → textura 1278, no topadora 1419"
        );
    }

    #[test]
    fn preview_sw_facing_maps_electric_texture_to_classic_block() {
        let map = flat_rail_x_map();
        let draws = signal_draws_for_face(&map, 2);
        assert!(!draws.is_empty());
        assert_eq!(
            draws[0].sprite_id, 1276,
            "SW eléctrica (1950+) → textura 1276, no topadora 1417"
        );
    }
}
