use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::MapSizePreset;
use crate::state::new_game::NewGameSettingsResource;

use super::super::labels::{adjust_seed, cycle_density, summary_text_for};
use super::super::widgets::{option_button_bg, seed_button_bg, toggle_button_bg};
use super::super::{
    MainMenuClimateButton, MainMenuDensityButton, MainMenuDensityTarget, MainMenuMapSizeButton,
    MainMenuPanel, MainMenuRoughnessButton, MainMenuSeedDecButton, MainMenuSeedIncButton,
    MainMenuStartYearButton, MainMenuStartingMoneyButton, MainMenuSummaryText, MainMenuToggle,
};

pub(crate) fn sync_main_menu_summary(
    settings: Res<NewGameSettingsResource>,
    panel: Res<MainMenuPanel>,
    prefs: Res<crate::settings::ClientPreferences>,
    mut q: Query<&mut Text, With<MainMenuSummaryText>>,
) {
    if !settings.is_changed() && !panel.is_changed() && !prefs.is_changed() {
        return;
    }
    if *panel != MainMenuPanel::NewGame {
        return;
    }
    for mut text in &mut q {
        text.0 = summary_text_for(prefs.locale(), settings.settings());
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_options_interaction(
    panel: Res<MainMenuPanel>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut button_sets: ParamSet<(
        Query<(&Interaction, &MainMenuClimateButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuMapSizeButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuStartYearButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuToggle, &mut BackgroundColor)>,
        Query<(&Interaction, &mut BackgroundColor), With<MainMenuSeedDecButton>>,
        Query<(&Interaction, &mut BackgroundColor), With<MainMenuSeedIncButton>>,
        Query<(&Interaction, &MainMenuDensityButton, &mut BackgroundColor)>,
        Query<(
            &Interaction,
            &MainMenuStartingMoneyButton,
            &mut BackgroundColor,
        )>,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if *panel != MainMenuPanel::NewGame {
        return;
    }

    if keys.just_pressed(KeyCode::Digit1) {
        settings.0.climate = Climate::Temperate;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        settings.0.climate = Climate::SubArctic;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        settings.0.climate = Climate::SubTropical;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        settings.0.climate = Climate::Toyland;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        adjust_seed(&mut settings.0.seed, -1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        adjust_seed(&mut settings.0.seed, 1);
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        cycle_density(&mut settings.0.town_density);
    }
    if keys.just_pressed(KeyCode::KeyX) {
        cycle_density(&mut settings.0.industry_density);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p0() {
        if *interaction == Interaction::Pressed {
            settings.0.climate = btn.0;
        }
        *bg = option_button_bg(settings.0.climate == btn.0, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p1() {
        if *interaction == Interaction::Pressed {
            match *btn {
                MainMenuMapSizeButton::Compact => {
                    settings.0.map_size = MapSizePreset::Compact;
                }
                MainMenuMapSizeButton::Width(axis) => {
                    settings.0.map_size.set_width(axis);
                    settings.0.preserve_demo = false;
                }
                MainMenuMapSizeButton::Height(axis) => {
                    settings.0.map_size.set_height(axis);
                    settings.0.preserve_demo = false;
                }
            }
            if !settings.0.map_size.is_compact() {
                settings.0.preserve_demo = false;
            }
        }
        let selected = match *btn {
            MainMenuMapSizeButton::Compact => settings.0.map_size.is_compact(),
            MainMenuMapSizeButton::Width(axis) => matches!(
                settings.0.map_size,
                MapSizePreset::Sized { width, .. } if width == axis
            ),
            MainMenuMapSizeButton::Height(axis) => matches!(
                settings.0.map_size,
                MapSizePreset::Sized { height, .. } if height == axis
            ),
        };
        *bg = option_button_bg(selected, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p2() {
        if *interaction == Interaction::Pressed {
            settings.0.start_year = btn.0;
        }
        *bg = option_button_bg(settings.0.start_year == btn.0, *interaction);
    }

    for (interaction, toggle, mut bg) in &mut button_sets.p3() {
        if *interaction == Interaction::Pressed {
            match toggle {
                MainMenuToggle::WorldGen => settings.0.world_gen = !settings.0.world_gen,
                MainMenuToggle::Island => settings.0.island = !settings.0.island,
                MainMenuToggle::PreserveDemo => {
                    if settings.0.map_size.is_compact() {
                        settings.0.preserve_demo = !settings.0.preserve_demo;
                    }
                }
                MainMenuToggle::RivalAi => settings.0.rival_ai = !settings.0.rival_ai,
                MainMenuToggle::Disasters => {
                    settings.0.disasters_enabled = !settings.0.disasters_enabled;
                }
            }
        }
        let on = match toggle {
            MainMenuToggle::WorldGen => settings.0.world_gen,
            MainMenuToggle::Island => settings.0.island,
            MainMenuToggle::PreserveDemo => {
                settings.0.preserve_demo && settings.0.map_size.is_compact()
            }
            MainMenuToggle::RivalAi => settings.0.rival_ai,
            MainMenuToggle::Disasters => settings.0.disasters_enabled,
        };
        *bg = toggle_button_bg(on, *interaction);
    }

    for (interaction, mut bg) in &mut button_sets.p4() {
        if *interaction == Interaction::Pressed {
            adjust_seed(&mut settings.0.seed, -1);
        }
        *bg = seed_button_bg(*interaction);
    }
    for (interaction, mut bg) in &mut button_sets.p5() {
        if *interaction == Interaction::Pressed {
            adjust_seed(&mut settings.0.seed, 1);
        }
        *bg = seed_button_bg(*interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p6() {
        if *interaction == Interaction::Pressed {
            match btn.1 {
                MainMenuDensityTarget::Town => settings.0.town_density = btn.0,
                MainMenuDensityTarget::Industry => settings.0.industry_density = btn.0,
            }
        }
        let selected = match btn.1 {
            MainMenuDensityTarget::Town => settings.0.town_density == btn.0,
            MainMenuDensityTarget::Industry => settings.0.industry_density == btn.0,
        };
        *bg = option_button_bg(selected, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p7() {
        if *interaction == Interaction::Pressed {
            settings.0.starting_money = btn.0;
        }
        *bg = option_button_bg(settings.0.starting_money == btn.0, *interaction);
    }
}

/// Relieve del terreno en sistema aparte (el `ParamSet` de opciones ya tiene 8 queries).
pub(crate) fn main_menu_roughness_interaction(
    panel: Res<MainMenuPanel>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut roughness_q: Query<(&Interaction, &MainMenuRoughnessButton, &mut BackgroundColor)>,
) {
    if *panel != MainMenuPanel::NewGame {
        return;
    }
    for (interaction, btn, mut bg) in &mut roughness_q {
        if *interaction == Interaction::Pressed {
            settings.0.terrain_roughness = btn.0;
        }
        *bg = option_button_bg(settings.0.terrain_roughness == btn.0, *interaction);
    }
}
