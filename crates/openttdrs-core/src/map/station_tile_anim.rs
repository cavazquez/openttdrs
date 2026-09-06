//! Animación de teselas de estación / aeropuerto (`AnimateTile_Airport`).

use std::collections::HashSet;
use std::hash::BuildHasher;

use crate::airport::{AirportPiece, airport_station_gfx_animation_frames};
use crate::airport_tile_spec::{
    AirportAnimationTrigger, AirportTileSpecDef, NEW_AIRPORT_TILE_OFFSET,
};
use crate::cargo::CargoType;
use crate::cargo_spec::CargoSpecDef;
use crate::company::Company;
use crate::industry::Industry;
use crate::map::{Map, TileCoord, TileKind};
use crate::newgrf_callback::{
    RoadStopCallbackWorld, advance_road_stop_animation_at_with_world,
    trigger_road_stop_animation_at_with_world, writeback_station_persistent_registers,
};
use crate::newgrf_sprites::{
    CALLBACK_FAILED, CBID_AIRPTILE_ANIMATION_NEXT_FRAME, CBID_AIRPTILE_ANIMATION_SPEED,
    CBID_AIRPTILE_ANIMATION_TRIGGER, CBID_STATION_ANIMATION_NEXT_FRAME,
    CBID_STATION_ANIMATION_SPEED, CBID_STATION_ANIMATION_TRIGGER,
};
use crate::road_stop_spec::{RoadStopSpecDef, road_stop_spec_def};
use crate::station::{
    STATION_TYPE_RAIL_WAYPOINT, Station, StopKind, station_at_tile, station_footprint_tiles,
    station_type_from_m6,
};
use crate::station_action2::{
    StationAction2WorldContext, action2_eval_ctx_for_station_tile_with_catalog,
    action2_eval_ctx_for_station_tile_with_catalog_and_world,
};
use crate::station_class::{StationAnimationTrigger, StationSpecDef, station_spec_def};
use crate::world_gen::Climate;

/// Frames del radar vanilla (`SPR_AIRPORT_RADAR_1` … `_12`).
pub const AIRPORT_RADAR_FRAMES: u8 = 12;

/// Avanza `m7` en las teselas airport animadas; coste O(aeropuertos), no O(mapa).
pub fn step_airport_tiles(map: &mut Map, tick: u64, stations: &[Station]) -> Vec<TileCoord> {
    // Un frame cada 3 ticks ≈ ritmo visual cercano a OpenTTD.
    if !tick.is_multiple_of(3) {
        return Vec::new();
    }
    let mut dirty = Vec::new();
    for station in stations {
        // Los saves importados pueden mezclar instalaciones bajo el mismo
        // StationID. En ese caso `ottd_station_id` identifica que `m5` es el
        // StationGfx airport real, aun si `stop_kind` no quedó Airport.
        let imported_station_gfx = station.ottd_station_id.is_some();
        if !imported_station_gfx && station.stop_kind != StopKind::Airport {
            continue;
        }
        let tiles = if station.airport_tiles.is_empty() {
            std::slice::from_ref(&station.pos)
        } else {
            station.airport_tiles.as_slice()
        };
        for &pos in tiles {
            let Some(mut tile) = map.get(pos) else {
                continue;
            };
            let frames = if imported_station_gfx {
                airport_station_gfx_animation_frames(tile.m5)
            } else if is_airport_tower_tile(tile.kind, tile.m5) {
                Some(AIRPORT_RADAR_FRAMES)
            } else {
                None
            };
            let Some(frames) = frames else {
                continue;
            };
            tile.m7 = tile.m7.wrapping_add(1) % frames;
            let _ = map.set_tile(pos, tile);
            dirty.push(pos);
        }
    }
    dirty.sort_by_key(|c| (c.x, c.y));
    dirty.dedup();
    dirty
}

/// Busca el gfx `AirportTile` global que corresponde a una coordenada.
///
/// Los aeropuertos construidos dentro del juego conservan la tabla explícita
/// `airport_tile_gfx`; los saves antiguos pueden no tenerla, en cuyo caso se
/// usa el byte `m5` como compatibilidad con el mapa nativo.
fn airport_tile_gfx(station: &Station, map: &Map, coord: TileCoord) -> Option<u16> {
    station
        .airport_tile_gfx
        .iter()
        .find(|(candidate, _)| *candidate == coord)
        .map(|(_, gfx)| *gfx)
        .or_else(|| map.get(coord).map(|tile| u16::from(tile.m5)))
}

fn airport_station_index(stations: &[Station], coord: TileCoord) -> Option<usize> {
    stations
        .iter()
        .position(|station| station.stop_kind == StopKind::Airport && station.covers_tile(coord))
}

fn airport_animation_context_with_towns(
    map: &Map,
    stations: &[Station],
    towns: &[crate::town::Town],
    catalog: &[AirportTileSpecDef],
    climate: Climate,
    newgrf_stack: &[crate::NewGrfEntry],
    coord: TileCoord,
) -> Option<(
    usize,
    AirportTileSpecDef,
    crate::newgrf_sprites::Action2EvalCtx,
)> {
    let station_index = airport_station_index(stations, coord)?;
    let gfx = airport_tile_gfx(&stations[station_index], map, coord)?;
    if gfx < NEW_AIRPORT_TILE_OFFSET {
        return None;
    }
    let def = catalog
        .iter()
        .find(|candidate| candidate.gfx.as_u16() == gfx && candidate.from_newgrf)
        .cloned()?;
    let mut ctx = crate::airport_tile_action2::action2_eval_ctx_for_airport_tile_with_towns(
        map, stations, towns, coord, catalog, &def, climate,
    );
    ctx.set_grf_params(crate::stack_params_for_grfid(
        newgrf_stack,
        def.newgrf_grfid,
    ));
    Some((station_index, def, ctx))
}

fn airport_animation_random_bits(station: &Station, map: &Map, coord: TileCoord, tick: u64) -> u32 {
    let x = coord.x.cast_unsigned();
    let y = coord.y.cast_unsigned();
    let tick = u32::try_from(tick).unwrap_or(u32::MAX);
    let tile_random = u32::from(map.get(coord).map_or(0, |tile| tile.m3));
    u32::from(station.newgrf_random_bits)
        | (tile_random << 16)
            ^ x.wrapping_mul(0x9E37_79B9)
            ^ y.wrapping_mul(0x85EB_CA6B)
            ^ tick.rotate_left(11)
}

fn airport_cargo_local_id(
    map: &Map,
    stations: &[Station],
    catalog: &[AirportTileSpecDef],
    climate: Climate,
    coord: TileCoord,
    cargo: Option<CargoType>,
    cargo_catalog: &[CargoSpecDef],
) -> u8 {
    let Some(cargo) = cargo else {
        return 0;
    };
    let Some(station_index) = airport_station_index(stations, coord) else {
        return crate::newgrf_type_tables::local_cargo_id_with_catalog(
            None,
            0,
            cargo,
            climate,
            cargo_catalog,
        );
    };
    let Some(gfx) = airport_tile_gfx(&stations[station_index], map, coord) else {
        return crate::newgrf_type_tables::local_cargo_id_with_catalog(
            None,
            0,
            cargo,
            climate,
            cargo_catalog,
        );
    };
    catalog
        .iter()
        .find(|candidate| candidate.gfx.as_u16() == gfx && candidate.from_newgrf)
        .map_or_else(
            || {
                crate::newgrf_type_tables::local_cargo_id_with_catalog(
                    None,
                    0,
                    cargo,
                    climate,
                    cargo_catalog,
                )
            },
            |def| def.newgrf_cargo_local_id_with_catalog(cargo, climate, cargo_catalog),
        )
}

fn resolve_airport_animation_callback(
    station: &mut Station,
    def: &AirportTileSpecDef,
    ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    callback: u16,
    param1: u32,
    param2: u32,
) -> u16 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };
    ctx.random_bits = param1;
    let result = runtime.resolve_callback_ctx(def.newgrf_local_id, callback, param1, param2, ctx);
    writeback_station_persistent_registers(station, ctx);
    result
}

/// Ejecuta `CBID_AIRPTILE_ANIMATION_TRIGGER` (`0x152`) para una tesela.
///
/// `OpenTTD` registra la tesela en `AnimatedTileList` cuando el callback
/// devuelve `0xFE`, la retira con `0xFF`, y fija `MAP7` para cualquier otro
/// byte. El resultado del callback se conserva junto a la instancia de
/// estación para que los eventos de carga/avión puedan reutilizarlo.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_airport_tile_animation<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    coord: TileCoord,
    trigger: AirportAnimationTrigger,
    random: Option<u32>,
    var18_extra: u8,
) -> bool {
    trigger_newgrf_airport_tile_animation_with_towns(
        map,
        tick,
        stations,
        &[],
        climate,
        catalog,
        active_tiles,
        newgrf_stack,
        coord,
        trigger,
        random,
        var18_extra,
    )
}

/// Variante del trigger de `AirportTile` con el catálogo de pueblos para la
/// variable de scope `0x42`.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_airport_tile_animation_with_towns<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    towns: &[crate::town::Town],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    coord: TileCoord,
    trigger: AirportAnimationTrigger,
    random: Option<u32>,
    var18_extra: u8,
) -> bool {
    let Some((station_index, def, mut ctx)) = airport_animation_context_with_towns(
        map,
        stations,
        towns,
        catalog,
        climate,
        newgrf_stack,
        coord,
    ) else {
        active_tiles.remove(&coord);
        return false;
    };
    if def.animation_triggers & trigger.mask() == 0 {
        return false;
    }
    let Some(mut tile) = map.get(coord) else {
        active_tiles.remove(&coord);
        return false;
    };
    let before = (tile.m7, active_tiles.contains(&coord));
    let random = random.unwrap_or_else(|| {
        airport_animation_random_bits(&stations[station_index], map, coord, tick)
    });
    let result = resolve_airport_animation_callback(
        &mut stations[station_index],
        &def,
        &mut ctx,
        CBID_AIRPTILE_ANIMATION_TRIGGER,
        random,
        trigger.callback_param(var18_extra),
    );
    if result == CALLBACK_FAILED {
        return false;
    }
    match (result & 0xFF) as u8 {
        0xFD => {}
        0xFE => {
            active_tiles.insert(coord);
        }
        0xFF => {
            active_tiles.remove(&coord);
        }
        frame => {
            tile.m7 = frame;
            active_tiles.insert(coord);
        }
    }
    if tile.m7 != before.0 {
        let _ = map.set_tile(coord, tile);
    }
    before != (tile.m7, active_tiles.contains(&coord))
}

/// Dispara un trigger de aeropuerto en todas las teselas que pertenecen a su
/// estación (`TA_WHOLE` equivalente para `AirportAnimationTrigger`).
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_airport_animation_for_station<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    station_anchor: TileCoord,
    trigger: AirportAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    trigger_newgrf_airport_animation_for_station_with_towns(
        map,
        tick,
        stations,
        &[],
        climate,
        catalog,
        active_tiles,
        newgrf_stack,
        station_anchor,
        trigger,
        cargo,
    )
}

/// Variante que conserva los pueblos para que cada tile evalúe `0x42` con la
/// misma selección de `ClosestTownFromTile` que OpenTTD.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_airport_animation_for_station_with_towns<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    towns: &[crate::town::Town],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    station_anchor: TileCoord,
    trigger: AirportAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    trigger_newgrf_airport_animation_for_station_with_towns_and_cargo_catalog(
        map,
        tick,
        stations,
        towns,
        &[],
        climate,
        catalog,
        active_tiles,
        newgrf_stack,
        station_anchor,
        trigger,
        cargo,
    )
}

/// Variante que entrega el catálogo global de `CargoSpec` para traducir
/// cargos custom a los índices CTT de cada `AirportTile`.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_airport_animation_for_station_with_towns_and_cargo_catalog<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    towns: &[crate::town::Town],
    cargo_catalog: &[CargoSpecDef],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    station_anchor: TileCoord,
    trigger: AirportAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    let Some(station) = stations
        .iter()
        .find(|station| station.pos == station_anchor && station.stop_kind == StopKind::Airport)
    else {
        return Vec::new();
    };
    let mut coords = if station.airport_tiles.is_empty() {
        vec![station.pos]
    } else {
        station.airport_tiles.clone()
    };
    coords.sort_by_key(|coord| (coord.x, coord.y));
    coords.dedup();
    coords
        .into_iter()
        .filter(|coord| {
            let var18_extra = airport_cargo_local_id(
                map,
                stations,
                catalog,
                climate,
                *coord,
                cargo,
                cargo_catalog,
            );
            trigger_newgrf_airport_tile_animation_with_towns(
                map,
                tick,
                stations,
                towns,
                climate,
                catalog,
                active_tiles,
                newgrf_stack,
                *coord,
                trigger,
                None,
                var18_extra,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn advance_newgrf_airport_tile<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    towns: &[crate::town::Town],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    coord: TileCoord,
) -> bool {
    let Some((station_index, def, mut ctx)) = airport_animation_context_with_towns(
        map,
        stations,
        towns,
        catalog,
        climate,
        newgrf_stack,
        coord,
    ) else {
        active_tiles.remove(&coord);
        return false;
    };
    let Some(mut tile) = map.get(coord) else {
        active_tiles.remove(&coord);
        return false;
    };
    let before = (tile.m7, active_tiles.contains(&coord));
    let mut speed = def.animation_speed.min(16);
    if def.has_animation_speed_callback() {
        let result = resolve_airport_animation_callback(
            &mut stations[station_index],
            &def,
            &mut ctx,
            CBID_AIRPTILE_ANIMATION_SPEED,
            0,
            0,
        );
        if result != CALLBACK_FAILED {
            speed = u8::try_from(result & 0xFF).unwrap_or(16).min(16);
        }
    }
    if !tick.is_multiple_of(1_u64 << u32::from(speed)) {
        return false;
    }

    let mut frame_set_by_callback = false;
    if def.has_animation_next_frame_callback() {
        let random = if def.animation_random_bits() {
            airport_animation_random_bits(&stations[station_index], map, coord, tick)
        } else {
            0
        };
        let result = resolve_airport_animation_callback(
            &mut stations[station_index],
            &def,
            &mut ctx,
            CBID_AIRPTILE_ANIMATION_NEXT_FRAME,
            random,
            0,
        );
        if result != CALLBACK_FAILED {
            match (result & 0xFF) as u8 {
                0xFF => {
                    active_tiles.remove(&coord);
                    frame_set_by_callback = true;
                }
                0xFE => {}
                frame => {
                    tile.m7 = frame;
                    frame_set_by_callback = true;
                }
            }
        }
    }

    if active_tiles.contains(&coord) && !frame_set_by_callback {
        if tile.m7 < def.animation_frames {
            tile.m7 = tile.m7.saturating_add(1);
        } else if tile.m7 == def.animation_frames && def.animation_loops() {
            tile.m7 = 0;
        } else {
            active_tiles.remove(&coord);
        }
    }
    if tile.m7 != before.0 {
        let _ = map.set_tile(coord, tile);
    }
    before != (tile.m7, active_tiles.contains(&coord))
}

/// Scheduler de `AirportTile` equivalente a `AnimateTile_Airport`.
///
/// Primero procesa `TileLoop` de las visitas recibidas y luego avanza la lista
/// persistida de teselas activas. Las teselas con `animation.status != 0xFF`
/// se registran al observarse por primera vez, lo que permite reanudar saves
/// antiguos que no tenían aún el `AnimatedTileList` serializado.
#[allow(clippy::too_many_arguments)]
pub fn step_newgrf_airport_tiles<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    step_newgrf_airport_tiles_with_towns(
        map,
        tick,
        stations,
        &[],
        climate,
        catalog,
        active_tiles,
        newgrf_stack,
        tile_loop_visits,
    )
}

/// Variante del scheduler con pueblos para evaluar el scope `AirportTile`
/// completo durante `TileLoop` y el avance periódico.
#[allow(clippy::too_many_arguments)]
pub fn step_newgrf_airport_tiles_with_towns<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    towns: &[crate::town::Town],
    climate: Climate,
    catalog: &[AirportTileSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    newgrf_stack: &[crate::NewGrfEntry],
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for (coord, _) in tile_loop_visits {
        if trigger_newgrf_airport_tile_animation_with_towns(
            map,
            tick,
            stations,
            towns,
            climate,
            catalog,
            active_tiles,
            newgrf_stack,
            *coord,
            AirportAnimationTrigger::TileLoop,
            None,
            0,
        ) {
            dirty.push(*coord);
        }
    }

    let mut candidates = Vec::new();
    for station in stations.iter() {
        if station.stop_kind != StopKind::Airport {
            continue;
        }
        let tiles = if station.airport_tiles.is_empty() {
            std::slice::from_ref(&station.pos)
        } else {
            station.airport_tiles.as_slice()
        };
        candidates.extend(tiles.iter().copied());
    }
    candidates.sort_by_key(|coord| (coord.x, coord.y));
    candidates.dedup();
    for coord in candidates {
        let Some((_, def, _)) = airport_animation_context_with_towns(
            map,
            stations,
            towns,
            catalog,
            climate,
            newgrf_stack,
            coord,
        ) else {
            continue;
        };
        if def.animation_status == 0xFF {
            continue;
        }
        if !active_tiles.contains(&coord) {
            // A non-looping animation that already reached its terminal frame
            // must not restart every tick. Fresh tiles start at MAP7=0.
            let frame = map.get(coord).map_or(0, |tile| tile.m7);
            if !def.animation_loops() && frame >= def.animation_frames && tick > 0 {
                continue;
            }
            active_tiles.insert(coord);
        }
    }

    let mut active: Vec<_> = active_tiles.iter().copied().collect();
    active.sort_by_key(|coord| (coord.x, coord.y));
    for coord in active {
        if advance_newgrf_airport_tile(
            map,
            tick,
            stations,
            towns,
            climate,
            catalog,
            active_tiles,
            newgrf_stack,
            coord,
        ) {
            dirty.push(coord);
        }
    }
    dirty.sort_by_key(|coord| (coord.x, coord.y));
    dirty.dedup();
    dirty
}

/// Ejecuta el subconjunto runtime de animación de paradas viales NewGRF.
///
/// La instancia `Station` contiene el frame y el registro activo equivalentes
/// a `roadstoptiledata`. Los eventos de carga, vehículos y aceptación se
/// conectan desde sus subsistemas; este scheduler sólo resuelve `TileLoop` y
/// el avance periódico de frames.
pub fn step_newgrf_road_stop_tiles(
    map: &Map,
    tick: u64,
    stations: &mut [Station],
    catalog: &[RoadStopSpecDef],
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    step_newgrf_road_stop_tiles_with_world(map, tick, stations, catalog, tile_loop_visits, None)
}

/// Variante que entrega al scheduler los pools del mundo para CB140–CB142.
pub fn step_newgrf_road_stop_tiles_with_world(
    map: &Map,
    tick: u64,
    stations: &mut [Station],
    catalog: &[RoadStopSpecDef],
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
    world: Option<RoadStopCallbackWorld<'_>>,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();

    for (coord, tile) in tile_loop_visits {
        if tile.kind != TileKind::Station {
            continue;
        }
        let Some(index) = stations
            .iter()
            .position(|station| station.covers_tile(*coord))
        else {
            continue;
        };
        let Some(spec_id) = stations[index].road_stop_spec_at(*coord) else {
            continue;
        };
        let Some(def) = road_stop_spec_def(catalog, spec_id) else {
            continue;
        };
        if trigger_road_stop_animation_at_with_world(
            def,
            &mut stations[index],
            *coord,
            tile.m5,
            StationAnimationTrigger::TileLoop,
            None,
            tick,
            world,
        ) {
            dirty.push(*coord);
        }
    }

    for station in stations.iter_mut() {
        if !matches!(station.stop_kind, StopKind::BusStop | StopKind::TruckStop) {
            continue;
        }
        let custom_tiles = station.road_stop_custom_tiles();
        for coord in custom_tiles {
            let Some(spec_id) = station.road_stop_spec_at(coord) else {
                continue;
            };
            let Some(def) = road_stop_spec_def(catalog, spec_id) else {
                continue;
            };
            let Some(tile) = map.get(coord) else {
                continue;
            };
            if advance_road_stop_animation_at_with_world(def, station, coord, tile.m5, tick, world)
            {
                dirty.push(coord);
            }
        }
    }

    dirty.sort_by_key(|coord| (coord.x, coord.y));
    dirty.dedup();
    dirty
}

/// Contexto y entidad para una tesela ferroviaria/waypoint `NewGRF` animable.
///
/// El frame reside en MAP7 (`m7`) como en `GetAnimationFrame`; los registros
/// persistentes siguen perteneciendo a la estación lógica. El contexto sale
/// de la misma ruta que usa el renderer, de modo que CB140–142 y Action2 ven
/// `40`/`42`/`43`/`4A`/`5F` de la tesela real.
#[allow(clippy::too_many_arguments)]
fn station_animation_context(
    map: &Map,
    stations: &[Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    coord: TileCoord,
) -> Option<(usize, crate::newgrf_sprites::Action2EvalCtx)> {
    let tile = map.get(coord)?;
    if tile.kind != TileKind::Station
        || !matches!(
            station_type_from_m6(tile.m6),
            0 | STATION_TYPE_RAIL_WAYPOINT
        )
    {
        return None;
    }
    let station_index = station_at_tile(map, stations, coord).and_then(|station| {
        stations
            .iter()
            .position(|candidate| candidate.pos == station.pos)
    })?;
    let station = &stations[station_index];
    if !matches!(
        station.stop_kind,
        StopKind::RailStation | StopKind::RailWaypoint
    ) {
        return None;
    }
    let def = station_spec_def(catalog, station.station_spec)?;
    if !def.from_newgrf || def.newgrf_runtime.is_none() {
        return None;
    }
    let owner_colour = companies
        .iter()
        .find(|company| company.id == station.owner)
        .map_or(0, |company| company.colour);
    let mut ctx = industries.map_or_else(
        || {
            action2_eval_ctx_for_station_tile_with_catalog(
                map,
                stations,
                catalog,
                coord,
                owner_colour,
                climate,
                def.newgrf_type_tables.as_ref(),
                def.newgrf_grf_version,
            )
        },
        |industries| {
            action2_eval_ctx_for_station_tile_with_catalog_and_world(
                map,
                stations,
                catalog,
                coord,
                owner_colour,
                climate,
                def.newgrf_type_tables.as_ref(),
                def.newgrf_grf_version,
                StationAction2WorldContext {
                    companies,
                    industries,
                    cargo_spec_catalog: cargo_catalog,
                },
            )
        },
    );
    ctx.persistent_registers
        .clone_from(&station.newgrf_persistent_regs);
    Some((station_index, ctx))
}

fn station_animation_random_bits(station: &Station, coord: TileCoord, tick: u64) -> u32 {
    let x = coord.x.cast_unsigned();
    let y = coord.y.cast_unsigned();
    let tick = u32::try_from(tick).unwrap_or(u32::MAX);
    x.wrapping_mul(0x9E37_79B9)
        ^ y.wrapping_mul(0x85EB_CA6B)
        ^ tick.rotate_left(11)
        ^ u32::from(station.newgrf_random_bits)
}

fn resolve_station_animation_callback(
    def: &StationSpecDef,
    station: &mut Station,
    ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    callback: u16,
    param1: u32,
    param2: u32,
) -> u16 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };
    ctx.random_bits = param1;
    let result = runtime.resolve_callback_ctx(def.newgrf_local_id, callback, param1, param2, ctx);
    writeback_station_persistent_registers(station, ctx);
    result
}

/// Ejecuta CB140 sobre una tesela ferroviaria/waypoint `NewGRF`.
///
/// `0xFD` conserva el estado, `0xFE` registra la tesela, `0xFF` la retira y
/// cualquier otro byte fija MAP7 y la registra. El `HashSet` es el equivalente
/// persistido del `AnimatedTileList` de OpenTTD para poder retomar el scheduler
/// tras guardar/cargar JSON.
#[allow(clippy::too_many_arguments)] // Firma explícita: el trigger muta mapa, estación y estado persistido.
fn trigger_newgrf_station_animation_inner<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    coord: TileCoord,
    trigger: StationAnimationTrigger,
    cargo: Option<CargoType>,
) -> bool {
    let Some((station_index, mut ctx)) = station_animation_context(
        map,
        stations,
        companies,
        industries,
        climate,
        catalog,
        cargo_catalog,
        coord,
    ) else {
        active_tiles.remove(&coord);
        return false;
    };
    let spec_id = stations[station_index].station_spec;
    let Some(def) = station_spec_def(catalog, spec_id) else {
        active_tiles.remove(&coord);
        return false;
    };
    if def.animation_triggers & trigger.mask() == 0 {
        return false;
    }
    let Some(mut tile) = map.get(coord) else {
        active_tiles.remove(&coord);
        return false;
    };
    let before_frame = tile.m7;
    let was_active = active_tiles.contains(&coord);
    let random = station_animation_random_bits(&stations[station_index], coord, tick);
    let result = resolve_station_animation_callback(
        def,
        &mut stations[station_index],
        &mut ctx,
        CBID_STATION_ANIMATION_TRIGGER,
        random,
        trigger
            .callback_param(cargo.map(|cargo| {
                def.newgrf_cargo_local_id_with_catalog(cargo, climate, cargo_catalog)
            })),
    );
    if result == CALLBACK_FAILED {
        return false;
    }
    match (result & 0xFF) as u8 {
        0xFD => {}
        0xFE => {
            active_tiles.insert(coord);
        }
        0xFF => {
            active_tiles.remove(&coord);
        }
        frame => {
            tile.m7 = frame;
            active_tiles.insert(coord);
        }
    }
    if tile.m7 != before_frame {
        let _ = map.set_tile(coord, tile);
    }
    tile.m7 != before_frame || was_active != active_tiles.contains(&coord)
}

/// Ejecuta CB140 sobre una tesela ferroviaria/waypoint `NewGRF`.
///
/// El trigger llega como ordinal tipado; Action0 `0x18` se compara usando su
/// máscara dentro de la función. `Built` y `TileLoop` no llevan cargo.
#[allow(clippy::too_many_arguments)] // Firma explícita: el trigger muta mapa, estación y estado persistido.
pub fn trigger_newgrf_station_animation<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    coord: TileCoord,
    trigger: StationAnimationTrigger,
) -> bool {
    trigger_newgrf_station_animation_with_industries(
        map,
        tick,
        stations,
        companies,
        None,
        climate,
        catalog,
        &[],
        active_tiles,
        coord,
        trigger,
    )
}

/// Variante con el pool de industrias para resolver `var 0x65` contra el
/// catchment vivo durante los callbacks de animación.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_station_animation_with_world<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    coord: TileCoord,
    trigger: StationAnimationTrigger,
) -> bool {
    trigger_newgrf_station_animation_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        &[],
        active_tiles,
        coord,
        trigger,
    )
}

/// Variante de [`trigger_newgrf_station_animation_with_world`] que propaga el
/// catálogo de cargos para `param2` y las variables `60`–`69` de CB140.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_station_animation_with_world_and_cargo_catalog<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    cargo_catalog: &[CargoSpecDef],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    coord: TileCoord,
    trigger: StationAnimationTrigger,
) -> bool {
    trigger_newgrf_station_animation_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        cargo_catalog,
        active_tiles,
        coord,
        trigger,
    )
}

#[allow(clippy::too_many_arguments)]
fn trigger_newgrf_station_animation_with_industries<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    coord: TileCoord,
    trigger: StationAnimationTrigger,
) -> bool {
    trigger_newgrf_station_animation_inner(
        map,
        tick,
        stations,
        companies,
        industries,
        climate,
        catalog,
        cargo_catalog,
        active_tiles,
        coord,
        trigger,
        None,
    )
}

/// Teselas ferroviarias/waypoint que pertenecen a la estación lógica anclada.
///
/// OpenTTD aplica `NewCargo`, `CargoTaken` y `AcceptanceTick` sobre toda la
/// estación, pero deja `Built`/`TileLoop` en una sola tesela. La asignación por
/// ancla evita que dos estaciones contiguas compartan por accidente un CB140.
fn station_animation_whole_tiles(
    map: &Map,
    stations: &[Station],
    station_anchor: TileCoord,
) -> Vec<TileCoord> {
    let Some(station) = stations
        .iter()
        .find(|station| station.pos == station_anchor)
    else {
        return Vec::new();
    };
    if !matches!(
        station.stop_kind,
        StopKind::RailStation | StopKind::RailWaypoint
    ) {
        return Vec::new();
    }
    let mut tiles: Vec<_> = station_footprint_tiles(map, station_anchor)
        .into_iter()
        .filter(|coord| {
            station_at_tile(map, stations, *coord)
                .is_some_and(|candidate| candidate.pos == station_anchor)
        })
        .collect();
    tiles.sort_by_key(|coord| (coord.x, coord.y));
    tiles.dedup();
    tiles
}

/// Ejecuta CB140 sobre todas las teselas de la estación (`TA_WHOLE`).
///
/// `cargo` se traduce al id local del GRF por cada spec antes de llenar los
/// bits 8..15 de `var 18`.
#[allow(clippy::too_many_arguments)] // La mutación toca mapa, estaciones y lista activa.
pub fn trigger_newgrf_station_animation_for_station<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger: StationAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    trigger_newgrf_station_animation_for_station_with_industries(
        map,
        tick,
        stations,
        companies,
        None,
        climate,
        catalog,
        &[],
        active_tiles,
        station_anchor,
        trigger,
        cargo,
    )
}

/// Variante de `TA_WHOLE` con el pool de industrias del mundo.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_station_animation_for_station_with_world<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger: StationAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    trigger_newgrf_station_animation_for_station_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        &[],
        active_tiles,
        station_anchor,
        trigger,
        cargo,
    )
}

/// Variante catálogo-aware de [`trigger_newgrf_station_animation_for_station_with_world`].
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_station_animation_for_station_with_world_and_cargo_catalog<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    cargo_catalog: &[CargoSpecDef],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger: StationAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    trigger_newgrf_station_animation_for_station_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        cargo_catalog,
        active_tiles,
        station_anchor,
        trigger,
        cargo,
    )
}

#[allow(clippy::too_many_arguments)]
fn trigger_newgrf_station_animation_for_station_with_industries<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger: StationAnimationTrigger,
    cargo: Option<CargoType>,
) -> Vec<TileCoord> {
    let tiles = station_animation_whole_tiles(map, stations, station_anchor);
    let mut dirty = Vec::new();
    for coord in tiles {
        if trigger_newgrf_station_animation_inner(
            map,
            tick,
            stations,
            companies,
            industries,
            climate,
            catalog,
            cargo_catalog,
            active_tiles,
            coord,
            trigger,
            cargo,
        ) {
            dirty.push(coord);
        }
    }
    dirty
}

/// Ejecuta CB140 en la plataforma que contiene `trigger_tile` (`TA_PLATFORM`).
///
/// Es la semántica necesaria para eventos de carga/descarga de tren. Las
/// plataformas vecinas pertenecientes a otra estación quedan filtradas por la
/// misma asignación lógica usada en `station_animation_whole_tiles`.
#[allow(clippy::too_many_arguments)] // La mutación toca mapa, estaciones y lista activa.
pub fn trigger_newgrf_station_animation_for_platform<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger_tile: TileCoord,
    trigger: StationAnimationTrigger,
) -> Vec<TileCoord> {
    trigger_newgrf_station_animation_for_platform_with_industries(
        map,
        tick,
        stations,
        companies,
        None,
        climate,
        catalog,
        &[],
        active_tiles,
        station_anchor,
        trigger_tile,
        trigger,
    )
}

/// Variante de `TA_PLATFORM` con el pool de industrias del mundo.
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_station_animation_for_platform_with_world<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger_tile: TileCoord,
    trigger: StationAnimationTrigger,
) -> Vec<TileCoord> {
    trigger_newgrf_station_animation_for_platform_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        &[],
        active_tiles,
        station_anchor,
        trigger_tile,
        trigger,
    )
}

/// Variante catálogo-aware de [`trigger_newgrf_station_animation_for_platform_with_world`].
#[allow(clippy::too_many_arguments)]
pub fn trigger_newgrf_station_animation_for_platform_with_world_and_cargo_catalog<
    S: BuildHasher,
>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    cargo_catalog: &[CargoSpecDef],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger_tile: TileCoord,
    trigger: StationAnimationTrigger,
) -> Vec<TileCoord> {
    trigger_newgrf_station_animation_for_platform_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        cargo_catalog,
        active_tiles,
        station_anchor,
        trigger_tile,
        trigger,
    )
}

#[allow(clippy::too_many_arguments)]
fn trigger_newgrf_station_animation_for_platform_with_industries<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    station_anchor: TileCoord,
    trigger_tile: TileCoord,
    trigger: StationAnimationTrigger,
) -> Vec<TileCoord> {
    let Some(station) = stations
        .iter()
        .find(|station| station.pos == station_anchor)
    else {
        return Vec::new();
    };
    if station.stop_kind != StopKind::RailStation {
        return Vec::new();
    }
    let mut tiles =
        crate::station::rail_station_platform_track_tiles(map, station_anchor, trigger_tile);
    tiles.retain(|coord| {
        station_at_tile(map, stations, *coord)
            .is_some_and(|candidate| candidate.pos == station_anchor)
    });
    tiles.sort_by_key(|coord| (coord.x, coord.y));
    tiles.dedup();

    let mut dirty = Vec::new();
    for coord in tiles {
        if trigger_newgrf_station_animation_inner(
            map,
            tick,
            stations,
            companies,
            industries,
            climate,
            catalog,
            cargo_catalog,
            active_tiles,
            coord,
            trigger,
            None,
        ) {
            dirty.push(coord);
        }
    }
    dirty
}

#[allow(clippy::too_many_arguments)] // Misma entidad mutada que el trigger público.
fn advance_newgrf_station_tile<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    coord: TileCoord,
) -> bool {
    let Some((station_index, mut ctx)) = station_animation_context(
        map,
        stations,
        companies,
        industries,
        climate,
        catalog,
        cargo_catalog,
        coord,
    ) else {
        active_tiles.remove(&coord);
        return false;
    };
    let spec_id = stations[station_index].station_spec;
    let Some(def) = station_spec_def(catalog, spec_id) else {
        active_tiles.remove(&coord);
        return false;
    };
    let Some(mut tile) = map.get(coord) else {
        active_tiles.remove(&coord);
        return false;
    };
    let before_frame = tile.m7;
    let was_active = active_tiles.contains(&coord);
    let mut speed = def.animation_speed.min(16);
    if def.has_animation_speed_callback() {
        let result = resolve_station_animation_callback(
            def,
            &mut stations[station_index],
            &mut ctx,
            CBID_STATION_ANIMATION_SPEED,
            0,
            0,
        );
        if result != CALLBACK_FAILED {
            speed = u8::try_from(result & 0xFF).unwrap_or(16).min(16);
        }
    }
    if !tick.is_multiple_of(1_u64 << u32::from(speed)) {
        return false;
    }

    let mut frame_set_by_callback = false;
    if def.has_animation_next_frame_callback() {
        let random = if def.animation_next_frame_uses_random_bits() {
            station_animation_random_bits(&stations[station_index], coord, tick)
        } else {
            0
        };
        let result = resolve_station_animation_callback(
            def,
            &mut stations[station_index],
            &mut ctx,
            CBID_STATION_ANIMATION_NEXT_FRAME,
            random,
            0,
        );
        if result != CALLBACK_FAILED {
            match (result & 0xFF) as u8 {
                0xFF => {
                    active_tiles.remove(&coord);
                    frame_set_by_callback = true;
                }
                0xFE => {}
                frame => {
                    tile.m7 = frame;
                    frame_set_by_callback = true;
                }
            }
        }
    }

    if active_tiles.contains(&coord) && !frame_set_by_callback {
        if tile.m7 < def.animation_frames {
            tile.m7 = tile.m7.saturating_add(1);
        } else if tile.m7 == def.animation_frames && def.animation_loops() {
            tile.m7 = 0;
        } else {
            active_tiles.remove(&coord);
        }
    }
    if tile.m7 != before_frame {
        let _ = map.set_tile(coord, tile);
    }
    tile.m7 != before_frame || was_active != active_tiles.contains(&coord)
}

/// Ejecuta el scheduler CB140–142 de estaciones ferroviarias y waypoints NewGRF.
///
/// El `TileLoop` se dispara aquí; carga, vehículos, aceptación y reserva se
/// conectan desde los eventos reales de sus respectivos subsistemas.
#[allow(clippy::too_many_arguments)] // Scheduler integrado: evita empaquetar préstamos mutables artificiales.
pub fn step_newgrf_station_tiles<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    step_newgrf_station_tiles_with_industries(
        map,
        tick,
        stations,
        companies,
        None,
        climate,
        catalog,
        &[],
        active_tiles,
        tile_loop_visits,
    )
}

/// Variante del scheduler con el pool de industrias del mundo.
#[allow(clippy::too_many_arguments)]
pub fn step_newgrf_station_tiles_with_world<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    step_newgrf_station_tiles_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        &[],
        active_tiles,
        tile_loop_visits,
    )
}

/// Variante del scheduler que entrega el catálogo de cargos a cada CB140–142
/// de estación ferroviaria/waypoint.
#[allow(clippy::too_many_arguments)]
pub fn step_newgrf_station_tiles_with_world_and_cargo_catalog<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: &[Industry],
    cargo_catalog: &[CargoSpecDef],
    climate: Climate,
    catalog: &[StationSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    step_newgrf_station_tiles_with_industries(
        map,
        tick,
        stations,
        companies,
        Some(industries),
        climate,
        catalog,
        cargo_catalog,
        active_tiles,
        tile_loop_visits,
    )
}

#[allow(clippy::too_many_arguments)]
fn step_newgrf_station_tiles_with_industries<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    stations: &mut [Station],
    companies: &[Company],
    industries: Option<&[Industry]>,
    climate: Climate,
    catalog: &[StationSpecDef],
    cargo_catalog: &[CargoSpecDef],
    active_tiles: &mut HashSet<TileCoord, S>,
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for (coord, _) in tile_loop_visits {
        if trigger_newgrf_station_animation_with_industries(
            map,
            tick,
            stations,
            companies,
            industries,
            climate,
            catalog,
            cargo_catalog,
            active_tiles,
            *coord,
            StationAnimationTrigger::TileLoop,
        ) {
            dirty.push(*coord);
        }
    }

    let mut active: Vec<_> = active_tiles.iter().copied().collect();
    active.sort_by_key(|coord| (coord.x, coord.y));
    for coord in active {
        if advance_newgrf_station_tile(
            map,
            tick,
            stations,
            companies,
            industries,
            climate,
            catalog,
            cargo_catalog,
            active_tiles,
            coord,
        ) {
            dirty.push(coord);
        }
    }
    dirty.sort_by_key(|coord| (coord.x, coord.y));
    dirty.dedup();
    dirty
}

/// Frame de radar 0..11 desde `m7`.
#[must_use]
pub const fn airport_radar_frame(m7: u8) -> u8 {
    m7 % AIRPORT_RADAR_FRAMES
}

#[must_use]
pub fn is_airport_tower_tile(kind: TileKind, m5: u8) -> bool {
    kind == TileKind::Airport && AirportPiece::from_m5(m5) == AirportPiece::Tower
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::road_stop_spec::{
        ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP, ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME,
        ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED, ROADSTOP_DRAW_MODE_DEFAULT, RoadStopSpecDef,
    };

    fn callback_literal(value: u8) -> Action2VarEntry {
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x1A,
                param: None,
                adjust: Action2VarAdjust {
                    shift: 0,
                    and_mask: u32::from(value),
                    ..Action2VarAdjust::default()
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        }
    }

    fn airport_animation_callbacks() -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: u32::MAX,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![
                    (
                        4,
                        u32::from(CBID_AIRPTILE_ANIMATION_TRIGGER),
                        u32::from(CBID_AIRPTILE_ANIMATION_TRIGGER),
                    ),
                    (
                        5,
                        u32::from(CBID_AIRPTILE_ANIMATION_NEXT_FRAME),
                        u32::from(CBID_AIRPTILE_ANIMATION_NEXT_FRAME),
                    ),
                    (
                        6,
                        u32::from(CBID_AIRPTILE_ANIMATION_SPEED),
                        u32::from(CBID_AIRPTILE_ANIMATION_SPEED),
                    ),
                ],
                default: 0,
            },
        );
        // Trigger registra, next-frame fija el frame 3 y speed espera 2^2 ticks.
        gfx.action2_var.insert(4, callback_literal(0xFE));
        gfx.action2_var.insert(5, callback_literal(3));
        gfx.action2_var.insert(6, callback_literal(2));
        gfx
    }

    /// Runtime sintético que distingue 0x140, 0x141 y 0x142 por el byte bajo
    /// de `var 0x0C`, como hacen los callbacks de 15 bits en Action2.
    fn road_stop_animation_callbacks() -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(4, 0x40, 0x40), (5, 0x41, 0x41), (6, 0x42, 0x42)],
                default: 0,
            },
        );
        // CB140 inicia; CB141 fija el frame 3; CB142 espera 2^2 ticks.
        gfx.action2_var.insert(4, callback_literal(0xFE));
        gfx.action2_var.insert(5, callback_literal(3));
        gfx.action2_var.insert(6, callback_literal(2));
        gfx
    }

    fn animated_road_stop_spec() -> RoadStopSpecDef {
        RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "Animada".into(),
            short_label: "ANIM".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 0x414E_494D,
            newgrf_local_id: 0,
            newgrf_grf_version: 0,
            draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            callback_mask: ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME
                | ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED,
            animation_status: 1,
            animation_frames: 5,
            animation_speed: 0,
            animation_triggers: ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(road_stop_animation_callbacks())),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
        }
    }

    /// Runtime sintético para CB140, CB141 y CB142 de estación ferroviaria.
    fn station_animation_callbacks() -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(4, 0x40, 0x40), (5, 0x41, 0x41), (6, 0x42, 0x42)],
                default: 0,
            },
        );
        // CB140 registra; CB141 fija frame 3; CB142 da espera 2^2 ticks.
        gfx.action2_var.insert(4, callback_literal(0xFE));
        gfx.action2_var.insert(5, callback_literal(3));
        gfx.action2_var.insert(6, callback_literal(2));
        gfx
    }

    /// Callback CB140 que devuelve un byte de `var 18`; permite verificar por
    /// separado el ordinal del trigger (byte bajo) y el cargo local (alto).
    fn station_trigger_parameter_callbacks(shift: u8) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x18,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    #[test]
    fn radar_frame_cycles_on_tower() {
        let mut map = Map::new_flat(4, 4, 1);
        let pos = TileCoord::new(1, 1);
        let mut tile = map.get(pos).unwrap();
        tile.kind = TileKind::Airport;
        tile.m5 = AirportPiece::Tower as u8;
        map.set_tile(pos, tile).unwrap();

        let mut station = Station::new_with_kind(pos, StopKind::Airport);
        station.airport_tiles = vec![pos];

        assert!(is_airport_tower_tile(
            TileKind::Airport,
            AirportPiece::Tower as u8
        ));
        assert!(!is_airport_tower_tile(
            TileKind::Airport,
            AirportPiece::Apron as u8
        ));

        let dirty = step_airport_tiles(&mut map, 3, &[station.clone()]);
        assert_eq!(dirty, vec![pos]);
        assert_eq!(map.get(pos).unwrap().m7, 1);
        assert_eq!(airport_radar_frame(1), 1);

        let _ = step_airport_tiles(&mut map, 6, &[station.clone()]);
        let _ = step_airport_tiles(&mut map, 9, &[station]);
        assert_eq!(map.get(pos).unwrap().m7, 3);

        assert!(step_airport_tiles(&mut map, 4, &[]).is_empty());
    }

    #[test]
    fn radar_ignores_map_tiles_not_listed_in_stations() {
        let mut map = Map::new_flat(8, 8, 1);
        let pos = TileCoord::new(3, 3);
        let mut tile = map.get(pos).unwrap();
        tile.kind = TileKind::Airport;
        tile.m5 = AirportPiece::Tower as u8;
        map.set_tile(pos, tile).unwrap();
        assert!(step_airport_tiles(&mut map, 3, &[]).is_empty());
        assert_eq!(map.get(pos).unwrap().m7, 0);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn newgrf_airport_animation_runs_trigger_speed_next_frame_and_roundtrips() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Airport;
        tile.mapt = 0x50;
        tile.m5 = AirportPiece::Apron as u8;
        map.set_tile(coord, tile).unwrap();

        let mut station = Station::new_with_kind(coord, StopKind::Airport);
        station.airport_tiles = vec![coord];
        station.airport_tile_gfx = vec![(coord, 74)];
        let mut stations = vec![station];
        let mut catalog = vec![AirportTileSpecDef {
            gfx: crate::AirportTileGfxId(74),
            subst_id: 24,
            from_newgrf: true,
            callback_mask: 0x03,
            animation_frames: 5,
            animation_status: 1,
            animation_speed: 0,
            animation_triggers: AirportAnimationTrigger::Built.mask()
                | AirportAnimationTrigger::TileLoop.mask()
                | AirportAnimationTrigger::NewCargo.mask(),
            animation_special_flags: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 0x4150_0001,
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(airport_animation_callbacks())),
        }];
        let stack = vec![crate::NewGrfEntry::new("airport.grf", 0x4150_0001)];
        let mut active = HashSet::new();

        assert!(trigger_newgrf_airport_tile_animation(
            &mut map,
            1,
            &mut stations,
            Climate::Temperate,
            &catalog,
            &mut active,
            &stack,
            coord,
            AirportAnimationTrigger::Built,
            None,
            0,
        ));
        assert!(active.contains(&coord));
        assert_eq!(map.get(coord).unwrap().m7, 0);

        // CB154 devuelve 2: el tick 1 no consulta CB153 todavía.
        let dirty = step_newgrf_airport_tiles(
            &mut map,
            1,
            &mut stations,
            Climate::Temperate,
            &catalog,
            &mut active,
            &stack,
            &[],
        );
        assert!(dirty.is_empty());
        assert_eq!(map.get(coord).unwrap().m7, 0);

        let dirty = step_newgrf_airport_tiles(
            &mut map,
            4,
            &mut stations,
            Climate::Temperate,
            &catalog,
            &mut active,
            &stack,
            &[],
        );
        assert_eq!(dirty, vec![coord]);
        assert_eq!(map.get(coord).unwrap().m7, 3);

        // Un CTT explícito debe ganar sobre el bitnum global al construir
        // `var18` para cada AirportTile del GRF.
        catalog[0].newgrf_grf_version = 8;
        catalog[0].newgrf_type_tables = Some(crate::newgrf_type_tables::GrfTypeTranslationTables {
            cargo: vec![
                *b"PASS", *b"MAIL", *b"GOOD", *b"WOOD", *b"GRAI", *b"COAL", *b"TOFU",
            ],
            ..Default::default()
        });
        catalog[0].newgrf_runtime = Some(Box::new(station_trigger_parameter_callbacks(8)));
        let dirty = trigger_newgrf_airport_animation_for_station(
            &mut map,
            5,
            &mut stations,
            Climate::Temperate,
            &catalog,
            &mut active,
            &stack,
            coord,
            AirportAnimationTrigger::NewCargo,
            Some(CargoType::Coal),
        );
        assert_eq!(dirty, vec![coord]);
        assert_eq!(map.get(coord).unwrap().m7, 5);

        let cargo_catalog = vec![crate::CargoSpecDef {
            id: CargoType::Custom(0).cargo_id(),
            label: "TOFU".to_owned(),
            name: "Tofu".to_owned(),
            from_newgrf: true,
            ..crate::CargoSpecDef::default()
        }];
        let dirty = trigger_newgrf_airport_animation_for_station_with_towns_and_cargo_catalog(
            &mut map,
            6,
            &mut stations,
            &[],
            &cargo_catalog,
            Climate::Temperate,
            &catalog,
            &mut active,
            &stack,
            coord,
            AirportAnimationTrigger::NewCargo,
            Some(CargoType::Custom(0)),
        );
        assert_eq!(dirty, vec![coord]);
        assert_eq!(map.get(coord).unwrap().m7, 6);

        let mut state = crate::GameState::from_map(map);
        state.stations = stations;
        state.airport_tile_spec_catalog = catalog;
        state.newgrf_stack = stack;
        state.newgrf_animated_airport_tiles = active;
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        assert_eq!(loaded.map.get(coord).unwrap().m7, 6);
        assert!(loaded.newgrf_animated_airport_tiles.contains(&coord));
    }

    #[test]
    fn imported_airport_animates_only_the_explicit_station_gfx_variants() {
        let mut map = Map::new_flat(8, 8, 1);
        let radar = TileCoord::new(1, 1);
        let flag = TileCoord::new(2, 1);
        let static_tower = TileCoord::new(3, 1);

        for (pos, gfx) in [(radar, 51), (flag, 39), (static_tower, 47)] {
            let mut tile = map.get(pos).unwrap();
            tile.kind = TileKind::Airport;
            tile.m5 = gfx;
            map.set_tile(pos, tile).unwrap();
        }

        let mut station = Station::new_with_kind(radar, StopKind::RailStation);
        station.ottd_station_id = Some(77);
        station.airport_tiles = vec![radar, flag, static_tower];

        let dirty = step_airport_tiles(&mut map, 3, &[station.clone()]);
        assert_eq!(dirty, vec![radar, flag]);
        assert_eq!(map.get(radar).unwrap().m7, 1);
        assert_eq!(map.get(flag).unwrap().m7, 1);
        assert_eq!(map.get(static_tower).unwrap().m7, 0);

        for tick in [6, 9, 12] {
            let _ = step_airport_tiles(&mut map, tick, &[station.clone()]);
        }
        assert_eq!(map.get(flag).unwrap().m7, 0, "flag has four frames");
        assert_eq!(map.get(radar).unwrap().m7, 4, "radar has twelve frames");
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures y JSON son locales a esta regresión.
    fn newgrf_road_stop_animation_runs_trigger_speed_next_frame_and_roundtrips() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Station;
        tile.m5 = crate::RSV_BAY_NW;
        map.set_tile(coord, tile).unwrap();

        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        let mut stations = vec![station];
        let catalog = vec![animated_road_stop_spec()];

        let dirty = step_newgrf_road_stop_tiles(
            &map,
            1,
            &mut stations,
            &catalog,
            &[(coord, map.get(coord).unwrap())],
        );
        assert_eq!(dirty, vec![coord]);
        assert!(stations[0].road_stop_animation_active);
        // CB142 = 2: en el tick 1 todavía no se consulta el frame siguiente.
        assert_eq!(stations[0].road_stop_animation_frame, 0);

        let dirty = step_newgrf_road_stop_tiles(&map, 4, &mut stations, &catalog, &[]);
        assert_eq!(dirty, vec![coord]);
        // CB141 = 3: el frame no es el avance lineal de fallback.
        assert_eq!(stations[0].road_stop_animation_frame, 3);

        let mut state = crate::GameState::from_map(map);
        state.stations = stations;
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        assert_eq!(loaded.stations[0].road_stop_animation_frame, 3);
        assert!(loaded.stations[0].road_stop_animation_active);
    }

    #[test]
    fn newgrf_station_animation_runs_built_tileloop_speed_next_frame_and_roundtrips() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Station;
        tile.mapt = 0x50;
        tile.m5 = 0;
        tile.m6 = 0; // estación ferroviaria
        map.set_tile(coord, tile).unwrap();

        let station = Station::new_with_kind(coord, StopKind::RailStation);
        let mut stations = vec![station];
        let mut catalog = crate::vanilla_station_spec_catalog();
        let def = &mut catalog[0];
        def.from_newgrf = true;
        def.callback_mask = crate::STATION_CALLBACK_ANIMATION_NEXT_FRAME_MASK
            | crate::STATION_CALLBACK_ANIMATION_SPEED_MASK;
        def.animation_status = 1;
        def.animation_frames = 5;
        def.animation_speed = 0;
        def.animation_triggers =
            crate::STATION_ANIMATION_TRIGGER_BUILT | crate::STATION_ANIMATION_TRIGGER_TILE_LOOP;
        def.newgrf_runtime = Some(Box::new(station_animation_callbacks()));
        let companies = vec![crate::Company::player(crate::CompanyEconomy::default(), 0)];
        let mut active = HashSet::new();

        assert!(trigger_newgrf_station_animation(
            &mut map,
            1,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            coord,
            StationAnimationTrigger::Built,
        ));
        assert!(active.contains(&coord));
        assert_eq!(map.get(coord).unwrap().m7, 0);

        // TileLoop también llega a CB140; con CB142=2 aún no avanza frame.
        active.clear();
        let visit = map.get(coord).unwrap();
        let dirty = step_newgrf_station_tiles(
            &mut map,
            1,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            &[(coord, visit)],
        );
        assert_eq!(dirty, vec![coord]);
        assert!(active.contains(&coord));
        assert_eq!(map.get(coord).unwrap().m7, 0);

        let dirty = step_newgrf_station_tiles(
            &mut map,
            4,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            &[],
        );
        assert_eq!(dirty, vec![coord]);
        assert_eq!(map.get(coord).unwrap().m7, 3, "CB141 fija el frame MAP7");

        let mut state = crate::GameState::from_map(map);
        state.stations = stations;
        state.station_spec_catalog = catalog;
        state.newgrf_animated_station_tiles = active;
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        assert_eq!(loaded.map.get(coord).unwrap().m7, 3);
        assert!(loaded.newgrf_animated_station_tiles.contains(&coord));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Comprueba de punta a punta el contrato CB140.
    fn station_cb140_uses_ordinals_cargo_translation_and_trigger_areas() {
        let first = TileCoord::new(1, 1);
        let second = TileCoord::new(2, 1);
        let mut map = Map::new_flat(5, 4, 0);
        for coord in [first, second] {
            let mut tile = map.get(coord).unwrap();
            tile.kind = TileKind::Station;
            tile.mapt = 0x50;
            tile.m5 = 0; // andén sobre eje X, ambas teselas misma plataforma
            tile.m6 = 0;
            map.set_tile(coord, tile).unwrap();
        }
        let mut stations = vec![Station::new_with_kind(first, StopKind::RailStation)];
        let mut catalog = crate::vanilla_station_spec_catalog();
        let def = &mut catalog[0];
        def.from_newgrf = true;
        def.animation_triggers = crate::STATION_ANIMATION_TRIGGER_BUILT
            | crate::STATION_ANIMATION_TRIGGER_NEW_CARGO
            | crate::STATION_ANIMATION_TRIGGER_CARGO_TAKEN
            | crate::STATION_ANIMATION_TRIGGER_VEHICLE_LOADS
            | crate::STATION_ANIMATION_TRIGGER_TILE_LOOP;
        def.newgrf_grf_version = 8;
        def.newgrf_type_tables = Some(crate::newgrf_type_tables::GrfTypeTranslationTables {
            cargo: vec![
                *b"PASS", *b"MAIL", *b"GOOD", *b"WOOD", *b"GRAI", *b"COAL", *b"TOFU",
            ],
            ..Default::default()
        });
        def.newgrf_runtime = Some(Box::new(station_trigger_parameter_callbacks(0)));
        let companies = vec![crate::Company::player(crate::CompanyEconomy::default(), 0)];
        let mut active = HashSet::new();

        assert_eq!(StationAnimationTrigger::Built.callback_param(None), 0);
        assert_eq!(StationAnimationTrigger::TileLoop.callback_param(None), 7);
        assert_eq!(
            StationAnimationTrigger::NewCargo.callback_param(Some(5)),
            0x0501
        );
        assert_eq!(
            StationAnimationTrigger::CargoTaken.callback_param(Some(5)),
            0x0502
        );

        assert!(trigger_newgrf_station_animation(
            &mut map,
            1,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            first,
            StationAnimationTrigger::Built,
        ));
        assert_eq!(map.get(first).unwrap().m7, 0);

        assert!(trigger_newgrf_station_animation(
            &mut map,
            2,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            first,
            StationAnimationTrigger::TileLoop,
        ));
        assert_eq!(
            map.get(first).unwrap().m7,
            7,
            "CB140 recibe el ordinal TileLoop=7, no la máscara 128"
        );

        catalog[0].newgrf_runtime = Some(Box::new(station_trigger_parameter_callbacks(8)));
        let dirty = trigger_newgrf_station_animation_for_station(
            &mut map,
            3,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            first,
            StationAnimationTrigger::NewCargo,
            Some(CargoType::Coal),
        );
        assert_eq!(dirty, vec![first, second]);
        assert_eq!(map.get(first).unwrap().m7, 5);
        assert_eq!(map.get(second).unwrap().m7, 5);

        // Los cargos Action0 definidos por un GRF no tienen label en
        // `CargoType`; el catálogo global debe resolverlo para que CB140 vea
        // el índice local CTT (TOFU=6), en vez del fallback sintético CSTM.
        let cargo_catalog = vec![crate::CargoSpecDef {
            id: CargoType::Custom(0).cargo_id(),
            label: "TOFU".to_owned(),
            name: "Tofu".to_owned(),
            from_newgrf: true,
            ..crate::CargoSpecDef::default()
        }];
        active.clear();
        let dirty = trigger_newgrf_station_animation_for_station_with_world_and_cargo_catalog(
            &mut map,
            4,
            &mut stations,
            &companies,
            &[],
            &cargo_catalog,
            Climate::Temperate,
            &catalog,
            &mut active,
            first,
            StationAnimationTrigger::NewCargo,
            Some(CargoType::Custom(0)),
        );
        assert_eq!(dirty, vec![first, second]);
        assert_eq!(map.get(first).unwrap().m7, 6);
        assert_eq!(map.get(second).unwrap().m7, 6);

        catalog[0].newgrf_runtime = Some(Box::new(station_trigger_parameter_callbacks(0)));
        active.clear();
        for coord in [first, second] {
            let mut tile = map.get(coord).unwrap();
            tile.m7 = 0;
            map.set_tile(coord, tile).unwrap();
        }
        let dirty = trigger_newgrf_station_animation_for_platform(
            &mut map,
            4,
            &mut stations,
            &companies,
            Climate::Temperate,
            &catalog,
            &mut active,
            first,
            first,
            StationAnimationTrigger::VehicleLoads,
        );
        assert_eq!(dirty, vec![first, second]);
        assert_eq!(map.get(first).unwrap().m7, 5);
        assert_eq!(map.get(second).unwrap().m7, 5);
    }
}
