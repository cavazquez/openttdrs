//! Preferencias del cliente persistidas en TOML (`SettingsPlugin` de Bevy 0.19).

use std::time::Duration;

use bevy::prelude::*;
use bevy::settings::{
    ReflectSettingsGroup, SaveSettingsDeferred, SaveSettingsSync, SettingsGroup, SettingsPlugin,
};
use bevy::window::{ExitCondition, WindowCloseRequested};

use crate::bevy_app::{CLIENT_SETTINGS_APP_ID, UpdateSet};
use crate::config::{self, DEFAULT_JSON_SAVE_PATH};
use crate::ui::SimHudControls;

/// Ruta de guardado JSON, minimapa, velocidad inicial, audio y flags de debug.
#[derive(Resource, SettingsGroup, Reflect, Clone)]
#[reflect(Resource, SettingsGroup, Default)]
pub(crate) struct ClientPreferences {
    pub(crate) json_save_path: String,
    pub(crate) minimap_visible: bool,
    pub(crate) default_sim_speed: f32,
    pub(crate) sfx_volume: f32,
    pub(crate) music_volume: f32,
    pub(crate) sound_vehicle: bool,
    pub(crate) sound_ambient: bool,
    pub(crate) sound_disaster: bool,
    pub(crate) sound_confirm: bool,
    pub(crate) show_debug_gizmos: bool,
    pub(crate) show_diagnostics_overlay: bool,
    /// 0=Off, 1=Summary, 2=Full — ver `news_prefs`.
    pub(crate) news_cargo_delivered: u8,
    pub(crate) news_first_cargo: u8,
    pub(crate) news_first_vehicle: u8,
    pub(crate) news_vehicle_advice: u8,
}

impl Default for ClientPreferences {
    fn default() -> Self {
        Self {
            json_save_path: DEFAULT_JSON_SAVE_PATH.into(),
            minimap_visible: true,
            default_sim_speed: 1.0,
            sfx_volume: 0.22,
            music_volume: 0.35,
            sound_vehicle: true,
            sound_ambient: true,
            sound_disaster: true,
            sound_confirm: true,
            show_debug_gizmos: false,
            show_diagnostics_overlay: false,
            news_cargo_delivered: crate::news_prefs::DISPLAY_FULL,
            news_first_cargo: crate::news_prefs::DISPLAY_FULL,
            news_first_vehicle: crate::news_prefs::DISPLAY_FULL,
            news_vehicle_advice: crate::news_prefs::DISPLAY_SUMMARY,
        }
    }
}

impl ClientPreferences {
    fn with_env_overrides(mut prefs: Self) -> Self {
        if config::env_flag("OPENTTDRS_GIZMOS") {
            prefs.show_debug_gizmos = true;
        }
        if config::env_flag("OPENTTDRS_DEBUG") {
            prefs.show_diagnostics_overlay = true;
        }
        prefs
    }
}

/// Marcador: las prefs en disco aún no se hidrataron en runtime.
#[derive(Resource, Default)]
struct SettingsHydrated(bool);

pub(crate) struct ClientSettingsPlugin;

impl Plugin for ClientSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SettingsPlugin::new(CLIENT_SETTINGS_APP_ID));
        app.init_resource::<SettingsHydrated>();
        app.add_systems(Startup, hydrate_runtime_from_preferences);
        app.add_systems(Startup, crate::news_prefs::hydrate_news_display_prefs);
        app.add_systems(
            Update,
            (
                queue_save_preferences,
                save_preferences_on_exit,
                sync_preferences_from_hud,
                crate::news_prefs::sync_news_display_prefs_to_client,
            )
                .in_set(UpdateSet::Status),
        );
    }
}

fn hydrate_runtime_from_preferences(
    prefs: Res<ClientPreferences>,
    mut hud: ResMut<SimHudControls>,
    mut hydrated: ResMut<SettingsHydrated>,
) {
    if hydrated.0 {
        return;
    }
    let effective = ClientPreferences::with_env_overrides((*prefs).clone());

    hud.json_save_path = effective.json_save_path.clone();
    hud.minimap_visible = effective.minimap_visible;
    hud.sim_speed = effective.default_sim_speed.clamp(0.25, 8.0);
    hud.sfx_volume = effective.sfx_volume.clamp(0.0, 1.0);
    hud.music_volume = effective.music_volume.clamp(0.0, 1.0);
    hud.sound_vehicle = effective.sound_vehicle;
    hud.sound_ambient = effective.sound_ambient;
    hud.sound_disaster = effective.sound_disaster;
    hud.sound_confirm = effective.sound_confirm;
    hydrated.0 = true;
}

/// Copia cambios de sesión relevantes de vuelta a `ClientPreferences` para persistir.
fn sync_preferences_from_hud(
    hud: Res<SimHudControls>,
    mut prefs: ResMut<ClientPreferences>,
    hydrated: Res<SettingsHydrated>,
) {
    if !hydrated.0 {
        return;
    }
    let mut changed = false;
    if prefs.json_save_path != hud.json_save_path {
        prefs.json_save_path = hud.json_save_path.clone();
        changed = true;
    }
    if prefs.minimap_visible != hud.minimap_visible {
        prefs.minimap_visible = hud.minimap_visible;
        changed = true;
    }
    if (prefs.default_sim_speed - hud.sim_speed).abs() > f32::EPSILON {
        prefs.default_sim_speed = hud.sim_speed;
        changed = true;
    }
    if (prefs.sfx_volume - hud.sfx_volume).abs() > f32::EPSILON {
        prefs.sfx_volume = hud.sfx_volume;
        changed = true;
    }
    if (prefs.music_volume - hud.music_volume).abs() > f32::EPSILON {
        prefs.music_volume = hud.music_volume;
        changed = true;
    }
    if prefs.sound_vehicle != hud.sound_vehicle {
        prefs.sound_vehicle = hud.sound_vehicle;
        changed = true;
    }
    if prefs.sound_ambient != hud.sound_ambient {
        prefs.sound_ambient = hud.sound_ambient;
        changed = true;
    }
    if prefs.sound_disaster != hud.sound_disaster {
        prefs.sound_disaster = hud.sound_disaster;
        changed = true;
    }
    if prefs.sound_confirm != hud.sound_confirm {
        prefs.sound_confirm = hud.sound_confirm;
        changed = true;
    }
    if changed {
        prefs.set_changed();
    }
}

fn queue_save_preferences(prefs: Res<ClientPreferences>, mut commands: Commands) {
    if prefs.is_changed() {
        commands.queue(SaveSettingsDeferred(Duration::from_secs_f32(0.5)));
    }
}

fn save_preferences_on_exit(
    mut close: MessageReader<WindowCloseRequested>,
    mut commands: Commands,
) {
    if close.read().next().is_some() {
        commands.queue(SaveSettingsSync::IfChanged);
        commands.write_message(AppExit::Success);
    }
}

/// Configura `ExitCondition::DontExit` para interceptar cierre y guardar prefs.
pub(crate) fn patch_window_plugin_for_settings(mut window_plugin: WindowPlugin) -> WindowPlugin {
    window_plugin.exit_condition = ExitCondition::DontExit;
    window_plugin
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use super::*;

    #[test]
    fn client_preferences_default_values() {
        let prefs = ClientPreferences::default();
        assert_eq!(prefs.json_save_path, DEFAULT_JSON_SAVE_PATH);
        assert!(prefs.minimap_visible);
        assert!((prefs.sfx_volume - 0.22).abs() < f32::EPSILON);
    }

    #[test]
    fn env_overrides_debug_flags() {
        // Sin env: defaults
        let prefs = ClientPreferences::default();
        assert!(!prefs.show_debug_gizmos);
        assert!(!prefs.show_diagnostics_overlay);
    }

    #[test]
    fn hydrate_applies_defaults_when_empty_path() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SettingsHydrated::default());
        world
            .run_system_once(hydrate_runtime_from_preferences)
            .unwrap();
        let hud = world.resource::<SimHudControls>();
        assert_eq!(hud.json_save_path, DEFAULT_JSON_SAVE_PATH);
        assert!(hud.minimap_visible);
        assert!((hud.sim_speed - 1.0).abs() < f32::EPSILON);
    }
}
