use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    RoadStopSpecDef, StationSpecDef, inclined_slope_direction, is_tunnel_entrance_slope,
    rail_type_from_tile, road_stop_spec_def, station_at_tile,
};

use super::bridge_draw::{bridge_span_at, spawn_bridge_deck};
use super::{sloped_or_flat_image, spawn_ground_sprite, spawn_rail_foundation};
use crate::iso::{
    SLOPE_HALF_H, TILE_HALF_H, remap_tile_offset, road_depot_build_sprite_center,
    road_stop_build_sprite_center, tile_pos, tile_pos_half,
};
use crate::render::catenary_newgrf::catenary_sprite_colored;
use crate::render::station_newgrf::{NewGrfStationSpriteCache, newgrf_station_def_for_tile};
use crate::render::{
    AirportRadarAnim, AtlasSprite, CompanyColoredSprites, MapVisualLayer, TileRenderContext,
    WorldAssets, sprite_from_atlas_or_company_white_colour,
};
use crate::sprites::{
    CompanyColour, StationTileClass, TransparencyOption, catenary_hidden, catenary_sprite_color,
    catenary_tunnel_wire_sprite, collect_catenary_pylons_from_map,
    collect_catenary_sprites_from_map, is_hidden, rail_station_draw_layers,
    rail_station_ground_track_sprite, rail_station_overlay_rel, rail_station_sprite_meta,
    rail_waypoint_draw_layers, rail_waypoint_layer_meta, rail_waypoint_sprite_center,
    road_depot_build_layers, road_depot_entrance_road_bits, road_depot_seq_gfx,
    road_flat_sprite_index, road_stop_build_layers, road_stop_ground_index, road_stop_seq_gfx,
    station_tile_class, with_to_alpha,
};

fn buildings_hidden() -> bool {
    is_hidden(TransparencyOption::Buildings)
}

fn tint_building_sprite(mut sprite: Sprite) -> Sprite {
    sprite.color = with_to_alpha(sprite.color, TransparencyOption::Buildings);
    sprite
}

fn spawn_airport_radar_overlay(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
) {
    let m7 = ctx.tile.map(|t| t.m7).unwrap_or(0);
    let frame = usize::from(openttdrs_core::airport_radar_frame(m7));
    let Some(radar) = assets.airport_radar.get(frame) else {
        return;
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        AirportRadarAnim { pos: ctx.coord },
        tint_building_sprite(radar.sprite()),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.055,
            half_h,
        )),
    ));
}

#[allow(clippy::too_many_arguments, clippy::needless_option_as_deref)]
pub(crate) fn spawn_station_tile(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    stations: &[Station],
    slope_half_ground: f32,
    station_catalog: &[StationSpecDef],
    road_stop_catalog: &[RoadStopSpecDef],
    mut station_sprites: Option<&mut NewGrfStationSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    roadstop_action5: &[Option<openttdrs_core::DecodedSprite>],
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
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
            let rail_base_z = spawn_rail_foundation(
                commands,
                assets,
                ctx,
                tileh,
                station_tb,
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
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
            // NewGRF: en plano, sustituir overlays OpenGFX por vista según tiletype `m5` (#46).
            let mut used_newgrf = false;
            let view_idx = openttdrs_core::station_newgrf_view_index(m5);
            if matches!(
                class,
                StationTileClass::Rail | StationTileClass::RailWaypoint
            ) && tileh == 0
                && !buildings_hidden()
                && let Some(def) =
                    newgrf_station_def_for_tile(station_catalog, map, stations, ctx.coord)
                && let Some(view) = def.newgrf_view(view_idx)
                && let (Some(cache), Some(images)) = (station_sprites.as_mut(), images.as_mut())
            {
                let colour_u8 = owner_colour.map(CompanyColour::as_u8).unwrap_or(0);
                let mut a2 = openttdrs_core::action2_eval_ctx_for_station_tile(
                    map,
                    stations,
                    ctx.coord,
                    colour_u8,
                    climate,
                    def.newgrf_type_tables.as_ref(),
                );
                a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
                    newgrf_stack,
                    def.newgrf_grfid,
                ));
                if let Some(handle) =
                    cache.handle_for_runtime(def, view_idx, owner_colour, &mut a2, images)
                {
                    let pos3 = crate::iso::overlay_pos(
                        ctx.iso_pos,
                        f32::from(view.x_offs),
                        f32::from(view.y_offs),
                        f32::from(view.width),
                        f32::from(view.height),
                        rail_base_z,
                        0.04,
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                    );
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        tint_building_sprite(Sprite {
                            image: handle,
                            color: Color::WHITE,
                            ..default()
                        }),
                        Transform::from_translation(pos3),
                    ));
                    used_newgrf = true;
                }
            }
            if !buildings_hidden() && !used_newgrf {
                for layer in overlay_layers {
                    let Some(img) = assets.rail.get(&layer.sprite_id) else {
                        continue;
                    };
                    let pos3 = if class == StationTileClass::RailWaypoint {
                        let Some((w, h, nfo_xrel, nfo_yrel)) =
                            rail_waypoint_layer_meta(layer.sprite_id)
                        else {
                            continue;
                        };
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
                        let Some((w, h, nfo_xrel, nfo_yrel)) =
                            rail_station_sprite_meta(layer.sprite_id)
                        else {
                            continue;
                        };
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
                    let sprite = if crate::sprites::rail_station_roof_glass_sprite(layer.sprite_id)
                    {
                        // OpenTTD: `PALETTE_TO_TRANSPARENT` oscurece el destino (máscara),
                        // no pinta el blob CC amarillo del PNG como vidrio tintado.
                        use crate::sprites::with_to_alpha;
                        let mut s = img.sprite_colored(Color::srgba(0.0, 0.0, 0.0, 0.28));
                        s.color = with_to_alpha(s.color, TransparencyOption::Buildings);
                        s
                    } else {
                        tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                            company,
                            owner_colour,
                            img,
                            &format!("rail_{}.png", layer.sprite_id),
                        ))
                    };
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(pos3),
                    ));
                }
            }
            if let Some(tile) = ctx.tile.filter(|t| {
                rail_type_from_tile(*t).has_catenary()
                    && openttdrs_core::station_tile_can_have_wires(t.m3)
            }) {
                let tint = catenary_sprite_color();
                let mut wires = Vec::new();
                collect_catenary_sprites_from_map(
                    map,
                    ctx.coord,
                    dims.0,
                    dims.1,
                    crate::sprites::OTTD_MP_RAIL,
                    station_tb,
                    tileh,
                    &mut wires,
                );
                for (i, sid) in wires.into_iter().enumerate() {
                    let Some(sprite) = catenary_sprite_colored(
                        assets,
                        sid,
                        tint,
                        catenary_newgrf,
                        catenary_sprites.as_deref_mut(),
                        images.as_deref_mut(),
                    ) else {
                        continue;
                    };
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(tile_pos_half(
                            ctx.tx_i32(),
                            ctx.ty_i32(),
                            rail_base_z,
                            0.035 + i as f32 * 0.0004,
                            rail_half_h,
                        )),
                    ));
                }
                if openttdrs_core::station_tile_can_have_pylons(tile.m3) {
                    let mut pylons = Vec::new();
                    collect_catenary_pylons_from_map(
                        map,
                        ctx.coord,
                        dims.0,
                        dims.1,
                        crate::sprites::OTTD_MP_RAIL,
                        station_tb,
                        tileh,
                        &mut pylons,
                    );
                    for draw in pylons {
                        let Some(sprite) = catenary_sprite_colored(
                            assets,
                            draw.sprite_id,
                            tint,
                            catenary_newgrf,
                            catenary_sprites.as_deref_mut(),
                            images.as_deref_mut(),
                        ) else {
                            continue;
                        };
                        let off = remap_tile_offset(draw.tile_dx, draw.tile_dy, 0.0) * 0.5;
                        let base = tile_pos_half(
                            ctx.tx_i32(),
                            ctx.ty_i32(),
                            rail_base_z,
                            draw.z_layer,
                            rail_half_h,
                        );
                        commands.spawn((
                            MapVisualLayer,
                            ctx.map_tile_chunk(),
                            sprite,
                            Transform::from_translation(base + Vec3::new(off.x, off.y, 0.0)),
                        ));
                    }
                }
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
            spawn_road_stop_buildings(
                commands,
                assets,
                company,
                owner_colour,
                map,
                stations,
                ctx,
                base_z,
                class,
                dir,
                road_stop_catalog,
                roadstop_action5,
                action5_sprites,
                images,
            );
        }
        StationTileClass::RoadWaypoint => {
            if tileh == 0 {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
            }
            let bits = ctx.tile.map_or(0x0A, |t| t.m3 & 0x0F);
            let flat_idx = match bits {
                0x05 => 5usize,
                _ => 10usize,
            };
            if let Some(img) = assets.road_flat.get(flat_idx) {
                spawn_stop_ground_sprite(commands, img, ctx, base_z, 0.03);
            }
        }
        StationTileClass::Dock => {
            if buildings_hidden() {
                return;
            }
            let dock_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            let axis = usize::from(m5 & 1);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tint_building_sprite(assets.dock_flat[axis].sprite()),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.03,
                    dock_half_h,
                )),
            ));
        }
        StationTileClass::Buoy => {
            if buildings_hidden() {
                return;
            }
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tint_building_sprite(assets.buoy.sprite()),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.04,
                    half_h,
                )),
            ));
        }
        StationTileClass::Airport => {
            if buildings_hidden() {
                return;
            }
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            // Los tiles `MP_STATION` importados conservan el índice
            // `StationGfx` vanilla completo, no el enum interno 0..7.
            let piece = openttdrs_core::AirportPiece::from_station_gfx(m5);
            let tower_pos = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.04, half_h);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tint_building_sprite(assets.airport_station_gfx_sprite(m5).sprite()),
                Transform::from_translation(tower_pos),
            ));
            if piece == openttdrs_core::AirportPiece::Tower {
                spawn_airport_radar_overlay(commands, assets, ctx, base_z, half_h);
            }
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

/// Tipo sintético en la caché Action5 para vistas Action3 del catálogo `RoadStops`.
const ROADSTOP_ACTION3_CACHE_TYPE: u8 = 0x14;

#[allow(clippy::too_many_arguments)]
fn spawn_road_stop_buildings(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    map: &Map,
    stations: &[Station],
    ctx: &TileRenderContext,
    base_z: u8,
    class: StationTileClass,
    dir: usize,
    road_stop_catalog: &[RoadStopSpecDef],
    roadstop_action5: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    if buildings_hidden() {
        return;
    }
    // Action3: vista NewGRF del spec persistido en la estación.
    if let Some(st) = station_at_tile(map, stations, ctx.coord)
        && let Some(spec_id) = st.road_stop_spec
        && let Some(def) = road_stop_spec_def(road_stop_catalog, spec_id)
        && let Some(view) = def.newgrf_view(dir)
        && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
    {
        let slot = spec_id
            .saturating_mul(4)
            .saturating_add(u16::try_from(dir).unwrap_or(0));
        let handle = cache.handle_for(ROADSTOP_ACTION3_CACHE_TYPE, slot, view, images);
        let pos3 = crate::iso::overlay_pos(
            ctx.iso_pos,
            f32::from(view.x_offs),
            f32::from(view.y_offs),
            f32::from(view.width),
            f32::from(view.height),
            base_z,
            0.05,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(Sprite {
                image: handle,
                color: Color::WHITE,
                ..default()
            }),
            Transform::from_translation(pos3),
        ));
        return;
    }
    let handles = match class {
        StationTileClass::Bus => &assets.bus_stop_builds,
        StationTileClass::Truck => &assets.truck_stop_builds,
        _ => return,
    };
    let is_truck = class == StationTileClass::Truck;
    for (layer_i, spec) in road_stop_build_layers(class, dir).iter().enumerate() {
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
        // Action5 `0x11`: sustituye la primera capa si hay sprite en el slot.
        if layer_i == 0
            && let Some(slot) = openttdrs_core::roadstop_action5_slot(is_truck, dir)
            && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
            && let Some(sprite) = cache.sprite_colored(
                openttdrs_core::ACTION5_TYPE_ROADSTOPS,
                slot,
                roadstop_action5,
                Color::WHITE,
                images,
            )
        {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(center),
            ));
            continue;
        }
        let image = &handles[dir][layer_i];
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                spec.path,
            )),
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_transport_object_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    map: &Map,
    dims: (u32, u32),
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
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
            // Wire de portal (`DrawRailCatenaryOnTunnel`) si la vía es eléctrica.
            if rail
                && !catenary_hidden()
                && ctx
                    .tile
                    .is_some_and(|t| rail_type_from_tile(t).has_catenary())
            {
                let sid = catenary_tunnel_wire_sprite(dir);
                if let Some(sprite) = catenary_sprite_colored(
                    assets,
                    sid,
                    catenary_sprite_color(),
                    catenary_newgrf,
                    catenary_sprites,
                    images,
                ) {
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(crate::sprites::tunnel_portal_translation(
                            ctx.tx_i32(),
                            ctx.ty_i32(),
                            base_z,
                            sprite_id,
                            0.085,
                        )),
                    ));
                }
            }
        }
        TileKind::RoadDepot => {
            let depot_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            spawn_road_depot_tile(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                base_z,
                depot_half_h,
                tileh,
            );
        }
        TileKind::RailDepot => {
            let depot_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[tileh as usize]
            };
            spawn_rail_depot_tile(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                base_z,
                depot_half_h,
            );
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
            if piece == openttdrs_core::AirportPiece::Tower {
                spawn_airport_radar_overlay(commands, assets, ctx, base_z, half_h);
            }
        }
        TileKind::RoadBridge | TileKind::RailBridge => {
            if let Some(span) = bridge_span_at(map, ctx.coord, dims) {
                spawn_bridge_deck(
                    commands,
                    assets,
                    ctx,
                    &span,
                    false,
                    catenary_newgrf,
                    catenary_sprites,
                    bridge_decks_newgrf,
                    action5_sprites,
                    images,
                );
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

#[allow(clippy::too_many_arguments)]
fn spawn_road_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
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
        if buildings_hidden() {
            break;
        }
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
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                spec.path,
            )),
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
    owner_colour: Option<CompanyColour>,
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
        if buildings_hidden() {
            break;
        }
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
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                &format!("rail_depot_{dir}_{layer_i}"),
            )),
            Transform::from_translation(center),
        ));
    }
}
