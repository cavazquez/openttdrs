use bevy::prelude::*;
use openttdrs_core::{Climate, Map, TileKind};

use super::{TRAM_OVERLAY_LAYER_FRAC, spawn_ground_sprite};
use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, overlay_pos, remap_tile_offset, tile_pos_half};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    RAIL_GROUND_SNOW_OR_DESERT, ROAD_FLAT_HALF_H, ROAD_STREETLIGHT_META, ROADSIDE_LAMPS,
    collect_rail_sprites, collect_signal_sprite_draws, is_road_level_crossing,
    level_crossing_has_rail_reservation, level_crossing_rail_sprite_id, rail_signal_subtile_offset,
    rail_tile_is_signals, rail_track_base_color, rail_trackbits_for_render, road_bits_for_render,
    road_flat_sprite_color, road_flat_sprite_index, road_tile_roadside, road_tile_snow_or_desert,
    road_tile_tram_visual_active, roadside_is_paved, tram_flat_sprite_index,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_road_tile(
    commands: &mut Commands,
    map: &Map,
    mw: u32,
    mh: u32,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    climate: Climate,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    let rb = road_bits_for_render(map, ctx.coord, mw, mh);
    let fi = road_flat_sprite_index(tileh, rb);
    let road_half_h = if tileh == 0 {
        ROAD_FLAT_HALF_H[fi]
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let road_paint = ctx.tile.map_or(Color::WHITE, |t| {
        if climate.uses_snow_ground() {
            Color::srgb(0.82, 0.88, 0.98)
        } else {
            road_flat_sprite_color(t.mapt, ctx.kind, t.m7)
        }
    });
    // `GetRoadGroundSprite`: acera pavimentada (Roadside >= Paved) usa el set
    // 1313..1331 salvo nieve/desierto, que mantiene el set sobre pasto + tinte.
    let roadside = ctx.tile.and_then(|t| road_tile_roadside(t.m5, t.m6));
    let snow_or_desert = ctx
        .tile
        .is_some_and(|t| road_tile_snow_or_desert(t.mapt, ctx.kind, t.m7))
        || climate.uses_snow_ground();
    let paved = roadside.is_some_and(roadside_is_paved) && !snow_or_desert;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            &assets.grass_slopes[tileh as usize - 1],
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let road_set = if paved {
        &assets.road_paved
    } else {
        &assets.road_flat
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        road_set[fi].sprite_colored(road_paint),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.02,
            road_half_h,
        )),
    ));

    if let Some(tfi) = ctx.tile.and_then(|t| tram_flat_sprite_index(tileh, t.m3)) {
        let tram_half_h = if tileh == 0 {
            ROAD_FLAT_HALF_H[tfi]
        } else {
            SLOPE_HALF_H[tileh as usize]
        };
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.tram_flat[tfi].sprite(),
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                TRAM_OVERLAY_LAYER_FRAC,
                tram_half_h,
            )),
        ));
    }

    // `Roadside::StreetLights` (3): faroles de `_roadside_lamps` en sus
    // subcoordenadas de mundo. Igual que upstream, solo con 2+ road bits.
    if roadside == Some(3) && rb.count_ones() > 1 {
        for &(lamp, dx, dy) in ROADSIDE_LAMPS[usize::from(rb & 0xF)] {
            let (w, h, xrel, yrel) = ROAD_STREETLIGHT_META[lamp];
            let off = remap_tile_offset(dx, dy, 0.0) * 0.5;
            let pos3 = overlay_pos(
                Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
                xrel,
                yrel,
                w,
                h,
                base_z,
                0.2,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.road_streetlights[lamp].sprite(),
                Transform::from_translation(pos3),
            ));
        }
    }

    // Cruce a nivel: carretera + sprite de vía encima (`base_sprites.crossing + rail_axis`).
    if ctx
        .tile
        .is_some_and(|t| is_road_level_crossing(t.mapt, t.m5, ctx.kind))
    {
        let sid = ctx
            .tile
            .map(|t| level_crossing_rail_sprite_id(t.m5))
            .unwrap_or(1370);
        if let Some(img) = assets.rail.get(&sid) {
            let crossing_paint = ctx.tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
                let mut c = rail_track_base_color(t.mapt, TileKind::Rail, t.m5, t.m3);
                if level_crossing_has_rail_reservation(t.m5) {
                    c = c.mix(&Color::srgb(0.95, 0.52, 0.42), 0.26);
                }
                if road_tile_tram_visual_active(t.m3, t.m8) {
                    c = c.mix(&Color::srgb(0.55, 0.88, 0.58), 0.12);
                }
                c
            });
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                img.sprite_colored(crossing_paint),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.045,
                    road_half_h,
                )),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    rail_layers: &mut Vec<u32>,
    climate: Climate,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            &assets.grass_slopes[tileh as usize - 1],
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let rail_half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let snow_ground = ctx
        .tile
        .is_some_and(|t| (t.m3 & 0x0F) == RAIL_GROUND_SNOW_OR_DESERT)
        || climate.uses_snow_ground();
    collect_rail_sprites(
        rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1),
        tileh,
        snow_ground,
        rail_layers,
    );
    let mut rail_paint = ctx.tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
        rail_track_base_color(t.mapt, ctx.kind, t.m5, t.m3)
    });
    if ctx.tile.is_some_and(|t| rail_tile_is_signals(t.m5)) {
        rail_paint = rail_paint.mix(&Color::srgb(0.95, 0.88, 0.55), 0.22);
    }
    for (i, sid) in rail_layers.iter().copied().enumerate() {
        let Some(img) = assets.rail.get(&sid) else {
            continue;
        };
        let z = 0.02 + i as f32 * 0.0004;
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            img.sprite_colored(rail_paint),
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                z,
                rail_half_h,
            )),
        ));
    }
    if let Some(t) = ctx.tile.filter(|t| rail_tile_is_signals(t.m5)) {
        let sig_draws = collect_signal_sprite_draws(t.m2, t.m3, t.m3hi, t.m5);
        for (si, draw) in sig_draws.iter().copied().enumerate() {
            let Some(img) = assets.rail.get(&draw.sprite_id) else {
                continue;
            };
            let offset = rail_signal_subtile_offset(draw.pos);
            let z = 0.032 + si as f32 * 0.0015;
            let base = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, z, rail_half_h);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                img.sprite(),
                Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
            ));
        }
    }
}
