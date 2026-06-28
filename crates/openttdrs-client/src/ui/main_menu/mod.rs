//! Menú principal: raíz, nueva partida, cargar, salir.

mod labels;
mod setup;
mod systems;
mod widgets;

#[cfg(test)]
mod tests;

pub(crate) use setup::setup_main_menu;
pub(crate) use systems::{
    leave_main_menu, main_menu_interaction, main_menu_options_interaction,
    sync_main_menu_panel_visibility, sync_main_menu_summary,
};

use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::{MapSizePreset, PopulationDensity};

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct MainMenuCamera;

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainMenuPanel {
    #[default]
    Root,
    NewGame,
    QuitConfirm,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuSubPanel(pub MainMenuPanel);

#[derive(Component)]
pub(crate) struct MainMenuTitleText;

#[derive(Component)]
pub(crate) struct MainMenuHintsText;

#[derive(Component)]
pub(crate) struct MainMenuNewGameButton;

#[derive(Component)]
pub(crate) struct MainMenuLoadButton;

#[derive(Component)]
pub(crate) struct MainMenuDemoButton;

#[derive(Component)]
pub(crate) struct MainMenuQuitButton;

#[derive(Component)]
pub(crate) struct MainMenuBackButton;

#[derive(Component)]
pub(crate) struct MainMenuStartButton;

#[derive(Component)]
pub(crate) struct MainMenuQuitConfirmYes;

#[derive(Component)]
pub(crate) struct MainMenuQuitConfirmNo;

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuClimateButton(pub Climate);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuMapSizeButton(pub MapSizePreset);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuStartYearButton(pub u32);

#[derive(Component)]
pub(crate) struct MainMenuSeedDecButton;

#[derive(Component)]
pub(crate) struct MainMenuSeedIncButton;

#[derive(Component, Clone, Copy)]
pub(crate) enum MainMenuToggle {
    WorldGen,
    Island,
    PreserveDemo,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum MainMenuDensityTarget {
    Town,
    Industry,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuDensityButton(pub PopulationDensity, pub MainMenuDensityTarget);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuStartingMoneyButton(pub i64);

#[derive(Component)]
pub(crate) struct MainMenuSummaryText;
