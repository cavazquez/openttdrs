//! Estado del mundo de simulación y generación procedural.

use bevy::prelude::*;
use openttdrs_core::{GameState, Map, OttdmapExtras};

use crate::state_bootstrap::{
    distribute_tile_kinds, log_detection_summary, place_industries, place_roads, place_stations,
    place_stations_from_footer_stxy, place_stations_from_map_tiles, place_vehicles,
};

/// Dimensiones del mapa generado proceduralmente (sin `OTTDMAP_FILE`).
pub const MAP_W: u32 = 24;
pub const MAP_H: u32 = 18;

/// Pantalla actual del cliente.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClientScreen {
    #[default]
    MainMenu,
    InGame,
}

/// Estado del mundo de simulación.
#[derive(Resource)]
pub struct SimWorld {
    pub state: GameState,
    /// Indica que el mapa se cargó desde un archivo .ottdmap externo.
    pub loaded_file: bool,
    /// Footers `INDP` / blobs opcionales si el mapa se cargó con `from_ottd_binary_with_extras`.
    pub ottdmap_extras: Option<OttdmapExtras>,
}

impl Default for SimWorld {
    fn default() -> Self {
        if let Ok(path) = std::env::var("OTTDJSON_LOAD") {
            match std::fs::read_to_string(&path) {
                Ok(text) => match openttdrs_core::save::load_from_str(&text) {
                    Ok(state) => {
                        info!("Estado de simulación cargado desde JSON: {path}");
                        log_detection_summary(&state, true, None);
                        return Self {
                            state,
                            loaded_file: true,
                            ottdmap_extras: None,
                        };
                    }
                    Err(e) => error!("OTTDJSON_LOAD no es JSON válido ({path}): {e}"),
                },
                Err(e) => error!("No se pudo leer OTTDJSON_LOAD={path}: {e}"),
            }
        }
        if let Ok(path) = std::env::var("OTTDMAP_FILE") {
            match std::fs::read(&path) {
                Ok(data) => match Map::from_ottd_binary_with_extras(&data) {
                    Ok((map, extras)) => {
                        info!("Mapa cargado desde {path}");
                        let mut state = GameState::from_map(map);
                        state.jgr_tunnels_from_footer = extras.jgr_tunnels_from_tnbp();
                        place_industries(&mut state, true, Some(&extras));
                        place_stations(&mut state);
                        place_stations_from_map_tiles(&mut state);
                        place_stations_from_footer_stxy(&mut state, Some(&extras));
                        place_vehicles(&mut state);
                        log_detection_summary(&state, true, Some(&extras));
                        return Self {
                            state,
                            loaded_file: true,
                            ottdmap_extras: Some(extras),
                        };
                    }
                    Err(e) => error!("Error al parsear {path}: {e:?}"),
                },
                Err(e) => error!("No se pudo leer {path}: {e}"),
            }
        }
        let mut state = GameState::new(MAP_W, MAP_H);
        distribute_tile_kinds(&mut state, 0xDEAD_BEEF_CAFE_1234);
        place_industries(&mut state, false, None);
        place_stations(&mut state);
        place_roads(&mut state);
        place_vehicles(&mut state);
        log_detection_summary(&state, false, None);
        Self {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        }
    }
}
