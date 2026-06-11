//! Parser nativo de savegames de `OpenTTD` (`.sav`): contenedor comprimido,
//! chunks de mapa, estaciones (`STNN`), ciudades (`CITY`), industrias
//! (`INDY`), vehículos (`VEHS`) y dinero de la empresa (`PLYR`).
//!
//! El mapa se reconstruye reutilizando el pipeline `.ottdmap` ya validado
//! (`Map::from_ottd_binary_with_extras`), generando el bloque en memoria.

mod build;
mod chunks;
mod container;
mod entities;
mod house_population_generated;
mod table;

use crate::game_state::GameState;
use crate::map::Map;
use crate::ottdmap_extras::OttdmapExtras;
use crate::station::{Station, StopKind};
use crate::town::Town;
use crate::vehicle::{Vehicle, VehicleKind};

pub use entities::{SavIndustry, SavStation, SavVehicle, SavVehicleKind};

/// Bits `FACIL_*` de `OpenTTD`.
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;

/// Error al cargar un `.sav`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavError {
    /// Compresión no soportada (p. ej. LZO de saves muy antiguos).
    UnsupportedCompression(String),
    /// Error al descomprimir el payload.
    Decompress(String),
    /// Formato/contenido inesperado.
    BadFormat(String),
}

impl std::fmt::Display for SavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedCompression(s) => write!(f, "compresión no soportada: {s}"),
            Self::Decompress(s) => write!(f, "error de descompresión: {s}"),
            Self::BadFormat(s) => write!(f, "formato inválido: {s}"),
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
    let stations = entities::stations_from_chunks(&chunk_list, map_w);
    let mut towns = entities::towns_from_chunks(&chunk_list, map_w);
    rebuild_town_populations(&map, &mut towns);
    let industries = entities::industries_from_chunks(&chunk_list, map_w);
    let vehicles = entities::vehicles_from_chunks(&chunk_list, map_w);
    let money = entities::company_money_from_chunks(&chunk_list);
    Ok(SavGame {
        version,
        map,
        extras,
        stations,
        towns,
        industries,
        vehicles,
        money,
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

#[must_use]
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
        let mut state = Self::from_map(sav.map);
        state.jgr_tunnels_from_footer = sav.extras.jgr_tunnels_from_tnbp();
        state.towns = sav.towns;
        if let Some(money) = sav.money {
            state.economy.money = money;
        }
        for st in sav.stations {
            let mut station =
                Station::new_with_kind(st.pos, stop_kind_from_facilities(st.facilities));
            station.name = st.name;
            state.stations.push(station);
        }
        for (i, v) in sav.vehicles.iter().enumerate() {
            let kind = match v.kind {
                SavVehicleKind::Train => VehicleKind::Train,
                // Pasajeros (cargo 0) → bus; el resto, camión.
                SavVehicleKind::RoadVehicle if v.cargo_type == 0 => VehicleKind::Bus,
                SavVehicleKind::RoadVehicle => VehicleKind::Truck,
            };
            #[allow(clippy::cast_possible_truncation)]
            state
                .vehicles
                .push(Vehicle::new(i as u32, kind, v.pos, v.pos));
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
            }],
            industries: Vec::new(),
            vehicles: vec![
                SavVehicle {
                    kind: SavVehicleKind::Train,
                    pos: crate::TileCoord::new(5, 5),
                    cargo_type: 9,
                },
                SavVehicle {
                    kind: SavVehicleKind::RoadVehicle,
                    pos: crate::TileCoord::new(6, 6),
                    cargo_type: 0,
                },
                SavVehicle {
                    kind: SavVehicleKind::RoadVehicle,
                    pos: crate::TileCoord::new(7, 7),
                    cargo_type: 5,
                },
            ],
            money: Some(123_456),
        };
        let state = GameState::from_sav_game(sav);
        assert_eq!(state.stations.len(), 1);
        assert_eq!(state.stations[0].stop_kind, StopKind::RailStation);
        assert_eq!(state.stations[0].name.as_deref(), Some("Estación Norte"));
        assert_eq!(state.towns.len(), 1);
        assert_eq!(state.towns[0].name, "Springfield");
        assert_eq!(state.economy.money, 123_456);
        assert_eq!(state.vehicles.len(), 3);
        assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
        assert_eq!(state.vehicles[1].kind, VehicleKind::Bus);
        assert_eq!(state.vehicles[2].kind, VehicleKind::Truck);
    }
}
