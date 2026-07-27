use bevy::prelude::*;

use super::SimHudControls;
use crate::render::RemapMapVisualsPending;
use crate::settings::ClientPreferences;
use crate::state::{SimRunState, sim_is_paused, toggle_sim_run_state};
use crate::ui::hotkeys::{UiCommandId, UiHotkeys};
use crate::ui::{BuildMenuAction, UiToolState};

/// **P** alterna pausa del tick de simulacion (`GameState::step`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_pause_toggle(
    hotkeys: Res<UiHotkeys>,
    mut hud: ResMut<SimHudControls>,
    mut prefs: ResMut<ClientPreferences>,
    mut pending_remap: ResMut<RemapMapVisualsPending>,
    run_state: Res<State<SimRunState>>,
    mut next_run: ResMut<NextState<SimRunState>>,
) {
    if hotkeys.fired(UiCommandId::Pause) {
        let will_pause = !sim_is_paused(&run_state);
        toggle_sim_run_state(&run_state, &mut next_run);
        info!("Pausa: {}", if will_pause { "ON" } else { "OFF" });
    }
    if hotkeys.fired(UiCommandId::SmallMap) {
        hud.minimap_visible = !hud.minimap_visible;
        if hud.minimap_visible {
            info!("Minimapa: visible");
        } else {
            info!("Minimapa: oculto");
        }
    }
    if hotkeys.fired(UiCommandId::ToggleReservations) {
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
    hotkeys: Res<UiHotkeys>,
    mut hud: ResMut<SimHudControls>,
) {
    if hotkeys.fired(UiCommandId::CycleSavePath) {
        hud.json_save_path = if hud.json_save_path.ends_with("autosave.json") {
            "save/openttdrs_sim.json".into()
        } else {
            "save/openttdrs_autosave.json".into()
        };
        info!("Ruta JSON (F5/F9): {}", hud.json_save_path);
    }
}

/// Hotkeys de herramienta: 1/2 carreteras, 3 estacion, C limpiar, Esc desactivar.
pub(crate) fn handle_tool_hotkeys(hotkeys: Res<UiHotkeys>, mut tool_state: ResMut<UiToolState>) {
    if hotkeys.fired(UiCommandId::RoadY) {
        tool_state.active_tool = Some(BuildMenuAction::RoadY);
    } else if hotkeys.fired(UiCommandId::RoadX) {
        tool_state.active_tool = Some(BuildMenuAction::RoadX);
    } else if hotkeys.fired(UiCommandId::Station) {
        tool_state.active_tool = Some(BuildMenuAction::Station);
    } else if hotkeys.fired(UiCommandId::BuildRail) {
        tool_state.active_tool = Some(BuildMenuAction::Rail);
    } else if hotkeys.fired(UiCommandId::Clear) {
        tool_state.active_tool = Some(BuildMenuAction::Clear);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ui::hotkeys::dispatch_ui_hotkeys;
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
        world.insert_resource(UiHotkeys::default());

        crate::state::insert_test_sim_run_state(&mut world);
        press_keys(&mut world, &[KeyCode::F1]);
        world.run_system_once(dispatch_ui_hotkeys).unwrap();
        world.run_system_once(handle_pause_toggle).unwrap();
        press_keys(&mut world, &[KeyCode::F4]);
        world.run_system_once(dispatch_ui_hotkeys).unwrap();
        world.run_system_once(handle_pause_toggle).unwrap();
        press_keys(&mut world, &[KeyCode::ControlLeft, KeyCode::F4]);
        world.run_system_once(dispatch_ui_hotkeys).unwrap();
        world.run_system_once(cycle_json_save_path_hotkey).unwrap();
        press_keys(&mut world, &[KeyCode::ControlLeft, KeyCode::F4]);
        world.run_system_once(dispatch_ui_hotkeys).unwrap();
        world.run_system_once(cycle_json_save_path_hotkey).unwrap();

        for k in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::KeyC,
        ] {
            press_keys(&mut world, &[k]);
            world.run_system_once(dispatch_ui_hotkeys).unwrap();
            world.run_system_once(handle_tool_hotkeys).unwrap();
        }
    }
}
