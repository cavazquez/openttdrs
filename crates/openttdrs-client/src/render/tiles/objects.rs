use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    RoadStopSpecDef, StationSpecDef, inclined_slope_direction, is_tunnel_entrance_slope,
    rail_type_from_tile, road_stop_spec_def, station_at_tile,
};

use super::bridge_draw::{bridge_span_at, spawn_bridge_deck};
use super::{
    catenary_under_low_bridge,
    helpers::{FLAT_WATER_LAYER_FRAC, spawn_forced_leveled_foundation},
    sloped_or_flat_image, spawn_ground_sprite, spawn_rail_foundation,
};
use crate::iso::{
    TILE_HALF_H, overlay_pos, remap_tile_offset, road_depot_build_sprite_center,
    road_stop_build_sprite_center, slope_half_h, tile_pos, tile_pos_half,
};
use crate::render::catenary_newgrf::catenary_sprite_colored;
use crate::render::station_newgrf::{NewGrfStationSpriteCache, newgrf_station_def_for_tile};
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    AirportRadarAnim, AtlasSprite, CompanyColoredSprites, MapVisualLayer, TileRenderContext,
    WaterTile, WorldAssets, sprite_from_atlas_or_company_white_colour,
};
use crate::sprites::{
    CompanyColour, StationTileClass, TransparencyOption, airport_station_overlay_rel,
    catenary_hidden, catenary_reference_sprite_id, catenary_sprite_color,
    catenary_tunnel_wire_sprite, collect_catenary_pylons_from_map_with_pcp_override,
    collect_catenary_wire_draws_from_map, is_hidden, log_unknown_station_type_once,
    rail_depot_build_layers, rail_depot_seq_gfx, rail_depot_visual_type_index,
    rail_ghost_overlay_offset, rail_pbs_reservation_offset, rail_station_draw_layers,
    rail_station_ground_track_sprite_for_type, rail_station_layer_bounds,
    rail_station_layer_for_type, rail_station_overlay_rel, rail_station_sprite_meta,
    rail_waypoint_draw_layers, rail_waypoint_layer_meta, rail_waypoint_sprite_center,
    remap_rail_sprite_id, road_depot_build_layers, road_depot_entrance_road_bits,
    road_depot_seq_gfx, road_flat_sprite_index, road_stop_build_layers,
    road_stop_drive_through_layers, road_stop_ground_index, road_stop_seq_gfx, station_tile_class,
    with_to_alpha,
};

fn buildings_hidden() -> bool {
    is_hidden(TransparencyOption::Buildings)
}

fn tint_building_sprite(mut sprite: Sprite) -> Sprite {
    sprite.color = with_to_alpha(sprite.color, TransparencyOption::Buildings);
    sprite
}

/// `DrawTile_Station` llama a `DrawFoundation(Leveled)` para toda estación
/// ferroviaria inclinada antes de emitir el rail y su reserva PBS. La capa
/// resultante es hija del cimiento con el mismo `OffsetGroundSprite` que el
/// oráculo exporta en píxeles `ZOOM_BASE`.
const fn station_rail_child_offset(tileh: u8) -> Option<(i32, i32, i32)> {
    if tileh == 0 { None } else { Some((0, -32, 0)) }
}

fn record_station_pbs_trace(tileh: u8, sprite_id: u32, fallback: bool) {
    if let Some(offset) = station_rail_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "station-pbs-reservation",
            sprite_id,
            804,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-pbs-reservation",
            "ground",
            sprite_id,
            804,
            fallback,
            (0, 0, 0),
            0,
            None,
        );
    }
}

fn station_company_palette(owner_colour: Option<CompanyColour>) -> u32 {
    775 + u32::from(owner_colour.unwrap_or_default().as_u8())
}

fn record_station_rail_ground_trace(tileh: u8, sprite_id: u32, fallback: bool) {
    if let Some(offset) = station_rail_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite(
            "station-rail-track",
            sprite_id,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-rail-track",
            "ground",
            sprite_id,
            0,
            fallback,
            (0, 0, 0),
            0,
            None,
        );
    }
}

fn record_station_rail_layer_trace(
    layer: &crate::sprites::RailStationLayer,
    owner_colour: Option<CompanyColour>,
    fallback: bool,
) {
    if crate::sprites::rail_station_roof_glass_sprite(layer.sprite_id) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "station-rail-glass",
            layer.sprite_id,
            802,
            fallback,
            (0, 0, 0),
        );
        return;
    }
    let bounds = rail_station_layer_bounds(layer.sprite_id).map(|(ex, ey, ez)| {
        TraceSpriteBounds::new(
            layer.dx as i32,
            layer.dy as i32,
            layer.dz as i32,
            ex,
            ey,
            ez,
        )
    });
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "station-rail-layer",
        "sortable",
        layer.sprite_id,
        station_company_palette(owner_colour),
        fallback,
        (0, 0, 0),
        0,
        bounds,
    );
}

/// Geometría de `_rail_catenary_sprite_data_tunnel`.
///
/// `DrawRailCatenaryOnTunnel` inicia un `SpriteCombine`; el cable queda como
/// padre sortable y el techo del túnel pasa a ser su hijo `combined`. Los
/// valores conservan `BB_Z_SEPARATOR = 7` y
/// `ELRAIL_TUNNEL_OFFSET = ELRAIL_ELEVATION - BB_Z_SEPARATOR = 3`.
const fn tunnel_catenary_trace_geometry(
    dir: u8,
) -> ((i32, i32, i32), (i32, i32, i32, i32, i32, i32)) {
    if dir & 1 == 0 {
        ((0, 7, 3), (0, 0, 7, 16, 15, 1))
    } else {
        ((7, 0, 3), (0, 0, 7, 15, 16, 1))
    }
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

/// Capas de `APT_PIER_NW_NE` / `APT_PIER` de `station_land.h`.
///
/// El sprite de suelo se emite primero mediante
/// [`WorldAssets::airport_station_gfx_sprite`]. Estas capas usan el origen
/// TILE_SEQ, las dimensiones NFO y la paleta del propietario; centrarlas en
/// la tesela convertía el túnel peatonal del aeropuerto en una pieza corrida.
fn spawn_airport_station_overlays(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    gfx: u8,
) {
    for layer in crate::sprites::airport_station_layers_for_gfx(gfx) {
        let image = match layer.sprite_id {
            2661 => &assets.airport_jetway_3,
            2662 => &assets.airport_passenger_tunnel,
            _ => continue,
        };
        let (xrel, yrel) = airport_station_overlay_rel(layer);
        let pos = overlay_pos(
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
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-airport-layer",
            "sortable",
            layer.sprite_id,
            station_company_palette(owner_colour),
            false,
            (0, 0, 0),
            0,
            Some(TraceSpriteBounds::new(
                layer.dx as i32,
                layer.dy as i32,
                layer.dz as i32,
                layer.sx,
                layer.sy,
                layer.sz,
            )),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                layer.path,
            )),
            Transform::from_translation(pos),
        ));
    }
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
    show_pbs_reservations: bool,
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
        let grass = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
        spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
    }

    let stop_kind = station_at_tile(map, stations, ctx.coord).map(|s| s.stop_kind);
    let m6 = ctx.tile.map_or(0, |t| t.m6);
    let m5 = ctx.tile.map_or(0, |t| t.m5);
    let class = station_tile_class(m6, stop_kind);
    let rail_type = ctx
        .tile
        .map_or(openttdrs_core::RailType::Rail, rail_type_from_tile);

    let rail_half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        slope_half_h(tileh)
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
                map,
                dims,
                assets,
                ctx,
                tileh,
                station_tb,
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            // OpenTTD: ground SPR_RAIL_TRACK_* bajo estación y waypoint (`station_land.h`).
            let track_sid = rail_station_ground_track_sprite_for_type(m5, tileh, rail_type);
            if class == StationTileClass::Rail {
                record_station_rail_ground_trace(
                    tileh,
                    track_sid,
                    !assets.rail.contains_key(&track_sid),
                );
            }
            if let Some(img) = assets.rail.get(&track_sid) {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    img.sprite(),
                    Transform::from_translation(tile_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        rail_base_z,
                        0.02,
                        rail_half_h,
                    )),
                ));
            }
            // `DrawStationTile`: una plataforma reservada no oscurece toda
            // la estación. OpenTTD vuelve a dibujar el SINGLE_X/Y de su eje
            // con PALETTE_CRASH. El bit vive en m6, no en la reserva m2 de
            // una vía normal.
            if show_pbs_reservations && m6 & 0x04 != 0 {
                let sid = remap_rail_sprite_id(1005 + u32::from(m5 & 1), rail_type);
                record_station_pbs_trace(tileh, sid, !assets.has_exact_pbs_rail_sprite(sid));
                if let Some(img) = assets.pbs_rail_sprite(sid) {
                    let base =
                        tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), rail_base_z, 0.026, rail_half_h);
                    let offset = rail_ghost_overlay_offset(sid);
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        img.sprite(),
                        Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
                    ));
                }
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
                for base_layer in overlay_layers {
                    // `DrawStationTile` deja los waypoints vanilla sin offset,
                    // pero suma el desplazamiento de railtype a cada capa de
                    // estación normal (`DrawRailTileSeq`).
                    let layer = if class == StationTileClass::RailWaypoint {
                        *base_layer
                    } else {
                        rail_station_layer_for_type(*base_layer, rail_type)
                    };
                    if class == StationTileClass::Rail && tileh == 0 {
                        record_station_rail_layer_trace(
                            &layer,
                            owner_colour,
                            !assets.rail.contains_key(&layer.sprite_id),
                        );
                    }
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
                            &layer,
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
                        let (xrel, yrel) = rail_station_overlay_rel(&layer, nfo_xrel, nfo_yrel);
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
                let low_bridge = catenary_under_low_bridge(map, ctx.coord, dims);
                let tint = catenary_sprite_color();
                let mut wires = Vec::new();
                if !low_bridge.hide_wires {
                    collect_catenary_wire_draws_from_map(
                        map,
                        ctx.coord,
                        dims.0,
                        dims.1,
                        crate::sprites::OTTD_MP_RAIL,
                        station_tb,
                        tileh,
                        &mut wires,
                    );
                }
                for (i, draw) in wires.into_iter().enumerate() {
                    let sid = draw.sprite_id;
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
                    collect_catenary_pylons_from_map_with_pcp_override(
                        map,
                        ctx.coord,
                        dims.0,
                        dims.1,
                        crate::sprites::OTTD_MP_RAIL,
                        station_tb,
                        tileh,
                        low_bridge.pylon_pcp_override,
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
            let is_drive_through = openttdrs_core::is_drive_through_orientation(m5);
            if tileh == 0 && !is_drive_through {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
            }
            let stub = ctx.tile.map_or(0, |t| t.m3 & 0x0F);
            if is_drive_through {
                let road_bits = if m5 == openttdrs_core::RSV_DRIVE_THROUGH_X {
                    0x0A
                } else {
                    0x05
                };
                spawn_paved_road_stop_link(
                    commands,
                    assets,
                    ctx,
                    base_z,
                    rail_half_h,
                    tileh,
                    road_bits,
                );
            } else if stub != 0 {
                spawn_road_stop_link(commands, assets, ctx, base_z, rail_half_h, tileh, stub);
            }
            let view_idx = usize::from(m5.min(5));
            if !is_drive_through {
                let ground_dir = road_stop_ground_index(m5).min(3);
                let image = if class == StationTileClass::Bus {
                    assets
                        .bus_stop_grounds
                        .get(ground_dir)
                        .cloned()
                        .unwrap_or_else(|| assets.bus_stop_grounds[0].clone())
                } else {
                    assets
                        .station_grounds
                        .get(ground_dir)
                        .cloned()
                        .unwrap_or_else(|| assets.station_grounds[0].clone())
                };
                spawn_stop_ground_sprite(commands, &image, ctx, base_z, 0.04);
            }
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
                view_idx,
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
                slope_half_h(tileh)
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
                slope_half_h(tileh)
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
                slope_half_h(tileh)
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
            spawn_airport_station_overlays(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                base_z,
                m5,
            );
            if piece == openttdrs_core::AirportPiece::Tower {
                spawn_airport_radar_overlay(commands, assets, ctx, base_z, half_h);
            }
        }
        StationTileClass::Other(_) => {
            // No dibujar una parada vial plausible cuando `m6` contiene un
            // tipo que todavía no implementamos. Magenta es el marcador de
            // recurso/tile desconocido usado por el renderer.
            log_unknown_station_type_once(m6);
            let warning_ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                warning_ground.sprite_colored(Color::srgb(1.0, 0.0, 1.0)),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.006,
                    if tileh == 0 {
                        TILE_HALF_H
                    } else {
                        slope_half_h(tileh)
                    },
                )),
            ));
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

/// Base de una parada pasante: OpenTTD usa `SPR_ROAD_PAVED_STRAIGHT_*`, no el
/// suelo de hierba/andén de una bahía convencional.
fn spawn_paved_road_stop_link(
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
        assets.road_paved[fi].sprite(),
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
            .saturating_mul(6)
            .saturating_add(u16::try_from(dir.min(5)).unwrap_or(0));
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
    let orientation = u8::try_from(dir).unwrap_or_default();
    let drive_through = road_stop_drive_through_layers(class, orientation);
    if !drive_through.is_empty() {
        let handles = match class {
            StationTileClass::Bus => &assets.bus_stop_drive_through,
            StationTileClass::Truck => &assets.truck_stop_drive_through,
            _ => return,
        };
        let axis = usize::from(orientation - openttdrs_core::RSV_DRIVE_THROUGH_X);
        for (layer_i, spec) in drive_through.iter().enumerate() {
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
            let image = &handles[axis][layer_i];
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
        return;
    }
    let handles = match class {
        StationTileClass::Bus => &assets.bus_stop_builds,
        StationTileClass::Truck => &assets.truck_stop_builds,
        _ => return,
    };
    let is_truck = class == StationTileClass::Truck;
    // OpenGFX / Action5 solo tienen bahía 0..3; DT 4/5 cae al eje.
    let build_dir = road_stop_ground_index(u8::try_from(dir).unwrap_or(0)).min(3);
    for (layer_i, spec) in road_stop_build_layers(class, build_dir).iter().enumerate() {
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
            && let Some(slot) = openttdrs_core::roadstop_action5_slot(is_truck, build_dir)
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
        let image = &handles[build_dir][layer_i];
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
    show_pbs_reservations: bool,
    map: &Map,
    dims: (u32, u32),
    stations: &[Station],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    if ctx.kind == TileKind::ShipDepot {
        // `DrawShipDepotSprite` siempre parte de `SPR_WATER_TILE`: aunque el
        // depósito tenga un TileKind propio, en el save sigue siendo MP_WATER.
        WorldDrawTrace::record_sprite("ship-depot-water", "ground", 4061, false);
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            WaterTile::ANIMATED,
            assets.water.sprite(),
            Transform::from_translation(tile_pos(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                FLAT_WATER_LAYER_FRAC,
            )),
        ));
    } else if !matches!(
        ctx.kind,
        TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge
            | TileKind::RailDepot
    ) {
        let ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
        spawn_ground_sprite(commands, &ground, Color::WHITE, ctx, slope_half_ground);
    }

    match ctx.kind {
        TileKind::RoadTunnel | TileKind::RailTunnel => {
            if !is_tunnel_entrance_slope(tileh) {
                return;
            }
            let rail = ctx.kind == TileKind::RailTunnel;
            let dir = inclined_slope_direction(tileh)
                .or_else(|| ctx.tile.map(|t| t.m5 & 0x03))
                .unwrap_or(0);
            let rail_type = ctx
                .tile
                .map_or(openttdrs_core::RailType::Rail, rail_type_from_tile);
            let rear_image = if rail {
                assets.rail_tunnel_portal_sprite(rail_type, dir)
            } else {
                assets.tunnel_portal_sprite(false, dir)
            };
            let rear_sprite_id = if rail {
                crate::sprites::rail_tunnel_rear_sprite_id(rail_type, dir)
            } else {
                crate::sprites::tunnel_rear_sprite_id(false, dir)
            };
            let front_image = if rail {
                assets.rail_tunnel_portal_front_sprite(rail_type, dir)
            } else {
                assets.tunnel_portal_front_sprite(false, dir)
            };
            let front_sprite_id = if rail {
                crate::sprites::rail_tunnel_front_sprite_id(rail_type, dir)
            } else {
                crate::sprites::tunnel_front_sprite_id(false, dir)
            };
            // OpenTTD dibuja el rear como suelo y el front como techo sortable.
            // Aunque el rear sea `DrawGroundSprite`, no es un rombo 64×31:
            // los portales mono/maglev tienen `xrel/yrel` propios. Centrar el
            // PNG como terreno desplazaba la boca hasta 20 px y dejaba la vía
            // aparentemente desconectada. Ambas capas usan su anclaje NFO.
            WorldDrawTrace::record_sprite("tunnel-rear", "ground", rear_sprite_id, false);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                rear_image.sprite(),
                Transform::from_translation(crate::sprites::tunnel_portal_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    rear_sprite_id,
                    0.0,
                )),
            ));
            // Igual que `DrawTunnelBridgeTile`: el bit de reserva de las
            // rampas/túneles ferroviarios está en m5 bit 4. La capa es un
            // SINGLE tipado, separada del portal, y debe ir entre el suelo y
            // el techo para no ocultar la boca del túnel.
            if rail
                && show_pbs_reservations
                && ctx
                    .tile
                    .is_some_and(openttdrs_core::tunnel_bridge_rail_reserved)
            {
                let sid = remap_rail_sprite_id(1005 + u32::from(dir & 1), rail_type);
                WorldDrawTrace::record_sprite_with_palette_and_geometry(
                    "tunnel-pbs-reservation",
                    "ground",
                    sid,
                    804,
                    !assets.has_exact_pbs_rail_sprite(sid),
                    (0, 0, 0),
                    0,
                    None,
                );
                if let Some(img) = assets.pbs_rail_sprite(sid) {
                    let base =
                        tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.025, slope_half_ground);
                    let offset = rail_ghost_overlay_offset(sid);
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        img.sprite(),
                        Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
                    ));
                }
            }
            let draw_tunnel_catenary = rail && !catenary_hidden() && rail_type.has_catenary();
            // El oráculo registra el cable antes del techo: es el padre del
            // `SpriteCombine` que contiene ambos. La capa Bevy conserva su
            // orden visual posterior, pero la traza modela el draw proc real.
            let tunnel_catenary_sprite = if draw_tunnel_catenary {
                let sid = catenary_tunnel_wire_sprite(dir);
                let sprite = catenary_sprite_colored(
                    assets,
                    sid,
                    catenary_sprite_color(),
                    catenary_newgrf,
                    catenary_sprites,
                    images,
                );
                let (offset, (ox, oy, oz, ex, ey, ez)) = tunnel_catenary_trace_geometry(dir);
                WorldDrawTrace::record_sprite_with_geometry(
                    "tunnel-catenary",
                    "sortable",
                    catenary_reference_sprite_id(sid),
                    sprite.is_none(),
                    offset,
                    0,
                    Some(crate::render::world_draw_trace::TraceSpriteBounds::new(
                        ox, oy, oz, ex, ey, ez,
                    )),
                );
                sprite
            } else {
                None
            };
            let (front_offset, (ox, oy, oz, ex, ey, ez)) =
                crate::sprites::tunnel_front_trace_geometry(dir);
            WorldDrawTrace::record_sprite_with_geometry(
                "tunnel-front",
                if draw_tunnel_catenary {
                    "combined"
                } else {
                    "sortable"
                },
                front_sprite_id,
                false,
                front_offset,
                0,
                Some(crate::render::world_draw_trace::TraceSpriteBounds::new(
                    ox, oy, oz, ex, ey, ez,
                )),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                front_image.sprite(),
                Transform::from_translation(crate::sprites::tunnel_portal_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    front_sprite_id,
                    0.08,
                )),
            ));
            // Wire de portal (`DrawRailCatenaryOnTunnel`) si la vía es eléctrica.
            if let Some(sprite) = tunnel_catenary_sprite {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(crate::sprites::tunnel_portal_translation(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        base_z,
                        front_sprite_id,
                        0.085,
                    )),
                ));
            }
        }
        TileKind::RoadDepot => {
            let depot_half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                slope_half_h(tileh)
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
            // `DrawTile_Rail` nivela cualquier depósito inclinado antes de
            // dibujar su suelo. No reutilizar el césped inclinado genérico:
            // las capas de suelo pasan a ser children de la fundación.
            let depot_base_z = spawn_forced_leveled_foundation(
                commands,
                map,
                dims,
                assets,
                ctx,
                tileh,
                "rail-depot",
                "rail-depot-foundation",
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            spawn_rail_depot_tile(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                depot_base_z,
                TILE_HALF_H,
                tileh,
                show_pbs_reservations,
            );
        }
        TileKind::ShipDepot => {
            spawn_ship_depot_tile(commands, assets, company, owner_colour, ctx, base_z);
        }
        TileKind::Airport => {
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                slope_half_h(tileh)
            };
            let m5 = ctx.tile.map(|t| t.m5).unwrap_or(0);
            let imported_station_gfx = ctx.tile.is_some_and(|tile| {
                let station_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
                stations.iter().any(|station| {
                    station.ottd_station_id == Some(station_id)
                        && station.airport_tiles.contains(&ctx.coord)
                })
            });
            let piece = if imported_station_gfx {
                openttdrs_core::AirportPiece::from_station_gfx(m5)
            } else {
                openttdrs_core::AirportPiece::from_m5(m5)
            };
            let sprite = if imported_station_gfx {
                assets.airport_station_gfx_sprite(m5).sprite()
            } else {
                assets.airport_piece_sprite(piece).sprite()
            };
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.04,
                    half_h,
                )),
            ));
            if imported_station_gfx {
                spawn_airport_station_overlays(
                    commands,
                    assets,
                    company,
                    owner_colour,
                    ctx,
                    base_z,
                    m5,
                );
            }
            if piece == openttdrs_core::AirportPiece::Tower {
                spawn_airport_radar_overlay(commands, assets, ctx, base_z, half_h);
            }
        }
        TileKind::RoadBridge | TileKind::RailBridge => {
            if let Some(span) = bridge_span_at(map, ctx.coord, dims) {
                spawn_bridge_deck(
                    commands,
                    map,
                    dims,
                    assets,
                    ctx,
                    &span,
                    false,
                    show_pbs_reservations,
                    catenary_newgrf,
                    catenary_sprites,
                    bridge_decks_newgrf,
                    foundation_newgrf,
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
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
) {
    if buildings_hidden() {
        return;
    }

    // `water_land.h::DrawShipDepotSprite`: WaterTileType::Depot usa dos
    // teselas. m5 bit 0 = parte norte/sur, bit 1 = eje X/Y. Los offsets y
    // metadatos vienen de los sprites vanilla 4070..4075 (OpenGFX NFO).
    let m5 = ctx.tile.map_or(0, |tile| tile.m5);
    let part_south = m5 & 0x01 != 0;
    let axis_y = m5 & 0x02 != 0;
    // `TILE_SEQ_LINE` usa un borde de 16×1 para Axis::X y uno de 1×16
    // para Axis::Y. No es el tamaño del PNG: es la caja de ordenación
    // isométrica con la que OpenTTD compone ambas mitades del depósito.
    let (extent_x, extent_y) = if axis_y { (1, 16) } else { (16, 1) };
    let layers: &[(usize, f32, f32, f32, f32, f32, f32)] = match (axis_y, part_south) {
        // Eje X, norte: 4072 / ship_depot_nw.
        (false, false) => &[(2, 0.0, 15.0, -61.0, -30.0, 64.0, 47.0)],
        // Eje X, sur: 4074 (parte trasera) + 4070 (frente SE).
        (false, true) => &[
            (4, 0.0, 0.0, -31.0, 5.0, 13.0, 12.0),
            (0, 0.0, 15.0, -61.0, -31.0, 64.0, 48.0),
        ],
        // Eje Y, norte: 4073 / ship_depot_ne.
        (true, false) => &[(3, 15.0, 0.0, -1.0, -30.0, 64.0, 47.0)],
        // Eje Y, sur: 4075 (parte trasera) + 4071 (frente SW).
        (true, true) => &[
            (5, 0.0, 0.0, 20.0, 5.0, 13.0, 12.0),
            (1, 15.0, 0.0, -1.0, -31.0, 64.0, 48.0),
        ],
    };

    const SHIP_DEPOT_PATHS: [&str; 6] = [
        "assets/opengfx/tiles/ship_depot_se_front.png",
        "assets/opengfx/tiles/ship_depot_sw_front.png",
        "assets/opengfx/tiles/ship_depot_nw.png",
        "assets/opengfx/tiles/ship_depot_ne.png",
        "assets/opengfx/tiles/ship_depot_se_rear.png",
        "assets/opengfx/tiles/ship_depot_sw_rear.png",
    ];
    // `GetCompanyPalette(owner)`: `PALETTE_RECOLOUR_START + colour`. En Kale
    // el owner es DarkBlue y por eso el oráculo expone 775.
    let company_palette = 775 + u32::from(owner_colour.unwrap_or_default().as_u8());

    for (layer_i, &(sprite_i, dx, dy, xrel, yrel, width, height)) in layers.iter().enumerate() {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "ship-depot",
            "sortable",
            4070 + sprite_i as u32,
            company_palette,
            false,
            (0, 0, 0),
            0,
            Some(crate::render::world_draw_trace::TraceSpriteBounds::new(
                dx as i32, dy as i32, 0, extent_x, extent_y, 20,
            )),
        );
        let local = remap_tile_offset(dx, dy, 0.0) * 0.5;
        let pos = overlay_pos(
            ctx.iso_pos + local,
            xrel,
            yrel,
            width,
            height,
            base_z,
            0.04 + layer_i as f32 * 0.0005,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                &assets.ship_depot[sprite_i],
                SHIP_DEPOT_PATHS[sprite_i],
            )),
            Transform::from_translation(pos),
        ));
    }
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
fn rail_depot_reservation_track_visible(dir: usize, buildings_are_hidden: bool) -> bool {
    buildings_are_hidden || matches!(dir, 1 | 2)
}

/// `DrawFoundation(Leveled)` desplaza los `DrawGroundSprite` posteriores 8
/// píxeles. El exportador los normaliza por `ZOOM_BASE`, de ahí -32.
const fn rail_depot_foundation_child_offset(tileh: u8) -> Option<(i32, i32, i32)> {
    if tileh == 0 { None } else { Some((0, -32, 0)) }
}

fn record_rail_depot_ground_trace(tileh: u8, role: &'static str, sprite_id: u32, fallback: bool) {
    if let Some(offset) = rail_depot_foundation_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite(role, sprite_id, fallback, offset);
    } else {
        WorldDrawTrace::record_sprite(role, "ground", sprite_id, fallback);
    }
}

fn record_rail_depot_reservation_trace(tileh: u8, sprite_id: u32, fallback: bool) {
    if let Some(offset) = rail_depot_foundation_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "rail-depot-pbs-reservation",
            sprite_id,
            804,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "rail-depot-pbs-reservation",
            "ground",
            sprite_id,
            804,
            fallback,
            (0, 0, 0),
            0,
            None,
        );
    }
}

#[allow(clippy::too_many_arguments)] // Parámetros del spawner comparten el contexto del tile.
fn spawn_rail_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    tileh: u8,
    show_pbs_reservations: bool,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    let rail_type = ctx
        .tile
        .map_or(openttdrs_core::RailType::Rail, rail_type_from_tile);
    if let Some(track_id) =
        crate::sprites::RAIL_DEPOT_GROUND_TRACK[dir].map(|id| remap_rail_sprite_id(id, rail_type))
    {
        let fallback = !assets.rail.contains_key(&track_id);
        record_rail_depot_ground_trace(tileh, "rail-depot-track", track_id, fallback);
        if let Some(image) = assets.rail.get(&track_id) {
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
    } else {
        // NE/NW usan `SPR_FLAT_GRASS_TILE` en `_depot_gfx_table`; no el
        // relieve de la tesela original. En pendiente también es child del
        // mismo parent de fundación que la vía de las salidas SE/SW.
        record_rail_depot_ground_trace(tileh, "rail-depot-ground", 3981, false);
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.grass.sprite(),
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                0.02,
                half_h,
            )),
        ));
    }
    // `HasDepotReservation` vive en m5 bit 4. En el depot no se codifican
    // TrackBits: la dirección fija si el overlay es SINGLE_X o SINGLE_Y.
    // `DrawTile_Rail`: con el edificio visible, las puertas NE/NW cubren
    // la vía de acceso; OpenTTD sólo muestra su reserva si el edificio se
    // vuelve transparente. Las salidas SE/SW sí permanecen a la vista.
    let reservation_track_visible = rail_depot_reservation_track_visible(dir, buildings_hidden());
    if show_pbs_reservations
        && reservation_track_visible
        && ctx.tile.is_some_and(|tile| tile.m5 & 0x10 != 0)
    {
        let sid = remap_rail_sprite_id(1005 + u32::from(dir as u8 & 1), rail_type);
        record_rail_depot_reservation_trace(tileh, sid, !assets.has_exact_pbs_rail_sprite(sid));
        if let Some(image) = assets.pbs_rail_sprite(sid) {
            let base = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.026, half_h);
            let offset = rail_pbs_reservation_offset(sid);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                image.sprite(),
                Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
            ));
        }
    }
    let depot_variant = rail_depot_visual_type_index(rail_type);
    let depot_builds = &assets.rail_depot_builds[depot_variant][dir];
    // `DrawRailTileSeq`: cada fachada es un sortable con las bounds del
    // TILE_SEQ_LINE y recolor de la compañía propietaria. En una pendiente
    // la fundación altera la altura de mundo, no la caja local de la pieza.
    let company_palette = 775 + u32::from(owner_colour.unwrap_or_default().as_u8());
    let foundation_z_delta = (i32::from(base_z) - i32::from(ctx.info.base_z)) * 8;
    for (layer_i, spec) in rail_depot_build_layers(rail_type, dir).iter().enumerate() {
        if buildings_hidden() {
            break;
        }
        let Some(image) = depot_builds.get(layer_i) else {
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "rail-depot-building",
                "sortable",
                spec.sprite_id,
                company_palette,
                true,
                (0, 0),
                foundation_z_delta,
                (0, 0, 0),
                Some(TraceSpriteBounds::new(
                    spec.dx as i32,
                    spec.dy as i32,
                    spec.dz as i32,
                    spec.sx,
                    spec.sy,
                    23,
                )),
            );
            continue;
        };
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "rail-depot-building",
            "sortable",
            spec.sprite_id,
            company_palette,
            false,
            (0, 0),
            foundation_z_delta,
            (0, 0, 0),
            Some(TraceSpriteBounds::new(
                spec.dx as i32,
                spec.dy as i32,
                spec.dz as i32,
                spec.sx,
                spec.sy,
                23,
            )),
        );
        let center = road_depot_build_sprite_center(
            ctx.iso_pos,
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            spec.z,
            rail_depot_seq_gfx(spec),
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

#[cfg(test)]
mod tests {
    use super::{
        rail_depot_foundation_child_offset, rail_depot_reservation_track_visible,
        station_rail_child_offset, tunnel_catenary_trace_geometry,
    };

    #[test]
    fn rail_depot_reservation_is_hidden_behind_visible_ne_and_nw_buildings() {
        assert!(!rail_depot_reservation_track_visible(0, false)); // NE
        assert!(rail_depot_reservation_track_visible(1, false)); // SE
        assert!(rail_depot_reservation_track_visible(2, false)); // SW
        assert!(!rail_depot_reservation_track_visible(3, false)); // NW
        assert!(rail_depot_reservation_track_visible(0, true));
        assert!(rail_depot_reservation_track_visible(3, true));
    }

    #[test]
    fn sloped_rail_station_pbs_is_child_of_the_leveled_foundation() {
        assert_eq!(station_rail_child_offset(0), None);
        assert_eq!(station_rail_child_offset(6), Some((0, -32, 0)));
        assert_eq!(station_rail_child_offset(12), Some((0, -32, 0)));
    }

    #[test]
    fn sloped_rail_depot_ground_is_child_of_the_leveled_foundation() {
        assert_eq!(rail_depot_foundation_child_offset(0), None);
        assert_eq!(rail_depot_foundation_child_offset(11), Some((0, -32, 0)));
        assert_eq!(rail_depot_foundation_child_offset(0x17), Some((0, -32, 0)));
    }

    #[test]
    fn tunnel_catenary_trace_uses_the_upstream_combined_parent_bounds() {
        // NE/SW: eje largo en X y cable desplazado 7 px en Y.
        assert_eq!(
            tunnel_catenary_trace_geometry(0),
            ((0, 7, 3), (0, 0, 7, 16, 15, 1))
        );
        // SE/NW: eje largo en Y y cable desplazado 7 px en X.
        assert_eq!(
            tunnel_catenary_trace_geometry(1),
            ((7, 0, 3), (0, 0, 7, 15, 16, 1))
        );
    }
}
