//! Preferencias Off / Summary / Full por tipo de noticia (N5).

use bevy::prelude::*;
use openttdrs_core::{NewsDisplayMode, NewsDisplaySettings};

use crate::settings::ClientPreferences;

pub(crate) const DISPLAY_OFF: u8 = 0;
pub(crate) const DISPLAY_SUMMARY: u8 = 1;
pub(crate) const DISPLAY_FULL: u8 = 2;

#[derive(Resource, Clone, PartialEq, Eq)]
pub(crate) struct NewsDisplayPrefs(pub NewsDisplaySettings);

impl Default for NewsDisplayPrefs {
    fn default() -> Self {
        Self(NewsDisplaySettings::openttd_defaults())
    }
}

/// Señal compartida entre la hidratación de arranque y la sincronización de
/// vuelta a preferencias persistentes.
///
/// No usar `Local<bool>` aquí: cada sistema obtiene su propia instancia local,
/// por lo que el sistema de escritura nunca observaba la hidratación hecha por
/// el sistema de arranque.
#[derive(Resource, Default)]
pub(crate) struct NewsDisplayPrefsHydrated(pub(crate) bool);

#[must_use]
pub(crate) fn mode_from_u8(value: u8) -> NewsDisplayMode {
    match value {
        DISPLAY_OFF => NewsDisplayMode::Off,
        DISPLAY_SUMMARY => NewsDisplayMode::Summary,
        _ => NewsDisplayMode::Full,
    }
}

#[must_use]
pub(crate) fn mode_to_u8(mode: NewsDisplayMode) -> u8 {
    match mode {
        NewsDisplayMode::Off => DISPLAY_OFF,
        NewsDisplayMode::Summary => DISPLAY_SUMMARY,
        NewsDisplayMode::Full => DISPLAY_FULL,
    }
}

#[must_use]
pub(crate) fn settings_from_client_prefs(prefs: &ClientPreferences) -> NewsDisplaySettings {
    NewsDisplaySettings {
        cargo_delivered: mode_from_u8(prefs.news_cargo_delivered),
        first_cargo_delivered: mode_from_u8(prefs.news_first_cargo),
        first_vehicle_running: mode_from_u8(prefs.news_first_vehicle),
        vehicle_advice: mode_from_u8(prefs.news_vehicle_advice),
        accident: mode_from_u8(prefs.news_accident),
        company_info: mode_from_u8(prefs.news_company_info),
        industry_open: mode_from_u8(prefs.news_industry_open),
        industry_close: mode_from_u8(prefs.news_industry_close),
        economy: mode_from_u8(prefs.news_economy),
    }
}

pub(crate) fn apply_settings_to_client_prefs(
    settings: &NewsDisplaySettings,
    prefs: &mut ClientPreferences,
) {
    prefs.news_cargo_delivered = mode_to_u8(settings.cargo_delivered);
    prefs.news_first_cargo = mode_to_u8(settings.first_cargo_delivered);
    prefs.news_first_vehicle = mode_to_u8(settings.first_vehicle_running);
    prefs.news_vehicle_advice = mode_to_u8(settings.vehicle_advice);
    prefs.news_accident = mode_to_u8(settings.accident);
    prefs.news_company_info = mode_to_u8(settings.company_info);
    prefs.news_industry_open = mode_to_u8(settings.industry_open);
    prefs.news_industry_close = mode_to_u8(settings.industry_close);
    prefs.news_economy = mode_to_u8(settings.economy);
}

pub(crate) fn hydrate_news_display_prefs(
    client: Res<ClientPreferences>,
    mut news: ResMut<NewsDisplayPrefs>,
    mut hydrated: ResMut<NewsDisplayPrefsHydrated>,
) {
    if hydrated.0 {
        return;
    }
    news.0 = settings_from_client_prefs(&client);
    hydrated.0 = true;
}

pub(crate) fn sync_news_display_prefs_to_client(
    news: Res<NewsDisplayPrefs>,
    mut client: ResMut<ClientPreferences>,
    hydrated: Res<NewsDisplayPrefsHydrated>,
) {
    if !hydrated.0 {
        return;
    }
    let mut scratch = ClientPreferences::default();
    apply_settings_to_client_prefs(&news.0, &mut scratch);
    let changed = client.news_cargo_delivered != scratch.news_cargo_delivered
        || client.news_first_cargo != scratch.news_first_cargo
        || client.news_first_vehicle != scratch.news_first_vehicle
        || client.news_vehicle_advice != scratch.news_vehicle_advice
        || client.news_accident != scratch.news_accident
        || client.news_company_info != scratch.news_company_info
        || client.news_industry_open != scratch.news_industry_open
        || client.news_industry_close != scratch.news_industry_close
        || client.news_economy != scratch.news_economy;
    if !changed {
        return;
    }
    apply_settings_to_client_prefs(&news.0, &mut client);
    client.set_changed();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::change_detection::DetectChanges;
    use openttdrs_core::NewsType;

    fn news_preferences_app(client: ClientPreferences) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(client)
            .init_resource::<NewsDisplayPrefs>()
            .init_resource::<NewsDisplayPrefsHydrated>()
            .add_systems(Startup, hydrate_news_display_prefs)
            .add_systems(Update, sync_news_display_prefs_to_client);
        app
    }

    #[test]
    fn mode_u8_roundtrip() {
        assert_eq!(mode_from_u8(DISPLAY_OFF), NewsDisplayMode::Off);
        assert_eq!(mode_from_u8(DISPLAY_SUMMARY), NewsDisplayMode::Summary);
        assert_eq!(mode_from_u8(DISPLAY_FULL), NewsDisplayMode::Full);
        assert_eq!(mode_from_u8(99), NewsDisplayMode::Full);
        assert_eq!(mode_to_u8(NewsDisplayMode::Summary), DISPLAY_SUMMARY);
    }

    #[test]
    fn new_client_preferences_keep_recurrent_cargo_in_the_ticker() {
        let prefs = ClientPreferences::default();
        assert_eq!(prefs.news_cargo_delivered, DISPLAY_SUMMARY);
        assert_eq!(
            settings_from_client_prefs(&prefs).cargo_delivered,
            NewsDisplayMode::Summary
        );
    }

    #[test]
    fn startup_edit_and_rehydrate_keep_all_news_preferences_persistent() {
        let stored = ClientPreferences {
            news_cargo_delivered: DISPLAY_OFF,
            news_first_cargo: DISPLAY_SUMMARY,
            news_first_vehicle: DISPLAY_FULL,
            news_vehicle_advice: DISPLAY_OFF,
            news_accident: DISPLAY_SUMMARY,
            news_company_info: DISPLAY_FULL,
            news_industry_open: DISPLAY_OFF,
            news_industry_close: DISPLAY_FULL,
            news_economy: DISPLAY_OFF,
            ..ClientPreferences::default()
        };
        let loaded_before_edit = stored.clone();

        let mut app = news_preferences_app(stored);
        let client_last_changed = |app: &App| {
            let Some(client) = app.world().get_resource_ref::<ClientPreferences>() else {
                panic!("ClientPreferences");
            };
            client.last_changed()
        };
        let client_tick_before_startup = client_last_changed(&app);
        // El estado cargado no debe verse cambiado sólo por la primera
        // hidratación NewsDisplayPrefs → ClientPreferences.
        app.world_mut().clear_trackers();
        app.update();
        assert_eq!(
            app.world().resource::<NewsDisplayPrefs>().0,
            settings_from_client_prefs(&loaded_before_edit),
            "Startup conserva los valores persistidos, no los defaults de NewsDisplayPrefs"
        );
        assert!(app.world().resource::<NewsDisplayPrefsHydrated>().0);
        assert_eq!(
            client_last_changed(&app),
            client_tick_before_startup,
            "la sincronización inicial equivalente no marca ClientPreferences"
        );

        let expected_after_edit = {
            let mut news = app.world_mut().resource_mut::<NewsDisplayPrefs>();
            for (kind, mode) in [
                (NewsType::CargoDelivered, NewsDisplayMode::Full),
                (NewsType::FirstCargoDelivered, NewsDisplayMode::Off),
                (NewsType::FirstVehicleRunning, NewsDisplayMode::Summary),
                (NewsType::VehicleAdvice, NewsDisplayMode::Full),
                (NewsType::Accident, NewsDisplayMode::Off),
                (NewsType::CompanyInfo, NewsDisplayMode::Summary),
                (NewsType::IndustryOpen, NewsDisplayMode::Full),
                (NewsType::IndustryClose, NewsDisplayMode::Summary),
                (NewsType::Economy, NewsDisplayMode::Full),
            ] {
                news.0.set_display(kind, mode);
            }
            news.0
        };
        app.update();
        assert_eq!(
            settings_from_client_prefs(app.world().resource::<ClientPreferences>()),
            expected_after_edit,
            "cada uno de los nueve modos vuelve al recurso persistido"
        );
        let client_tick_after_edit = client_last_changed(&app);
        assert_ne!(
            client_tick_after_edit, client_tick_before_startup,
            "una edición de noticias marca la preferencia para SettingsPlugin"
        );

        let persisted_after_edit = app.world().resource::<ClientPreferences>().clone();
        app.world_mut().clear_trackers();
        app.update();
        assert_eq!(
            client_last_changed(&app),
            client_tick_after_edit,
            "un Update posterior sin edición no vuelve a marcar preferencias"
        );

        // Simula el siguiente arranque después de que SettingsPlugin escribió
        // `ClientPreferences`: la hidratación recibe exactamente el modo
        // elegido, sin depender de Local de otro sistema.
        let mut restarted = news_preferences_app(persisted_after_edit);
        restarted.world_mut().clear_trackers();
        restarted.update();
        assert_eq!(
            restarted.world().resource::<NewsDisplayPrefs>().0,
            expected_after_edit,
            "la edición persistida se rehidrata en el siguiente arranque"
        );
    }
}
