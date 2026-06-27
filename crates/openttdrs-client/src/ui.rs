//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;

mod buy_window;
mod finances_window;
mod floating_window;
pub(crate) mod font;
mod hud;
mod industry_panel;
mod main_menu;
mod save_window;
mod statusbar;
mod toolbar;
mod town_window;
mod vehicle_window;
mod windows_shot;
use buy_window::{
    BuyVehicleWindowState, buy_window_on_closed, handle_buy_window_buttons, setup_buy_window,
    sync_buy_window,
};
use finances_window::{
    FinancesWindowState, finances_window_on_closed, handle_open_finances_window,
    setup_finances_window, sync_finances_window,
};
pub(crate) use hud::SimHudControls;
use hud::{
    HoveredTileCoord, HudBuildFeedback, HudSfxHandles, PlayHudSfx, SelectedTileInfo,
    animate_income_popups, cycle_json_save_path_hotkey, flush_hud_sfx, handle_pause_toggle,
    handle_tool_hotkeys, load_hud_sfx, play_hud_sfx, setup_tile_info_ui, spawn_income_popups,
    update_tile_info_text,
};
use industry_panel::{
    IndustryPanelState, industry_panel_close_interaction, setup_industry_panel, sync_industry_panel,
};
use main_menu::{main_menu_interaction, setup_main_menu, setup_main_menu_camera};
pub(crate) use save_window::SaveWindowState;
use save_window::{
    handle_save_load_toolbar_buttons, handle_save_window_buttons, prepare_save_window_name,
    save_window_editable_keyboard, save_window_keyboard, save_window_name_click_focus,
    setup_save_window, sync_save_window,
};
use statusbar::{
    NewsHistoryState, NewsUiState, drain_news_events, handle_news_history_row_click,
    handle_news_popup_close, handle_news_popup_focus, handle_open_news_history,
    handle_status_bar_center_click, news_history_on_closed, setup_news_history_window,
    setup_status_bar, sync_news_history_window, sync_status_bar, update_news_playback,
};
use toolbar::depot_panel_on_closed;
pub(crate) use toolbar::{BuildMenuAction, OrderEditState};
use toolbar::{
    DepotPanelState, DragBuildState, StationBuildState, StationCargoPanelState, ToolbarState,
    UiToolState, build_menu_interaction, close_toolbar_button_interaction,
    close_toolbar_panel_on_escape, handle_company_colour_swatches, handle_depot_panel_buttons,
    handle_minimap_click, handle_order_panel_buttons, handle_rail_station_picker_buttons,
    handle_settings_menu_buttons, handle_station_cargo_panel_buttons, handle_tile_click,
    hide_tool_when_panel_closed, rail_station_picker_on_closed, rotate_station_with_right_click,
    setup_build_menu, setup_depot_panel, setup_minimap, setup_order_panel,
    setup_rail_station_picker, setup_station_cargo_panel, setup_top_toolbar,
    sync_company_colour_swatch_visuals, sync_depot_panel, sync_minimap, sync_order_panel,
    sync_orders_pick_cursor, sync_rail_station_picker, sync_station_cargo_panel,
    toolbar_group_interaction, update_build_ghost_preview, update_cursor_tile,
    update_tool_button_visuals, update_toolbar_group_visuals, update_toolbar_tool_visibility,
    update_toolbar_tooltip,
};
use town_window::{
    TownWindowState, handle_town_window_buttons, setup_town_window, sync_town_window,
    town_window_on_closed,
};
use vehicle_window::{
    VehicleWindowState, handle_vehicle_window_buttons, setup_vehicle_window, sync_vehicle_window,
    vehicle_window_on_closed,
};
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            floating_window::FloatingWindowPlugin,
            windows_shot::WindowsShotPlugin,
        ))
        .init_resource::<NewsUiState>()
        .init_resource::<NewsHistoryState>()
        .init_resource::<FinancesWindowState>()
        .init_resource::<SelectedTileInfo>()
        .init_resource::<HoveredTileCoord>()
        .init_resource::<SimHudControls>()
        .init_resource::<HudBuildFeedback>()
        .init_resource::<HudSfxHandles>()
        .add_message::<PlayHudSfx>()
        .init_resource::<UiToolState>()
        .init_resource::<StationBuildState>()
        .init_resource::<DragBuildState>()
        .init_resource::<OrderEditState>()
        .init_resource::<DepotPanelState>()
        .init_resource::<StationCargoPanelState>()
        .init_resource::<ToolbarState>()
        .init_resource::<IndustryPanelState>()
        .init_resource::<SaveWindowState>()
        .init_resource::<TownWindowState>()
        .init_resource::<BuyVehicleWindowState>()
        .init_resource::<VehicleWindowState>()
        .add_systems(
            OnEnter(ClientScreen::MainMenu),
            (setup_main_menu_camera, setup_main_menu),
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            (
                setup_tile_info_ui,
                setup_status_bar,
                setup_news_history_window,
                setup_finances_window,
                setup_top_toolbar,
                setup_build_menu,
                setup_minimap,
                setup_order_panel,
                setup_depot_panel,
                setup_station_cargo_panel,
                setup_rail_station_picker,
                setup_industry_panel,
                setup_save_window,
                setup_town_window,
                setup_buy_window,
                setup_vehicle_window,
                setup_rail_station_picker,
                load_hud_sfx,
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
                save_window_keyboard,
                save_window_editable_keyboard,
                save_window_name_click_focus,
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
                handle_depot_panel_buttons,
                handle_station_cargo_panel_buttons,
                handle_settings_menu_buttons,
                handle_company_colour_swatches,
                sync_company_colour_swatch_visuals,
                handle_save_load_toolbar_buttons,
                handle_save_window_buttons,
                sync_save_window,
                prepare_save_window_name,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_town_window_buttons,
                town_window_on_closed,
                handle_buy_window_buttons,
                buy_window_on_closed,
                depot_panel_on_closed,
                handle_vehicle_window_buttons,
                vehicle_window_on_closed,
                handle_rail_station_picker_buttons,
                rail_station_picker_on_closed,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (update_cursor_tile, handle_tile_click, flush_hud_sfx)
                .chain()
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                drain_news_events,
                update_news_playback.after(drain_news_events),
                sync_status_bar.after(update_news_playback),
                handle_status_bar_center_click.after(sync_status_bar),
                handle_news_popup_close.after(update_news_playback),
                handle_news_popup_focus.after(update_news_playback),
                handle_open_news_history,
                handle_open_finances_window,
                handle_news_history_row_click,
                news_history_on_closed,
                finances_window_on_closed,
                sync_news_history_window,
                sync_finances_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                spawn_income_popups,
                animate_income_popups,
                update_build_ghost_preview,
                sync_minimap,
                sync_order_panel,
                sync_orders_pick_cursor,
                sync_depot_panel,
                sync_station_cargo_panel,
                sync_industry_panel,
                sync_town_window,
                sync_buy_window,
                sync_rail_station_picker,
                sync_vehicle_window,
                play_hud_sfx,
                update_tile_info_text,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}
