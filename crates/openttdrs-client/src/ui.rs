//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;

mod hud;
mod main_menu;
mod toolbar;
use hud::{
    SelectedTileInfo, cycle_json_save_path_hotkey, handle_pause_toggle, handle_tool_hotkeys,
    setup_tile_info_ui, update_tile_info_text,
};
use main_menu::{main_menu_interaction, setup_main_menu, setup_main_menu_camera};
use toolbar::{
    ToolbarState, UiToolState, build_menu_interaction, handle_tile_click, setup_build_menu,
    setup_top_toolbar, toolbar_group_interaction, update_tool_button_visuals,
    update_toolbar_group_visuals, update_toolbar_tool_visibility, update_toolbar_tooltip,
};
pub(crate) use hud::SimHudControls;
pub(crate) use toolbar::BuildMenuAction;
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedTileInfo>()
            .init_resource::<SimHudControls>()
            .init_resource::<UiToolState>()
            .init_resource::<ToolbarState>()
            .add_systems(
                OnEnter(ClientScreen::MainMenu),
                (setup_main_menu_camera, setup_main_menu),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (setup_tile_info_ui, setup_top_toolbar, setup_build_menu).in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                main_menu_interaction.run_if(in_state(ClientScreen::MainMenu)),
            )
            .add_systems(
                Update,
                (
                    handle_pause_toggle,
                    cycle_json_save_path_hotkey,
                    handle_tool_hotkeys,
                )
                    .in_set(UpdateSet::Input)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    toolbar_group_interaction,
                    build_menu_interaction,
                    update_toolbar_group_visuals,
                    update_toolbar_tool_visibility,
                    update_tool_button_visuals,
                    update_toolbar_tooltip,
                    handle_tile_click,
                    update_tile_info_text,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

