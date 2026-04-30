//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;

mod hud;
mod industry_panel;
mod main_menu;
mod toolbar;
pub(crate) use hud::SimHudControls;
use hud::{
    SelectedTileInfo, cycle_json_save_path_hotkey, handle_pause_toggle, handle_tool_hotkeys,
    setup_tile_info_ui, update_tile_info_text,
};
use industry_panel::{
    IndustryPanelState, industry_panel_close_interaction, setup_industry_panel, sync_industry_panel,
};
use main_menu::{main_menu_interaction, setup_main_menu, setup_main_menu_camera};
pub(crate) use toolbar::{BuildMenuAction, OrderEditState};
use toolbar::{
    DragBuildState, StationBuildState, ToolbarState, UiToolState, build_menu_interaction,
    close_toolbar_button_interaction, close_toolbar_panel_on_escape, handle_minimap_click,
    handle_order_panel_buttons, handle_save_menu_buttons, handle_tile_click,
    hide_tool_when_panel_closed, rotate_station_with_right_click, setup_build_menu, setup_minimap,
    setup_order_panel, setup_top_toolbar, sync_minimap, sync_order_panel,
    toolbar_group_interaction, update_build_ghost_preview, update_tool_button_visuals,
    update_toolbar_group_visuals, update_toolbar_tool_visibility, update_toolbar_tooltip,
};
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedTileInfo>()
            .init_resource::<SimHudControls>()
            .init_resource::<UiToolState>()
            .init_resource::<StationBuildState>()
            .init_resource::<DragBuildState>()
            .init_resource::<OrderEditState>()
            .init_resource::<ToolbarState>()
            .init_resource::<IndustryPanelState>()
            .add_systems(
                OnEnter(ClientScreen::MainMenu),
                (setup_main_menu_camera, setup_main_menu),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_tile_info_ui,
                    setup_top_toolbar,
                    setup_build_menu,
                    setup_minimap,
                    setup_order_panel,
                    setup_industry_panel,
                )
                    .in_set(StartupSet::Ui),
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
                    rotate_station_with_right_click,
                    close_toolbar_panel_on_escape,
                )
                    .in_set(UpdateSet::Input)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    toolbar_group_interaction,
                    close_toolbar_button_interaction,
                    build_menu_interaction,
                    update_toolbar_group_visuals,
                    update_toolbar_tool_visibility,
                    hide_tool_when_panel_closed,
                    update_tool_button_visuals,
                    update_toolbar_tooltip,
                    industry_panel_close_interaction,
                    handle_minimap_click,
                    handle_order_panel_buttons,
                    handle_save_menu_buttons,
                    handle_tile_click,
                    update_build_ghost_preview,
                    sync_minimap,
                    sync_order_panel,
                    sync_industry_panel,
                    update_tile_info_text,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
