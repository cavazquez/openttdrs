//! Parser nativo de savegames de `OpenTTD` (`.sav`): contenedor comprimido,
//! chunks de mapa, estaciones (`STNN`) y ciudades (`CITY`).
//!
//! El mapa se reconstruye reutilizando el pipeline `.ottdmap` ya validado
//! (`Map::from_ottd_binary_with_extras`), generando el bloque en memoria.

mod build;
mod chunks;
mod container;
mod entities;
mod table;

use crate::game_state::GameState;
use crate::map::Map;
use crate::ottdmap_extras::OttdmapExtras;
use crate::station::{Station, StopKind};
use crate::town::Town;

pub use entities::SavStation;

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
    let towns = entities::towns_from_chunks(&chunk_list, map_w);
    Ok(SavGame {
        version,
        map,
        extras,
        stations,
        towns,
    })
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
    /// Estado jugable desde un save de `OpenTTD`: mapa + estaciones + ciudades.
    ///
    /// Las industrias por heurística de teselas las añade el cliente
    /// (`place_industries`) usando `SavGame::extras`.
    #[must_use]
    pub fn from_sav_game(sav: SavGame) -> Self {
        let mut state = Self::from_map(sav.map);
        state.jgr_tunnels_from_footer = sav.extras.jgr_tunnels_from_tnbp();
        state.towns = sav.towns;
        for st in sav.stations {
            let mut station =
                Station::new_with_kind(st.pos, stop_kind_from_facilities(st.facilities));
            station.name = st.name;
            state.stations.push(station);
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
        };
        let state = GameState::from_sav_game(sav);
        assert_eq!(state.stations.len(), 1);
        assert_eq!(state.stations[0].stop_kind, StopKind::RailStation);
        assert_eq!(state.stations[0].name.as_deref(), Some("Estación Norte"));
        assert_eq!(state.towns.len(), 1);
        assert_eq!(state.towns[0].name, "Springfield");
    }
}
