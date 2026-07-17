//! Parser y export mínimo de savegames de `OpenTTD` (`.sav`): contenedor
//! comprimido, chunks de mapa, estaciones (`STNN`), ciudades (`CITY`),
//! industrias (`INDY`), vehículos (`VEHS`), órdenes (`ORDL`) y dinero (`PLYR`).
//!
//! El mapa se reconstruye reutilizando el pipeline `.ottdmap` ya validado
//! (`Map::from_ottd_binary_with_extras`), generando el bloque en memoria.
//!
//! Export: [`save`] / [`save_to_bytes`] escriben mapa + `DATE` + `PLYR`
//! (ver `docs/ROADMAP_SAV_EXPORT.md`).

mod array_legacy;
mod build;
mod chunks;
mod container;
mod date;
mod entities;
mod house_population_generated;

/// Población de un `HouseID` original (`HouseSpec::population`).
#[must_use]
pub fn house_spec_population(house_id: u16) -> u16 {
    house_population_generated::HOUSE_POPULATION
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
    use super::{house_spec_is_size_1x1, house_spec_population};

    #[test]
    fn large_office_is_1x1_with_high_population() {
        assert!(house_spec_is_size_1x1(4));
        assert!(house_spec_is_size_1x1(5));
        assert_eq!(house_spec_population(4), 220);
        assert_eq!(house_spec_population(5), 220);
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

use crate::command::{bridge_collinear_rail_gaps, normalize_rail_trackbits_from_neighbors};
use crate::game_state::GameState;
use crate::link_graph::LinkGraphStats;
use crate::map::{Map, TileCoord, TileKind};
use crate::ottdmap_extras::OttdmapExtras;
use crate::pathfinder;
use crate::station::{Station, StopKind};
use crate::town::Town;
use crate::vehicle::{Vehicle, VehicleKind};

pub use entities::{SavIndustry, SavStation, SavVehicle, SavVehicleKind};
pub use write::{
    EXPORT_SAVE_VERSION, REQUIRED_EXPORT_CHUNKS, SavContainer, exported_chunk_names, save,
    save_to_bytes, save_to_bytes_with, save_with,
};

/// Bits `FACIL_*` de `OpenTTD`.
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;

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
    let link_graph = linkgraph::link_graph_from_chunks(&chunk_list, map_w, &station_index, version);
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

/// Si un vehículo importado no tiene ruta (p. ej. en depósito sin boca a la red),
/// lo coloca en la tesela de red más cercana con ruta al destino de la orden.
fn reconcile_imported_vehicle_position(map: &Map, vehicle: &mut Vehicle) {
    if vehicle.orders.is_empty() {
        return;
    }
    vehicle.sync_order_destination(map);
    let net = pathfinder::path_network_for_vehicle(vehicle.kind);
    if pathfinder::find_path(map, vehicle.pos, vehicle.dest, net).is_some() {
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

fn stop_kind_from_facilities(facilities: u8) -> StopKind {
    if facilities & FACIL_TRAIN != 0 {
        StopKind::RailStation
    } else if facilities & FACIL_BUS_STOP != 0 {
        StopKind::BusStop
    } else {
        // Camión por defecto (incluye FACIL_TRUCK_STOP, aeropuertos y muelles).
        let _ = FACIL_TRUCK_STOP;
        StopKind::TruckStop
    }
}

impl GameState {
    /// Estado jugable desde un save de `OpenTTD`: mapa, estaciones, ciudades,
    /// vehículos (cabezas de convoy) y dinero de la empresa.
    ///
    /// Las industrias las añade el cliente (`place_industries`) usando
    /// `SavGame::industries` cuando hay chunk `INDY` de tabla, o la heurística
    /// de teselas con `SavGame::extras` en saves antiguos.
    #[must_use]
    pub fn from_sav_game(sav: SavGame) -> Self {
        let mut map = sav.map;
        normalize_rail_trackbits_from_neighbors(&mut map);
        bridge_collinear_rail_gaps(&mut map);
        let mut state = Self::from_map(map);
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
            let mut station =
                Station::new_with_kind(st.pos, stop_kind_from_facilities(st.facilities));
            station.name = st.name;
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
            };
            #[allow(clippy::cast_possible_truncation)]
            let id = i as u32;
            let mut vehicle = Vehicle::new(id, kind, v.pos, v.pos);
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
                vehicle.current_order = v.current_order.min(vehicle.orders.len().saturating_sub(1));
                // Saves reales suelen traer trenes parados en depósito; arrancar si hay órdenes.
                vehicle.running = true;
                reconcile_imported_vehicle_position(&state.map, &mut vehicle);
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
        assert_eq!(stop_kind_from_facilities(0x08), StopKind::TruckStop);
    }

    #[test]
    fn from_sav_game_builds_state_with_entities() {
        let map = Map::new_flat(64, 64, 0);
        let sav = SavGame {
            version: 300,
            map,
            extras: OttdmapExtras::default(),
            stations: vec![SavStation {
                pos: crate::TileCoord::new(3, 3),
                name: Some("Estación Norte".into()),
                facilities: 0x01,
            }],
            towns: vec![Town {
                id: 0,
                pos: crate::TileCoord::new(10, 10),
                name: "Springfield".into(),
                population: 500,
                local_authority_rating: 0,
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
                    cargo_type: 9,
                    orders: Vec::new(),
                    current_order: 0,
                    running: true,
                    is_wagon: false,
                },
                SavVehicle {
                    kind: SavVehicleKind::RoadVehicle,
                    pos: crate::TileCoord::new(6, 6),
                    cargo_type: 0,
                    orders: Vec::new(),
                    current_order: 0,
                    running: true,
                    is_wagon: false,
                },
                SavVehicle {
                    kind: SavVehicleKind::RoadVehicle,
                    pos: crate::TileCoord::new(7, 7),
                    cargo_type: 5,
                    orders: Vec::new(),
                    current_order: 0,
                    running: true,
                    is_wagon: false,
                },
            ],
            money: Some(123_456),
            company_colour: Some(9),
            station_index: std::collections::HashMap::new(),
            game_time: None,
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
