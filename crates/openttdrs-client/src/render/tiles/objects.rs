use bevy::prelude::*;
use openttdrs_core::{Map, Station, TileKind, inclined_slope_direction, is_tunnel_entrance_slope};

use super::bridge_draw::{bridge_span_at, spawn_bridge_deck};
use super::{sloped_or_flat_image, spawn_ground_sprite, spawn_rail_foundation};
use crate::iso::{
    SLOPE_HALF_H, TILE_HALF_H, road_depot_build_sprite_center, road_stop_build_sprite_center,
    tile_pos, tile_pos_half,
};
use crate::render::{
    AtlasSprite, CompanyColoredSprites, MapVisualLayer, TileRenderContext, WorldAssets,
    sprite_from_atlas_or_company_white,
};
use crate::sprites::{
    StationTileClass, rail_station_draw_layers, rail_station_ground_track_sprite,
    rail_station_overlay_rel, rail_station_sprite_meta, rail_waypoint_draw_layers,
    rail_waypoint_sprite_center, road_depot_build_layers, road_depot_entrance_road_bits,
    road_depot_seq_gfx, road_flat_sprite_index, road_stop_build_layers, road_stop_ground_index,
    road_stop_seq_gfx, station_tile_class,
};

pub(crate) fn spawn_station_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    ctx: &TileRenderContext,
    stations: &[Station],
    slope_half_ground: f32,
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
        StationTileClass::Rail | StationTileClass::RailWaypoint => {
            if tileh == 0 {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
            }
            // En pendiente el suelo ya se pintó arriba (evita hierba duplicada).
            let station_tb = if m5 & 1 != 0 { 0x02 } else { 0x01 };
            let rail_base_z = spawn_rail_foundation(commands, assets, ctx, tileh, station_tb);
            // OpenTTD: ground SPR_RAIL_TRACK_* bajo estación y waypoint (`station_land.h`).
            let track_sid = rail_station_ground_track_sprite(m5, tileh);
            if let Some(img) = assets.rail.get(&track_sid) {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    img.sprite_colored(Color::srgb(0.88, 0.88, 0.97)),
                    Transform::from_translation(tile_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        rail_base_z,
                        0.02,
                        rail_half_h,
                    )),
                ));
            }
            let overlay_layers = if class == StationTileClass::RailWaypoint {
                rail_waypoint_draw_layers(m5)
            } else {
                rail_station_draw_layers(m5)
            };
            for layer in overlay_layers {
                let Some(img) = assets.rail.get(&layer.sprite_id) else {
                    continue;
                };
                let Some((w, h, nfo_xrel, nfo_yrel)) = rail_station_sprite_meta(layer.sprite_id)
                else {
                    continue;
                };
                let pos3 = if class == StationTileClass::RailWaypoint {
                    rail_waypoint_sprite_center(
                        ctx.iso_pos,
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        rail_base_z,
                        layer.z,
                        layer,
                        nfo_xrel,
                        nfo_yrel,
                        w,
                        h,
                    )
                } else {
                    let (xrel, yrel) = rail_station_overlay_rel(layer, nfo_xrel, nfo_yrel);
                    crate::iso::overlay_pos(
                        ctx.iso_pos,
                        xrel,
                        yrel,
                        w,
                        h,
                        rail_base_z,
                        layer.z,
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                    )
                };
                let sprite = sprite_from_atlas_or_company_white(
                    company,
                    img,
                    &format!("rail_{}.png", layer.sprite_id),
                );
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(pos3),
                ));
            }
        }
        StationTileClass::Bus | StationTileClass::Truck => {
            if tileh == 0 {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
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
            spawn_stop_ground_sprite(commands, &image, ctx, base_z, 0.04);
            spawn_road_stop_buildings(commands, assets, company, ctx, base_z, class, dir);
        }
        StationTileClass::Dock => {
            let dock_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            let axis = usize::from(m5 & 1);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.dock_flat[axis].sprite(),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.03,
                    dock_half_h,
                )),
            ));
        }
        StationTileClass::Airport => {
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            let piece = openttdrs_core::AirportPiece::from_m5(m5);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.airport_piece_sprite(piece).sprite(),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.04,
                    half_h,
                )),
            ));
        }
        StationTileClass::Other(_) => {
            let dir = road_stop_ground_index(m5).min(3);
            let image = assets
                .station_grounds
                .get(dir)
                .cloned()
                .unwrap_or_else(|| assets.station_grounds[0].clone());
            spawn_stop_ground_sprite(commands, &image, ctx, base_z, 0.01);
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
        ctx.map_tile_chunk(),
        assets.road_flat[fi].sprite(),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.025,
            half_h,
        )),
    ));
}

fn spawn_road_stop_buildings(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
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
        let image = &handles[dir][layer_i];
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
            ctx.map_tile_chunk(),
            sprite_from_atlas_or_company_white(company, image, spec.path),
            Transform::from_translation(center),
        ));
    }
}

fn spawn_stop_ground_sprite(
    commands: &mut Commands,
    image: &AtlasSprite,
    ctx: &TileRenderContext,
    base_z: u8,
    layer: f32,
) {
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        image.sprite(),
        Transform::from_translation(tile_pos(ctx.tx_i32(), ctx.ty_i32(), base_z, layer)),
    ));
}

pub(crate) fn spawn_transport_object_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    map: &Map,
    dims: (u32, u32),
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    let ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    spawn_ground_sprite(commands, &ground, Color::WHITE, ctx, slope_half_ground);

    match ctx.kind {
        TileKind::RoadTunnel | TileKind::RailTunnel => {
            if !is_tunnel_entrance_slope(tileh) {
                return;
            }
            let rail = ctx.kind == TileKind::RailTunnel;
            let dir = inclined_slope_direction(tileh)
                .or_else(|| ctx.tile.map(|t| t.m5 & 0x03))
                .unwrap_or(0);
            let image = assets.tunnel_portal_sprite(rail, dir);
            let sprite_id = crate::sprites::tunnel_rear_sprite_id(rail, dir);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                image.sprite(),
                Transform::from_translation(crate::sprites::tunnel_portal_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    sprite_id,
                    0.08,
                )),
            ));
        }
        TileKind::RoadDepot => {
            let depot_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            spawn_road_depot_tile(commands, assets, company, ctx, base_z, depot_half_h, tileh);
        }
        TileKind::RailDepot => {
            let depot_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            spawn_rail_depot_tile(commands, assets, company, ctx, base_z, depot_half_h);
        }
        TileKind::ShipDepot => {
            let depot_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            spawn_ship_depot_tile(commands, assets, ctx, base_z, depot_half_h);
        }
        TileKind::Airport => {
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            let m5 = ctx.tile.map(|t| t.m5).unwrap_or(0);
            let piece = openttdrs_core::AirportPiece::from_m5(m5);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.airport_piece_sprite(piece).sprite(),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.04,
                    half_h,
                )),
            ));
        }
        TileKind::RoadBridge | TileKind::RailBridge => {
            if let Some(span) = bridge_span_at(map, ctx.coord, dims) {
                spawn_bridge_deck(commands, assets, ctx, &span, false);
            }
        }
        _ => {}
    }
}

fn spawn_ship_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        assets.ship_depot[dir].sprite(),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.04,
            half_h,
        )),
    ));
}

fn spawn_road_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    tileh: u8,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        assets.road_depot_ground.sprite(),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.02,
            half_h,
        )),
    ));
    spawn_road_stop_link(
        commands,
        assets,
        ctx,
        base_z,
        half_h,
        tileh,
        road_depot_entrance_road_bits(dir as u8),
    );
    for (layer_i, spec) in road_depot_build_layers(dir).iter().enumerate() {
        let Some(image) = assets.road_depot_builds[dir].get(layer_i) else {
            continue;
        };
        let center = road_depot_build_sprite_center(
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
            ctx.map_tile_chunk(),
            sprite_from_atlas_or_company_white(company, image, spec.path),
            Transform::from_translation(center),
        ));
    }
}

/// Depósito de vía según `_depot_gfx_table` (`track_land.h`): suelo de vía en
/// SE/SW (la salida mira a cámara) y capas BUILD por dirección (`m5 & 3`).
fn spawn_rail_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    if let Some(track_id) = crate::sprites::RAIL_DEPOT_GROUND_TRACK[dir]
        && let Some(image) = assets.rail.get(&track_id)
    {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            image.sprite(),
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                0.02,
                half_h,
            )),
        ));
    }
    for (layer_i, spec) in crate::sprites::rail_depot_build_layers(dir)
        .iter()
        .enumerate()
    {
        let Some(image) = assets.rail_depot_builds[dir].get(layer_i) else {
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
            ctx.map_tile_chunk(),
            sprite_from_atlas_or_company_white(company, image, spec.path),
            Transform::from_translation(center),
        ));
    }
}
