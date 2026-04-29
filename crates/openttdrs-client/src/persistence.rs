//! Hotkeys de guardado/carga JSON del estado de simulación.

use bevy::prelude::*;
use openttdrs_core::save;

use crate::bevy_app::UpdateSet;
use crate::state::SimWorld;
use crate::ui::SimHudControls;
use crate::vehicle_render::VehicleIndex;
use crate::world_render::RemapMapVisualsPending;

pub(crate) struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            handle_sim_json_hotkeys.in_set(UpdateSet::Persistence),
        );
    }
}

pub(crate) fn handle_sim_json_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    hud: Res<SimHudControls>,
) {
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
                    } else {
                        info!("Estado cargado desde {save_path}; recarga visual.");
                    }
                }
                Err(e) => error!("Carga: JSON invalido ({save_path}): {e}"),
            },
            Err(e) => error!("Carga: no se pudo leer {save_path}: {e}"),
        }
    }
}
