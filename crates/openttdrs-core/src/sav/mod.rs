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
mod economy;
mod entities;
mod fleet;
pub(crate) mod house_population_generated;
mod import;
mod landscape;
mod settings;

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
use crate::command::normalize_rail_trackbits_from_neighbors;
use crate::game_state::GameState;
use crate::link_graph::LinkGraphStats;
use crate::map::{Map, TileCoord, TileKind};
use crate::ottdmap_extras::OttdmapExtras;
use crate::pathfinder;
use crate::station::{Station, StopKind};
use crate::town::Town;
use crate::vehicle::{AircraftPhase, Vehicle, VehicleKind};
use std::collections::HashMap;

pub use entities::{
    SavCargoPacket, SavIndustry, SavIndustryAcceptedCargo, SavIndustryProducedCargo, SavStation,
    SavStationCargo, SavVehicle, SavVehicleKind, format_generated_station_name,
    resolve_sav_station_name,
};
pub use import::{
    hydrate_industries_from_map_tiles, industry_group_from_gfx, industry_kind_from_gfx,
    industry_kind_from_ottd_type, industry_random_colour_from_instance, industry_spec_from_gfx,
};
pub use write::{
    EXPORT_SAVE_VERSION, REQUIRED_EXPORT_CHUNKS, SavContainer, exported_chunk_names, save,
    save_to_bytes, save_to_bytes_with, save_with,
};

/// Chunk nativo que se conserva sin interpretar al hacer round-trip de un
/// savegame. El cuerpo incluye el header gamma de tablas o el payload RIFF,
/// exactamente como aparece después de descomprimir el contenedor.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SavOpaqueChunk {
    pub name: [u8; 4],
    pub ch_type: u8,
    pub body: Vec<u8>,
}

/// Chunks cuyo contenido es necesario para que `OpenTTD` reconstruya pools,
/// mappings o configuración que el core aún no ejecuta.
fn opaque_chunks_from_chunks(chunks: &[chunks::RawChunk]) -> Vec<SavOpaqueChunk> {
    const PASSTHROUGH: &[[u8; 4]] = &[
        *b"ENGN", *b"ENGS", *b"EIDS", *b"GSET", *b"NGRF", *b"OBJS", *b"OBID", *b"SRND", *b"PSAC",
        *b"IIDS", *b"TIDS", *b"APID", *b"ATID", *b"RAIL", *b"ROTT", *b"GLOG", *b"GOAL", *b"STPE",
        *b"STPA", *b"SIGN",
    ];
    chunks
        .iter()
        .filter(|chunk| PASSTHROUGH.contains(&chunk.name))
        .map(|chunk| SavOpaqueChunk {
            name: chunk.name,
            ch_type: chunk.ch_type,
            body: chunk.body.clone(),
        })
        .collect()
}

/// Bits `FACIL_*` de `OpenTTD`.
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;
const FACIL_AIRPORT: u8 = 0x08;
const FACIL_DOCK: u8 = 0x10;

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
    /// Dimensiones MAPS fuera de los límites admitidos por el formato.
    InvalidMapDimensions { width: u64, height: u64 },
    /// No se pudo reservar memoria para una estructura de tamaño ya validado.
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
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
            Self::InvalidMapDimensions { width, height } => {
                write!(
                    f,
                    "dimensiones MAPS inválidas: {width}×{height}; se admiten potencias de dos entre 1 y 4096"
                )
            }
            Self::AllocationFailed { context, requested } => {
                write!(f, "no se pudo reservar {requested} bytes para {context}")
            }
        }
    }
}

impl std::error::Error for SavError {}

/// Contenido decodificado de un `.sav`.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Paquetes de carga físicos del chunk `CAPA`.
    pub cargo_packets: Vec<SavCargoPacket>,
    /// Pagos activos del pool `CAPY` (preservados, sin ejecución de runtime).
    pub cargo_payments: Vec<crate::CargoPaymentState>,
    /// Vehículos del chunk `VEHS` (cabezas de convoy tren/carretera).
    pub vehicles: Vec<SavVehicle>,
    /// Empresas del pool `PLYR` (dinero y color por `CompanyID`).
    pub companies: Vec<entities::SavCompany>,
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
    /// Lado de conducción y señales de `PATS` / `OPTS`.
    pub construction: crate::ConstructionSettings,
    /// Ajustes PBS/pathfinding persistidos en `PATS` / `OPTS`.
    pub pathfinding: crate::PathfindingSettings,
    /// Modelo de aceleración de tren persistido en `PATS` / `OPTS`.
    pub train_acceleration_model: crate::engine::TrainAccelerationModel,
    /// Límite de ruido de aeropuerto persistido en `PATS` / `OPTS`.
    pub station_noise_level: bool,
    /// Nivel de averías persistido en `PATS` / `OPTS`.
    pub vehicle_breakdowns: u8,
    /// No mandar vehículos a servicio sin averías (`PATS` / `OPTS`).
    pub no_servicing_if_no_breakdowns: bool,
    /// Duración de subsidios en años (`PATS` / `OPTS`).
    pub subsidy_duration: u16,
    /// Multiplicador de subsidios (`PATS` / `OPTS`).
    pub subsidy_multiplier: u8,
    /// Desastres activos (`PATS` / `OPTS`).
    pub disasters_enabled: bool,
    /// Tolerancia de la autoridad municipal (`PATS` / `OPTS`).
    pub town_council_tolerance: crate::town::TownCouncilTolerance,
    /// Unidades de tiempo de economía en modo wallclock (`PATS` / `OPTS`).
    pub using_wallclock_units: bool,
    /// Inflación compuesta habilitada (`PATS` / `OPTS`).
    pub inflation_enabled: bool,
    /// Recesiones habilitadas (`PATS` / `OPTS`).
    pub recessions_enabled: bool,
    /// Estado económico global del chunk `ECMY`.
    pub global_economy: crate::economy::GlobalEconomy,
    /// Grupos de vehículos del chunk `GRPS` (nombres/índices básicos).
    pub vehicle_groups: Vec<crate::vehicle_group::VehicleGroup>,
    /// Reglas de autoreemplazo del chunk `ERNW`.
    pub autoreplace_rules: Vec<crate::autoreplace::AutoReplaceRule>,
    /// Chunks nativos no modelados que se conservan para round-trip.
    pub opaque_chunks: Vec<SavOpaqueChunk>,
}

/// `SLV_100`: desde esta versión `OpenTTD` persiste las reservas PBS de
/// depósitos. `AfterLoadGame()` sólo limpia el bit en saves anteriores.
const SLV_DEPOT_RESERVATION_PERSISTED: u16 = 100;

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
    let (mut map, extras) = Map::from_ottd_binary_with_extras(&ottdmap)
        .map_err(|e| SavError::BadFormat(format!("mapa reconstruido inválido: {e:?}")))?;
    // `export_ottdmap` sólo es un transporte interno para el pipeline común;
    // el MAPH original del `.sav` es autoritativo. No apliques la heurística de
    // los viejos `.ottdmap`: cambia pendientes empinadas reales de costa.
    map.set_legacy_zero_water_height_repair(false);
    let (map_w, _) = map.dimensions();
    let stations = entities::stations_from_chunks(&chunk_list, map_w, version);
    let mut towns = entities::towns_from_chunks(&chunk_list, map_w, version);
    rebuild_town_populations(&map, &mut towns);
    let industries = entities::industries_from_chunks(&chunk_list, map_w, version);
    let cargo_packets = entities::cargo_packets_from_chunks(&chunk_list, map_w, version);
    let cargo_payments = economy::cargo_payments_from_chunks(&chunk_list);
    let order_import = orders::SavOrderImport::from_chunks(&chunk_list, version);
    let station_index = entities::station_index_from_chunks(&chunk_list, map_w, version);
    let vehicles = entities::vehicles_from_chunks(&chunk_list, map_w, &order_import, version);
    let companies = entities::companies_from_chunks(&chunk_list, version);
    let game_time = date::game_time_from_chunks(&chunk_list, version);
    let money = entities::company_money_from_chunks(&chunk_list, version);
    let company_colour = entities::company_colour_from_chunks(&chunk_list, version);
    let climate = landscape::climate_from_chunks(&chunk_list).unwrap_or_default();
    let parsed_settings = settings::settings_from_chunks(&chunk_list);
    let mut global_economy = economy::global_economy_from_chunks(&chunk_list);
    global_economy.inflation_enabled = parsed_settings.inflation_enabled;
    global_economy.recessions_enabled = parsed_settings.recessions_enabled;
    let vehicle_groups = fleet::vehicle_groups_from_chunks(&chunk_list);
    let mut autoreplace_rules = fleet::autoreplace_rules_from_chunks(&chunk_list);
    fleet::assign_autoreplace_owners(&mut autoreplace_rules, &companies);
    let opaque_chunks = opaque_chunks_from_chunks(&chunk_list);
    let link_graph =
        linkgraph::link_graph_from_chunks(&chunk_list, map_w, &station_index, version, climate);
    Ok(SavGame {
        version,
        map,
        extras,
        stations,
        towns,
        industries,
        cargo_packets,
        cargo_payments,
        vehicles,
        companies,
        money,
        company_colour,
        station_index,
        game_time,
        link_graph,
        climate,
        construction: parsed_settings.construction,
        pathfinding: parsed_settings.pathfinding,
        train_acceleration_model: parsed_settings.train_acceleration_model,
        station_noise_level: parsed_settings.station_noise_level,
        vehicle_breakdowns: parsed_settings.vehicle_breakdowns,
        no_servicing_if_no_breakdowns: parsed_settings.no_servicing_if_no_breakdowns,
        subsidy_duration: parsed_settings.subsidy_duration,
        subsidy_multiplier: parsed_settings.subsidy_multiplier,
        disasters_enabled: parsed_settings.disasters_enabled,
        town_council_tolerance: parsed_settings.town_council_tolerance,
        using_wallclock_units: parsed_settings.using_wallclock_units,
        inflation_enabled: parsed_settings.inflation_enabled,
        recessions_enabled: parsed_settings.recessions_enabled,
        global_economy,
        vehicle_groups,
        autoreplace_rules,
        opaque_chunks,
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
    } else if facilities & FACIL_DOCK != 0 {
        StopKind::Dock
    } else {
        // Camión por defecto.
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

/// Convierte las referencias `STNN.goods` + `CAPA` a la cola de packets core.
///
/// La reserva se conserva como total de estación porque el modelo actual de
/// `StationCargoList` todavía no la separa por cargo. Los packets sí mantienen
/// cargo, origen, edad y próximo hop individualmente.
fn hydrate_sav_station_cargo(
    station: &mut Station,
    saved_cargo: &[entities::SavStationCargo],
    packets_by_id: &HashMap<u32, &entities::SavCargoPacket>,
    station_positions: &HashMap<u32, TileCoord>,
    climate: crate::Climate,
) {
    let mut imported = Vec::new();
    let mut reserved = 0_u32;
    for entry in saved_cargo {
        reserved = reserved.saturating_add(entry.reserved);
        let Some(cargo) = crate::CargoType::from_climate_slot(climate, entry.cargo_slot) else {
            continue;
        };
        for packet_id in &entry.packet_ids {
            let Some(saved) = packets_by_id.get(packet_id) else {
                continue;
            };
            let mut packet =
                crate::CargoPacket::new(cargo, saved.count, saved.source_xy.unwrap_or(station.pos));
            packet.source_xy = saved.source_xy;
            packet.periods_in_transit = saved.periods_in_transit;
            packet.feeder_share = saved.feeder_share;
            packet.first_station = saved
                .source_station_id
                .and_then(|id| station_positions.get(&id).copied());
            packet.next_hop = saved
                .next_hop_station_id
                .and_then(|id| station_positions.get(&id).copied());
            imported.push(packet);
        }
    }
    station.push_waiting_packets(imported);
    station.cargo_packets.reserved = reserved.min(station.cargo_packets.total_count());
}

fn quarterly_entry_from_sav(
    entry: &entities::SavCompanyEconomy,
) -> crate::economy_quarterly::QuarterlyEconomyEntry {
    crate::economy_quarterly::QuarterlyEconomyEntry {
        income: u64::try_from(entry.income).unwrap_or(0),
        expenses: quarterly_expense_from_sav(entry.expenses),
        deliveries: crate::economy_quarterly::delivered_cargo_total(&entry.delivered_cargo),
        delivered_cargo: entry.delivered_cargo.clone(),
        performance_history: entry.performance_history,
        company_value: entry.company_value,
    }
}

/// El core acumula costes como magnitudes positivas, pero `OpenTTD` los guarda
/// en `CompanyEconomyEntry::expenses` como `Money` negativo.
fn quarterly_expense_from_sav(value: i64) -> u64 {
    if value.is_negative() {
        value.unsigned_abs()
    } else {
        u64::try_from(value).unwrap_or(0)
    }
}

/// Hidrata `PLYR.cur_economy` y `PLYR.old_economy` en el historial que usa el
/// runtime. `OpenTTD` guarda el histórico más reciente primero; el core lo
/// mantiene cronológico para que los gráficos puedan iterarlo naturalmente.
fn hydrate_company_economy_history(
    company: &mut crate::company::Company,
    cur_economy: Option<&entities::SavCompanyEconomy>,
    old_economy: &[entities::SavCompanyEconomy],
    economy_month: u8,
) {
    let history = &mut company.quarterly_economy;
    if let Some(current) = cur_economy {
        history.cur_income = u64::try_from(current.income).unwrap_or(0);
        history.cur_expenses = quarterly_expense_from_sav(current.expenses);
        history.cur_deliveries =
            crate::economy_quarterly::delivered_cargo_total(&current.delivered_cargo);
        history
            .cur_delivered_cargo
            .clone_from(&current.delivered_cargo);
        history.cur_company_value = current.company_value;
        history.cur_performance_history = current.performance_history;
        // Los cierres de OpenTTD ocurren en los meses 0/3/6/9. Restaurar la
        // fase evita sumar tres meses nuevos antes del siguiente cierre.
        history.months_in_quarter = economy_month % 3;
    }
    if !old_economy.is_empty() {
        history.samples = old_economy
            .iter()
            .take(crate::economy_quarterly::ECONOMY_HISTORY_QUARTERS)
            .rev()
            .map(quarterly_entry_from_sav)
            .collect();
    }
}

impl GameState {
    /// Estado jugable desde un save de `OpenTTD`: mapa, estaciones, ciudades,
    /// vehículos (cabezas de convoy) y dinero de la empresa.
    ///
    /// Industrias, stock de producción y paquetes de carga se hidratan en el
    /// core desde `INDY`/`STNN`/`CAPA`, para que cliente, herramientas y
    /// servidores partan del mismo estado importado.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_sav_game(sav: SavGame) -> Self {
        let clear_legacy_depot_reservations = sav.version < SLV_DEPOT_RESERVATION_PERSISTED;
        let mut map = sav.map;
        normalize_rail_trackbits_from_neighbors(&mut map);
        // `AfterLoadGame()` de OpenTTD sólo reconstruye estas reservas para
        // saves anteriores a `SLV_100`; en una partida moderna el bit es parte
        // del estado visible (overlay PBS del depósito).
        if clear_legacy_depot_reservations {
            crate::depot::clear_all_depot_reservations(&mut map);
        }
        let mut state = Self::from_map(map);
        state.climate = sav.climate;
        state.global_economy = sav.global_economy;
        state.global_economy.inflation_enabled = sav.inflation_enabled;
        state.global_economy.recessions_enabled = sav.recessions_enabled;
        state.sync_scaled_max_loan();
        state.cargo_payments = sav.cargo_payments;
        state.construction = sav.construction;
        state.pathfinding = sav.pathfinding;
        state.train_acceleration_model = sav.train_acceleration_model;
        state.station_noise_level = sav.station_noise_level;
        state.vehicle_breakdowns = sav.vehicle_breakdowns;
        state.no_servicing_if_no_breakdowns = sav.no_servicing_if_no_breakdowns;
        state.subsidy_duration = sav.subsidy_duration;
        state.subsidy_multiplier = sav.subsidy_multiplier;
        state.disasters_enabled = sav.disasters_enabled;
        state.town_council_tolerance = sav.town_council_tolerance;
        state.using_wallclock_units = sav.using_wallclock_units;
        state.vehicle_groups = sav.vehicle_groups;
        state.autoreplace_rules = sav.autoreplace_rules;
        state.sav_opaque_chunks = sav.opaque_chunks;
        if let Some(time) = sav.game_time {
            state.tick = date::game_tick_from_sav_time(time);
        }
        // El modo de tiempo y el tick deben configurar el mismo reloj
        // económico antes de hidratar vehículos/órdenes; de lo contrario un
        // save wallclock vuelve a calendario al primer mes simulado.
        state.sync_timers_from_tick();
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
        let has_company_rows = !sav.companies.is_empty();
        let economy_month = state.economy_timer.month;
        for company in sav.companies {
            let Ok(id) = u8::try_from(company.id) else {
                continue;
            };
            let index = usize::from(id);
            while state.companies.len() <= index {
                let next_id = u8::try_from(state.companies.len()).unwrap_or(u8::MAX);
                let mut created = crate::company::Company::player(
                    crate::game_state::CompanyEconomy::default(),
                    crate::company::first_free_company_colour(&state.companies),
                );
                created.id = crate::CompanyId(next_id);
                created.name = format!("Compañía {}", u16::from(next_id) + 1);
                state.companies.push(created);
            }
            if let Some(target) = state.companies.get_mut(index) {
                target.economy.money = company.money;
                if let Some(loan) = company.loan {
                    target.economy.loan = loan;
                }
                target.set_colour(company.colour);
                if company.liveries.is_empty() {
                    // OpenTTD llama `ResetCompanyLivery` para saves anteriores
                    // al campo: ambos canales vuelven al color de compañía.
                    target.reset_liveries();
                } else {
                    target.set_liveries(company.liveries);
                }
                if let Some(name) = company.name {
                    target.name = name;
                }
                if let Some(name) = company.president_name {
                    target.president_name = Some(name);
                }
                if let Some(face) = company.manager_face {
                    target.manager_face = face;
                }
                if let Some(style) = company.manager_face_style {
                    target.manager_face_style = Some(style);
                }
                if let Some(is_ai) = company.is_ai {
                    target.is_ai = is_ai;
                }
                if let Some(months) = company.bankruptcy_months {
                    target.bankruptcy_months = months;
                }
                hydrate_company_economy_history(
                    target,
                    company.cur_economy.as_ref(),
                    &company.old_economy,
                    economy_month,
                );
                if let Some(value) = company.engine_renew {
                    target.engine_renew = value;
                }
                if let Some(value) = company.engine_renew_months {
                    target.engine_renew_months = value;
                }
                if let Some(value) = company.engine_renew_money {
                    target.engine_renew_money = i64::from(value);
                }
                target.engine_renew_list_head = company.engine_renew_list_head;
                if let Some(value) = company.renew_keep_length {
                    target.renew_keep_length = value;
                }
                if let Some(value) = company.servint_ispercent {
                    target.servint_ispercent = value;
                }
                if let Some(value) = company.servint_trains {
                    target.servint_trains = value;
                }
                if let Some(value) = company.servint_roadveh {
                    target.servint_roadveh = value;
                }
                if let Some(value) = company.servint_aircraft {
                    target.servint_aircraft = value;
                }
                if let Some(value) = company.servint_ships {
                    target.servint_ships = value;
                }
            }
        }
        if has_company_rows {
            state.sync_mirrors_from_active();
        } else {
            // Los saves sintéticos/legacy sólo traen los espejos PLYR
            // `money`/`colour`; absorberlos en la compañía evita que el pool
            // por defecto (100 000) los pise.
            state.sync_active_from_mirrors();
        }
        let station_positions: HashMap<u32, TileCoord> = sav
            .station_index
            .iter()
            .map(|(&station_id, index)| (station_id, index.pos))
            .collect();
        let cargo_packets_by_id: HashMap<u32, &entities::SavCargoPacket> = sav
            .cargo_packets
            .iter()
            .map(|packet| (packet.packet_id, packet))
            .collect();
        // Indexar una sola vez las piezas de aeropuerto importadas. `m2` es
        // el `StationID` y `m6` identifica el tipo de estación del tile.
        let mut imported_airport_tiles: HashMap<u32, Vec<TileCoord>> = HashMap::new();
        let (map_w, map_h) = state.map.dimensions();
        for y in 0..map_h {
            let Ok(y) = i32::try_from(y) else {
                continue;
            };
            for x in 0..map_w {
                let Ok(x) = i32::try_from(x) else {
                    continue;
                };
                let c = TileCoord::new(x, y);
                let Some(tile) = state.map.get(c) else {
                    continue;
                };
                if tile.kind != TileKind::Station
                    || crate::station::stop_kind_from_m6(tile.m6) != StopKind::Airport
                {
                    continue;
                }
                let station_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
                imported_airport_tiles
                    .entry(station_id)
                    .or_default()
                    .push(c);
            }
        }
        for st in sav.stations {
            let stop_kind = stop_kind_from_facilities(st.facilities);
            let mut station = Station::new_with_kind(st.pos, stop_kind);
            station.ottd_station_id = Some(st.station_id);
            station.name = entities::resolve_sav_station_name(&st, &state.towns);
            // Una misma estación puede combinar tren, bus y aeropuerto. No
            // deducir el aeropuerto del `StopKind`: éste sólo conserva una
            // facilidad principal para la simulación simplificada.
            if st.facilities & FACIL_AIRPORT != 0 {
                let spec = AirportSpecId::from_ottd_airport_type(st.airport_type);
                let axis_y = airport_axis_y_from_saved_footprint(spec, st.airport_w, st.airport_h);
                station.airport_spec = spec;
                station.airport_blocks = st.airport_blocks;
                // La huella real ya está en el mapa: `m2` es `StationID` y
                // `m6` codifica `StationType::Airport`. Esto también cubre
                // terminales combinadas tren + bus + avión, cuyo `pos` no es
                // necesariamente el origen del aeropuerto.
                station.airport_tiles = imported_airport_tiles
                    .remove(&st.station_id)
                    .unwrap_or_default();
                // Fallback para fixtures antiguos que no preservan los bytes
                // `m2`/`m6` de estación. Oilrig también es una facilidad
                // aérea, por lo que conserva esta huella para el FTA; sólo su
                // tipo visual se mantiene como estación marítima más abajo.
                if station.airport_tiles.is_empty() {
                    station.airport_tiles = airport_spec_tiles(st.pos, spec, axis_y)
                        .map(|(c, _piece)| c)
                        .collect();
                }
                // El chunk de mapa reconstruido tipa `MP_STATION` genérico;
                // retaguear únicamente los tiles `StationType::Airport`
                // reales. Oilrig comparte la facilidad aérea, pero sus
                // teselas `MP_STATION` deben seguir siendo marítimas para que
                // OpenTTD y el renderer las dibujen sobre agua.
                for &c in &station.airport_tiles {
                    if let Some(mut tile) = state.map.get(c)
                        && tile.kind == TileKind::Station
                        && crate::station::station_type_from_m6(tile.m6) == 1
                    {
                        tile.kind = TileKind::Airport;
                        let _ = state.map.set_tile(c, tile);
                    }
                }
            }
            hydrate_sav_station_cargo(
                &mut station,
                &st.cargo,
                &cargo_packets_by_id,
                &station_positions,
                state.climate,
            );
            state.stations.push(station);
        }
        import::hydrate_sav_industries(&mut state, &sav.industries, &sav.extras);
        state.link_graph = sav.link_graph;
        if !matches!(
            state.cargo_dist.distribution,
            crate::flow_stat::DistributionType::Manual
        ) {
            state.rebuild_station_flows();
        }
        for v in &sav.vehicles {
            let kind = match v.kind {
                SavVehicleKind::Train => VehicleKind::Train,
                // Pasajeros (cargo 0) → bus; el resto, camión.
                SavVehicleKind::RoadVehicle if v.cargo_type == 0 => VehicleKind::Bus,
                SavVehicleKind::RoadVehicle => VehicleKind::Truck,
                SavVehicleKind::Ship => VehicleKind::Ship,
                SavVehicleKind::Aircraft => VehicleKind::Aircraft,
            };
            let id = v.sav_id;
            if kind == VehicleKind::Aircraft {
                let mut vehicle = Vehicle::new(id, kind, v.pos, v.pos);
                vehicle.group_id = v.group_id;
                vehicle.timetable_start =
                    u32::try_from(v.timetable_start.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
                vehicle.current_order_time = v.current_order_time;
                vehicle.timetable_lateness = v.timetable_lateness;
                vehicle.vehicle_flags = v.vehicle_flags;
                vehicle.service_interval_days = v.service_interval;
                vehicle.timetable_started = v.vehicle_flags & (1 << 3) != 0;
                vehicle.timetable_autofill = v.vehicle_flags & (1 << 4) != 0;
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
                vehicle.timetable_autofill_samples = vehicle
                    .orders
                    .iter()
                    .map(|order| (order.wait_ticks(), order.travel_ticks()))
                    .collect();
                vehicle.timetable_active = v.timetable_start != 0
                    || vehicle.orders.iter().any(|order| {
                        order.wait_ticks() != 0
                            || order.travel_ticks() != 0
                            || order.max_speed_limit() != 0
                    });
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
                continue;
            }
            let mut vehicle = Vehicle::new(id, kind, v.pos, v.pos);
            vehicle.group_id = v.group_id;
            vehicle.timetable_start =
                u32::try_from(v.timetable_start.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            vehicle.current_order_time = v.current_order_time;
            vehicle.timetable_lateness = v.timetable_lateness;
            vehicle.vehicle_flags = v.vehicle_flags;
            vehicle.service_interval_days = v.service_interval;
            vehicle.timetable_started = v.vehicle_flags & (1 << 3) != 0;
            vehicle.timetable_autofill = v.vehicle_flags & (1 << 4) != 0;
            vehicle.running = v.running;
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
                reconcile_imported_vehicle_position(&state.map, &mut vehicle);
            }
            vehicle.timetable_autofill_samples = vehicle
                .orders
                .iter()
                .map(|order| (order.wait_ticks(), order.travel_ticks()))
                .collect();
            vehicle.timetable_active = v.timetable_start != 0
                || vehicle.orders.iter().any(|order| {
                    order.wait_ticks() != 0
                        || order.travel_ticks() != 0
                        || order.max_speed_limit() != 0
                });
            // `set_vehicle_orders` reinicia progreso para comandos nuevos, pero
            // al importar debe conservar exactamente el estado sub-tesela del save.
            vehicle.progress = v.progress;
            if kind == VehicleKind::Train {
                vehicle.rail_pixel = rail_pixel_from_openttd_pos(v.x_pos, v.y_pos, v.direction);
            }
            state.vehicles.push(vehicle);
        }

        // La tabla sparse `VEHS` puede intercalar unidades de distintos
        // vehículos. Reconstruir el consist por `Vehicle::next`, no por el
        // orden de las filas: tratar un vagón huérfano como cabeza dispara PBS
        // y routing como si fuesen cientos de trenes adicionales.
        let slots_by_sav_id: HashMap<u32, usize> = state
            .vehicles
            .iter()
            .enumerate()
            .map(|(slot, vehicle)| (vehicle.id, slot))
            .collect();
        for v in &sav.vehicles {
            if v.kind != SavVehicleKind::Train {
                continue;
            }
            let Some(next_sav_id) = v.next_sav_id else {
                continue;
            };
            let (Some(&from), Some(&to)) = (
                slots_by_sav_id.get(&v.sav_id),
                slots_by_sav_id.get(&next_sav_id),
            ) else {
                continue;
            };
            if from == to
                || state.vehicles[from].kind != VehicleKind::Train
                || state.vehicles[to].kind != VehicleKind::Train
                || state.vehicles[from].next_unit.is_some()
                || state.vehicles[to].prev_unit.is_some()
            {
                continue;
            }
            let (from_id, to_id) = (state.vehicles[from].id, state.vehicles[to].id);
            state.vehicles[from].next_unit = Some(to_id);
            state.vehicles[to].prev_unit = Some(from_id);
        }

        // Fallback para saves antiguos que no traen la referencia `next`.
        // Solo enlaza el bloque contiguo de trenes, sin inventar conexiones a
        // través de una fila de otro vehículo.
        let mut fallback_head: Option<u32> = None;
        for v in &sav.vehicles {
            if v.kind != SavVehicleKind::Train {
                fallback_head = None;
                continue;
            }
            let Some(&slot) = slots_by_sav_id.get(&v.sav_id) else {
                continue;
            };
            if !v.is_wagon {
                fallback_head = Some(v.sav_id);
                continue;
            }
            if state.vehicles[slot].prev_unit.is_some() {
                continue;
            }
            if let Some(head) = fallback_head {
                let _ = crate::train_consist::attach_wagon(&mut state.vehicles, head, v.sav_id);
            }
        }

        let train_heads: Vec<u32> = state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
            .map(|vehicle| vehicle.id)
            .collect();
        for head in train_heads {
            crate::train_consist::consist_changed(&mut state.vehicles, head);
        }

        // En saves modernos `VEHS.common.orders` es una referencia al pool
        // `ORDL`, no una copia inline. Agrupar por ese índice vuelve a
        // materializar la identidad de shared orders que el parser ya había
        // usado para obtener el contenido de la lista.
        let mut shared_vehicle_ids: HashMap<u32, Vec<u32>> = HashMap::new();
        for vehicle in &sav.vehicles {
            if let Some(order_list_id) = vehicle.order_list_id {
                shared_vehicle_ids
                    .entry(order_list_id)
                    .or_default()
                    .push(vehicle.sav_id);
            }
        }
        for (order_list_id, sav_ids) in shared_vehicle_ids {
            let Some(first_id) = sav_ids.first().copied() else {
                continue;
            };
            let Some(orders) = state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == first_id)
                .filter(|vehicle| !vehicle.orders.is_empty())
                .map(|vehicle| vehicle.orders.clone())
            else {
                continue;
            };
            state
                .shared_order_lists
                .push(crate::shared_orders::SharedOrderList {
                    id: order_list_id,
                    orders,
                });
            for vehicle in &mut state.vehicles {
                if sav_ids.contains(&vehicle.id) {
                    vehicle.shared_order_id = Some(order_list_id);
                }
            }
        }
        state
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn empty_sav(version: u16, map: Map) -> SavGame {
        SavGame {
            version,
            map,
            extras: OttdmapExtras::default(),
            stations: Vec::new(),
            towns: Vec::new(),
            industries: Vec::new(),
            cargo_packets: Vec::new(),
            cargo_payments: Vec::new(),
            vehicles: Vec::new(),
            companies: Vec::new(),
            money: None,
            company_colour: None,
            station_index: HashMap::new(),
            game_time: None,
            link_graph: LinkGraphStats::default(),
            climate: crate::Climate::Temperate,
            construction: crate::ConstructionSettings::default(),
            pathfinding: crate::PathfindingSettings::default(),
            train_acceleration_model: crate::engine::TrainAccelerationModel::Realistic,
            station_noise_level: false,
            vehicle_breakdowns: 2,
            no_servicing_if_no_breakdowns: true,
            subsidy_duration: 1,
            subsidy_multiplier: 1,
            disasters_enabled: true,
            town_council_tolerance: crate::town::TownCouncilTolerance::default(),
            using_wallclock_units: false,
            inflation_enabled: true,
            recessions_enabled: false,
            global_economy: crate::economy::GlobalEconomy::new(),
            vehicle_groups: Vec::new(),
            autoreplace_rules: Vec::new(),
            opaque_chunks: Vec::new(),
        }
    }

    #[test]
    fn stop_kind_mapping() {
        assert_eq!(stop_kind_from_facilities(0x01), StopKind::RailStation);
        assert_eq!(stop_kind_from_facilities(0x05), StopKind::RailStation);
        assert_eq!(stop_kind_from_facilities(0x04), StopKind::BusStop);
        assert_eq!(stop_kind_from_facilities(0x02), StopKind::TruckStop);
        assert_eq!(stop_kind_from_facilities(0x08), StopKind::Airport);
        assert_eq!(stop_kind_from_facilities(0x10), StopKind::Dock);
    }

    #[test]
    fn from_sav_game_preserves_signal_side_settings() {
        let mut sav = empty_sav(352, Map::new_flat(4, 4, 0));
        sav.construction.road_vehicle_driving_side = crate::RoadVehicleDrivingSide::Right;
        sav.construction.train_signal_side = crate::TrainSignalSide::RoadVehicleDrivingSide;

        let state = GameState::from_sav_game(sav);
        assert!(state.construction.signals_on_right());
    }

    #[test]
    fn airport_fallback_does_not_retag_oilrig_station_tiles() {
        let oilrig = TileCoord::new(3, 3);
        let mut sav = empty_sav(352, Map::new_flat(8, 8, 0));
        sav.map
            .set_tile(
                oilrig,
                crate::Tile {
                    height: 0,
                    kind: TileKind::Station,
                    mapt: 0x50,
                    m5: 0,
                    m1: 0,
                    m6: crate::station::STATION_TYPE_OILRIG << 3,
                    m8: 0,
                    m3: 0,
                    m2: 0,
                    m2_hi: 0,
                    m7: 0,
                    m3hi: 0,
                },
            )
            .expect("oilrig tile");
        sav.stations.push(SavStation {
            station_id: 7,
            pos: oilrig,
            name: Some("Plataforma".to_string()),
            facilities: FACIL_AIRPORT,
            string_id: None,
            town_id: None,
            airport_type: 9,
            airport_w: 1,
            airport_h: 1,
            airport_layout: 0,
            airport_blocks: 0,
            cargo: Vec::new(),
        });

        let state = GameState::from_sav_game(sav);
        let station = state.stations.first().expect("station");
        assert_eq!(station.airport_tiles, vec![oilrig]);
        assert_eq!(state.map.get_kind(oilrig), Some(TileKind::Station));
        assert_eq!(
            state.map.get(oilrig).map(|tile| tile.m6),
            Some(crate::station::STATION_TYPE_OILRIG << 3)
        );
    }

    #[test]
    fn from_sav_game_imports_station_capa_packets_in_core() {
        let source = TileCoord::new(1, 1);
        let destination = TileCoord::new(5, 5);
        let mut sav = empty_sav(352, Map::new_flat(8, 8, 0));
        sav.stations = vec![
            SavStation {
                station_id: 0,
                pos: source,
                name: Some("Origen".to_string()),
                facilities: FACIL_TRAIN,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
                cargo: vec![entities::SavStationCargo {
                    cargo_slot: 1,
                    packet_ids: vec![42],
                    reserved: 2,
                }],
            },
            SavStation {
                station_id: 1,
                pos: destination,
                name: Some("Destino".to_string()),
                facilities: FACIL_TRAIN,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
                cargo: Vec::new(),
            },
        ];
        sav.cargo_packets = vec![entities::SavCargoPacket {
            packet_id: 42,
            source_station_id: Some(0),
            source_xy: Some(TileCoord::new(2, 2)),
            next_hop_station_id: Some(1),
            count: 9,
            periods_in_transit: 7,
            feeder_share: 11,
        }];
        for (station_id, pos) in [(0, source), (1, destination)] {
            sav.station_index.insert(
                station_id,
                entities::SavStationIndex {
                    pos,
                    is_waypoint: false,
                    facilities: FACIL_TRAIN,
                    name: None,
                    string_id: None,
                    town_id: None,
                    airport_type: 0,
                    airport_w: 0,
                    airport_h: 0,
                    airport_layout: 0,
                    airport_blocks: 0,
                },
            );
        }

        let state = GameState::from_sav_game(sav);
        let station = &state.stations[0];
        assert_eq!(station.cargo_stock.coal, 9);
        assert_eq!(station.cargo_packets.reserved, 2);
        let packet = station
            .cargo_packets
            .packets()
            .next()
            .expect("imported packet");
        assert_eq!(packet.source, TileCoord::new(2, 2));
        assert_eq!(packet.first_station, Some(source));
        assert_eq!(packet.next_hop, Some(destination));
        assert_eq!(packet.periods_in_transit, 7);
        assert_eq!(packet.feeder_share, 11);
    }

    #[test]
    fn modern_sav_keeps_rail_depot_pbs_reservation() {
        let pos = TileCoord::new(7, 9);
        let mut map = Map::new_flat(64, 64, 0);
        let mut depot = map.get(pos).expect("rail depot tile");
        depot.kind = TileKind::RailDepot;
        depot.mapt = 0x10;
        depot.m5 = 0xD2;
        map.set_tile(pos, depot).expect("set rail depot tile");

        let state = GameState::from_sav_game(empty_sav(SLV_DEPOT_RESERVATION_PERSISTED, map));
        assert_eq!(state.map.get(pos).map(|tile| tile.m5), Some(0xD2));
        assert!(crate::depot::has_depot_reservation(&state.map, pos));
    }

    #[test]
    fn legacy_sav_rebuilds_rail_depot_pbs_reservation() {
        let pos = TileCoord::new(7, 9);
        let mut map = Map::new_flat(64, 64, 0);
        let mut depot = map.get(pos).expect("rail depot tile");
        depot.kind = TileKind::RailDepot;
        depot.mapt = 0x10;
        depot.m5 = 0xD2;
        map.set_tile(pos, depot).expect("set rail depot tile");

        let state = GameState::from_sav_game(empty_sav(SLV_DEPOT_RESERVATION_PERSISTED - 1, map));
        assert_eq!(state.map.get(pos).map(|tile| tile.m5), Some(0xC2));
        assert!(!crate::depot::has_depot_reservation(&state.map, pos));
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
        let mut map = Map::new_flat(64, 64, 0);
        // Dos vías colineales separadas por bosque no forman un puente. El
        // importador debe preservar el hueco tal cual venía en el save.
        let forest_gap = crate::TileCoord::new(2, 1);
        for c in [crate::TileCoord::new(1, 1), crate::TileCoord::new(3, 1)] {
            let mut tile = map.get(c).expect("rail endpoint");
            tile.kind = TileKind::Rail;
            tile.mapt = 0x10;
            tile.m5 = crate::map::RAIL_TB_X;
            map.set_tile(c, tile).expect("set rail endpoint");
        }
        let mut forest = map.get(forest_gap).expect("forest gap");
        forest.kind = TileKind::Forest;
        forest.mapt = 0x40;
        map.set_tile(forest_gap, forest).expect("set forest gap");
        // Estación integrada: tren + buses + aeropuerto. El tile de avión no
        // coincide con el ancla de la estación y conserva el `StationGfx`
        // vanilla en `m5`.
        let airport_tile = crate::TileCoord::new(12, 12);
        let mut airport = map.get(airport_tile).expect("airport tile");
        airport.kind = TileKind::Station;
        airport.mapt = 0x50;
        airport.m2 = 1;
        airport.m6 = 1 << 3;
        airport.m5 = 14;
        map.set_tile(airport_tile, airport)
            .expect("set airport tile");
        let sav = SavGame {
            version: 300,
            map,
            extras: OttdmapExtras::default(),
            stations: vec![
                SavStation {
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
                    cargo: Vec::new(),
                },
                SavStation {
                    station_id: 1,
                    pos: crate::TileCoord::new(10, 10),
                    name: Some("Intermodal".into()),
                    facilities: FACIL_TRAIN | FACIL_BUS_STOP | FACIL_AIRPORT,
                    string_id: None,
                    town_id: None,
                    airport_type: 7,
                    airport_w: 9,
                    airport_h: 11,
                    airport_layout: 0,
                    airport_blocks: 0,
                    cargo: Vec::new(),
                },
            ],
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
            cargo_packets: Vec::new(),
            cargo_payments: Vec::new(),
            link_graph: LinkGraphStats::default(),
            vehicles: vec![
                SavVehicle {
                    sav_id: 0,
                    // Las unidades pueden estar separadas por filas de otros
                    // tipos de vehículo en `VEHS`.
                    next_sav_id: Some(3),
                    group_id: None,
                    timetable_start: 0,
                    current_order_time: 0,
                    timetable_lateness: 0,
                    vehicle_flags: 0b1_1000,
                    service_interval: 150,
                    order_list_id: None,
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
                    sav_id: 1,
                    next_sav_id: None,
                    group_id: None,
                    timetable_start: 0,
                    current_order_time: 0,
                    timetable_lateness: 0,
                    vehicle_flags: 0,
                    service_interval: 150,
                    order_list_id: None,
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
                    sav_id: 2,
                    next_sav_id: None,
                    group_id: None,
                    timetable_start: 0,
                    current_order_time: 0,
                    timetable_lateness: 0,
                    vehicle_flags: 0,
                    service_interval: 150,
                    order_list_id: None,
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
                SavVehicle {
                    sav_id: 3,
                    next_sav_id: None,
                    group_id: None,
                    timetable_start: 0,
                    current_order_time: 0,
                    timetable_lateness: 0,
                    vehicle_flags: 0,
                    service_interval: 150,
                    order_list_id: None,
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
                    is_wagon: true,
                    is_helicopter: false,
                    airport_pos: 0,
                    airport_previous_pos: 0,
                    airport_state: 0,
                    airport_targetairport: 0,
                },
            ],
            companies: Vec::new(),
            money: Some(123_456),
            company_colour: Some(9),
            station_index: std::collections::HashMap::new(),
            game_time: None,
            climate: crate::Climate::Temperate,
            construction: crate::ConstructionSettings::default(),
            pathfinding: crate::PathfindingSettings::default(),
            train_acceleration_model: crate::engine::TrainAccelerationModel::Realistic,
            station_noise_level: false,
            vehicle_breakdowns: 2,
            no_servicing_if_no_breakdowns: true,
            subsidy_duration: 1,
            subsidy_multiplier: 1,
            disasters_enabled: true,
            town_council_tolerance: crate::town::TownCouncilTolerance::default(),
            using_wallclock_units: false,
            inflation_enabled: true,
            recessions_enabled: false,
            global_economy: crate::economy::GlobalEconomy::new(),
            vehicle_groups: Vec::new(),
            autoreplace_rules: Vec::new(),
            opaque_chunks: Vec::new(),
        };
        let state = GameState::from_sav_game(sav);
        assert_eq!(state.stations.len(), 2);
        assert_eq!(state.stations[0].stop_kind, StopKind::RailStation);
        assert_eq!(state.stations[0].name.as_deref(), Some("Estación Norte"));
        assert_eq!(state.towns.len(), 1);
        assert_eq!(state.towns[0].name, "Springfield");
        assert_eq!(state.economy.money, 123_456);
        assert_eq!(state.company_colour, 9);
        assert_eq!(state.vehicles.len(), 4);
        assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
        assert_eq!(state.vehicles[1].kind, VehicleKind::Bus);
        assert_eq!(state.vehicles[2].kind, VehicleKind::Truck);
        assert_eq!(state.vehicles[0].next_unit, Some(3));
        assert!(state.vehicles[0].timetable_started);
        assert!(state.vehicles[0].timetable_autofill);
        assert_eq!(state.vehicles[0].vehicle_flags, 0b1_1000);
        assert_eq!(state.vehicles[0].service_interval_days, 150);
        assert_eq!(state.vehicles[3].prev_unit, Some(0));
        assert_eq!(state.map.get_kind(forest_gap), Some(TileKind::Forest));
        let imported_airport = state
            .stations
            .iter()
            .find(|station| station.ottd_station_id == Some(1))
            .expect("integrated airport station");
        assert_eq!(imported_airport.stop_kind, StopKind::RailStation);
        assert_eq!(imported_airport.airport_tiles, vec![airport_tile]);
        assert_eq!(state.map.get_kind(airport_tile), Some(TileKind::Airport));
        assert_eq!(state.map.get(airport_tile).map(|tile| tile.m5), Some(14));
    }
}
