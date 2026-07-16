//! Estado del mundo de simulacion y generacion procedural.

pub(crate) mod bootstrap;
pub(crate) mod editor_session;
pub(crate) mod ingame_lifecycle;
pub(crate) mod new_game;
pub(crate) mod stations;

pub(crate) use editor_session::{
    EditorSession, apply_editor_sandbox, editor_new_game_settings, regenerate_landscape_in_place,
    scenarios_save_dir,
};

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
    state.runtime.pending_sim_events.discard_all();
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
    if settings.gamescript_demo {
        openttdrs_core::seed_gs_demo(&mut state);
    }
    if settings.world_gen {
        info!(
            "Mapa procedural: clima={:?}, seed={}, isla={}, demo={}",
            settings.climate, state.world_seed, settings.island, settings.preserve_demo
        );
    }
    state.runtime.pending_sim_events.discard_all();
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
    /// Si la sesión suspendida era el editor de escenarios (#42).
    pub editor: bool,
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

/// Fuente de una carga explícita pedida por variable de entorno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapLoadSource {
    Json,
    OttdMap,
}

impl BootstrapLoadSource {
    fn env_var(self) -> &'static str {
        match self {
            Self::Json => "OTTDJSON_LOAD",
            Self::OttdMap => "OTTDMAP_FILE",
        }
    }
}

/// Error tipado al cargar un mundo solicitado vía `OTTDJSON_LOAD` / `OTTDMAP_FILE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapLoadError {
    pub source: BootstrapLoadSource,
    pub path: String,
    pub cause: String,
}

impl std::fmt::Display for BootstrapLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "no se pudo cargar {} ({}={}): {}",
            match self.source {
                BootstrapLoadSource::Json => "JSON de simulación",
                BootstrapLoadSource::OttdMap => "mapa .ottdmap",
            },
            self.source.env_var(),
            self.path,
            self.cause
        )
    }
}

impl std::error::Error for BootstrapLoadError {}

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
    /// Nueva partida / intro / editor: solo procedural (no lee paths de carga).
    #[must_use]
    pub fn from_new_game(settings: &NewGameSettings) -> Self {
        Self::new_procedural(settings)
    }

    /// Mundo procedural sin I/O de save.
    #[must_use]
    pub fn new_procedural(settings: &NewGameSettings) -> Self {
        Self {
            state: bootstrap_procedural_state(settings),
            loaded_file: false,
            ottdmap_extras: None,
        }
    }

    /// Carga opcional por paths explícitos (prioridad JSON > ottdmap). `Ok(None)` si ambos `None`.
    ///
    /// Un path inválido o parseo fallido es `Err` (nunca cae a procedural en silencio).
    pub fn load_requested(
        json_path: Option<String>,
        ottdmap_path: Option<String>,
    ) -> Result<Option<Self>, BootstrapLoadError> {
        if let Some(path) = json_path {
            return Ok(Some(Self::load_json_file(&path)?));
        }
        if let Some(path) = ottdmap_path {
            return Ok(Some(Self::load_ottdmap_file(&path)?));
        }
        Ok(None)
    }

    /// Carga JSON desde un path (misma lógica que `OTTDJSON_LOAD`).
    pub fn load_json_file(path: &str) -> Result<Self, BootstrapLoadError> {
        Self::load_json_path(path)
    }

    /// Carga `.ottdmap` desde un path (misma lógica que `OTTDMAP_FILE`).
    pub fn load_ottdmap_file(path: &str) -> Result<Self, BootstrapLoadError> {
        Self::load_ottdmap_path(path)
    }

    /// Arranque: carga pedida por env (`OTTDJSON_LOAD` / `OTTDMAP_FILE`), o procedural.
    pub fn try_bootstrap(settings: &NewGameSettings) -> Result<Self, BootstrapLoadError> {
        Self::try_bootstrap_with_loads(
            settings,
            std::env::var("OTTDJSON_LOAD").ok(),
            std::env::var("OTTDMAP_FILE").ok(),
        )
    }

    /// Como [`try_bootstrap`] con paths de carga inyectables (tests / headless).
    pub fn try_bootstrap_with_loads(
        settings: &NewGameSettings,
        json_path: Option<String>,
        ottdmap_path: Option<String>,
    ) -> Result<Self, BootstrapLoadError> {
        match Self::load_requested(json_path, ottdmap_path)? {
            Some(world) => Ok(world),
            None => Ok(Self::new_procedural(settings)),
        }
    }

    /// Como [`try_bootstrap`] con settings derivados del entorno (clima/seed/…).
    pub fn try_bootstrap_from_env() -> Result<Self, BootstrapLoadError> {
        Self::try_bootstrap(&settings_from_env())
    }

    fn load_json_path(path: &str) -> Result<Self, BootstrapLoadError> {
        let text = std::fs::read_to_string(path).map_err(|e| BootstrapLoadError {
            source: BootstrapLoadSource::Json,
            path: path.to_owned(),
            cause: e.to_string(),
        })?;
        let mut state =
            openttdrs_core::save::load_from_str(&text).map_err(|e| BootstrapLoadError {
                source: BootstrapLoadSource::Json,
                path: path.to_owned(),
                cause: e.to_string(),
            })?;
        apply_test_company_colour(&mut state);
        info!("Estado de simulacion cargado desde JSON: {path}");
        log_detection_summary(&state, true, None);
        state.runtime.pending_sim_events.discard_all();
        Ok(Self {
            state,
            loaded_file: true,
            ottdmap_extras: None,
        })
    }

    fn load_ottdmap_path(path: &str) -> Result<Self, BootstrapLoadError> {
        let data = std::fs::read(path).map_err(|e| BootstrapLoadError {
            source: BootstrapLoadSource::OttdMap,
            path: path.to_owned(),
            cause: e.to_string(),
        })?;
        let (map, extras) =
            Map::from_ottd_binary_with_extras(&data).map_err(|e| BootstrapLoadError {
                source: BootstrapLoadSource::OttdMap,
                path: path.to_owned(),
                cause: format!("{e:?}"),
            })?;
        info!("Mapa cargado desde {path}");
        let mut state = GameState::from_map(map);
        state.jgr_tunnels_from_footer = extras.jgr_tunnels_from_tnbp();
        place_industries(&mut state, true, Some(&extras));
        place_stations(&mut state);
        place_stations_from_map_tiles(&mut state);
        place_stations_from_footer_stxy(&mut state, Some(&extras));
        apply_test_company_colour(&mut state);
        log_detection_summary(&state, true, Some(&extras));
        state.runtime.pending_sim_events.discard_all();
        Ok(Self {
            state,
            loaded_file: true,
            ottdmap_extras: Some(extras),
        })
    }
}

impl Default for SimWorld {
    /// Default seguro para registro Bevy / tests: procedural, sin I/O de carga.
    /// La carga por env va en [`SimWorld::try_bootstrap_from_env`] al arrancar.
    fn default() -> Self {
        Self::new_procedural(&settings_from_env())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod sim_world_coverage_tests {
    use super::{BootstrapLoadSource, SimWorld};
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
        assert!(w.state.runtime.pending_sim_events.is_empty());
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

    #[test]
    fn try_bootstrap_without_loads_is_procedural() {
        assert!(SimWorld::load_requested(None, None).unwrap().is_none());
        let w =
            SimWorld::try_bootstrap_with_loads(&NewGameSettings::default(), None, None).unwrap();
        assert!(!w.loaded_file);
    }

    #[test]
    fn missing_json_path_errors_with_path_and_cause() {
        let path = "/nonexistent/openttdrs_missing_bootstrap.json";
        let err = match SimWorld::try_bootstrap_with_loads(
            &NewGameSettings::default(),
            Some(path.to_owned()),
            None,
        ) {
            Ok(_) => panic!("debe fallar con path inexistente"),
            Err(e) => e,
        };
        assert_eq!(err.source, BootstrapLoadSource::Json);
        assert_eq!(err.path, path);
        let msg = err.to_string();
        assert!(msg.contains(path), "{msg}");
        assert!(msg.contains("OTTDJSON_LOAD"), "{msg}");
    }

    #[test]
    fn invalid_json_errors_without_procedural_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{not valid json").unwrap();
        let path_str = path.to_str().unwrap();
        let err = match SimWorld::try_bootstrap_with_loads(
            &NewGameSettings::default(),
            Some(path_str.to_owned()),
            None,
        ) {
            Ok(_) => panic!("JSON inválido no debe caer a procedural"),
            Err(e) => e,
        };
        assert_eq!(err.source, BootstrapLoadSource::Json);
        assert!(err.to_string().contains(path_str), "{err}");
        assert!(!err.cause.is_empty());
    }

    #[test]
    fn invalid_ottdmap_errors_without_procedural_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.ottdmap");
        std::fs::write(&path, b"not-an-ottdmap").unwrap();
        let path_str = path.to_str().unwrap();
        let err = match SimWorld::try_bootstrap_with_loads(
            &NewGameSettings::default(),
            None,
            Some(path_str.to_owned()),
        ) {
            Ok(_) => panic!("mapa inválido no debe caer a procedural"),
            Err(e) => e,
        };
        assert_eq!(err.source, BootstrapLoadSource::OttdMap);
        let msg = err.to_string();
        assert!(msg.contains(path_str), "{msg}");
        assert!(msg.contains("OTTDMAP_FILE"), "{msg}");
    }
}

#[cfg(test)]
mod ui_command_integration;
