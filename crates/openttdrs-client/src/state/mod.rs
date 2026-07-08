//! Estado del mundo de simulacion y generacion procedural.

pub(crate) mod bootstrap;
pub(crate) mod ingame_lifecycle;
pub(crate) mod new_game;
pub(crate) mod stations;

use bevy::prelude::*;
use openttdrs_core::{GameState, Map, OttdmapExtras};

use crate::config::{apply_test_company_colour, climate_from_env, env_u64, world_gen_enabled};
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, START_YEARS, build_procedural_demo_world,
    log_detection_summary, log_gameplay_showcase_zones, log_procedural_demo_zones,
    place_industries, place_industries_from_sav, place_stations, place_stations_from_footer_stxy,
    place_stations_from_map_tiles,
};

/// Dimensiones del mapa demo compacto (24×18).
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
    state.pending_sim_events.discard_all();
    Ok(state)
}

fn settings_from_env() -> NewGameSettings {
    NewGameSettings {
        climate: climate_from_env(),
        map_size: MapSizePreset::Compact,
        start_year: START_YEARS[0],
        world_gen: world_gen_enabled(),
        island: std::env::var_os("OPENTTDRS_WORLD_ISLAND").is_some(),
        preserve_demo: !world_gen_enabled() || std::env::var_os("OPENTTDRS_WORLD_FULL").is_none(),
        seed: env_u64("OPENTTDRS_WORLD_SEED", 0),
        ..NewGameSettings::default()
    }
}

fn bootstrap_procedural_state(settings: &NewGameSettings) -> GameState {
    let mut state = build_procedural_demo_world(settings);
    if settings.preserve_demo {
        log_procedural_demo_zones();
        log_gameplay_showcase_zones();
    }
    log_detection_summary(&state, false, None);
    apply_test_company_colour(&mut state);
    if settings.world_gen {
        info!(
            "Mapa procedural: clima={:?}, seed={}, isla={}, demo={}",
            settings.climate, state.world_seed, settings.island, settings.preserve_demo
        );
    }
    state.pending_sim_events.discard_all();
    state
}

/// Pantalla actual del cliente.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClientScreen {
    #[default]
    MainMenu,
    InGame,
}

/// Sub-estado de simulación: solo existe mientras `ClientScreen::InGame`.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(ClientScreen = ClientScreen::InGame)]
pub enum SimRunState {
    #[default]
    Running,
    Paused,
}

/// Sub-estado al elegir destino de orden en el mapa (`InGame` únicamente).
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(ClientScreen = ClientScreen::InGame)]
pub enum OrderPickState {
    #[default]
    Idle,
    Picking,
}

/// Partida suspendida al volver al menú (el `SimWorld` sigue en memoria).
#[derive(Resource, Default, Debug)]
pub struct SuspendedGameSession {
    pub active: bool,
}

/// Alterna entre ejecución y pausa de la simulación.
pub(crate) fn toggle_sim_run_state(
    current: &State<SimRunState>,
    next: &mut NextState<SimRunState>,
) {
    next.set(match current.get() {
        SimRunState::Running => SimRunState::Paused,
        SimRunState::Paused => SimRunState::Running,
    });
}

pub(crate) fn sim_is_paused(run: &State<SimRunState>) -> bool {
    *run.get() == SimRunState::Paused
}

pub(crate) fn order_pick_active(pick: &State<OrderPickState>) -> bool {
    *pick.get() == OrderPickState::Picking
}

/// Estado de simulación para tests unitarios que ejecutan sistemas de partida.
#[cfg(test)]
pub(crate) fn insert_test_sim_run_state(world: &mut World) {
    if !world.contains_resource::<State<SimRunState>>() {
        world.insert_resource(State::new(SimRunState::Running));
    }
    if !world.contains_resource::<NextState<SimRunState>>() {
        world.insert_resource(NextState::<SimRunState>::default());
    }
}

/// Sub-estado de órdenes para tests unitarios que ejecutan sistemas de partida.
#[cfg(test)]
pub(crate) fn insert_test_order_pick_state(world: &mut World) {
    if !world.contains_resource::<State<OrderPickState>>() {
        world.insert_resource(State::new(OrderPickState::Idle));
    }
    if !world.contains_resource::<NextState<OrderPickState>>() {
        world.insert_resource(NextState::<OrderPickState>::default());
    }
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

impl SimWorld {
    /// Crea el mundo según opciones del menú (o variables de entorno en CI).
    #[must_use]
    pub fn from_new_game(settings: &NewGameSettings) -> Self {
        if let Ok(path) = std::env::var("OTTDJSON_LOAD")
            && let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(mut state) = openttdrs_core::save::load_from_str(&text)
        {
            apply_test_company_colour(&mut state);
            info!("Estado de simulacion cargado desde JSON: {path}");
            log_detection_summary(&state, true, None);
            state.pending_sim_events.discard_all();
            return Self {
                state,
                loaded_file: true,
                ottdmap_extras: None,
            };
        }
        if let Ok(path) = std::env::var("OTTDMAP_FILE")
            && let Ok(data) = std::fs::read(&path)
            && let Ok((map, extras)) = Map::from_ottd_binary_with_extras(&data)
        {
            info!("Mapa cargado desde {path}");
            let mut state = GameState::from_map(map);
            state.jgr_tunnels_from_footer = extras.jgr_tunnels_from_tnbp();
            place_industries(&mut state, true, Some(&extras));
            place_stations(&mut state);
            place_stations_from_map_tiles(&mut state);
            place_stations_from_footer_stxy(&mut state, Some(&extras));
            apply_test_company_colour(&mut state);
            log_detection_summary(&state, true, Some(&extras));
            state.pending_sim_events.discard_all();
            return Self {
                state,
                loaded_file: true,
                ottdmap_extras: Some(extras),
            };
        }
        Self {
            state: bootstrap_procedural_state(settings),
            loaded_file: false,
            ottdmap_extras: None,
        }
    }
}

impl Default for SimWorld {
    fn default() -> Self {
        Self::from_new_game(&settings_from_env())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod sim_world_coverage_tests {
    use super::SimWorld;
    use crate::state::bootstrap::NewGameSettings;

    #[test]
    fn sim_world_default_runs_procedural_bootstrap() {
        let w = SimWorld::default();
        assert!(!w.loaded_file);
        assert!(w.ottdmap_extras.is_none());
        assert_eq!(
            w.state.industries.len(),
            4,
            "mina demo + fábrica demo + bosque + fábrica showcase"
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

    #[test]
    fn bootstrap_world_has_no_pending_sim_events() {
        let w = SimWorld::default();
        assert!(w.state.pending_sim_events.is_empty());
    }

    #[test]
    fn procedural_island_has_towns_and_industries_without_demo_vehicles() {
        let w = SimWorld::from_new_game(&NewGameSettings::procedural_island(
            openttdrs_core::Climate::Temperate,
            42,
        ));
        assert!(!w.state.towns.is_empty());
        assert!(!w.state.industries.is_empty());
        assert_eq!(w.state.vehicles.len(), 0);
        assert!(w.state.world_seed != 0);
    }
}

#[cfg(test)]
mod ui_command_integration;
