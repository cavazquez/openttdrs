use bevy::prelude::*;

use super::SimHudControls;
use crate::render::RemapMapVisualsPending;
use crate::settings::ClientPreferences;
use crate::state::{SimRunState, sim_is_paused, toggle_sim_run_state};
use crate::ui::save_window::SaveWindowState;
use crate::ui::{BuildMenuAction, UiToolState};

/// Con la ventana de partidas abierta, el teclado edita el nombre del archivo.
fn save_window_open(save_window: Option<&Res<SaveWindowState>>) -> bool {
    save_window.is_some_and(|w| w.open)
}

fn dev_console_open(console: Option<&Res<crate::ui::dev_console::DevConsoleState>>) -> bool {
    console.is_some_and(|c| crate::ui::dev_console::dev_console_captures_keyboard(c))
}

/// **P** alterna pausa del tick de simulacion (`GameState::step`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_pause_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
    mut prefs: ResMut<ClientPreferences>,
    mut pending_remap: ResMut<RemapMapVisualsPending>,
    save_window: Option<Res<SaveWindowState>>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
    run_state: Res<State<SimRunState>>,
    mut next_run: ResMut<NextState<SimRunState>>,
) {
    if save_window_open(save_window.as_ref()) || dev_console_open(console.as_ref()) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        let will_pause = !sim_is_paused(&run_state);
        toggle_sim_run_state(&run_state, &mut next_run);
        info!("Pausa: {}", if will_pause { "ON" } else { "OFF" });
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        hud.minimap_visible = !hud.minimap_visible;
        if hud.minimap_visible {
            info!("Minimapa: visible");
        } else {
            info!("Minimapa: oculto");
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        prefs.show_pbs_reservations = !prefs.show_pbs_reservations;
        pending_remap.pending = true;
        pending_remap.full = true;
        info!(
            "Reservas PBS: {}",
            if prefs.show_pbs_reservations {
                "visibles"
            } else {
                "ocultas"
            }
        );
    }
}

/// **F4** alterna entre dos rutas de archivo predefinidas para F5/F9.
pub(crate) fn cycle_json_save_path_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
    save_window: Option<Res<SaveWindowState>>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
) {
    if save_window_open(save_window.as_ref()) || dev_console_open(console.as_ref()) {
        return;
    }
    if keyboard.just_pressed(KeyCode::F4) {
        hud.json_save_path = if hud.json_save_path.ends_with("autosave.json") {
            "save/openttdrs_sim.json".into()
        } else {
            "save/openttdrs_autosave.json".into()
        };
        info!("Ruta JSON (F5/F9): {}", hud.json_save_path);
    }
}

/// Hotkeys de herramienta: 1/2 carreteras, 3 estacion, C limpiar, Esc desactivar.
pub(crate) fn handle_tool_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tool_state: ResMut<UiToolState>,
    save_window: Option<Res<SaveWindowState>>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
) {
    if save_window_open(save_window.as_ref()) || dev_console_open(console.as_ref()) {
        return;
    }
    if keyboard.just_pressed(KeyCode::Digit1) {
        tool_state.active_tool = Some(BuildMenuAction::RoadY);
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        tool_state.active_tool = Some(BuildMenuAction::RoadX);
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        tool_state.active_tool = Some(BuildMenuAction::Station);
    } else if keyboard.just_pressed(KeyCode::Digit4) {
        tool_state.active_tool = Some(BuildMenuAction::Rail);
    } else if keyboard.just_pressed(KeyCode::KeyC) {
        tool_state.active_tool = Some(BuildMenuAction::Clear);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;

    fn press_keys(world: &mut World, keys: &[KeyCode]) {
        let mut kb = ButtonInput::<KeyCode>::default();
        for k in keys {
            kb.press(*k);
        }
        world.insert_resource(kb);
    }

    #[test]
    fn hud_hotkey_systems_cover_branches() {
        let mut world = World::new();
        world.insert_resource(SimHudControls::default());
        world.insert_resource(crate::settings::ClientPreferences::default());
        world.insert_resource(crate::render::RemapMapVisualsPending::default());
        world.insert_resource(UiToolState::default());

        crate::state::insert_test_sim_run_state(&mut world);
        press_keys(&mut world, &[KeyCode::KeyP]);
        world.run_system_once(handle_pause_toggle).unwrap();
        press_keys(&mut world, &[KeyCode::KeyM]);
        world.run_system_once(handle_pause_toggle).unwrap();
        press_keys(&mut world, &[KeyCode::F4]);
        world.run_system_once(cycle_json_save_path_hotkey).unwrap();
        press_keys(&mut world, &[KeyCode::F4]);
        world.run_system_once(cycle_json_save_path_hotkey).unwrap();

        for k in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::KeyC,
        ] {
            press_keys(&mut world, &[k]);
            world.run_system_once(handle_tool_hotkeys).unwrap();
        }
    }
}
