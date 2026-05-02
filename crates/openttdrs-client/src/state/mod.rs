//! Estado del mundo de simulacion y generacion procedural.

pub(crate) mod bootstrap;
pub(crate) mod stations;

use bevy::prelude::*;
use openttdrs_core::{GameState, Map, OttdmapExtras};

use crate::state::bootstrap::{
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

/// Estado del mundo de simulacion.
#[derive(Resource)]
pub struct SimWorld {
    pub state: GameState,
    /// Indica que el mapa se cargo desde un archivo .ottdmap externo.
    pub loaded_file: bool,
    /// Footers `INDP` / blobs opcionales si el mapa se cargo con `from_ottd_binary_with_extras`.
    pub ottdmap_extras: Option<OttdmapExtras>,
}

impl Default for SimWorld {
    fn default() -> Self {
        if let Ok(path) = std::env::var("OTTDJSON_LOAD") {
            match std::fs::read_to_string(&path) {
                Ok(text) => match openttdrs_core::save::load_from_str(&text) {
                    Ok(state) => {
                        info!("Estado de simulacion cargado desde JSON: {path}");
                        log_detection_summary(&state, true, None);
                        return Self {
                            state,
                            loaded_file: true,
                            ottdmap_extras: None,
                        };
                    }
                    Err(e) => error!("OTTDJSON_LOAD no es JSON valido ({path}): {e}"),
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
                        // En mapas reales no inyectar vehículos de demo:
                        // se renderizan solo los realmente presentes en datos cargados.
                        log_detection_summary(&state, true, Some(&extras));
                        return Self {
                            state,
                            loaded_file: true,
                            ottdmap_extras: Some(extras),
                        };
                    }
                    Err(e) => {
                        let magic_hint = if data.len() >= 4 {
                            let m = String::from_utf8_lossy(&data[0..4]);
                            format!(
                                " primeros 4 bytes: {m:?} (hex {:02x}{:02x}{:02x}{:02x})",
                                data[0], data[1], data[2], data[3],
                            )
                        } else {
                            String::new()
                        };
                        panic!(
                            "Error al parsear OTTDMAP_FILE={path}: {e:?}.{magic_hint} \
Se espera cabecera MAP1 + planos densos actuales; regenera con: \
python3 scripts/parse_sav.py tu.sav {path}"
                        );
                    }
                },
                Err(e) => panic!("No se pudo leer OTTDMAP_FILE={path}: {e}"),
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

#[cfg(test)]
mod sim_world_coverage_tests {
    use super::SimWorld;

    #[test]
    fn sim_world_default_runs_procedural_bootstrap() {
        let w = SimWorld::default();
        assert!(!w.loaded_file);
        assert!(w.ottdmap_extras.is_none());
    }
}

#[cfg(test)]
mod ui_command_integration;
