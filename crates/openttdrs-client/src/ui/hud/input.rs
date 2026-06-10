use bevy::prelude::*;

use super::SimHudControls;
use crate::ui::save_window::SaveWindowState;
use crate::ui::{BuildMenuAction, UiToolState};

/// Con la ventana de partidas abierta, el teclado edita el nombre del archivo.
fn save_window_open(save_window: Option<&Res<SaveWindowState>>) -> bool {
    save_window.is_some_and(|w| w.open)
}

/// **P** alterna pausa del tick de simulacion (`GameState::step`).
pub(crate) fn handle_pause_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
    save_window: Option<Res<SaveWindowState>>,
) {
    if save_window_open(save_window.as_ref()) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        hud.paused = !hud.paused;
        if hud.paused {
            info!("Pausa: ON");
        } else {
            info!("Pausa: OFF");
        }
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        hud.minimap_visible = !hud.minimap_visible;
        if hud.minimap_visible {
            info!("Minimapa: visible");
        } else {
            info!("Minimapa: oculto");
        }
    }
}

/// **F4** alterna entre dos rutas de archivo predefinidas para F5/F9.
pub(crate) fn cycle_json_save_path_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
    save_window: Option<Res<SaveWindowState>>,
) {
    if save_window_open(save_window.as_ref()) {
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
) {
    if save_window_open(save_window.as_ref()) {
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
    } else if keyboard.just_pressed(KeyCode::Escape) {
        tool_state.active_tool = None;
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
        world.insert_resource(UiToolState::default());

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
            KeyCode::Escape,
        ] {
            press_keys(&mut world, &[k]);
            world.run_system_once(handle_tool_hotkeys).unwrap();
        }
    }
}
