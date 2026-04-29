use bevy::prelude::*;

use super::SimHudControls;
use crate::ui::{BuildMenuAction, UiToolState};

/// **P** alterna pausa del tick de simulacion (`GameState::step`).
pub(crate) fn handle_pause_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        hud.paused = !hud.paused;
        if hud.paused {
            info!("Pausa: ON");
        } else {
            info!("Pausa: OFF");
        }
    }
}

/// **F4** alterna entre dos rutas de archivo predefinidas para F5/F9.
pub(crate) fn cycle_json_save_path_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
) {
    if keyboard.just_pressed(KeyCode::F4) {
        hud.json_save_path = if hud.json_save_path.ends_with("autosave.json") {
            "openttdrs_sim.json".into()
        } else {
            "openttdrs_autosave.json".into()
        };
        info!("Ruta JSON (F5/F9): {}", hud.json_save_path);
    }
}

/// Hotkeys de herramienta: 1 carretera, 2 estacion, C limpiar, Esc desactivar.
pub(crate) fn handle_tool_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tool_state: ResMut<UiToolState>,
) {
    if keyboard.just_pressed(KeyCode::Digit1) {
        tool_state.active_tool = Some(BuildMenuAction::Road);
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        tool_state.active_tool = Some(BuildMenuAction::Station);
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        tool_state.active_tool = Some(BuildMenuAction::Rail);
    } else if keyboard.just_pressed(KeyCode::KeyC) {
        tool_state.active_tool = Some(BuildMenuAction::Clear);
    } else if keyboard.just_pressed(KeyCode::Escape) {
        tool_state.active_tool = None;
    }
}
