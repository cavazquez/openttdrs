#![allow(clippy::unwrap_used)]

use super::labels::{adjust_seed, cycle_density, summary_text, summary_text_for};
use super::{
    MainMenuCamera, MainMenuDemoButton, MainMenuDynamicText, MainMenuFirstRouteButton,
    MainMenuLanguageButton, MainMenuLocalizedText, MainMenuNewGameButton, MainMenuPanel,
    MainMenuPreferencesButton, MainMenuResolutionButton, MainMenuUi, setup_main_menu,
};
use crate::camera::{CameraFocusRequest, tile_camera_world_pos};
use crate::network::{NetCli, NetworkStatus};
use crate::render::MapVisualLayer;
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, STARTING_MONEY_OPTIONS,
};
use crate::state::new_game::NewGameSettingsResource;
use crate::state::{ClientScreen, EditorSession, SimRunState, SimWorld, SuspendedGameSession};
use crate::ui::SaveWindowState;
use bevy::app::Update;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::{
    App, BackgroundColor, ChildOf, Interaction, MinimalPlugins, State, Text, World,
};
use bevy::state::app::{AppExtStates, StatesPlugin};
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter};
use openttdrs_core::parity::{FIRST_ROUTE_COAL_MINE, FIRST_ROUTE_POWER_STATION, FIRST_ROUTE_YEAR};
use openttdrs_core::{Climate, TileCoord};

#[test]
fn setup_main_menu_and_camera_run() {
    let mut world = World::new();
    world.init_resource::<NewGameSettingsResource>();
    world.insert_resource(NetCli::Offline);
    world.insert_resource(NetworkStatus::default());
    world.run_system_once(setup_main_menu).unwrap();
    assert_eq!(world.resource::<MainMenuPanel>(), &MainMenuPanel::Root);
    assert_eq!(
        world
            .query::<&MainMenuFirstRouteButton>()
            .iter(&world)
            .count(),
        1
    );
    assert_eq!(
        world.query::<&MainMenuNewGameButton>().iter(&world).count(),
        1
    );
    assert_eq!(world.query::<&MainMenuDemoButton>().iter(&world).count(), 1);
}

fn first_route_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin));
    app.init_state::<ClientScreen>();
    app.add_sub_state::<SimRunState>();
    app.init_resource::<MainMenuPanel>();
    app.init_resource::<SaveWindowState>();
    app.init_resource::<SuspendedGameSession>();
    app.init_resource::<CameraFocusRequest>();
    app.add_plugins(crate::ui::lifecycle::InGameLifecyclePlugin);
    app.add_systems(
        OnEnter(ClientScreen::InGame),
        super::systems::prepare_first_route_session,
    );
    app.add_systems(
        Update,
        super::systems::main_menu_first_route_interaction.run_if(in_state(ClientScreen::MainMenu)),
    );
    app
}

fn spawn_first_route_menu(world: &mut World) -> bevy::ecs::entity::Entity {
    let root = world.spawn(MainMenuUi).id();
    world.spawn(MainMenuCamera);
    world
        .spawn((
            MainMenuFirstRouteButton,
            Interaction::Pressed,
            BackgroundColor::default(),
            ChildOf(root),
        ))
        .id()
}

fn advance_to_first_route(app: &mut App) {
    // El clic solicita InGame; las dos transiciones de estado siguientes
    // materializan el subestado Paused de la sesión limitada.
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn first_route_interaction_starts_two_fresh_paused_sessions() {
    let mut app = first_route_test_app();
    let first_menu_button = spawn_first_route_menu(app.world_mut());
    advance_to_first_route(&mut app);

    assert_eq!(
        app.world().resource::<State<ClientScreen>>().get(),
        &ClientScreen::InGame
    );
    assert_eq!(
        app.world().resource::<State<SimRunState>>().get(),
        &SimRunState::Paused
    );
    assert!(app.world().get_entity(first_menu_button).is_err());
    assert!(!app.world().resource::<SuspendedGameSession>().active);
    assert!(!app.world().resource::<EditorSession>().active);

    let expected_focus = {
        let sim = app.world().resource::<SimWorld>();
        assert!(!sim.loaded_file);
        assert_eq!(sim.state.map.dimensions(), (64, 64));
        assert_eq!(sim.state.calendar.year, FIRST_ROUTE_YEAR);
        assert_eq!(sim.state.industries.len(), 2);
        assert!(sim.state.vehicles.is_empty());
        assert!(sim.state.stations.is_empty());
        tile_camera_world_pos(
            &sim.state.map,
            TileCoord::new(
                (FIRST_ROUTE_COAL_MINE.x + FIRST_ROUTE_POWER_STATION.x) / 2,
                (FIRST_ROUTE_COAL_MINE.y + FIRST_ROUTE_POWER_STATION.y) / 2,
            ),
        )
    };
    assert_eq!(
        app.world().resource::<CameraFocusRequest>().target,
        Some(expected_focus)
    );

    // Simula una sesión que deja entidades y datos propios antes de volver al menú.
    let prior_layer = app.world_mut().spawn(MapVisualLayer).id();
    app.world_mut().resource_mut::<SimWorld>().state.world_seed = 0;
    app.world_mut()
        .resource_mut::<SuspendedGameSession>()
        .active = true;
    app.world_mut()
        .resource_mut::<NextState<ClientScreen>>()
        .set(ClientScreen::MainMenu);
    app.update();
    assert_eq!(
        app.world().resource::<State<ClientScreen>>().get(),
        &ClientScreen::MainMenu
    );
    assert!(app.world().get_entity(prior_layer).is_err());

    let second_menu_button = spawn_first_route_menu(app.world_mut());
    advance_to_first_route(&mut app);
    let second = app.world().resource::<SimWorld>();
    assert!(app.world().get_entity(second_menu_button).is_err());
    assert_eq!(
        second.state.world_seed,
        openttdrs_core::parity::FIRST_ROUTE_WORLD_SEED
    );
    assert_eq!(second.state.calendar.year, FIRST_ROUTE_YEAR);
    assert_eq!(second.state.industries.len(), 2);
    assert!(second.state.vehicles.is_empty());
    assert!(second.state.stations.is_empty());
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
