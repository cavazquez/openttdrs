//! Hotkeys de guardado/carga del estado de simulación.

use std::path::Path;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{sav, save};

use crate::bevy_app::UpdateSet;
use crate::render::{RemapMapVisualsPending, VehicleIndex};
use crate::state::{ClientScreen, SimRunState, SimWorld};
use crate::ui::{SaveWindowState, SimHudControls};

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
    remap.pending = true;
    remap.sync_camera = true;
    // La importación de un SAV grande restituye rutas y reservas de forma
    // incremental. Detener el reloj evita que el primer frame de la UI quede
    // esperando esa recuperación y deja el control al jugador.
    commands.insert_resource(PauseAfterLoad);
    if prev != nw {
        info!("Mapa {prev:?} -> {nw:?}; recarga visual y camara.");
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

pub(crate) fn handle_sim_save_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    mut commands: Commands,
    hud: Res<SimHudControls>,
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
        match save_state_to_path(&sim.state, Path::new(&save_path)) {
            Ok(()) => info!("Guardado en {save_path}"),
            Err(e) => error!("No se pudo guardar en {save_path}: {e}"),
        }
    }
    if load_shortcut {
        match load_state_from_path(Path::new(&save_path)) {
            Ok(loaded) => {
                apply_loaded_state(
                    &mut sim,
                    &mut vehicle_index,
                    &mut remap,
                    &mut commands,
                    loaded,
                );
                info!("Estado cargado desde {save_path}; recarga visual.");
            }
            Err(e) => error!("Carga: no se pudo cargar {save_path}: {e}"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{ActiveSaveFormat, PauseAfterLoad, active_save_format, handle_sim_save_hotkeys};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::render::{RemapMapVisualsPending, VehicleIndex};
    use crate::state::SimWorld;
    use crate::ui::SimHudControls;

    #[test]
    fn save_and_load_json_shortcuts_work() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("sim.json");
        let save_path_s = save_path.to_string_lossy().to_string();

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            json_save_path: save_path_s,
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });

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

        let mut load_keys = ButtonInput::<KeyCode>::default();
        load_keys.press(KeyCode::F9);
        world.insert_resource(load_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        let remap = world.resource::<RemapMapVisualsPending>();
        assert!(remap.pending);
        assert!(remap.sync_camera);
        assert!(world.contains_resource::<PauseAfterLoad>());
    }

    #[test]
    fn ctrl_shortcuts_preserve_json_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("roundtrip.json");
        let save_path_s = save_path.to_string_lossy().to_string();

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            json_save_path: save_path_s,
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });

        let mut ctrl_save = ButtonInput::<KeyCode>::default();
        ctrl_save.press(KeyCode::ControlLeft);
        ctrl_save.press(KeyCode::KeyS);
        world.insert_resource(ctrl_save);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

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
    }

    #[test]
    fn invalid_sav_load_keeps_state_and_source_untouched() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("broken.sav");
        let source = b"not an OpenTTD save";
        std::fs::write(&save_path, source).expect("write invalid SAV");

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.resource_mut::<SimWorld>().state.economy.money = 9_999;
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            json_save_path: save_path.to_string_lossy().to_string(),
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });

        let mut load_keys = ButtonInput::<KeyCode>::default();
        load_keys.press(KeyCode::F9);
        world.insert_resource(load_keys);
        world.run_system_once(handle_sim_save_hotkeys).unwrap();

        assert_eq!(world.resource::<SimWorld>().state.economy.money, 9_999);
        assert_eq!(std::fs::read(&save_path).expect("source remains"), source);
        assert!(!world.contains_resource::<PauseAfterLoad>());
    }

    #[test]
    fn save_and_load_sav_shortcuts_preserve_container_and_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("sim.SAV");
        let save_path_s = save_path.to_string_lossy().to_string();

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.resource_mut::<SimWorld>().state.economy.money = 246_810;
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            json_save_path: save_path_s,
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });

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
        assert!(world.resource::<RemapMapVisualsPending>().pending);
        assert!(world.contains_resource::<PauseAfterLoad>());
    }
}
