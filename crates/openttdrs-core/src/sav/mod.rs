//! Parser y export mínimo de savegames de `OpenTTD` (`.sav`): contenedor
//! comprimido, chunks de mapa, estaciones (`STNN`), ciudades (`CITY`),
//! industrias (`INDY`), vehículos (`VEHS`), órdenes (`ORDL`) y dinero (`PLYR`).
//!
//! El mapa se reconstruye reutilizando el pipeline `.ottdmap` ya validado
//! (`Map::from_ottd_binary_with_extras`), generando el bloque en memoria.
//!
//! Export: [`save`] / [`save_to_bytes`] escriben mapa + `DATE` + `PLYR`
//! (ver `docs/PLANIFICACION.md`).

mod array_legacy;
mod build;
mod chunks;
mod container;
mod date;
mod entities;
pub(crate) mod house_population_generated;
mod landscape;

/// Población de un `HouseID` original (`HouseSpec::population`).
#[must_use]
pub fn house_spec_population(house_id: u16) -> u16 {
    house_population_generated::HOUSE_POPULATION
        .get(usize::from(house_id))
        .copied()
        .unwrap_or(0)
}

/// Generación de correo de un `HouseID` original (`HouseSpec::mail_generation`).
#[must_use]
pub fn house_spec_mail_generation(house_id: u16) -> u16 {
    house_population_generated::HOUSE_MAIL_GENERATION
        .get(usize::from(house_id))
        .copied()
        .unwrap_or(0)
}

/// `true` si el `HouseID` original tiene footprint `Size1x1`.
#[must_use]
pub fn house_spec_is_size_1x1(house_id: u16) -> bool {
    house_population_generated::HOUSE_SIZE_1X1
        .get(usize::from(house_id))
        .copied()
        .unwrap_or(false)
}

#[cfg(test)]
mod house_spec_tests {
    use super::{house_spec_is_size_1x1, house_spec_mail_generation, house_spec_population};

    #[test]
    fn large_office_is_1x1_with_high_population() {
        assert!(house_spec_is_size_1x1(4));
        assert!(house_spec_is_size_1x1(5));
        assert_eq!(house_spec_population(4), 220);
        assert_eq!(house_spec_population(5), 220);
        assert_eq!(house_spec_mail_generation(4), 85);
    }

    #[test]
    fn hotel_multi_tile_is_not_1x1() {
        assert!(!house_spec_is_size_1x1(7));
        assert_eq!(house_spec_population(7), 140);
        assert!(!house_spec_is_size_1x1(8));
        assert_eq!(house_spec_population(8), 0);
    }
}
mod linkgraph;
mod orders;
mod orders_codec;
mod table;
pub mod write;

use crate::airport::airport_spec_tiles;
use crate::airport_class::{AirportSpecId, airport_spec_def};
use crate::airport_fta::AirportHeading;
use crate::command::{bridge_collinear_rail_gaps, normalize_rail_trackbits_from_neighbors};
use crate::game_state::GameState;
use crate::link_graph::LinkGraphStats;
use crate::map::{Map, TileCoord, TileKind};
use crate::ottdmap_extras::OttdmapExtras;
use crate::pathfinder;
use crate::station::{Station, StopKind};
use crate::town::Town;
use crate::vehicle::{AircraftPhase, Vehicle, VehicleKind};

pub use entities::{
    SavIndustry, SavStation, SavVehicle, SavVehicleKind, format_generated_station_name,
    resolve_sav_station_name,
};
pub use write::{
    EXPORT_SAVE_VERSION, REQUIRED_EXPORT_CHUNKS, SavContainer, exported_chunk_names, save,
    save_to_bytes, save_to_bytes_with, save_with,
};

/// Bits `FACIL_*` de `OpenTTD`.
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;
const FACIL_AIRPORT: u8 = 0x08;

/// Error al cargar o guardar un `.sav`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavError {
    /// Compresión no soportada (p. ej. LZO de saves muy antiguos).
    UnsupportedCompression(String),
    /// Error al descomprimir el payload.
    Decompress(String),
    /// Formato/contenido inesperado.
    BadFormat(String),
    /// Error de E/S al escribir el archivo.
    Io(String),
    /// Valor fuera del rango permitido para codificación gamma.
    ValueOutOfRange { field: &'static str, value: u32 },
    /// Payload descomprimido excede el límite de seguridad.
    DecompressedSizeExceeded { actual: u64, limit: u64 },
}

impl std::fmt::Display for SavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedCompression(s) => write!(f, "compresión no soportada: {s}"),
            Self::Decompress(s) => write!(f, "error de descompresión: {s}"),
            Self::BadFormat(s) => write!(f, "formato inválido: {s}"),
            Self::Io(s) => write!(f, "error de E/S: {s}"),
            Self::ValueOutOfRange { field, value } => {
                write!(
                    f,
                    "valor fuera de rango para gamma en campo '{field}': {value} >= 2^14"
                )
            }
            Self::DecompressedSizeExceeded { actual, limit } => {
                write!(
                    f,
                    "payload descomprimido excede el límite: {actual} bytes > {limit} bytes"
                )
            }
        }
    }
}

impl std::error::Error for SavError {}

/// Contenido decodificado de un `.sav`.
#[derive(Debug, Clone)]
pub struct SavGame {
    /// Versión del savegame (`SLV_*`).
    pub version: u16,
    pub map: Map,
    /// Footers equivalentes a los del `.ottdmap` (INDP/STNN/TNBP/STXY).
    pub extras: OttdmapExtras,
    /// Estaciones del chunk `STNN` (saves con tablas, SLV ≥ 295).
    pub stations: Vec<SavStation>,
    /// Ciudades del chunk `CITY`.
    pub towns: Vec<Town>,
    /// Industrias del chunk `INDY` (posición/tamaño/tipo reales).
    pub industries: Vec<SavIndustry>,
    /// Vehículos del chunk `VEHS` (cabezas de convoy tren/carretera).
    pub vehicles: Vec<SavVehicle>,
    /// Dinero de la primera empresa (`PLYR`), si está presente.
    pub money: Option<i64>,
    /// Color de compañía (`Colours`) de la primera empresa (`PLYR`), si está presente.
    pub company_colour: Option<u8>,
    /// Índice `StationID` → tesela (incluye waypoints) para órdenes importadas.
    pub(crate) station_index: std::collections::HashMap<u32, entities::SavStationIndex>,
    /// Reloj de simulación del chunk `DATE`, si está presente.
    pub game_time: Option<date::SavGameTime>,
    /// Grafo de enlaces observado (`LGRP`); vacío si el chunk falta o es legacy.
    pub link_graph: LinkGraphStats,
    /// Landscape del save (`game_creation.landscape`); default temperate.
    pub climate: crate::Climate,
}

/// Carga un savegame de `OpenTTD` desde sus bytes.
///
/// # Errors
///
/// Falla si la compresión no está soportada, el stream está corrupto o faltan
/// los chunks de mapa. Estaciones y ciudades son best-effort (vacías si su
/// decodificación falla).
pub fn load(raw: &[u8]) -> Result<SavGame, SavError> {
    let (data, version) = container::decompress(raw)?;
    let chunk_list = chunks::parse_chunks(&data)?;
    let ottdmap = build::export_ottdmap(&chunk_list, version)?;
    let (map, extras) = Map::from_ottd_binary_with_extras(&ottdmap)
        .map_err(|e| SavError::BadFormat(format!("mapa reconstruido inválido: {e:?}")))?;
    let (map_w, _) = map.dimensions();
    let stations = entities::stations_from_chunks(&chunk_list, map_w, version);
    let mut towns = entities::towns_from_chunks(&chunk_list, map_w, version);
    rebuild_town_populations(&map, &mut towns);
    let industries = entities::industries_from_chunks(&chunk_list, map_w, version);
    let order_import = orders::SavOrderImport::from_chunks(&chunk_list, version);
    let station_index = entities::station_index_from_chunks(&chunk_list, map_w, version);
    let vehicles = entities::vehicles_from_chunks(&chunk_list, map_w, &order_import, version);
    let game_time = date::game_time_from_chunks(&chunk_list, version);
    let money = entities::company_money_from_chunks(&chunk_list, version);
    let company_colour = entities::company_colour_from_chunks(&chunk_list, version);
    let climate = landscape::climate_from_chunks(&chunk_list).unwrap_or_default();
    let link_graph =
        linkgraph::link_graph_from_chunks(&chunk_list, map_w, &station_index, version, climate);
    Ok(SavGame {
        version,
        map,
        extras,
        stations,
        towns,
        industries,
        vehicles,
        money,
        company_colour,
        station_index,
        game_time,
        link_graph,
        climate,
    })
}

/// Reconstruye `Town::population` como `RebuildTownCaches` (`town_sl.cpp`):
/// `OpenTTD` no guarda la población en el save, la recalcula sumando
/// `HouseSpec::population` de cada tesela `MP_HOUSE` completada (bit 7 de
/// `m3`), atribuida a la ciudad indicada por `m2` (`GetTownIndex`).
fn rebuild_town_populations(map: &Map, towns: &mut [Town]) {
    use house_population_generated::HOUSE_POPULATION;
    if towns.is_empty() {
        return;
    }
    let mut pop_by_id: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let (w, h) = map.dimensions();
    for y in 0..h {
        for x in 0..w {
            #[allow(clippy::cast_possible_wrap)]
            let Some(t) = map.get(crate::map::TileCoord::new(x as i32, y as i32)) else {
                continue;
            };
            if t.kind != crate::map::TileKind::House || t.m3 & 0x80 == 0 {
                continue;
            }
            let house_id = usize::from(t.m8 & 0x0FFF);
            // HouseIDs NewGRF (≥ 110) no tienen spec original: se omiten.
            let Some(&pop) = HOUSE_POPULATION.get(house_id) else {
                continue;
            };
            let town_id = u32::from(t.m2) | (u32::from(t.m2_hi) << 8);
            *pop_by_id.entry(town_id).or_insert(0) += u32::from(pop);
        }
    }
    for town in towns {
        town.population = pop_by_id.get(&town.id).copied().unwrap_or(0);
    }
}

fn nearest_network_tile(
    map: &Map,
    from: TileCoord,
    kind: VehicleKind,
    max_dist: i32,
) -> Option<TileCoord> {
    let mut best: Option<(i32, TileCoord)> = None;
    for dy in -max_dist..=max_dist {
        for dx in -max_dist..=max_dist {
            if dx == 0 && dy == 0 {
                continue;
            }
            let c = TileCoord::new(from.x + dx, from.y + dy);
            let Some(tile_kind) = map.get_kind(c) else {
                continue;
            };
            let on_network = match kind {
                VehicleKind::Train => match map.get(c) {
                    Some(t)
                        if matches!(
                            t.kind,
                            TileKind::Rail
                                | TileKind::RailDepot
                                | TileKind::RailTunnel
                                | TileKind::RailBridge
                        ) =>
                    {
                        true
                    }
                    Some(t) if t.kind == TileKind::Station => {
                        let st = crate::station::station_type_from_m6(t.m6);
                        st == 0 || st == crate::station::STATION_TYPE_RAIL_WAYPOINT
                    }
                    _ => false,
                },
                VehicleKind::Bus | VehicleKind::Truck => matches!(
                    tile_kind,
                    TileKind::Road | TileKind::RoadTunnel | TileKind::RoadBridge
                ),
                VehicleKind::Tram => map.get(c).is_some_and(|t| {
                    matches!(
                        t.kind,
                        TileKind::Road
                            | TileKind::RoadDepot
                            | TileKind::RoadTunnel
                            | TileKind::RoadBridge
                    ) && crate::road_type::tram_track_bits(&t) != 0
                }),
                VehicleKind::Ship => matches!(tile_kind, TileKind::Water | TileKind::ShipDepot),
                VehicleKind::Aircraft => true,
            };
            if !on_network {
                continue;
            }
            let d = dx.abs() + dy.abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, c));
            }
        }
    }
    best.map(|(_, c)| c)
}

/// `true` si el vehículo ya está sobre una tesela válida de su red (no hay que
/// teletransportarlo aunque YAPF falle por path signal / PBS).
fn vehicle_already_on_own_network(map: &Map, vehicle: &Vehicle) -> bool {
    let Some(tile) = map.get(vehicle.pos) else {
        return false;
    };
    match vehicle.kind {
        VehicleKind::Train => {
            matches!(
                tile.kind,
                TileKind::Rail
                    | TileKind::RailDepot
                    | TileKind::RailTunnel
                    | TileKind::RailBridge
                    | TileKind::Station
            ) && (tile.kind != TileKind::Station || {
                let st = crate::station::station_type_from_m6(tile.m6);
                st == 0 || st == crate::station::STATION_TYPE_RAIL_WAYPOINT
            })
        }
        VehicleKind::Bus | VehicleKind::Truck => matches!(
            tile.kind,
            TileKind::Road
                | TileKind::RoadDepot
                | TileKind::RoadTunnel
                | TileKind::RoadBridge
                | TileKind::Station
        ),
        VehicleKind::Tram => {
            matches!(
                tile.kind,
                TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
            ) && crate::road_type::tram_track_bits(&tile) != 0
        }
        VehicleKind::Ship => matches!(tile.kind, TileKind::Water | TileKind::ShipDepot),
        VehicleKind::Aircraft => true,
    }
}

/// Si un vehículo importado no tiene ruta (p. ej. en depósito sin boca a la red),
/// lo coloca en la tesela de red más cercana con ruta al destino de la orden.
///
/// No mueve vehículos que ya están sobre su red: un path signal oneway puede
/// hacer fallar `find_path` sin que la posición del `.sav` sea inválida.
fn reconcile_imported_vehicle_position(map: &Map, vehicle: &mut Vehicle) {
    if vehicle.orders.is_empty() {
        return;
    }
    vehicle.sync_order_destination(map);
    let net = pathfinder::path_network_for_vehicle(vehicle.kind);
    if pathfinder::find_path(map, vehicle.pos, vehicle.dest, net).is_some() {
        return;
    }
    if vehicle_already_on_own_network(map, vehicle) {
        return;
    }
    let Some(net_tile) = nearest_network_tile(map, vehicle.pos, vehicle.kind, 12) else {
        return;
    };
    if pathfinder::find_path(map, net_tile, vehicle.dest, net).is_some() {
        vehicle.pos = net_tile;
        vehicle.origin = net_tile;
    }
}

/// `true` si el footprint guardado (`w`×`h`, ya rotado) está transpuesto
/// respecto al spec base → hay que iterar `airport_spec_tiles` con eje Y.
fn airport_axis_y_from_saved_footprint(spec: AirportSpecId, w: u16, h: u16) -> bool {
    let Some(def) = airport_spec_def(spec) else {
        return false;
    };
    let (w, h) = (i32::from(w), i32::from(h));
    def.size_x != def.size_y && w == def.size_y && h == def.size_x
}

/// Fase de vuelo MVP aproximada desde el heading FTA importado del save.
///
/// Best-effort: `OpenTTD` no persiste una fase equivalente; se infiere del
/// heading (`AirportMovementStates`) para que el FSM MVP arranque coherente.
fn aircraft_phase_from_airport_heading(h: AirportHeading) -> AircraftPhase {
    match h {
        AirportHeading::Hangar => AircraftPhase::InHangar,
        AirportHeading::Takeoff
        | AirportHeading::StartTakeoff
        | AirportHeading::EndTakeoff
        | AirportHeading::HeliTakeoff => AircraftPhase::Takeoff,
        AirportHeading::Landing
        | AirportHeading::EndLanding
        | AirportHeading::HeliLanding
        | AirportHeading::HeliEndLanding => AircraftPhase::Landing,
        AirportHeading::Flying => AircraftPhase::Flying,
        _ => AircraftPhase::Taxi,
    }
}

fn stop_kind_from_facilities(facilities: u8) -> StopKind {
    if facilities & FACIL_TRAIN != 0 {
        StopKind::RailStation
    } else if facilities & FACIL_BUS_STOP != 0 {
        StopKind::BusStop
    } else if facilities & FACIL_AIRPORT != 0 {
        StopKind::Airport
    } else {
        // Camión por defecto (incluye FACIL_TRUCK_STOP y muelles).
        let _ = FACIL_TRUCK_STOP;
        StopKind::TruckStop
    }
}

/// Píxeles consumidos en la tesela hacia el cruce, desde `x_pos`/`y_pos`/`direction`.
///
/// Deltas de tesela `OpenTTD` (`_tileoffs_by_dir` en `map.cpp`):
/// `NE=(-1,0)`, `SE=(0,+1)`, `SW=(+1,0)`, `NW=(0,-1)`. El eje que avanza
/// determina la fracción; sentido negativo → `15 - fract`.
fn rail_pixel_from_openttd_pos(x_pos: i32, y_pos: i32, direction: u8) -> u8 {
    use crate::vehicle::{DIR_E, DIR_N, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W};
    let xf = u8::try_from(x_pos.rem_euclid(16)).unwrap_or(0);
    let yf = u8::try_from(y_pos.rem_euclid(16)).unwrap_or(0);
    match direction {
        DIR_SW => xf,
        DIR_SE => yf,
        DIR_NW => 15u8.saturating_sub(yf),
        DIR_N => 15u8.saturating_sub(xf).min(15u8.saturating_sub(yf)),
        DIR_S => xf.min(yf),
        DIR_E => 15u8.saturating_sub(xf).min(yf),
        DIR_W => xf.min(15u8.saturating_sub(yf)),
        // `DIR_NE` y fallback: eje X decreciente.
        _ => 15u8.saturating_sub(xf),
    }
}

/// IDs vanilla de motor rail de `OpenTTD` no son contiguos con el catálogo Rust:
/// entre Kirby (0) y Chaney (8) están locomotoras de otros climas.
fn vanilla_train_engine_id(openttd_id: u16) -> Option<u16> {
    let id = match openttd_id {
        0 => 100,  // Kirby Paul Tank
        8 => 101,  // Chaney 'Jubilee'
        9 => 102,  // Ginzu 'A4'
        10 => 103, // SH '8P'
        11 => 104, // Manley-Morel
        12 => 105, // Dash
        13 => 106, // SH/Hendry '25'
        14 => 107, // UU '37'
        15 => 108, // Floss '47'
        22 => 109, // SH '125'
        23 => 110, // SH '30'
        24 => 111, // SH '40'
        25 => 112, // T.I.M.
        26 => 113, // AsiaStar
        _ => return None,
    };
    Some(id)
}

impl GameState {
    /// Estado jugable desde un save de `OpenTTD`: mapa, estaciones, ciudades,
    /// vehículos (cabezas de convoy) y dinero de la empresa.
    ///
    /// Las industrias las añade el cliente (`place_industries`) usando
    /// `SavGame::industries` cuando hay chunk `INDY` de tabla, o la heurística
    /// de teselas con `SavGame::extras` en saves antiguos.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_sav_game(sav: SavGame) -> Self {
        let mut map = sav.map;
        normalize_rail_trackbits_from_neighbors(&mut map);
        bridge_collinear_rail_gaps(&mut map);
        // OpenTTD afterload: reservas de depósito no son autoritativas al cargar.
        crate::depot::clear_all_depot_reservations(&mut map);
        let mut state = Self::from_map(map);
        // OpenTTD ≥15 default `train_acceleration_model = 1` (realista).
        state.train_acceleration_model = crate::engine::TrainAccelerationModel::Realistic;
        state.climate = sav.climate;
        if let Some(time) = sav.game_time {
            state.tick = date::game_tick_from_sav_time(time);
        }
        state.jgr_tunnels_from_footer = sav.extras.jgr_tunnels_from_tnbp();
        state.towns = sav.towns;
        for town in &mut state.towns {
            town.init_growth_goals(state.climate);
        }
        if let Some(money) = sav.money {
            state.economy.money = money;
        }
        if let Some(colour) = sav.company_colour {
            state.company_colour = colour;
        }
        for st in sav.stations {
            let stop_kind = stop_kind_from_facilities(st.facilities);
            let mut station = Station::new_with_kind(st.pos, stop_kind);
            station.name = entities::resolve_sav_station_name(&st, &state.towns);
            if stop_kind == StopKind::Airport {
                let spec = AirportSpecId::from_ottd_airport_type(st.airport_type);
                let axis_y = airport_axis_y_from_saved_footprint(spec, st.airport_w, st.airport_h);
                station.airport_spec = spec;
                station.airport_blocks = st.airport_blocks;
                station.airport_tiles = airport_spec_tiles(st.pos, spec, axis_y)
                    .map(|(c, _piece)| c)
                    .collect();
                // El chunk de mapa reconstruido tipa `MP_STATION` genérico
                // (`TileKind::Station`); retaguear a `Airport` como haría el
                // engine al construir, para que hangares/heliports se detecten.
                for &c in &station.airport_tiles {
                    if let Some(mut tile) = state.map.get(c)
                        && tile.kind == TileKind::Station
                    {
                        tile.kind = TileKind::Airport;
                        let _ = state.map.set_tile(c, tile);
                    }
                }
            }
            state.stations.push(station);
        }
        state.link_graph = sav.link_graph;
        if !matches!(
            state.cargo_dist.distribution,
            crate::flow_stat::DistributionType::Manual
        ) {
            state.rebuild_station_flows();
        }
        let mut last_train_head: Option<u32> = None;
        for (i, v) in sav.vehicles.iter().enumerate() {
            let kind = match v.kind {
                SavVehicleKind::Train => VehicleKind::Train,
                // Pasajeros (cargo 0) → bus; el resto, camión.
                SavVehicleKind::RoadVehicle if v.cargo_type == 0 => VehicleKind::Bus,
                SavVehicleKind::RoadVehicle => VehicleKind::Truck,
                SavVehicleKind::Ship => VehicleKind::Ship,
                SavVehicleKind::Aircraft => VehicleKind::Aircraft,
            };
            #[allow(clippy::cast_possible_truncation)]
            let id = i as u32;
            if kind == VehicleKind::Aircraft {
                let mut vehicle = Vehicle::new(id, kind, v.pos, v.pos);
                vehicle.running = v.running;
                vehicle.cur_speed = v.cur_speed;
                vehicle.subspeed = v.subspeed;
                vehicle.direction = v.direction;
                // `ENGINE_AIRCRAFT_TRICARIO`/`ENGINE_AIRCRAFT_DAKOTA`: OpenTTD
                // trae IDs vanilla (`engine_type`) que no coinciden con
                // nuestro catálogo; best-effort por `is_helicopter` (subtype).
                vehicle.engine_id = Some(if v.is_helicopter {
                    crate::engine::ENGINE_AIRCRAFT_TRICARIO
                } else {
                    crate::engine::ENGINE_AIRCRAFT_DAKOTA
                });
                let map_w = state.map.dimensions().0;
                let imported_orders =
                    orders::vehicle_orders_from_sav(&v.orders, &sav.station_index, map_w);
                if !imported_orders.is_empty() {
                    vehicle.set_vehicle_orders(imported_orders);
                    let last = vehicle.orders.len().saturating_sub(1);
                    vehicle.current_order = v.current_order.min(last);
                    vehicle.cur_implicit_order_index = v.cur_implicit_order_index.min(last);
                }
                if let Some(target) = sav.station_index.get(&u32::from(v.airport_targetairport)) {
                    vehicle.dest = target.pos;
                }
                vehicle.airport_pos = v.airport_pos;
                vehicle.airport_prev_pos = v.airport_previous_pos;
                vehicle.airport_heading = AirportHeading::from_u8(v.airport_state);
                vehicle.aircraft_phase =
                    aircraft_phase_from_airport_heading(vehicle.airport_heading);
                // El save no persiste el motor FTA como tal: si el avión está
                // en/entrando a un aeropuerto (heading ≠ vuelo libre), asumimos
                // control FTA activo, que es el caso relevante para este oráculo.
                vehicle.airport_fta_active = true;
                // El save no persiste el contador de espera (`aircraft_phase_ticks`);
                // aproximar el dwell restante del nodo actual por sus flags, para
                // que el FSM MVP no complete el nodo instantáneamente al primer tick.
                vehicle.aircraft_phase_ticks = state
                    .stations
                    .iter()
                    .find(|s| s.covers_tile(vehicle.pos))
                    .and_then(|s| crate::airport_fta::fta_profile_for_spec(s.airport_spec))
                    .and_then(|p| {
                        p.moving_data
                            .get(usize::from(vehicle.airport_pos))
                            .map(|md| md.flags)
                    })
                    .map_or(0, |flags| {
                        if flags
                            & (crate::airport_fta::FLAG_TAKEOFF
                                | crate::airport_fta::FLAG_HELI_RAISE)
                            != 0
                        {
                            12
                        } else {
                            0
                        }
                    });
                state.vehicles.push(vehicle);
                last_train_head = None;
                continue;
            }
            let mut vehicle = Vehicle::new(id, kind, v.pos, v.pos);
            vehicle.progress = v.progress;
            vehicle.cur_speed = v.cur_speed;
            vehicle.subspeed = v.subspeed;
            vehicle.direction = v.direction;
            vehicle.cargo_type = crate::CargoType::from_climate_slot(sav.climate, v.cargo_type);
            if kind == VehicleKind::Train {
                vehicle.rail_pixel = rail_pixel_from_openttd_pos(v.x_pos, v.y_pos, v.direction);
                if let Some(candidate) = vanilla_train_engine_id(v.engine_type)
                    && crate::engine::engine_by_id(candidate).is_some()
                {
                    vehicle.engine_id = Some(candidate);
                }
            }
            if v.is_wagon && kind == VehicleKind::Train {
                vehicle.engine_id = Some(crate::engine::ENGINE_WAGON_GOODS);
                vehicle.capacity = crate::engine::engine_by_id(crate::engine::ENGINE_WAGON_GOODS)
                    .map_or(25, |e| e.capacity);
                vehicle.running = false;
                state.vehicles.push(vehicle);
                if let Some(head) = last_train_head {
                    let _ = crate::train_consist::attach_wagon(&mut state.vehicles, head, id);
                }
                continue;
            }
            let map_w = state.map.dimensions().0;
            let imported_orders =
                orders::vehicle_orders_from_sav(&v.orders, &sav.station_index, map_w);
            if !imported_orders.is_empty() {
                vehicle.set_vehicle_orders(imported_orders);
                let last = vehicle.orders.len().saturating_sub(1);
                vehicle.current_order = v.current_order.min(last);
                vehicle.cur_implicit_order_index = v.cur_implicit_order_index.min(last);
                // Saves reales suelen traer trenes parados en depósito; arrancar si hay órdenes.
                vehicle.running = true;
                reconcile_imported_vehicle_position(&state.map, &mut vehicle);
            }
            // `set_vehicle_orders` reinicia progreso para comandos nuevos, pero
            // al importar debe conservar exactamente el estado sub-tesela del save.
            vehicle.progress = v.progress;
            if kind == VehicleKind::Train {
                vehicle.rail_pixel = rail_pixel_from_openttd_pos(v.x_pos, v.y_pos, v.direction);
            }
            state.vehicles.push(vehicle);
            if kind == VehicleKind::Train {
                last_train_head = Some(id);
                crate::train_consist::consist_changed(&mut state.vehicles, id);
            } else {
                last_train_head = None;
            }
        }
        state
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn stop_kind_mapping() {
        assert_eq!(stop_kind_from_facilities(0x01), StopKind::RailStation);
        assert_eq!(stop_kind_from_facilities(0x05), StopKind::RailStation);
        assert_eq!(stop_kind_from_facilities(0x04), StopKind::BusStop);
        assert_eq!(stop_kind_from_facilities(0x02), StopKind::TruckStop);
        assert_eq!(stop_kind_from_facilities(0x08), StopKind::Airport);
    }

    #[test]
    fn rail_pixel_uses_axis_of_openttd_direction() {
        use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE};
        // Fixture PBS: DIR_NE, x_fract=10 → 15-10=5.
        assert_eq!(rail_pixel_from_openttd_pos(762, 600, DIR_NE), 5);
        // Dual norte: DIR_NW, y_fract=14 → 15-14=1.
        assert_eq!(rail_pixel_from_openttd_pos(424, 126, DIR_NW), 1);
        // Dual sur: DIR_SE, y_fract=3 → 3.
        assert_eq!(rail_pixel_from_openttd_pos(408, 227, DIR_SE), 3);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn from_sav_game_builds_state_with_entities() {
        let map = Map::new_flat(64, 64, 0);
        let sav = SavGame {
            version: 300,
            map,
            extras: OttdmapExtras::default(),
            stations: vec![SavStation {
                station_id: 0,
                pos: crate::TileCoord::new(3, 3),
                name: Some("Estación Norte".into()),
                facilities: 0x01,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
            }],
            towns: vec![Town {
                id: 0,
                pos: crate::TileCoord::new(10, 10),
                name: "Springfield".into(),
                population: 500,
                passengers_served: 0,
                mail_served: 0,
                growth_funded: 0,
                ..Default::default()
            }],
            industries: Vec::new(),
            link_graph: LinkGraphStats::default(),
            vehicles: vec![
                SavVehicle {
                    kind: SavVehicleKind::Train,
                    pos: crate::TileCoord::new(5, 5),
                    raw_tile: crate::TileCoord::new(5, 5),
                    progress: 0,
                    x_pos: 5 * 16,
                    y_pos: 5 * 16,
                    z_pos: 0,
                    cur_speed: 0,
                    subspeed: 0,
                    direction: 0,
                    engine_type: 0,
                    cargo_type: 9,
                    orders: Vec::new(),
                    current_order: 0,
                    cur_implicit_order_index: 0,
                    running: true,
                    is_wagon: false,
                    is_helicopter: false,
                    airport_pos: 0,
                    airport_previous_pos: 0,
                    airport_state: 0,
                    airport_targetairport: 0,
                },
                SavVehicle {
                    kind: SavVehicleKind::RoadVehicle,
                    pos: crate::TileCoord::new(6, 6),
                    raw_tile: crate::TileCoord::new(6, 6),
                    progress: 0,
                    x_pos: 6 * 16,
                    y_pos: 6 * 16,
                    z_pos: 0,
                    cur_speed: 0,
                    subspeed: 0,
                    direction: 0,
                    engine_type: 0,
                    cargo_type: 0,
                    orders: Vec::new(),
                    current_order: 0,
                    cur_implicit_order_index: 0,
                    running: true,
                    is_wagon: false,
                    is_helicopter: false,
                    airport_pos: 0,
                    airport_previous_pos: 0,
                    airport_state: 0,
                    airport_targetairport: 0,
                },
                SavVehicle {
                    kind: SavVehicleKind::RoadVehicle,
                    pos: crate::TileCoord::new(7, 7),
                    raw_tile: crate::TileCoord::new(7, 7),
                    progress: 0,
                    x_pos: 7 * 16,
                    y_pos: 7 * 16,
                    z_pos: 0,
                    cur_speed: 0,
                    subspeed: 0,
                    direction: 0,
                    engine_type: 0,
                    cargo_type: 5,
                    orders: Vec::new(),
                    current_order: 0,
                    cur_implicit_order_index: 0,
                    running: true,
                    is_wagon: false,
                    is_helicopter: false,
                    airport_pos: 0,
                    airport_previous_pos: 0,
                    airport_state: 0,
                    airport_targetairport: 0,
                },
            ],
            money: Some(123_456),
            company_colour: Some(9),
            station_index: std::collections::HashMap::new(),
            game_time: None,
            climate: crate::Climate::Temperate,
        };
        let state = GameState::from_sav_game(sav);
        assert_eq!(state.stations.len(), 1);
        assert_eq!(state.stations[0].stop_kind, StopKind::RailStation);
        assert_eq!(state.stations[0].name.as_deref(), Some("Estación Norte"));
        assert_eq!(state.towns.len(), 1);
        assert_eq!(state.towns[0].name, "Springfield");
        assert_eq!(state.economy.money, 123_456);
        assert_eq!(state.company_colour, 9);
        assert_eq!(state.vehicles.len(), 3);
        assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
        assert_eq!(state.vehicles[1].kind, VehicleKind::Bus);
        assert_eq!(state.vehicles[2].kind, VehicleKind::Truck);
    }
}
