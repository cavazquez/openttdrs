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
    prefs.news_industry_close = mode_to_u8(settings.industry_close);
    prefs.news_economy = mode_to_u8(settings.economy);
}

pub(crate) fn hydrate_news_display_prefs(
    client: Res<ClientPreferences>,
    mut news: ResMut<NewsDisplayPrefs>,
    mut hydrated: Local<bool>,
) {
    if *hydrated {
        return;
    }
    news.0 = settings_from_client_prefs(&client);
    *hydrated = true;
}

pub(crate) fn sync_news_display_prefs_to_client(
    news: Res<NewsDisplayPrefs>,
    mut client: ResMut<ClientPreferences>,
    hydrated: Local<bool>,
) {
    if !*hydrated {
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
}
