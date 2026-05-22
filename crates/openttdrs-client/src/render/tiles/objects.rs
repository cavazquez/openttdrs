use bevy::prelude::*;
use openttdrs_core::{Station, TileKind};

use super::{TILE_OVERLAP_SCALE, sloped_or_flat_image, spawn_ground_sprite};
use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos, tile_pos_half};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    StationTileClass, rail_station_axis_y, rail_station_draw_layers, road_stop_ground_index,
    station_tile_class,
};

pub(crate) fn spawn_station_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    stations: &[Station],
    slope_half_ground: f32,
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

    let stop_kind = stations
        .iter()
        .find(|s| s.pos == ctx.coord)
        .map(|s| s.stop_kind);
    let m6 = ctx.tile.map_or(0, |t| t.m6);
    let m5 = ctx.tile.map_or(0, |t| t.m5);
    let class = station_tile_class(m6, stop_kind);

    let rail_half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };

    match class {
        StationTileClass::Rail => {
            if tileh == 0 {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, grass, Color::WHITE, ctx, slope_half_ground);
            }
            let axis_y = rail_station_axis_y(m5);
            for &(z, sid) in rail_station_draw_layers(axis_y) {
                if let Some(img) = assets.rail.get(&sid) {
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
                        ))
                        .with_scale(Vec3::new(
                            TILE_OVERLAP_SCALE,
                            TILE_OVERLAP_SCALE,
                            1.0,
                        )),
                    ));
                }
            }
        }
        StationTileClass::Bus => {
            let dir = road_stop_ground_index(m5).min(3);
            let image = assets
                .bus_stop_grounds
                .get(dir)
                .cloned()
                .unwrap_or_else(|| assets.bus_stop_grounds[0].clone());
            spawn_stop_ground_sprite(commands, image, ctx, base_z);
        }
        StationTileClass::Truck | StationTileClass::Airport | StationTileClass::Other(_) => {
            let dir = road_stop_ground_index(m5).min(3);
            let image = assets
                .station_grounds
                .get(dir)
                .cloned()
                .unwrap_or_else(|| assets.station_grounds[0].clone());
            spawn_stop_ground_sprite(commands, image, ctx, base_z);
        }
    }
}

fn spawn_stop_ground_sprite(
    commands: &mut Commands,
    image: Handle<Image>,
    ctx: &TileRenderContext,
    base_z: u8,
) {
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.01))
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
