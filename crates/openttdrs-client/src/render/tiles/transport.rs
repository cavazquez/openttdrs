use bevy::prelude::*;
use openttdrs_core::{Map, TileKind};

use super::{TILE_OVERLAP_SCALE, TRAM_OVERLAY_LAYER_FRAC, spawn_ground_sprite};
use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos_half};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    RAIL_GROUND_SNOW_OR_DESERT, ROAD_FLAT_HALF_H, collect_rail_sprites, collect_signal_sprite_ids,
    is_road_level_crossing, level_crossing_has_rail_reservation, level_crossing_rail_sprite_id,
    rail_tile_is_signals, rail_track_base_color, rail_trackbits_for_render, road_bits_for_render,
    road_flat_sprite_color, road_flat_sprite_index, road_tile_tram_visual_active,
    tram_flat_sprite_index,
};

pub(crate) fn spawn_road_tile(
    commands: &mut Commands,
    map: &Map,
    mw: u32,
    mh: u32,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
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
        road_flat_sprite_color(t.mapt, ctx.kind, t.m7)
    });
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            assets.grass_slopes[tileh as usize - 1].clone(),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image: assets.road_flat[fi].clone(),
            color: road_paint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.02,
            road_half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));

    if let Some(tfi) = ctx.tile.and_then(|t| tram_flat_sprite_index(tileh, t.m3)) {
        let tram_half_h = if tileh == 0 {
            ROAD_FLAT_HALF_H[tfi]
        } else {
            SLOPE_HALF_H[tileh as usize]
        };
        commands.spawn((
            MapVisualLayer,
            Sprite {
                image: assets.tram_flat[tfi].clone(),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                TRAM_OVERLAY_LAYER_FRAC,
                tram_half_h,
            ))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
        ));
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
                Sprite {
                    image: img.clone(),
                    color: crossing_paint,
                    ..default()
                },
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

pub(crate) fn spawn_rail_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    rail_layers: &mut Vec<u32>,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            assets.grass_slopes[tileh as usize - 1].clone(),
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
        .is_some_and(|t| (t.m3 & 0x0F) == RAIL_GROUND_SNOW_OR_DESERT);
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
            Sprite {
                image: img.clone(),
                color: rail_paint,
                ..default()
            },
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                z,
                rail_half_h,
            ))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
        ));
    }
    if let Some(t) = ctx.tile.filter(|t| rail_tile_is_signals(t.m5)) {
        let sig_ids = collect_signal_sprite_ids(t.m2, t.m3, t.m3hi, t.m5);
        for (si, sid) in sig_ids.iter().copied().enumerate() {
            let Some(img) = assets.rail.get(&sid) else {
                continue;
            };
            let z = 0.032 + si as f32 * 0.0015;
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image: img.clone(),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    z,
                    rail_half_h,
                )),
            ));
        }
    }
}
