#![allow(clippy::unwrap_used)]

use super::labels::{adjust_seed, cycle_density, summary_text, summary_text_for};
use super::{
    MainMenuDemoButton, MainMenuDynamicText, MainMenuLanguageButton, MainMenuLocalizedText,
    MainMenuNewGameButton, MainMenuNewGameOptionsColumn, MainMenuPanel, MainMenuPreferencesButton,
    MainMenuResolutionButton, MainMenuSeedInput, MainMenuSeedInputState, setup_main_menu,
};
use crate::network::{NetCli, NetworkStatus};
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, STARTING_MONEY_OPTIONS,
};
use crate::state::new_game::{NewGameSeedSequence, NewGameSettingsResource};
use bevy::ecs::system::RunSystemOnce;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::InputFocus;
use bevy::prelude::{Entity, KeyCode, Messages, Text, World};
use bevy::text::EditableText;
use openttdrs_core::Climate;

#[test]
fn setup_main_menu_and_camera_run() {
    let mut world = World::new();
    world.init_resource::<NewGameSettingsResource>();
    world.insert_resource(NetCli::Offline);
    world.insert_resource(NetworkStatus::default());
    world.run_system_once(setup_main_menu).unwrap();
    assert_eq!(world.resource::<MainMenuPanel>(), &MainMenuPanel::Root);
    assert_eq!(
        world.query::<&MainMenuNewGameButton>().iter(&world).count(),
        1
    );
    assert_eq!(world.query::<&MainMenuDemoButton>().iter(&world).count(), 1);
    assert_eq!(world.query::<&MainMenuSeedInput>().iter(&world).count(), 1);
    assert_eq!(
        world
            .query::<&MainMenuNewGameOptionsColumn>()
            .iter(&world)
            .count(),
        2
    );
}

#[test]
fn seed_input_accepts_a_player_supplied_number() {
    let mut world = World::new();
    world.insert_resource(MainMenuPanel::NewGame);
    world.insert_resource(NewGameSettingsResource::default());
    world.init_resource::<NewGameSeedSequence>();
    world.init_resource::<Messages<KeyboardInput>>();
    let input = world
        .spawn((
            MainMenuSeedInput,
            MainMenuSeedInputState::default(),
            bevy::prelude::Interaction::default(),
            EditableText::new(""),
        ))
        .id();
    world.insert_resource(InputFocus::from_entity(input));
    world.write_message(KeyboardInput {
        key_code: KeyCode::Digit7,
        logical_key: Key::Character("7".into()),
        state: ButtonState::Pressed,
        text: Some("7".into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });

    world
        .run_system_once(super::main_menu_seed_input_interaction)
        .unwrap();

    assert_eq!(world.resource::<NewGameSettingsResource>().0.seed, 7);
    let mut editable = world.query::<&EditableText>();
    assert_eq!(editable.single(&world).unwrap().value(), "7");
}

#[test]
fn localized_label_sync_runs_with_static_and_dynamic_labels() {
    let mut world = World::new();
    world.insert_resource(crate::settings::ClientPreferences::default());
    world.spawn((MainMenuLocalizedText("Nueva partida"), Text::new("")));
    world.spawn((
        MainMenuDynamicText::Climate(Climate::Temperate),
        Text::new(""),
    ));

    world
        .run_system_once(super::systems::sync_main_menu_localized_labels)
        .unwrap();
}

#[test]
fn preferences_sync_runs_with_resolution_and_language_buttons() {
    let mut world = World::new();
    world.insert_resource(MainMenuPanel::Preferences);
    world.insert_resource(crate::settings::ClientPreferences::default());
    world.spawn((
        MainMenuResolutionButton {
            width: 1280,
            height: 720,
        },
        bevy::prelude::BackgroundColor::default(),
        bevy::prelude::Interaction::default(),
    ));
    world.spawn((
        MainMenuLanguageButton(crate::i18n::Locale::Es),
        bevy::prelude::BackgroundColor::default(),
        bevy::prelude::Interaction::default(),
    ));

    world
        .run_system_once(super::systems::sync_main_menu_preferences)
        .unwrap();
}

#[test]
fn preferences_interaction_accepts_all_button_queries_without_query_conflict() {
    let mut world = World::new();
    world.insert_resource(MainMenuPanel::Preferences);
    world.insert_resource(crate::settings::ClientPreferences::default());
    world.spawn((
        MainMenuPreferencesButton,
        bevy::prelude::BackgroundColor::default(),
        bevy::prelude::Interaction::default(),
    ));
    world.spawn((
        MainMenuResolutionButton {
            width: 1280,
            height: 720,
        },
        bevy::prelude::BackgroundColor::default(),
        bevy::prelude::Interaction::default(),
    ));
    world.spawn((
        MainMenuLanguageButton(crate::i18n::Locale::Es),
        bevy::prelude::BackgroundColor::default(),
        bevy::prelude::Interaction::default(),
    ));

    world
        .run_system_once(super::systems::main_menu_preferences_interaction)
        .unwrap();
}

#[test]
fn summary_text_includes_density_and_money() {
    let text = summary_text(NewGameSettings {
        map_size: MapSizePreset::SMALL,
        climate: Climate::Temperate,
        town_density: PopulationDensity::Dense,
        industry_density: PopulationDensity::Sparse,
        starting_money: STARTING_MONEY_OPTIONS[3],
        world_gen: true,
        island: true,
        ..NewGameSettings::default()
    });
    assert!(text.contains("Alta"));
    assert!(text.contains("Baja"));
    assert!(text.contains("$1.0M"));
    assert!(text.contains("lagos"));
}

#[test]
fn english_summary_translates_dynamic_options() {
    let text = summary_text_for(
        crate::i18n::Locale::En,
        NewGameSettings {
            climate: Climate::SubArctic,
            town_density: PopulationDensity::Sparse,
            industry_density: PopulationDensity::Dense,
            ..NewGameSettings::default()
        },
    );
    assert!(text.contains("Arctic"));
    assert!(text.contains("Sparse"));
    assert!(text.contains("Dense"));
    assert!(text.contains("climate"));
}

#[test]
fn adjust_seed_increments_and_saturates_at_zero() {
    let mut seed = 0_u64;
    adjust_seed(&mut seed, -1);
    assert_eq!(seed, 0);
    adjust_seed(&mut seed, 1);
    assert_eq!(seed, 1);
    adjust_seed(&mut seed, 1);
    assert_eq!(seed, 2);
}

#[test]
fn cycle_density_rotates_sparse_normal_dense() {
    let mut d = PopulationDensity::Sparse;
    cycle_density(&mut d);
    assert_eq!(d, PopulationDensity::Normal);
    cycle_density(&mut d);
    assert_eq!(d, PopulationDensity::Dense);
    cycle_density(&mut d);
    assert_eq!(d, PopulationDensity::Sparse);
}
