//! Hotkeys de guardado/carga del estado de simulación.

use std::path::Path;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{sav, save};

use crate::bevy_app::UpdateSet;
use crate::i18n::Locale;
use crate::render::{RemapMapVisualsPending, VehicleIndex};
use crate::settings::ClientPreferences;
use crate::state::{ClientScreen, SimRunState, SimWorld};
use crate::ui::{HudBuildFeedback, SaveWindowState, SimHudControls, push_hud_feedback};

pub(crate) struct PersistencePlugin;

/// Formato de una partida activa. La ruta es el contrato compartido por la
/// ventana de partidas y los atajos rápidos: `.sav` conserva el contenedor
/// OpenTTD y cualquier otra extensión conserva el JSON versionado propio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveSaveFormat {
    Json,
    Sav,
}

/// Determina el formato de la partida a partir de su ruta persistida.
#[must_use]
pub(crate) fn active_save_format(path: &Path) -> ActiveSaveFormat {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sav"))
    {
        ActiveSaveFormat::Sav
    } else {
        ActiveSaveFormat::Json
    }
}

/// Guarda `state` sin cambiar el formato de la ruta activa.
pub(crate) fn save_state_to_path(state: &GameState, path: &Path) -> Result<(), String> {
    match active_save_format(path) {
        ActiveSaveFormat::Json => save::save(state, path).map_err(|error| error.to_string()),
        ActiveSaveFormat::Sav => sav::save(state, path).map_err(|error| error.to_string()),
    }
}

/// Carga una partida según el formato elegido por su ruta activa.
///
/// No modifica el mundo en caso de error. El caller aplica el estado resultante
/// únicamente después de que ambos lectores hayan terminado correctamente.
pub(crate) fn load_state_from_path(path: &Path) -> Result<GameState, String> {
    match active_save_format(path) {
        ActiveSaveFormat::Json => save::load(path).map_err(|error| error.to_string()),
        ActiveSaveFormat::Sav => {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            crate::state::load_sav_state(&bytes)
        }
    }
}

/// Señal de una carga que debe dejar la simulación detenida antes de recuperar
/// rutas y reservas en ticks posteriores.
#[derive(Resource)]
pub(crate) struct PauseAfterLoad;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            handle_sim_save_hotkeys
                .in_set(UpdateSet::Persistence)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(Update, pause_after_load.in_set(UpdateSet::Persistence));
    }
}

/// Reemplaza el estado de simulación en caliente y dispara la recarga visual.
pub(crate) fn apply_loaded_state(
    sim: &mut SimWorld,
    vehicle_index: &mut VehicleIndex,
    remap: &mut RemapMapVisualsPending,
    commands: &mut Commands,
    loaded: GameState,
) {
    let prev = sim.state.map.dimensions();
    let nw = loaded.map.dimensions();
    sim.state = loaded;
    // Rehidratar catálogos NewGRF (vistas Action1/3) tras save/load.
    openttdrs_core::apply_newgrf_stack_catalogs_default_dirs(&mut sim.state);
    sim.ottdmap_extras = None;
    sim.loaded_file = true;
    vehicle_index.rebuild(&sim.state.vehicles);
    // Una carga sustituye el estado aunque conserve dimensiones: nunca se
    // pueden reutilizar sprites/chunks de la partida anterior.
    remap.request_full_and_sync_camera();
    // La importación de un SAV grande restituye rutas y reservas de forma
    // incremental. Detener el reloj evita que el primer frame de la UI quede
    // esperando esa recuperación y deja el control al jugador.
    commands.insert_resource(PauseAfterLoad);
    if prev != nw {
        info!("Mapa {prev:?} -> {nw:?}; recarga visual y camara.");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistenceAction {
    Save,
    Load,
}

/// Texto de resultado de los atajos rápidos. Se limita al nombre del archivo
/// activo (no su ruta completa) y aplana diagnósticos multilínea para que el
/// toast pueda mostrarlos sin invadir la pantalla.
fn persistence_feedback_text(
    locale: Locale,
    action: PersistenceAction,
    path: &Path,
    error: Option<&str>,
) -> String {
    let filename = persistence_feedback_filename(path);
    let Some(error) = error else {
        return match (locale, action) {
            (Locale::Es, PersistenceAction::Save) => format!("Partida guardada: {filename}"),
            (Locale::Es, PersistenceAction::Load) => format!("Partida cargada: {filename}"),
            (Locale::En, PersistenceAction::Save) => format!("Game saved: {filename}"),
            (Locale::En, PersistenceAction::Load) => format!("Game loaded: {filename}"),
        };
    };
    let detail = one_line_persistence_error(locale, error);
    match (locale, action) {
        (Locale::Es, PersistenceAction::Save) => {
            format!("No se pudo guardar {filename}: {detail}")
        }
        (Locale::Es, PersistenceAction::Load) => {
            format!("No se pudo cargar {filename}: {detail}")
        }
        (Locale::En, PersistenceAction::Save) => format!("Could not save {filename}: {detail}"),
        (Locale::En, PersistenceAction::Load) => format!("Could not load {filename}: {detail}"),
    }
}

fn persistence_feedback_filename(path: &Path) -> String {
    let name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let sanitized: String = name
        .chars()
        .map(|character| {
            if matches!(character, '\n' | '\r') {
                ' '
            } else {
                character
            }
        })
        .collect();
    if sanitized.trim().is_empty() {
        "(sin nombre)".to_owned()
    } else {
        sanitized
    }
}

fn one_line_persistence_error(locale: Locale, error: &str) -> String {
    let detail = error.split_whitespace().collect::<Vec<_>>().join(" ");
    if detail.is_empty() {
        match locale {
            Locale::Es => "error sin detalle".to_owned(),
            Locale::En => "unspecified error".to_owned(),
        }
    } else {
        detail
    }
}

fn pause_after_load(
    pause_requested: Option<Res<PauseAfterLoad>>,
    mut commands: Commands,
    mut next_run: Option<ResMut<NextState<SimRunState>>>,
) {
    if pause_requested.is_some()
        && let Some(next_run) = next_run.as_deref_mut()
    {
        next_run.set(SimRunState::Paused);
        commands.remove_resource::<PauseAfterLoad>();
    }
}

#[allow(clippy::too_many_arguments)] // recursos ECS de guardado, UI y feedback.
pub(crate) fn handle_sim_save_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    mut commands: Commands,
    hud: Res<SimHudControls>,
    prefs: Res<ClientPreferences>,
    mut feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
    save_window: Option<Res<SaveWindowState>>,
) {
    // Con la ventana de partidas abierta el teclado edita el nombre del archivo.
    if save_window.is_some_and(|w| w.open) {
        return;
    }
    let save_path = hud.json_save_path.clone();
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let save_shortcut =
        keyboard.just_pressed(KeyCode::F5) || (ctrl && keyboard.just_pressed(KeyCode::KeyS));
    let load_shortcut =
        keyboard.just_pressed(KeyCode::F9) || (ctrl && keyboard.just_pressed(KeyCode::KeyL));

    if save_shortcut {
        let path = Path::new(&save_path);
        match save_state_to_path(&sim.state, path) {
            Ok(()) => {
                push_hud_feedback(
                    &mut feedback,
                    persistence_feedback_text(prefs.locale(), PersistenceAction::Save, path, None),
                    time.elapsed_secs(),
                    false,
                );
                info!("Guardado en {save_path}");
            }
            Err(error) => {
                push_hud_feedback(
                    &mut feedback,
                    persistence_feedback_text(
                        prefs.locale(),
                        PersistenceAction::Save,
                        path,
                        Some(&error),
                    ),
                    time.elapsed_secs(),
                    true,
                );
                error!("No se pudo guardar en {save_path}: {error}");
            }
        }
    }
    if load_shortcut {
        let path = Path::new(&save_path);
        match load_state_from_path(path) {
            Ok(loaded) => {
                apply_loaded_state(
                    &mut sim,
                    &mut vehicle_index,
                    &mut remap,
                    &mut commands,
                    loaded,
                );
                push_hud_feedback(
                    &mut feedback,
                    persistence_feedback_text(prefs.locale(), PersistenceAction::Load, path, None),
                    time.elapsed_secs(),
                    false,
                );
                info!("Estado cargado desde {save_path}; recarga visual.");
            }
            Err(error) => {
                push_hud_feedback(
                    &mut feedback,
                    persistence_feedback_text(
                        prefs.locale(),
                        PersistenceAction::Load,
                        path,
                        Some(&error),
                    ),
                    time.elapsed_secs(),
                    true,
                );
                error!("Carga: no se pudo cargar {save_path}: {error}");
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::path::Path;

    use super::{
        ActiveSaveFormat, PauseAfterLoad, PersistenceAction, active_save_format,
        handle_sim_save_hotkeys, persistence_feedback_text,
    };
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::i18n::Locale;
    use crate::render::{RemapMapVisualsPending, VehicleIndex};
    use crate::settings::ClientPreferences;
    use crate::state::SimWorld;
    use crate::ui::{HudBuildFeedback, SaveWindowState, SimHudControls};

    fn shortcut_world(save_path: &Path) -> World {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            json_save_path: save_path.to_string_lossy().to_string(),
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(HudBuildFeedback::default());
        world.insert_resource(Time::<()>::default());
        world
    }

    #[test]
    fn persistence_feedback_is_localized_and_keeps_filename_and_error() {
        let path = Path::new("/tmp/mi_partida.json");
        assert_eq!(
            persistence_feedback_text(Locale::Es, PersistenceAction::Save, path, None),
            "Partida guardada: mi_partida.json"
        );
        assert_eq!(
            persistence_feedback_text(Locale::En, PersistenceAction::Load, path, None),
            "Game loaded: mi_partida.json"
        );
        assert_eq!(
            persistence_feedback_text(
                Locale::Es,
                PersistenceAction::Load,
                path,
                Some("archivo\nilegible"),
            ),
            "No se pudo cargar mi_partida.json: archivo ilegible"
        );
        assert_eq!(
            persistence_feedback_text(Locale::En, PersistenceAction::Save, path, Some("disk full"),),
            "Could not save mi_partida.json: disk full"
        );
        assert_eq!(
            persistence_feedback_text(Locale::Es, PersistenceAction::Save, path, Some("")),
            "No se pudo guardar mi_partida.json: error sin detalle"
        );
    }

    #[test]
    fn save_and_load_json_shortcuts_work() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("sim.json");
        let mut world = shortcut_world(&save_path);

        let mut save_keys = ButtonInput::<KeyCode>::default();
        save_keys.press(KeyCode::F5);
        world.insert_resource(save_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert_eq!(active_save_format(&save_path), ActiveSaveFormat::Json);
        assert!(
            std::fs::read_to_string(&save_path)
                .expect("JSON save")
                .contains("version")
        );
        assert_eq!(
            world.resource::<HudBuildFeedback>().message.as_deref(),
            Some("Partida guardada: sim.json")
        );
        assert!(world.resource::<HudBuildFeedback>().expires_at_secs > 0.0);

        let mut load_keys = ButtonInput::<KeyCode>::default();
        load_keys.press(KeyCode::F9);
        world.insert_resource(load_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        let remap = world.resource::<RemapMapVisualsPending>();
        assert!(remap.is_pending());
        assert!(remap.is_full());
        assert!(remap.sync_camera_requested());
        assert!(world.contains_resource::<PauseAfterLoad>());
        assert_eq!(
            world.resource::<HudBuildFeedback>().message.as_deref(),
            Some("Partida cargada: sim.json")
        );
        assert!(!world.resource::<HudBuildFeedback>().pending_soft_ping);
    }

    #[test]
    fn ctrl_shortcuts_preserve_json_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("roundtrip.json");
        let mut world = shortcut_world(&save_path);
        world.resource_mut::<ClientPreferences>().language = "en".into();

        let mut ctrl_save = ButtonInput::<KeyCode>::default();
        ctrl_save.press(KeyCode::ControlLeft);
        ctrl_save.press(KeyCode::KeyS);
        world.insert_resource(ctrl_save);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();
        assert_eq!(
            world.resource::<HudBuildFeedback>().message.as_deref(),
            Some("Game saved: roundtrip.json")
        );

        let mut ctrl_load = ButtonInput::<KeyCode>::default();
        ctrl_load.press(KeyCode::ControlLeft);
        ctrl_load.press(KeyCode::KeyL);
        world.insert_resource(ctrl_load);
        let saved = std::fs::read(&save_path).expect("save remains readable");
        let money_before_load = world.resource::<SimWorld>().state.economy.money;
        world.resource_mut::<SimWorld>().state.economy.money = 1;
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert_eq!(
            world.resource::<SimWorld>().state.economy.money,
            money_before_load
        );
        assert_eq!(
            std::fs::read(&save_path).expect("source remains intact"),
            saved
        );
        assert!(world.contains_resource::<PauseAfterLoad>());
        assert_eq!(
            world.resource::<HudBuildFeedback>().message.as_deref(),
            Some("Game loaded: roundtrip.json")
        );
    }

    #[test]
    fn invalid_sav_load_keeps_state_and_source_untouched() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("broken.sav");
        let source = b"not an OpenTTD save";
        std::fs::write(&save_path, source).expect("write invalid SAV");

        let mut world = shortcut_world(&save_path);
        world.resource_mut::<SimWorld>().state.economy.money = 9_999;

        let mut load_keys = ButtonInput::<KeyCode>::default();
        load_keys.press(KeyCode::F9);
        world.insert_resource(load_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert_eq!(world.resource::<SimWorld>().state.economy.money, 9_999);
        assert_eq!(std::fs::read(&save_path).expect("source remains"), source);
        assert!(!world.contains_resource::<PauseAfterLoad>());
        let feedback = world.resource::<HudBuildFeedback>();
        assert!(
            feedback
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with("No se pudo cargar broken.sav: "))
        );
        assert!(feedback.pending_soft_ping);
    }

    #[test]
    fn save_and_load_sav_shortcuts_preserve_container_and_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("sim.SAV");
        let mut world = shortcut_world(&save_path);
        world.resource_mut::<SimWorld>().state.economy.money = 246_810;

        let mut save_keys = ButtonInput::<KeyCode>::default();
        save_keys.press(KeyCode::F5);
        world.insert_resource(save_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        let bytes = std::fs::read(&save_path).expect("SAV saved");
        assert_eq!(active_save_format(&save_path), ActiveSaveFormat::Sav);
        assert!(
            bytes.starts_with(b"OTT"),
            "the active SAV must not become JSON"
        );
        crate::state::load_sav_state(&bytes).expect("saved container loads as SAV");

        world.resource_mut::<SimWorld>().state.economy.money = 1;
        let mut load_keys = ButtonInput::<KeyCode>::default();
        load_keys.press(KeyCode::F9);
        world.insert_resource(load_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert_eq!(world.resource::<SimWorld>().state.economy.money, 246_810);
        assert!(world.resource::<RemapMapVisualsPending>().is_pending());
        assert!(world.contains_resource::<PauseAfterLoad>());
    }

    #[test]
    fn failed_save_reports_the_filename_and_preserves_the_active_game() {
        let dir = tempfile::tempdir().expect("tempdir");
        let blocker = dir.path().join("not_a_directory");
        std::fs::write(&blocker, "not a directory").expect("write blocker");
        let save_path = blocker.join("slot.json");
        let mut world = shortcut_world(&save_path);
        world.resource_mut::<SimWorld>().state.economy.money = 321;

        let mut save_keys = ButtonInput::<KeyCode>::default();
        save_keys.press(KeyCode::F5);
        world.insert_resource(save_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert!(!save_path.exists());
        assert_eq!(world.resource::<SimWorld>().state.economy.money, 321);
        let feedback = world.resource::<HudBuildFeedback>();
        assert!(
            feedback
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with("No se pudo guardar slot.json: "))
        );
        assert!(feedback.pending_soft_ping);
    }

    #[test]
    fn open_save_window_captures_quick_save_and_load_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("captured.json");
        let mut world = shortcut_world(&save_path);
        world.resource_mut::<SimWorld>().state.economy.money = 654;
        world.insert_resource(SaveWindowState {
            open: true,
            ..Default::default()
        });

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::F5);
        keys.press(KeyCode::F9);
        world.insert_resource(keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert!(!save_path.exists());
        assert_eq!(world.resource::<SimWorld>().state.economy.money, 654);
        assert!(world.resource::<HudBuildFeedback>().message.is_none());
        assert!(!world.contains_resource::<PauseAfterLoad>());
    }
}
