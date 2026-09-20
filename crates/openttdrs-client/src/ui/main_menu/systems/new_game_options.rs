use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::Climate;

use crate::state::bootstrap::MapSizePreset;
use crate::state::new_game::{NewGameSeedSequence, NewGameSettingsResource};

use super::super::labels::{adjust_seed, cycle_density, summary_text_for};
use super::super::widgets::{option_button_bg, seed_button_bg, toggle_button_bg};
use super::super::{
    MainMenuClimateButton, MainMenuDensityButton, MainMenuDensityTarget, MainMenuMapSizeButton,
    MainMenuPanel, MainMenuRoughnessButton, MainMenuSeedDecButton, MainMenuSeedIncButton,
    MainMenuSeedInput, MainMenuSeedInputState, MainMenuSeedRandomButton, MainMenuStartYearButton,
    MainMenuStartingMoneyButton, MainMenuSummaryText, MainMenuToggle,
};

const MAX_SEED_DIGITS: usize = 20;

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

/// Refleja el valor materializado de la semilla en el campo editable.
pub(crate) fn sync_main_menu_seed_input(
    panel: Res<MainMenuPanel>,
    settings: Res<NewGameSettingsResource>,
    mut inputs: Query<(&mut EditableText, &mut MainMenuSeedInputState), With<MainMenuSeedInput>>,
) {
    if *panel != MainMenuPanel::NewGame || (!panel.is_changed() && !settings.is_changed()) {
        return;
    }
    let seed = settings.0.seed.to_string();
    for (mut editable, mut edit_state) in &mut inputs {
        if edit_state.draft == seed {
            continue;
        }
        edit_state.draft.clone_from(&seed);
        edit_state.replace_on_next_edit = false;
        editable.editor_mut().set_text(&seed);
    }
}

/// Campo numérico y botón de aleatorización de la semilla de nueva partida.
#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_seed_input_interaction(
    panel: Res<MainMenuPanel>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut auto_seeds: ResMut<NewGameSeedSequence>,
    mut input_focus: ResMut<InputFocus>,
    mut key_events: MessageReader<KeyboardInput>,
    mut input_q: Query<
        (
            Entity,
            Ref<Interaction>,
            &mut EditableText,
            &mut MainMenuSeedInputState,
        ),
        With<MainMenuSeedInput>,
    >,
    mut randomize_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuSeedRandomButton>),
    >,
) {
    if *panel != MainMenuPanel::NewGame {
        key_events.clear();
        return;
    }
    let Ok((entity, interaction, mut editable, mut edit_state)) = input_q.single_mut() else {
        key_events.clear();
        return;
    };

    if interaction.is_changed() && *interaction == Interaction::Pressed {
        edit_state.draft = settings.0.seed.to_string();
        edit_state.replace_on_next_edit = true;
        editable.editor_mut().set_text(&edit_state.draft);
        input_focus.set(entity, FocusCause::Pressed);
    }

    for (interaction, mut background) in &mut randomize_q {
        if *interaction == Interaction::Pressed {
            settings.0.seed = auto_seeds.next_seed();
            edit_state.draft = settings.0.seed.to_string();
            edit_state.replace_on_next_edit = false;
            editable.editor_mut().set_text(&edit_state.draft);
        }
        *background = seed_button_bg(*interaction);
    }

    if input_focus.get() != Some(entity) {
        key_events.clear();
        return;
    }

    let mut changed = false;
    for event in key_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if matches!(event.logical_key, Key::Backspace) {
            if edit_state.replace_on_next_edit {
                edit_state.replace_on_next_edit = false;
                changed |= !edit_state.draft.is_empty();
                edit_state.draft.clear();
            } else {
                changed |= edit_state.draft.pop().is_some();
            }
            continue;
        }
        if matches!(event.logical_key, Key::Delete) {
            edit_state.replace_on_next_edit = false;
            changed |= !edit_state.draft.is_empty();
            edit_state.draft.clear();
            continue;
        }
        let Some(text) = &event.text else {
            continue;
        };
        for character in text.chars().filter(char::is_ascii_digit) {
            if edit_state.replace_on_next_edit {
                edit_state.draft.clear();
                edit_state.replace_on_next_edit = false;
            }
            if edit_state.draft.len() >= MAX_SEED_DIGITS {
                continue;
            }
            let mut candidate = edit_state.draft.clone();
            candidate.push(character);
            if candidate.parse::<u64>().is_ok() {
                edit_state.draft = candidate;
                changed = true;
            }
        }
    }
    if !changed {
        return;
    }
    editable.editor_mut().set_text(&edit_state.draft);
    if let Ok(seed) = edit_state.draft.parse::<u64>() {
        settings.0.seed = seed;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_options_interaction(
    panel: Res<MainMenuPanel>,
    mut settings: ResMut<NewGameSettingsResource>,
    input_focus: Option<Res<InputFocus>>,
    seed_inputs: Query<(), With<MainMenuSeedInput>>,
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

    let seed_input_focused = input_focus
        .as_deref()
        .and_then(InputFocus::get)
        .is_some_and(|entity| seed_inputs.get(entity).is_ok());
    if !seed_input_focused {
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
