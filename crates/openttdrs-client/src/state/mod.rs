//! Estado del mundo de simulacion y generacion procedural.

pub(crate) mod bootstrap;
pub(crate) mod stations;

use bevy::prelude::*;
use openttdrs_core::{GameState, Map, OttdmapExtras};

use crate::config::apply_test_company_colour;
use crate::state::bootstrap::{
    fill_flat_grass, log_detection_summary, log_gameplay_showcase_zones, log_procedural_demo_zones,
    place_bridge_demo_gap, place_clean_demo_transport, place_demo_economy_loop,
    place_gameplay_showcase, place_industries, place_industries_from_sav, place_stations,
    place_stations_from_footer_stxy, place_stations_from_map_tiles, place_tunnel_demo_ridge,
};

/// Dimensiones del mapa generado proceduralmente (sin `OTTDMAP_FILE`).
pub const MAP_W: u32 = 24;
pub const MAP_H: u32 = 18;

/// Carga un save de `OpenTTD` (`.sav`) y aplica el bootstrap de mapas reales:
/// industrias del chunk `INDY` (o heurística en saves sin tablas), estaciones
/// por teselas deduplicadas con las del chunk `STNN`, vehículos y dinero.
pub(crate) fn load_sav_state(bytes: &[u8]) -> Result<GameState, String> {
    let sav = openttdrs_core::sav::load(bytes).map_err(|e| e.to_string())?;
    let extras = sav.extras.clone();
    let sav_industries = sav.industries.clone();
    let mut state = GameState::from_sav_game(sav);
    apply_test_company_colour(&mut state);
    if sav_industries.is_empty() {
        place_industries(&mut state, true, Some(&extras));
    } else {
        place_industries_from_sav(&mut state, &sav_industries);
    }
    place_stations_from_map_tiles(&mut state);
    place_stations_from_footer_stxy(&mut state, Some(&extras));
    info!(
        "Save OpenTTD cargado: {} estaciones, {} ciudades, {} industrias, {} vehículos, ${}",
        state.stations.len(),
        state.towns.len(),
        state.industries.len(),
        state.vehicles.len(),
        state.economy.money,
    );
    log_detection_summary(&state, true, Some(&extras));
    Ok(state)
}

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
                    Ok(mut state) => {
                        apply_test_company_colour(&mut state);
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
                        apply_test_company_colour(&mut state);
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
        fill_flat_grass(&mut state);
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);
        place_gameplay_showcase(&mut state);
        place_tunnel_demo_ridge(&mut state);
        place_bridge_demo_gap(&mut state);
        log_procedural_demo_zones();
        log_gameplay_showcase_zones();
        log_detection_summary(&state, false, None);
        apply_test_company_colour(&mut state);
        Self {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod sim_world_coverage_tests {
    use super::SimWorld;

    #[test]
    fn sim_world_default_runs_procedural_bootstrap() {
        let w = SimWorld::default();
        assert!(!w.loaded_file);
        assert!(w.ottdmap_extras.is_none());
        assert_eq!(
            w.state.industries.len(),
            3,
            "mina demo + bosque + fábrica showcase"
        );
        assert_eq!(
            w.state.stations.len(),
            8,
            "2 camión demo + 2 bus + 2 camión showcase + 2 estación tren"
        );
        assert!(
            w.state.vehicles.iter().any(|v| v.id == 9102),
            "tren showcase en bootstrap procedural"
        );
        let truck = w
            .state
            .vehicles
            .iter()
            .find(|v| v.id == 9010)
            .expect("camión económico demo");
        assert_eq!(truck.orders.len(), 2);
    }
}

#[cfg(test)]
mod ui_command_integration;
