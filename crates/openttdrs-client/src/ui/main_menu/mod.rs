//! Menú principal: raíz, nueva partida, cargar, salir.

mod labels;
mod setup;
mod systems;
mod widgets;

#[cfg(test)]
mod tests;

pub(crate) use setup::setup_main_menu;
pub(crate) use systems::{
    apply_pending_heightmap_on_enter, auto_start_preloaded_json, leave_main_menu,
    main_menu_continue_interaction, main_menu_editor_interaction, main_menu_highscores_interaction,
    main_menu_interaction, main_menu_options_interaction, main_menu_preferences_interaction,
    main_menu_roughness_interaction, main_menu_scenarios_interaction, main_menu_sound_interaction,
    return_to_main_menu, sync_main_menu_continue_button, sync_main_menu_heightmap_slots,
    sync_main_menu_highscores, sync_main_menu_panel_visibility, sync_main_menu_preferences,
    sync_main_menu_summary,
};

use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::{MapAxisSize, PopulationDensity, TerrainRoughness};

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct MainMenuCamera;

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainMenuPanel {
    #[default]
    Root,
    NewGame,
    Highscores,
    Scenarios,
    Preferences,
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
pub(crate) struct MainMenuContinueButton;

#[derive(Component)]
pub(crate) struct MainMenuContinueWrap;

#[derive(Component)]
pub(crate) struct MainMenuLoadButton;

#[derive(Component)]
pub(crate) struct MainMenuDemoButton;

#[derive(Component)]
pub(crate) struct MainMenuEditorButton;

#[derive(Component)]
pub(crate) struct MainMenuHighscoresButton;

#[derive(Component)]
pub(crate) struct MainMenuHighscoresText;

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

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainMenuMapSizeButton {
    Compact,
    Width(MapAxisSize),
    Height(MapAxisSize),
}

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
    RivalAi,
    Disasters,
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

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuRoughnessButton(pub TerrainRoughness);

#[derive(Component)]
pub(crate) struct MainMenuScenariosButton;

#[derive(Component)]
pub(crate) struct MainMenuPreferencesButton;

#[derive(Component)]
pub(crate) struct MainMenuSoundButton;

#[derive(Component)]
pub(crate) struct MainMenuOpenScenariosDirButton;

#[derive(Component)]
pub(crate) struct MainMenuOpenHeightmapsDirButton;

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuHeightmapSlot(pub usize);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuResolutionButton {
    pub width: u32,
    pub height: u32,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuLanguageButton(pub(crate) crate::i18n::Locale);

#[derive(Component)]
pub(crate) struct MainMenuLanguageLabel;

#[derive(Component)]
pub(crate) struct MainMenuSummaryText;
