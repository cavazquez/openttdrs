use bevy::prelude::*;
use openttdrs_core::{Station, StopKind, TileKind};

use super::{TILE_OVERLAP_SCALE, sloped_or_flat_image, spawn_ground_sprite};
use crate::iso::tile_pos;
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};

pub(crate) fn spawn_station_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    stations: &[Station],
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            assets.grass_slopes[tileh as usize - 1].clone(),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let dir = ctx
        .tile
        .map_or(0, |t| (t.m5 & 0x03) as usize)
        .min(assets.station_grounds.len().saturating_sub(1));
    let grounds = if stations
        .iter()
        .find(|station| station.pos == ctx.coord)
        .is_some_and(|station| station.stop_kind == StopKind::BusStop)
    {
        &assets.bus_stop_grounds
    } else {
        &assets.station_grounds
    };
    let image = grounds
        .get(dir)
        .cloned()
        .unwrap_or_else(|| assets.station_grounds[dir].clone());
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(ctx.tx_i32(), ctx.ty_i32(), ctx.info.base_z, 0.01))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

pub(crate) fn spawn_transport_object_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    let ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    spawn_ground_sprite(commands, ground, Color::WHITE, ctx, slope_half_ground);

    let image = match ctx.kind {
        TileKind::RoadDepot => assets
            .road_depots
            .get(ctx.tile.map_or(0, |t| (t.m5 & 0x03) as usize))
            .cloned()
            .unwrap_or_else(|| assets.road_depots[0].clone()),
        TileKind::RailDepot => assets.rail_depot.clone(),
        TileKind::RoadTunnel => assets.road_tunnel.clone(),
        TileKind::RailTunnel => assets.rail_tunnel.clone(),
        TileKind::RoadBridge => assets.road_bridge.clone(),
        TileKind::RailBridge => assets.rail_bridge.clone(),
        _ => unreachable!(),
    };

    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(ctx.tx_i32(), ctx.ty_i32(), ctx.info.base_z, 0.08))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}
