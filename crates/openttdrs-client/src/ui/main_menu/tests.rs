#![allow(clippy::unwrap_used)]

use super::labels::{adjust_seed, cycle_density, summary_text};
use super::{MainMenuPanel, setup_main_menu};
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, STARTING_MONEY_OPTIONS,
};
use crate::state::new_game::NewGameSettingsResource;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::World;
use openttdrs_core::Climate;

#[test]
fn setup_main_menu_and_camera_run() {
    let mut world = World::new();
    world.init_resource::<NewGameSettingsResource>();
    world.run_system_once(setup_main_menu).unwrap();
    assert_eq!(world.resource::<MainMenuPanel>(), &MainMenuPanel::Root);
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
