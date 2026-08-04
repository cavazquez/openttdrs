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
    pub(crate) sound_click_beep: bool,
    /// Ejecutar una entrada al arrastrar desde el ancla y soltar sobre el dropdown.
    pub(crate) toolbar_dropdown_autoselect: bool,
    /// Overrides `command_id=Ctrl+F1;...` de la tabla central de hotkeys.
    pub(crate) toolbar_hotkeys: String,
    /// Año previo al cual el selector de señales inicia en semáforo.
    pub(crate) semaphore_build_before: u32,
    pub(crate) show_debug_gizmos: bool,
    pub(crate) show_diagnostics_overlay: bool,
    /// Tinte naranja en vías con reserva PBS activa.
    pub(crate) show_pbs_reservations: bool,
    /// Dibujar aristas del Link Graph en el mapa (también se activa al abrir la ventana).
    pub(crate) show_link_graph_overlay: bool,
    /// Mostrar carteles de nombre/población de pueblos.
    pub(crate) show_town_labels: bool,
    /// Mostrar nombres de estaciones en el mapa.
    pub(crate) show_station_labels: bool,
    /// Ciclos de paleta (agua, refinería, burbujas…). Off o pausa → congelados.
    pub(crate) full_animation: bool,
    /// Densidad de humo de locomotoras (`vehicle.smoke_amount`): 0=off, 1=bajo, 2=normal.
    pub(crate) smoke_amount: u8,
    /// Detalle extra de mapa (faroles, árboles de acera, cercas). Off → sin detalle.
    pub(crate) full_detail: bool,
    /// Bits `_transparency_opt` (`TransparencyOption`).
    pub(crate) transparency_opt: u32,
    /// Bits `_invisibility_opt` (oculta = bit aquí + en transparency_opt).
    pub(crate) invisibility_opt: u32,
    /// 0=Off, 1=Summary, 2=Full — ver `news_prefs`.
    pub(crate) news_cargo_delivered: u8,
    pub(crate) news_first_cargo: u8,
    pub(crate) news_first_vehicle: u8,
    pub(crate) news_vehicle_advice: u8,
    pub(crate) news_accident: u8,
    pub(crate) news_company_info: u8,
    /// Preferencia de noticias de cierre de industria (0=Off, 1=Summary, 2=Full).
    pub(crate) news_industry_close: u8,
    /// Preferencia de noticias económicas / recesión (0=Off, 1=Summary, 2=Full).
    pub(crate) news_economy: u8,
    /// Posiciones de ventanas flotantes: `Id=x,y;Id2=x,y` (UI-7).
    pub(crate) window_layouts: String,
    /// Highscores locales: `name|value|year|B|R;…` (UI-8).
    pub(crate) highscores: String,
    /// Ancho de ventana primaria (preferencias de menú).
    pub(crate) window_width: u32,
    /// Alto de ventana primaria.
    pub(crate) window_height: u32,
    /// Código de idioma (`es` por ahora; placeholder i18n).
    pub(crate) language: String,
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
            sound_click_beep: true,
            toolbar_dropdown_autoselect: true,
            toolbar_hotkeys: String::new(),
            semaphore_build_before: openttdrs_core::SEMAPHORE_BUILD_BEFORE_YEAR,
            show_debug_gizmos: false,
            show_diagnostics_overlay: false,
            show_pbs_reservations: true,
            show_link_graph_overlay: false,
            show_town_labels: true,
            show_station_labels: true,
            full_animation: true,
            smoke_amount: 2,
            full_detail: true,
            transparency_opt: 0,
            invisibility_opt: 0,
            // Las entregas regulares son frecuentes: se informan en el ticker
            // sin convertir cada descarga en un popup con sonido.
            news_cargo_delivered: crate::news_prefs::DISPLAY_SUMMARY,
            news_first_cargo: crate::news_prefs::DISPLAY_FULL,
            news_first_vehicle: crate::news_prefs::DISPLAY_FULL,
            news_vehicle_advice: crate::news_prefs::DISPLAY_SUMMARY,
            news_accident: crate::news_prefs::DISPLAY_FULL,
            news_company_info: crate::news_prefs::DISPLAY_SUMMARY,
            news_industry_close: crate::news_prefs::DISPLAY_SUMMARY,
            news_economy: crate::news_prefs::DISPLAY_SUMMARY,
            window_layouts: String::new(),
            highscores: String::new(),
            window_width: 1280,
            window_height: 720,
            language: "es".into(),
        }
    }
}

impl ClientPreferences {
    #[must_use]
    pub(crate) fn transparency_mode(
        &self,
        to: crate::sprites::TransparencyOption,
    ) -> crate::sprites::TransparencyMode {
        crate::sprites::mode_from_bits(self.transparency_opt, self.invisibility_opt, to)
    }

    pub(crate) fn set_transparency_mode(
        &mut self,
        to: crate::sprites::TransparencyOption,
        mode: crate::sprites::TransparencyMode,
    ) {
        let (t, i) = crate::sprites::apply_mode_to_bits(
            self.transparency_opt,
            self.invisibility_opt,
            to,
            mode,
        );
        self.transparency_opt = t;
        self.invisibility_opt = i;
    }

    /// Posición guardada de una ventana flotante, si existe.
    #[must_use]
    #[allow(dead_code)] // API layout; hoy se usa `window_layout_by_key`.
    pub(crate) fn window_pos_by_key(&self, key: &str) -> Option<bevy::math::Vec2> {
        self.window_layout_by_key(key).map(|(pos, _)| pos)
    }

    /// Layout guardado: `Id=x,y` o `Id=x,y,w,h` (#243).
    #[must_use]
    pub(crate) fn window_layout_by_key(
        &self,
        key: &str,
    ) -> Option<(bevy::math::Vec2, Option<bevy::math::Vec2>)> {
        for entry in self.window_layouts.split(';').filter(|s| !s.is_empty()) {
            let Some((k, rest)) = entry.split_once('=') else {
                continue;
            };
            if k != key {
                continue;
            }
            let mut parts = rest.split(',');
            let xs = parts.next()?;
            let ys = parts.next()?;
            let x: f32 = xs.parse().ok()?;
            let y: f32 = ys.parse().ok()?;
            let size = match (parts.next(), parts.next()) {
                (Some(ws), Some(hs)) => {
                    let w: f32 = ws.parse().ok()?;
                    let h: f32 = hs.parse().ok()?;
                    Some(bevy::math::Vec2::new(w, h))
                }
                _ => None,
            };
            return Some((bevy::math::Vec2::new(x, y), size));
        }
        None
    }

    #[allow(dead_code)] // API layout; hoy se usa `set_window_layout_by_key`.
    pub(crate) fn set_window_pos_by_key(&mut self, key: &str, pos: bevy::math::Vec2) {
        let size = self.window_layout_by_key(key).and_then(|(_, size)| size);
        self.set_window_layout_by_key(key, pos, size);
    }

    pub(crate) fn set_window_layout_by_key(
        &mut self,
        key: &str,
        pos: bevy::math::Vec2,
        size: Option<bevy::math::Vec2>,
    ) {
        let mut parts: Vec<String> = self
            .window_layouts
            .split(';')
            .filter(|s| !s.is_empty())
            .filter(|s| !s.starts_with(&format!("{key}=")))
            .map(str::to_string)
            .collect();
        let entry = if let Some(size) = size {
            format!(
                "{key}={:.0},{:.0},{:.0},{:.0}",
                pos.x, pos.y, size.x, size.y
            )
        } else {
            format!("{key}={:.0},{:.0}", pos.x, pos.y)
        };
        parts.push(entry);
        self.window_layouts = parts.join(";");
    }

    pub(crate) const HIGHSCORE_LIMIT: usize = 10;

    /// Entradas ordenadas por valor descendente.
    #[must_use]
    pub(crate) fn highscore_entries(&self) -> Vec<openttdrs_core::GameScore> {
        let mut out = Vec::new();
        for entry in self.highscores.split(';').filter(|s| !s.is_empty()) {
            let mut parts = entry.split('|');
            let (Some(name), Some(value), Some(year), Some(reason)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let Ok(company_value) = value.parse::<i64>() else {
                continue;
            };
            let Ok(calendar_year) = year.parse::<u32>() else {
                continue;
            };
            let Some(reason) = reason
                .chars()
                .next()
                .and_then(openttdrs_core::GameOverReason::from_storage_code)
            else {
                continue;
            };
            out.push(openttdrs_core::GameScore {
                company_name: name.to_string(),
                company_value,
                calendar_year,
                reason,
            });
        }
        out.sort_by_key(|b| std::cmp::Reverse(b.company_value));
        out
    }

    /// Inserta un score y recorta al top [`Self::HIGHSCORE_LIMIT`]. Devuelve el rango 1-based.
    pub(crate) fn insert_highscore(&mut self, score: &openttdrs_core::GameScore) -> usize {
        let mut entries = self.highscore_entries();
        entries.push(score.clone());
        entries.sort_by_key(|b| std::cmp::Reverse(b.company_value));
        entries.truncate(Self::HIGHSCORE_LIMIT);
        let rank = entries
            .iter()
            .position(|e| e == score)
            .map(|i| i + 1)
            .unwrap_or(entries.len().max(1));
        self.highscores = entries
            .iter()
            .map(|e| {
                format!(
                    "{}|{}|{}|{}",
                    e.company_name.replace('|', "/").replace(';', ","),
                    e.company_value,
                    e.calendar_year,
                    e.reason.storage_code()
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        rank
    }

    /// Aplica un preset de cliente (Display / QoL).
    pub(crate) fn apply_preset(&mut self, preset: ClientSettingsPreset) {
        match preset {
            ClientSettingsPreset::Classic => {
                self.minimap_visible = true;
                self.full_animation = true;
                self.smoke_amount = 2;
                self.full_detail = true;
                self.show_town_labels = true;
                self.show_station_labels = true;
                self.show_pbs_reservations = true;
                self.show_link_graph_overlay = false;
                self.show_debug_gizmos = false;
                self.show_diagnostics_overlay = false;
                self.default_sim_speed = 1.0;
            }
            ClientSettingsPreset::Performance => {
                self.minimap_visible = true;
                self.full_animation = false;
                self.smoke_amount = 0;
                self.full_detail = false;
                self.show_town_labels = false;
                self.show_station_labels = false;
                self.show_pbs_reservations = false;
                self.show_link_graph_overlay = false;
                self.show_debug_gizmos = false;
                self.show_diagnostics_overlay = false;
                self.default_sim_speed = 1.0;
            }
            ClientSettingsPreset::Dev => {
                self.minimap_visible = true;
                self.full_animation = true;
                self.smoke_amount = 2;
                self.full_detail = true;
                self.show_town_labels = true;
                self.show_station_labels = true;
                self.show_pbs_reservations = true;
                self.show_link_graph_overlay = true;
                self.show_debug_gizmos = true;
                self.show_diagnostics_overlay = true;
                self.default_sim_speed = 1.0;
            }
        }
    }

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

/// Presets de `ClientPreferences` (UI-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientSettingsPreset {
    Classic,
    Performance,
    Dev,
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
            Startup,
            apply_window_resolution_from_preferences.after(hydrate_runtime_from_preferences),
        );
        app.add_systems(
            Update,
            (
                queue_save_preferences,
                save_preferences_on_exit,
                sync_preferences_from_hud,
                sync_transparency_render_preferences,
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
    hud.sound_click_beep = effective.sound_click_beep;
    crate::sprites::set_transparency_preferences(
        effective.transparency_opt,
        effective.invisibility_opt,
    );
    hydrated.0 = true;
}

fn sync_transparency_render_preferences(prefs: Res<ClientPreferences>) {
    if prefs.is_changed() {
        crate::sprites::set_transparency_preferences(
            prefs.transparency_opt,
            prefs.invisibility_opt,
        );
    }
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
    if prefs.sound_click_beep != hud.sound_click_beep {
        prefs.sound_click_beep = hud.sound_click_beep;
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
    if let Some(window) = window_plugin.primary_window.as_mut() {
        // Preferencias se hidratan tras Startup; el tamaño inicial sigue siendo 1280×720
        // y el menú Preferencias lo actualiza en caliente + persiste para el próximo arranque
        // vía `apply_window_resolution_from_preferences`.
        let _ = window;
    }
    window_plugin
}

/// Aplica resolución guardada tras hidratar preferencias (arranque).
fn apply_window_resolution_from_preferences(
    prefs: Res<ClientPreferences>,
    hydrated: Res<SettingsHydrated>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut applied: Local<bool>,
) {
    if *applied || !hydrated.0 {
        return;
    }
    if crate::bevy_app::visual_capture_requested() {
        *applied = true;
        return;
    }
    let w = prefs.window_width.max(640);
    let h = prefs.window_height.max(480);
    if let Ok(mut window) = windows.single_mut() {
        window.resolution.set(w as f32, h as f32);
    }
    *applied = true;
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

    #[test]
    fn window_layouts_roundtrip() {
        let mut prefs = ClientPreferences::default();
        prefs.set_window_pos_by_key("Help", bevy::math::Vec2::new(40.0, 80.0));
        prefs.set_window_pos_by_key("NewGrf", bevy::math::Vec2::new(100.0, 120.0));
        assert_eq!(
            prefs.window_pos_by_key("Help"),
            Some(bevy::math::Vec2::new(40.0, 80.0))
        );
        prefs.set_window_pos_by_key("Help", bevy::math::Vec2::new(50.0, 90.0));
        assert_eq!(
            prefs.window_pos_by_key("Help"),
            Some(bevy::math::Vec2::new(50.0, 90.0))
        );
        assert_eq!(
            prefs.window_pos_by_key("NewGrf"),
            Some(bevy::math::Vec2::new(100.0, 120.0))
        );
        prefs.set_window_layout_by_key(
            "Town",
            bevy::math::Vec2::new(60.0, 90.0),
            Some(bevy::math::Vec2::new(260.0, 180.0)),
        );
        assert_eq!(
            prefs.window_layout_by_key("Town"),
            Some((
                bevy::math::Vec2::new(60.0, 90.0),
                Some(bevy::math::Vec2::new(260.0, 180.0))
            ))
        );
    }

    #[test]
    fn apply_preset_performance_disables_detail() {
        let mut prefs = ClientPreferences::default();
        prefs.apply_preset(ClientSettingsPreset::Performance);
        assert!(!prefs.full_animation);
        assert_eq!(prefs.smoke_amount, 0);
        assert!(!prefs.full_detail);
        assert!(!prefs.show_town_labels);
        assert!(!prefs.show_debug_gizmos);
        prefs.apply_preset(ClientSettingsPreset::Dev);
        assert!(prefs.show_debug_gizmos);
        assert!(prefs.show_diagnostics_overlay);
        prefs.apply_preset(ClientSettingsPreset::Classic);
        assert!(prefs.full_animation);
        assert_eq!(prefs.smoke_amount, 2);
        assert!(!prefs.show_debug_gizmos);
    }

    #[test]
    fn insert_highscore_ranks_by_value() {
        let mut prefs = ClientPreferences::default();
        let low = openttdrs_core::GameScore {
            company_name: "A".into(),
            company_value: 10_000,
            calendar_year: 1960,
            reason: openttdrs_core::GameOverReason::Retired,
        };
        let high = openttdrs_core::GameScore {
            company_name: "B".into(),
            company_value: 90_000,
            calendar_year: 1970,
            reason: openttdrs_core::GameOverReason::Bankruptcy,
        };
        assert_eq!(prefs.insert_highscore(&low), 1);
        assert_eq!(prefs.insert_highscore(&high), 1);
        let entries = prefs.highscore_entries();
        assert_eq!(entries[0].company_name, "B");
        assert_eq!(entries[1].company_name, "A");
    }
}
