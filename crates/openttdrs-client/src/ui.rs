//! UI de información de tile seleccionado y menú de construcción (I6).

#[path = "ui_ingame_lifecycle.rs"]
mod ingame_lifecycle;

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;

mod ai_settings_window;
pub(crate) mod audio_settings_window;
mod autoreplace_window;
mod buy_window;
mod cargo_dist_settings_window;
mod cargo_payment_window;
mod cheat_window;
mod destination_window;
mod dev_console;
mod display_options_window;
mod endscreen;
mod extra_viewport_window;
mod finances_window;
mod floating_window;
pub(crate) mod font;
mod genland_window;
mod graph_window;
mod help_window;
mod hud;
mod industry_directory;
mod industry_panel;
mod list_window;
mod main_menu;
mod main_menu_intro;
mod menu;
mod navigation;
mod newgrf_window;
mod news_settings_window;
mod pathfinding_settings_window;
mod refit_window;
mod save_window;
mod shared_orders_window;
mod sign_list_window;
mod sparkline;
mod station_directory;
mod statusbar;
mod subsidy_list;
mod tile_inspector_window;
mod timetable_window;
mod toolbar;
mod town_directory;
mod town_window;
mod ui5_blocked_stubs;
#[cfg(test)]
mod ui_enum_inventory_test;
mod vehicle_list;
mod vehicle_window;
mod windows_shot;
use ai_settings_window::{
    AiSettingsWindowState, ai_settings_on_closed, handle_ai_settings_buttons,
    setup_ai_settings_window, sync_ai_settings_window,
};
use audio_settings_window::{
    SoundMusicWindowState, handle_audio_settings_buttons, handle_music_window_buttons,
    handle_sound_music_toolbar_button, handle_volume_sliders, setup_sound_music_window,
    sound_music_window_on_closed, sync_sound_music_window,
};
use autoreplace_window::{
    AutoreplaceWindowState, autoreplace_window_on_closed, handle_autoreplace_buttons,
    setup_autoreplace_window, sync_autoreplace_window,
};
use buy_window::{
    BuyVehicleWindowState, NewGrfTrainPreviewCache, buy_window_on_closed,
    buy_window_search_keyboard, handle_buy_window_buttons, setup_buy_window, sync_buy_window,
};
use cargo_dist_settings_window::{
    CargoDistSettingsWindowState, cargo_dist_settings_on_closed,
    handle_cargo_dist_settings_buttons, setup_cargo_dist_settings_window,
    sync_cargo_dist_settings_window,
};
use cargo_payment_window::{
    CargoPaymentWindowState, cargo_payment_window_on_closed, open_cargo_payment_from_routes,
    setup_cargo_payment_window, sync_cargo_payment_window,
};
use cheat_window::{
    CheatWindowState, cheat_window_on_closed, handle_cheat_window_buttons,
    handle_cheat_window_hotkey, setup_cheat_window, sync_cheat_window,
};
use destination_window::{
    DestinationPickerState, destination_picker_on_closed, handle_destination_picker_buttons,
    setup_destination_picker, sync_destination_picker,
};
use dev_console::{
    DevConsoleState, dev_console_window_on_closed, handle_dev_console_buttons,
    handle_dev_console_keyboard, setup_dev_console, sync_dev_console,
};
use display_options_window::{
    DisplayOptionsWindowState, display_options_window_on_closed, handle_display_options_buttons,
    setup_display_options_window, sync_display_options_window,
};
use endscreen::{
    EndScreenState, RetireGameRequested, handle_endscreen_menu_button, process_retire_game_request,
    setup_endscreen, sync_endscreen, watch_game_over_events,
};
use extra_viewport_window::{
    ExtraViewportWindowState, extra_viewport_window_on_closed, setup_extra_viewport_window,
    sync_extra_viewport_window,
};
use finances_window::{
    FinancesWindowState, finances_window_on_closed, handle_finances_window_buttons,
    handle_open_finances_window, open_finances_from_routes, setup_finances_window,
    sync_finances_window,
};
use genland_window::{
    GenLandWindowState, genland_window_on_closed, handle_genland_buttons, setup_genland_window,
    sync_genland_window,
};
use graph_window::{
    GraphWindowState, graph_window_on_closed, handle_graph_window_buttons, open_graph_from_routes,
    setup_graph_window, sync_graph_window,
};
use help_window::{
    HelpWindowState, handle_help_hotkey, help_window_on_closed, setup_help_window, sync_help_window,
};
pub(crate) use hud::SimHudControls;
use hud::{
    HoveredTileCoord, HudBuildFeedback, HudSfxHandles, PlayHudSfx, SelectedTileInfo,
    animate_build_place_flash, animate_income_popups, cycle_json_save_path_hotkey, flush_hud_sfx,
    handle_pause_toggle, handle_tool_hotkeys, load_hud_sfx, play_hud_sfx, setup_tile_info_ui,
    spawn_build_place_flash, spawn_income_popups, update_tile_info_text,
};
use industry_directory::{
    IndustryDirectoryState, handle_industry_directory_buttons, industry_directory_on_closed,
    industry_directory_search_keyboard, open_industry_directory_from_routes,
    setup_industry_directory, sync_industry_directory,
};
use industry_panel::{
    IndustryPanelState, industry_panel_center_interaction, industry_panel_close_interaction,
    setup_industry_panel, sync_industry_panel,
};
use main_menu::{
    apply_pending_heightmap_on_enter, auto_start_preloaded_json, main_menu_continue_interaction,
    main_menu_editor_interaction, main_menu_highscores_interaction, main_menu_interaction,
    main_menu_options_interaction, main_menu_preferences_interaction,
    main_menu_roughness_interaction, main_menu_scenarios_interaction, main_menu_sound_interaction,
    setup_main_menu, sync_main_menu_continue_button, sync_main_menu_heightmap_slots,
    sync_main_menu_highscores, sync_main_menu_panel_visibility, sync_main_menu_preferences,
    sync_main_menu_summary,
};
use main_menu_intro::{
    animate_main_menu_intro_traffic, cleanup_main_menu_on_exit, pan_main_menu_intro_camera,
    setup_main_menu_intro,
};
use navigation::{
    OpenUiRoute, ToolbarMenuState, dismiss_toolbar_menu_on_outside_click,
    handle_toolbar_menu_entries, handle_toolbar_menu_keyboard, handle_toolbar_navigation_button,
    sync_toolbar_navigation_menu,
};
use newgrf_window::{
    NewGrfWindowState, handle_newgrf_window_buttons, newgrf_window_on_closed, setup_newgrf_window,
    sync_newgrf_window,
};
use news_settings_window::{
    NewsSettingsWindowState, handle_news_settings_buttons, news_settings_on_closed,
    setup_news_settings_window, sync_news_settings_window,
};
use pathfinding_settings_window::{
    PathfindingSettingsWindowState, handle_pathfinding_settings_buttons,
    pathfinding_settings_on_closed, setup_pathfinding_settings_window,
    sync_pathfinding_settings_window,
};
use refit_window::{
    RefitWindowState, handle_refit_window_buttons, refit_window_on_closed, setup_refit_window,
    sync_refit_window,
};
pub(crate) use save_window::SaveWindowState;
use save_window::{
    handle_save_load_toolbar_buttons, handle_save_window_buttons, prepare_save_window_name,
    save_window_editable_keyboard, save_window_keyboard, save_window_name_click_focus,
    setup_save_window, sync_save_window,
};
use shared_orders_window::{
    SharedOrdersWindowState, handle_shared_orders_buttons, setup_shared_orders_window,
    shared_orders_window_on_closed, sync_shared_orders_window,
};
use sign_list_window::{
    SignListWindowState, handle_sign_list_body_click, handle_sign_list_buttons,
    open_sign_list_from_routes, setup_sign_list_window, sign_list_rename_keyboard,
    sign_list_window_on_closed, sync_sign_list_window,
};
use station_directory::{
    StationDirectoryState, handle_station_directory_buttons, open_station_directory_from_routes,
    setup_station_directory, station_directory_on_closed, sync_station_directory,
};
use statusbar::{
    NewsHistoryState, NewsUiState, drain_news_events, handle_news_history_row_click,
    handle_news_popup_close, handle_news_popup_focus, handle_open_news_history,
    handle_status_bar_center_click, news_history_on_closed, setup_news_history_window,
    setup_status_bar, sync_news_history_window, sync_status_bar, update_news_playback,
};
use subsidy_list::{
    SubsidyListState, handle_subsidy_list_buttons, open_subsidy_list_from_routes,
    setup_subsidy_list, subsidy_list_on_closed, sync_subsidy_list,
};
use tile_inspector_window::{
    TileInspectorWindowState, draw_selected_tile_bounds, handle_tile_inspector_hotkey,
    setup_tile_inspector_window, sync_tile_inspector_window, tile_inspector_window_on_closed,
};
use timetable_window::{
    TimetableWindowState, handle_timetable_window_buttons, setup_timetable_window,
    sync_timetable_window, timetable_window_on_closed,
};
use toolbar::depot_panel_on_closed;
use toolbar::{
    BridgeBuildState, DepotPanelState, DragBuildState, EditorTownMenuState, MinimapLayerState,
    NewGrfRoadTypePreviewCache, NewGrfStationPreviewCache, RailSignalGhostState,
    RoadTypeEscapeConsumed, RoadTypePickerState, StationBuildState, StationCargoPanelState,
    StationCatalogPickerState, UiToolState, airport_picker_on_closed, begin_depot_list_drag,
    bridge_picker_on_closed, build_menu_interaction, close_road_type_picker_on_escape,
    close_toolbar_button_interaction, finish_depot_list_drag, handle_airport_picker_buttons,
    handle_bridge_picker_buttons, handle_cheats_menu_button, handle_company_colour_swatches,
    handle_company_selector_buttons, handle_depot_panel_buttons,
    handle_editor_toolbar_build_buttons, handle_editor_toolbar_control_buttons,
    handle_editor_toolbar_tool_buttons, handle_editor_town_dropdown, handle_ingame_escape,
    handle_minimap_click, handle_minimap_layer_buttons, handle_order_panel_buttons,
    handle_rail_station_picker_buttons, handle_rail_type_select_buttons,
    handle_road_type_class_buttons, handle_road_type_select_buttons, handle_settings_menu_buttons,
    handle_signal_picker_buttons, handle_station_cargo_panel_buttons,
    handle_station_catalog_open_buttons, handle_station_class_select_buttons,
    handle_station_rename_buttons, handle_station_spec_select_buttons, handle_tile_click,
    hide_tool_when_panel_closed, lerp_ghost_previews, rail_station_picker_on_closed,
    road_type_filter_keyboard, rotate_station_with_right_click, setup_airport_picker,
    setup_bridge_picker, setup_build_menu, setup_depot_panel, setup_editor_toolbar, setup_minimap,
    setup_order_panel, setup_rail_station_picker, setup_signal_picker, setup_station_cargo_panel,
    setup_top_toolbar, signal_picker_on_closed, station_catalog_filter_keyboard,
    station_rename_editable_keyboard, station_rename_keyboard, sync_airport_picker,
    sync_bridge_picker, sync_build_pointer_modifiers, sync_climate_industry_tools,
    sync_company_colour_swatch_visuals, sync_company_selector, sync_depot_panel,
    sync_editor_only_build_tools, sync_editor_toolbar_button_visuals, sync_editor_toolbar_date,
    sync_editor_toolbar_visibility, sync_editor_town_dropdown, sync_minimap, sync_order_panel,
    sync_orders_pick_cursor, sync_rail_station_picker, sync_rail_toolbar_icons,
    sync_rail_type_select_visuals, sync_road_type_catalog_entries, sync_road_type_class_labels,
    sync_road_type_entry_previews, sync_road_type_entry_visibility, sync_road_type_popovers,
    sync_signal_picker, sync_station_cargo_panel, sync_station_catalog_entries,
    sync_station_spec_entry_previews, toolbar_click_beep, toolbar_group_interaction,
    update_build_ghost_preview, update_cursor_tile, update_tool_button_visuals,
    update_toolbar_group_visuals, update_toolbar_tool_visibility, update_toolbar_tooltip,
};
pub(crate) use toolbar::{BuildMenuAction, OrderEditState, ToolbarState};
use town_directory::{
    TownDirectoryState, handle_town_directory_buttons, open_town_directory_from_routes,
    setup_town_directory, sync_town_directory, town_directory_on_closed,
    town_directory_search_keyboard,
};
use town_window::{
    TownWindowState, handle_town_window_buttons, setup_town_window, sync_town_window,
    town_window_on_closed,
};
pub(crate) use ui5_blocked_stubs::{LinkGraphView, LinkGraphWindowState};
use ui5_blocked_stubs::{
    handle_link_graph_filter_button, handle_link_graph_view_button, link_graph_window_on_closed,
    open_link_graph_from_routes, setup_link_graph_window, sync_link_graph_window,
};
use vehicle_list::{
    VehicleListState, handle_vehicle_list_buttons, open_vehicle_list_from_routes,
    setup_vehicle_list, sync_vehicle_list, vehicle_list_on_closed,
};
use vehicle_window::{
    VehicleWindowState, handle_vehicle_rename_buttons, handle_vehicle_window_buttons,
    setup_vehicle_window, sync_vehicle_window, vehicle_window_on_closed,
    vehicle_window_rename_editable_keyboard, vehicle_window_rename_keyboard,
};
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            floating_window::FloatingWindowPlugin,
            windows_shot::WindowsShotPlugin,
            ingame_lifecycle::InGameLifecyclePlugin,
        ))
        .init_resource::<NewsUiState>()
        .init_resource::<NewsHistoryState>()
        .init_resource::<FinancesWindowState>()
        .init_resource::<GraphWindowState>()
        .init_resource::<CargoPaymentWindowState>()
        .init_resource::<NewsSettingsWindowState>()
        .init_resource::<DisplayOptionsWindowState>()
        .init_resource::<ExtraViewportWindowState>()
        .init_resource::<SignListWindowState>()
        .init_resource::<LinkGraphWindowState>()
        .init_resource::<PathfindingSettingsWindowState>()
        .init_resource::<CargoDistSettingsWindowState>()
        .init_resource::<AiSettingsWindowState>()
        .init_resource::<NewGrfWindowState>()
        .init_resource::<HelpWindowState>()
        .init_resource::<DevConsoleState>()
        .init_resource::<TileInspectorWindowState>()
        .init_resource::<CheatWindowState>()
        .init_resource::<GenLandWindowState>()
        .init_resource::<EditorTownMenuState>()
        .init_resource::<EndScreenState>()
        .init_resource::<RetireGameRequested>()
        .init_resource::<SoundMusicWindowState>()
        .init_resource::<crate::news_prefs::NewsDisplayPrefs>()
        .init_resource::<SelectedTileInfo>()
        .init_resource::<HoveredTileCoord>()
        .init_resource::<SimHudControls>()
        .init_resource::<HudBuildFeedback>()
        .init_resource::<HudSfxHandles>()
        .init_resource::<RailSignalGhostState>()
        .add_message::<PlayHudSfx>()
        .init_resource::<UiToolState>()
        .init_resource::<StationBuildState>()
        .init_resource::<StationCatalogPickerState>()
        .init_resource::<NewGrfStationPreviewCache>()
        .init_resource::<RoadTypePickerState>()
        .init_resource::<RoadTypeEscapeConsumed>()
        .init_resource::<NewGrfRoadTypePreviewCache>()
        .init_resource::<DragBuildState>()
        .init_resource::<BridgeBuildState>()
        .init_resource::<OrderEditState>()
        .init_resource::<DepotPanelState>()
        .init_resource::<StationCargoPanelState>()
        .init_resource::<ToolbarState>()
        .init_resource::<MinimapLayerState>()
        .init_resource::<IndustryPanelState>()
        .init_resource::<IndustryDirectoryState>()
        .init_resource::<SaveWindowState>()
        .init_resource::<TownWindowState>()
        .init_resource::<TownDirectoryState>()
        .init_resource::<StationDirectoryState>()
        .init_resource::<VehicleListState>()
        .init_resource::<SubsidyListState>()
        .init_resource::<BuyVehicleWindowState>()
        .init_resource::<NewGrfTrainPreviewCache>()
        .init_resource::<DestinationPickerState>()
        .init_resource::<VehicleWindowState>()
        .init_resource::<RefitWindowState>()
        .init_resource::<SharedOrdersWindowState>()
        .init_resource::<AutoreplaceWindowState>()
        .init_resource::<TimetableWindowState>()
        .init_resource::<ToolbarMenuState>()
        .add_message::<OpenUiRoute>()
        .init_resource::<crate::state::new_game::NewGameSettingsResource>()
        .add_systems(OnExit(ClientScreen::MainMenu), cleanup_main_menu_on_exit)
        .add_systems(
            OnEnter(ClientScreen::MainMenu),
            (
                setup_main_menu_intro,
                setup_main_menu,
                setup_save_window,
                setup_sound_music_window,
                load_hud_sfx,
            ),
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            apply_pending_heightmap_on_enter,
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            (
                setup_tile_info_ui,
                setup_status_bar,
                setup_news_history_window,
                setup_finances_window,
                setup_news_settings_window,
                setup_pathfinding_settings_window,
                setup_sound_music_window,
                setup_top_toolbar,
                setup_build_menu,
                setup_minimap,
                setup_order_panel,
                setup_depot_panel,
                setup_station_cargo_panel,
                setup_rail_station_picker,
                setup_bridge_picker,
                setup_industry_panel,
                setup_save_window,
                setup_town_window,
                setup_buy_window,
                setup_destination_picker,
            )
                .chain()
                .in_set(StartupSet::Ui),
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            (setup_editor_toolbar, setup_genland_window).in_set(StartupSet::Ui),
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            (
                setup_signal_picker,
                setup_airport_picker,
                setup_ai_settings_window,
                setup_cargo_dist_settings_window,
            )
                .in_set(StartupSet::Ui),
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            (
                setup_graph_window,
                setup_cargo_payment_window,
                setup_display_options_window,
                setup_extra_viewport_window,
                setup_sign_list_window,
                setup_link_graph_window,
                setup_newgrf_window,
                setup_help_window,
                setup_vehicle_window,
                setup_refit_window,
                setup_shared_orders_window,
                setup_autoreplace_window,
                setup_timetable_window,
                setup_town_directory,
                setup_industry_directory,
                setup_station_directory,
                setup_vehicle_list,
                setup_subsidy_list,
                load_hud_sfx,
            )
                .in_set(StartupSet::Ui),
        )
        .add_systems(
            OnEnter(ClientScreen::InGame),
            (
                setup_dev_console,
                setup_tile_inspector_window,
                setup_cheat_window,
                setup_endscreen,
            )
                .in_set(StartupSet::Ui),
        )
        .add_systems(
            Update,
            (
                pan_main_menu_intro_camera,
                animate_main_menu_intro_traffic,
                auto_start_preloaded_json,
                (main_menu_interaction, main_menu_continue_interaction).chain(),
                main_menu_editor_interaction,
                main_menu_highscores_interaction,
                main_menu_scenarios_interaction,
                main_menu_preferences_interaction,
                main_menu_sound_interaction,
                main_menu_options_interaction,
                main_menu_roughness_interaction,
                sync_main_menu_panel_visibility,
                sync_main_menu_summary,
                sync_main_menu_continue_button,
                sync_main_menu_highscores,
                sync_main_menu_heightmap_slots,
                sync_main_menu_preferences,
            )
                .run_if(in_state(ClientScreen::MainMenu)),
        )
        .add_systems(
            Update,
            (
                handle_audio_settings_buttons,
                handle_volume_sliders,
                handle_music_window_buttons,
                sound_music_window_on_closed,
                sync_sound_music_window,
                toolbar_click_beep,
                play_hud_sfx,
            )
                .run_if(in_state(ClientScreen::MainMenu)),
        )
        .add_systems(
            Update,
            (
                save_window_keyboard,
                save_window_editable_keyboard,
                save_window_name_click_focus,
                handle_save_window_buttons,
                sync_save_window,
                prepare_save_window_name,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::MainMenu)),
        )
        .add_systems(
            Update,
            (
                save_window_keyboard,
                save_window_editable_keyboard,
                save_window_name_click_focus,
                handle_pause_toggle,
                cycle_json_save_path_hotkey,
                handle_help_hotkey,
                handle_tile_inspector_hotkey,
                handle_dev_console_keyboard,
                handle_cheat_window_hotkey,
                handle_tool_hotkeys,
                rotate_station_with_right_click,
                close_road_type_picker_on_escape,
                handle_ingame_escape,
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
                handle_cheats_menu_button,
                handle_company_colour_swatches,
                handle_company_selector_buttons,
                sync_company_colour_swatch_visuals,
                sync_company_selector,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                sync_editor_toolbar_visibility,
                sync_editor_toolbar_date,
                sync_editor_toolbar_button_visuals,
                sync_editor_town_dropdown,
                handle_editor_toolbar_control_buttons,
                handle_editor_toolbar_tool_buttons,
                handle_editor_toolbar_build_buttons,
                handle_editor_town_dropdown,
                genland_window_on_closed,
                sync_genland_window,
                handle_genland_buttons,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
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
            (begin_depot_list_drag, finish_depot_list_drag)
                .chain()
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                industry_panel_center_interaction,
                handle_minimap_layer_buttons,
                sync_climate_industry_tools,
                sync_editor_only_build_tools,
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
                buy_window_search_keyboard,
                buy_window_on_closed,
                handle_destination_picker_buttons,
                destination_picker_on_closed,
                depot_panel_on_closed,
                handle_vehicle_window_buttons,
                handle_vehicle_rename_buttons,
                vehicle_window_rename_keyboard,
                vehicle_window_rename_editable_keyboard,
                vehicle_window_on_closed,
                handle_timetable_window_buttons,
                timetable_window_on_closed,
                handle_rail_station_picker_buttons,
                rail_station_picker_on_closed,
                handle_bridge_picker_buttons,
                bridge_picker_on_closed,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_station_catalog_open_buttons,
                handle_station_class_select_buttons,
                handle_station_spec_select_buttons,
                station_catalog_filter_keyboard,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_signal_picker_buttons,
                signal_picker_on_closed,
                handle_airport_picker_buttons,
                airport_picker_on_closed,
                handle_rail_type_select_buttons,
                handle_road_type_class_buttons,
                handle_road_type_select_buttons,
                road_type_filter_keyboard,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_station_rename_buttons,
                station_rename_keyboard,
                station_rename_editable_keyboard,
                handle_refit_window_buttons,
                refit_window_on_closed,
                handle_shared_orders_buttons,
                shared_orders_window_on_closed,
                handle_autoreplace_buttons,
                autoreplace_window_on_closed,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            sync_build_pointer_modifiers
                .after(build_menu_interaction)
                .after(hide_tool_when_panel_closed)
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            draw_selected_tile_bounds
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                update_cursor_tile,
                update_build_ghost_preview,
                lerp_ghost_previews,
                handle_tile_click,
                spawn_build_place_flash,
                flush_hud_sfx,
            )
                .chain()
                .after(sync_build_pointer_modifiers)
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
                handle_finances_window_buttons,
                handle_news_settings_buttons,
                news_settings_on_closed,
                sync_news_settings_window,
                handle_pathfinding_settings_buttons,
                pathfinding_settings_on_closed,
                sync_pathfinding_settings_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_cargo_dist_settings_buttons,
                cargo_dist_settings_on_closed,
                sync_cargo_dist_settings_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_ai_settings_buttons,
                ai_settings_on_closed,
                sync_ai_settings_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                graph_window_on_closed,
                cargo_payment_window_on_closed,
                handle_graph_window_buttons,
                sync_graph_window,
                sync_cargo_payment_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                handle_display_options_buttons,
                display_options_window_on_closed,
                sync_display_options_window,
                extra_viewport_window_on_closed,
                sync_extra_viewport_window,
                open_sign_list_from_routes,
                open_link_graph_from_routes,
                sign_list_window_on_closed,
                link_graph_window_on_closed,
                sync_sign_list_window,
                sync_link_graph_window,
                handle_link_graph_filter_button,
                handle_link_graph_view_button,
                handle_sign_list_buttons,
                handle_sign_list_body_click,
                sign_list_rename_keyboard,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                newgrf_window_on_closed,
                sync_newgrf_window,
                handle_newgrf_window_buttons,
                help_window_on_closed,
                sync_help_window,
                process_retire_game_request,
                watch_game_over_events,
                sync_endscreen,
                handle_endscreen_menu_button,
                dev_console_window_on_closed,
                sync_dev_console,
                handle_dev_console_buttons,
                tile_inspector_window_on_closed,
                sync_tile_inspector_window,
                handle_sound_music_toolbar_button,
                handle_audio_settings_buttons,
                handle_volume_sliders,
                handle_music_window_buttons,
                sound_music_window_on_closed,
                sync_sound_music_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                cheat_window_on_closed,
                sync_cheat_window,
                handle_cheat_window_buttons,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                spawn_income_popups,
                animate_income_popups,
                animate_build_place_flash,
                sync_minimap,
                sync_order_panel,
                sync_orders_pick_cursor,
                sync_depot_panel,
                sync_station_cargo_panel,
                sync_industry_panel,
                sync_town_window,
                sync_buy_window,
                sync_destination_picker,
                sync_rail_station_picker,
                sync_station_catalog_entries,
                sync_bridge_picker,
                sync_vehicle_window,
                sync_timetable_window,
                toolbar_click_beep,
                play_hud_sfx,
                update_tile_info_text,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                sync_signal_picker,
                sync_airport_picker,
                sync_rail_type_select_visuals,
                sync_rail_toolbar_icons,
                sync_road_type_popovers,
                sync_road_type_entry_visibility,
                sync_road_type_catalog_entries,
                sync_road_type_entry_previews,
                sync_road_type_class_labels,
                sync_station_spec_entry_previews,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                sync_refit_window,
                sync_shared_orders_window,
                sync_autoreplace_window,
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        )
        .add_systems(
            Update,
            (
                (
                    handle_toolbar_navigation_button,
                    handle_toolbar_menu_entries,
                    handle_toolbar_menu_keyboard,
                    dismiss_toolbar_menu_on_outside_click,
                    sync_toolbar_navigation_menu,
                )
                    .chain(),
                (
                    open_town_directory_from_routes,
                    handle_town_directory_buttons,
                    town_directory_search_keyboard,
                    town_directory_on_closed,
                    sync_town_directory,
                )
                    .chain()
                    .after(handle_toolbar_menu_entries),
                (
                    open_industry_directory_from_routes,
                    handle_industry_directory_buttons,
                    industry_directory_search_keyboard,
                    industry_directory_on_closed,
                    sync_industry_directory,
                )
                    .chain()
                    .after(handle_toolbar_menu_entries),
                (
                    open_station_directory_from_routes,
                    handle_station_directory_buttons,
                    station_directory_on_closed,
                    sync_station_directory,
                )
                    .chain()
                    .after(handle_toolbar_menu_entries),
                (
                    open_vehicle_list_from_routes,
                    handle_vehicle_list_buttons,
                    vehicle_list_on_closed,
                    sync_vehicle_list,
                )
                    .chain()
                    .after(handle_toolbar_menu_entries),
                (
                    open_subsidy_list_from_routes,
                    handle_subsidy_list_buttons,
                    subsidy_list_on_closed,
                    sync_subsidy_list,
                )
                    .chain()
                    .after(handle_toolbar_menu_entries),
                (
                    open_finances_from_routes,
                    open_graph_from_routes,
                    open_cargo_payment_from_routes,
                )
                    .after(handle_toolbar_menu_entries),
            )
                .in_set(UpdateSet::Ui)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}
