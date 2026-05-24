use bevy::prelude::*;
use openttdrs_core::{Station, TileKind, is_tunnel_entrance_slope};

use super::{TILE_OVERLAP_SCALE, sloped_or_flat_image, spawn_ground_sprite};
use crate::iso::overlay_pos;
use crate::iso::{
    SLOPE_HALF_H, TILE_HALF_H, road_stop_build_sprite_center, tile_pos, tile_pos_half,
};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    StationTileClass, rail_station_draw_layers, rail_station_ground_track_sprite,
    rail_station_overlay_rel, road_depot_build_layers, road_depot_entrance_road_bits,
    road_depot_seq_gfx, road_flat_sprite_index, road_stop_build_layers, road_stop_ground_index,
    road_stop_seq_gfx, station_tile_class,
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
            let track_sid = rail_station_ground_track_sprite(m5, tileh);
            if let Some(img) = assets.rail.get(&track_sid) {
                commands.spawn((
                    MapVisualLayer,
                    Sprite {
                        image: img.clone(),
                        color: Color::srgb(0.88, 0.88, 0.97),
                        ..default()
                    },
                    Transform::from_translation(tile_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        base_z,
                        0.02,
                        rail_half_h,
                    ))
                    .with_scale(Vec3::new(
                        TILE_OVERLAP_SCALE,
                        TILE_OVERLAP_SCALE,
                        1.0,
                    )),
                ));
            }
            for layer in rail_station_draw_layers(m5) {
                let Some(img) = assets.rail.get(&layer.sprite_id) else {
                    continue;
                };
                let (xrel, yrel) = rail_station_overlay_rel(layer.dx, layer.dy);
                let pos3 = overlay_pos(
                    ctx.iso_pos,
                    xrel,
                    yrel,
                    layer.w,
                    layer.h,
                    base_z,
                    layer.z,
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                );
                commands.spawn((
                    MapVisualLayer,
                    Sprite {
                        image: img.clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(pos3).with_scale(Vec3::new(
                        TILE_OVERLAP_SCALE,
                        TILE_OVERLAP_SCALE,
                        1.0,
                    )),
                ));
            }
        }
        StationTileClass::Bus | StationTileClass::Truck => {
            if tileh == 0 {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, grass, Color::WHITE, ctx, slope_half_ground);
            }
            let stub = ctx.tile.map_or(0, |t| t.m3 & 0x0F);
            if stub != 0 {
                spawn_road_stop_link(commands, assets, ctx, base_z, rail_half_h, tileh, stub);
            }
            let dir = road_stop_ground_index(m5).min(3);
            let image = if class == StationTileClass::Bus {
                assets
                    .bus_stop_grounds
                    .get(dir)
                    .cloned()
                    .unwrap_or_else(|| assets.bus_stop_grounds[0].clone())
            } else {
                assets
                    .station_grounds
                    .get(dir)
                    .cloned()
                    .unwrap_or_else(|| assets.station_grounds[0].clone())
            };
            spawn_stop_ground_sprite(commands, image, ctx, base_z, 0.04);
            spawn_road_stop_buildings(commands, assets, ctx, base_z, class, dir);
        }
        StationTileClass::Airport | StationTileClass::Other(_) => {
            let dir = road_stop_ground_index(m5).min(3);
            let image = assets
                .station_grounds
                .get(dir)
                .cloned()
                .unwrap_or_else(|| assets.station_grounds[0].clone());
            spawn_stop_ground_sprite(commands, image, ctx, base_z, 0.01);
        }
    }
}

fn spawn_road_stop_link(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    tileh: u8,
    road_bits: u8,
) {
    let fi = road_flat_sprite_index(tileh, road_bits);
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image: assets.road_flat[fi].clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.025,
            half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

fn spawn_road_stop_buildings(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    class: StationTileClass,
    dir: usize,
) {
    let handles = match class {
        StationTileClass::Bus => &assets.bus_stop_builds,
        StationTileClass::Truck => &assets.truck_stop_builds,
        _ => return,
    };
    for (layer_i, spec) in road_stop_build_layers(class, dir).iter().enumerate() {
        let image = handles[dir][layer_i].clone();
        let scale = TILE_OVERLAP_SCALE;
        let center = road_stop_build_sprite_center(
            ctx.iso_pos,
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            spec.z,
            road_stop_seq_gfx(spec),
            spec.w,
            spec.h,
        );
        commands.spawn((
            MapVisualLayer,
            Sprite {
                image,
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(center).with_scale(Vec3::new(scale, scale, 1.0)),
        ));
    }
}

fn spawn_stop_ground_sprite(
    commands: &mut Commands,
    image: Handle<Image>,
    ctx: &TileRenderContext,
    base_z: u8,
    layer: f32,
) {
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(ctx.tx_i32(), ctx.ty_i32(), base_z, layer))
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
    let base_z = ctx.info.base_z;
    let ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    spawn_ground_sprite(commands, ground, Color::WHITE, ctx, slope_half_ground);

    match ctx.kind {
        TileKind::RoadTunnel | TileKind::RailTunnel => {
            if !is_tunnel_entrance_slope(tileh) {
                return;
            }
            let image = if ctx.kind == TileKind::RoadTunnel {
                assets.road_tunnel.clone()
            } else {
                assets.rail_tunnel.clone()
            };
            let portal_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image,
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.08,
                    portal_half_h,
                ))
                .with_scale(Vec3::new(
                    TILE_OVERLAP_SCALE,
                    TILE_OVERLAP_SCALE,
                    1.0,
                )),
            ));
        }
        TileKind::RoadDepot => {
            spawn_road_depot_tile(commands, assets, ctx, base_z, TILE_HALF_H);
        }
        TileKind::RailDepot => {
            spawn_object_sprite(
                commands,
                assets.rail_depot.clone(),
                ctx,
                base_z,
                TILE_HALF_H,
            );
        }
        TileKind::RoadBridge => {
            let axis_y = ctx.tile.is_some_and(|t| t.m5 & 0x10 != 0);
            let image = if axis_y {
                assets.road_bridge_y.clone()
            } else {
                assets.road_bridge.clone()
            };
            spawn_object_sprite(commands, image, ctx, base_z, TILE_HALF_H);
        }
        TileKind::RailBridge => {
            let axis_y = ctx.tile.is_some_and(|t| t.m5 & 0x10 != 0);
            let image = if axis_y {
                assets.rail_bridge_y.clone()
            } else {
                assets.rail_bridge.clone()
            };
            spawn_object_sprite(commands, image, ctx, base_z, TILE_HALF_H);
        }
        _ => unreachable!(),
    }
}

fn spawn_road_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image: assets.road_depot_ground.clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.02,
            half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
    spawn_road_stop_link(
        commands,
        assets,
        ctx,
        base_z,
        half_h,
        ctx.info.tileh,
        road_depot_entrance_road_bits(dir as u8),
    );
    for (layer_i, spec) in road_depot_build_layers(dir).iter().enumerate() {
        let Some(image) = assets.road_depot_builds[dir].get(layer_i) else {
            continue;
        };
        let center = road_stop_build_sprite_center(
            ctx.iso_pos,
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            spec.z,
            road_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
        commands.spawn((
            MapVisualLayer,
            Sprite {
                image: image.clone(),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(center).with_scale(Vec3::new(
                TILE_OVERLAP_SCALE,
                TILE_OVERLAP_SCALE,
                1.0,
            )),
        ));
    }
}

fn spawn_object_sprite(
    commands: &mut Commands,
    image: Handle<Image>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
) {
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.08,
            half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}
