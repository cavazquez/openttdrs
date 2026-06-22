//! Hotkeys de guardado/carga JSON del estado de simulación.

use bevy::prelude::*;
use openttdrs_core::{GameState, save};

use crate::bevy_app::UpdateSet;
use crate::render::{RemapMapVisualsPending, VehicleIndex};
use crate::state::{ClientScreen, SimWorld};
use crate::ui::{SaveWindowState, SimHudControls};

pub(crate) struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            handle_sim_json_hotkeys
                .in_set(UpdateSet::Persistence)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Reemplaza el estado de simulación en caliente y dispara la recarga visual.
pub(crate) fn apply_loaded_state(
    sim: &mut SimWorld,
    vehicle_index: &mut VehicleIndex,
    remap: &mut RemapMapVisualsPending,
    loaded: GameState,
) {
    let prev = sim.state.map.dimensions();
    let nw = loaded.map.dimensions();
    sim.state = loaded;
    sim.ottdmap_extras = None;
    sim.loaded_file = true;
    vehicle_index.rebuild(&sim.state.vehicles);
    remap.pending = true;
    remap.sync_camera = true;
    if prev != nw {
        info!("Mapa {prev:?} -> {nw:?}; recarga visual y camara.");
    }
}

pub(crate) fn handle_sim_json_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
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
        match save::save(&sim.state, std::path::Path::new(&save_path)) {
            Ok(()) => info!("Guardado en {save_path}"),
            Err(e) => error!("No se pudo guardar en {save_path}: {e}"),
        }
    }
    if load_shortcut {
        match std::fs::read_to_string(&save_path) {
            Ok(text) => match save::load_from_str(&text) {
                Ok(loaded) => {
                    apply_loaded_state(&mut sim, &mut vehicle_index, &mut remap, loaded);
                    info!("Estado cargado desde {save_path}; recarga visual.");
                }
                Err(e) => error!("Carga: JSON invalido ({save_path}): {e}"),
            },
            Err(e) => error!("Carga: no se pudo leer {save_path}: {e}"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::handle_sim_json_hotkeys;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::render::{RemapMapVisualsPending, VehicleIndex};
    use crate::state::SimWorld;
    use crate::ui::SimHudControls;

    #[test]
    fn save_and_load_shortcuts_work() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("sim.json");
        let save_path_s = save_path.to_string_lossy().to_string();

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            paused: false,
            sim_speed: 1.0,
            json_save_path: save_path_s,
            minimap_visible: true,
            sfx_volume: 0.22,
        });

        let mut save_keys = ButtonInput::<KeyCode>::default();
        save_keys.press(KeyCode::F5);
        world.insert_resource(save_keys);
        world.run_system_once(handle_sim_json_hotkeys).unwrap();

        let mut load_keys = ButtonInput::<KeyCode>::default();
        load_keys.press(KeyCode::F9);
        world.insert_resource(load_keys);
        world.run_system_once(handle_sim_json_hotkeys).unwrap();

        let remap = world.resource::<RemapMapVisualsPending>();
        assert!(remap.pending);
        assert!(remap.sync_camera);
    }

    #[test]
    fn ctrl_shortcuts_and_invalid_json_paths_are_handled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("bad.json");
        std::fs::write(&save_path, "{not-json").expect("write");
        let save_path_s = save_path.to_string_lossy().to_string();

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            paused: false,
            sim_speed: 1.0,
            json_save_path: save_path_s,
            minimap_visible: true,
            sfx_volume: 0.22,
        });

        let mut ctrl_save = ButtonInput::<KeyCode>::default();
        ctrl_save.press(KeyCode::ControlLeft);
        ctrl_save.press(KeyCode::KeyS);
        world.insert_resource(ctrl_save);
        world.run_system_once(handle_sim_json_hotkeys).unwrap();

        let mut ctrl_load = ButtonInput::<KeyCode>::default();
        ctrl_load.press(KeyCode::ControlLeft);
        ctrl_load.press(KeyCode::KeyL);
        world.insert_resource(ctrl_load);
        world.run_system_once(handle_sim_json_hotkeys).unwrap();
    }
}
