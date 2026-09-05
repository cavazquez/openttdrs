//! Plugin de UI para toolbar, menú de construcción, minimapa, depot, órdenes.

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;
use crate::ui::hotkeys::{
    UiHotkeys, dispatch_ui_hotkeys, handle_toolbar_command_hotkeys, handle_zoom_hotkeys,
};
use crate::ui::hud::{
    cycle_json_save_path_hotkey, handle_hud_toggle, handle_pause_toggle, handle_tool_hotkeys,
};
use crate::ui::industry_panel::{
    IndustryPanelState, industry_panel_center_interaction, industry_panel_on_closed,
    setup_industry_panel, sync_industry_panel,
};
use crate::ui::industry_production_window::{
    IndustryProductionWindowState, industry_production_window_on_closed,
    setup_industry_production_window, sync_industry_production_window,
};
use crate::ui::save_window::{
    SaveWindowState, handle_save_window_buttons, prepare_save_window_name,
    save_window_editable_keyboard, save_window_keyboard, save_window_name_click_focus,
    setup_save_window, sync_save_window,
};
use crate::ui::station_pool::StationPoolRegistry;
use crate::ui::toolbar::{
    BridgeBuildState, DepotPanelState, DragBuildState, MinimapLayerState,
    NewGrfRoadTypePreviewCache, NewGrfStationPreviewCache, OrderEditState, RoadTypeEscapeConsumed,
    RoadTypePickerState, StationBuildState, StationCargoPanelState, StationCatalogPickerState,
    ToolbarLayoutState, ToolbarState, UiToolState, airport_picker_on_closed, begin_depot_list_drag,
    begin_order_list_drag, bridge_picker_on_closed, build_menu_interaction, buoy_picker_on_closed,
    close_road_type_picker_on_escape, close_toolbar_button_interaction,
    depot_build_picker_on_closed, depot_panel_on_closed, dock_picker_on_closed,
    finish_depot_list_drag, finish_order_list_drag, handle_airport_picker_buttons,
    handle_bridge_picker_buttons, handle_cheats_menu_button, handle_company_colour_swatches,
    handle_company_selector_buttons, handle_depot_build_picker_buttons, handle_depot_panel_buttons,
    handle_dock_picker_buttons, handle_freight_trains_menu_button, handle_ingame_escape,
    handle_minimap_click, handle_minimap_layer_buttons, handle_object_picker_buttons,
    handle_order_panel_buttons, handle_rail_station_picker_buttons,
    handle_rail_type_select_buttons, handle_road_driving_side_menu_button,
    handle_road_stop_picker_buttons, handle_road_type_class_buttons,
    handle_road_type_select_buttons, handle_settings_menu_buttons, handle_settings_zoom_buttons,
    handle_signal_picker_buttons, handle_station_cargo_panel_buttons,
    handle_station_catalog_open_buttons, handle_station_class_select_buttons,
    handle_station_rename_buttons, handle_station_spec_select_buttons, handle_tile_click,
    handle_toolbar_switch, handle_vehicle_breakdowns_menu_button, hide_tool_when_panel_closed,
    lerp_ghost_previews, object_picker_on_closed, order_panel_on_closed,
    rail_station_picker_on_closed, rail_waypoint_picker_on_closed, road_stop_picker_on_closed,
    road_type_filter_keyboard, road_waypoint_picker_on_closed, rotate_station_with_right_click,
    setup_airport_picker, setup_bridge_picker, setup_build_menu, setup_buoy_picker,
    setup_depot_build_picker, setup_depot_panel, setup_dock_picker, setup_minimap,
    setup_object_picker, setup_order_panel, setup_rail_station_picker, setup_rail_waypoint_picker,
    setup_road_stop_picker, setup_road_waypoint_picker, setup_sign_picker, setup_signal_picker,
    setup_station_cargo_panel, setup_terraform_picker, setup_top_toolbar, setup_tree_picker,
    sign_picker_on_closed, signal_picker_on_closed, station_catalog_filter_keyboard,
    station_rename_editable_keyboard, station_rename_keyboard, station_view_on_closed,
    sync_action5_gui_toolbar_icons, sync_airport_picker, sync_airport_preview_image,
    sync_bridge_picker, sync_build_pointer_modifiers, sync_buoy_picker,
    sync_climate_industry_tools, sync_company_colour_swatch_visuals, sync_company_selector,
    sync_depot_build_picker, sync_depot_panel, sync_dock_picker, sync_editor_only_build_tools,
    sync_freight_trains_button_label, sync_minimap, sync_object_catalog_entries,
    sync_object_picker, sync_object_preview_image, sync_order_panel, sync_orders_pick_cursor,
    sync_rail_station_picker, sync_rail_toolbar_icons, sync_rail_type_select_visuals,
    sync_rail_waypoint_picker, sync_road_driving_side_button_label, sync_road_stop_catalog_entries,
    sync_road_stop_picker, sync_road_stop_preview_image, sync_road_type_catalog_entries,
    sync_road_type_class_labels, sync_road_type_entry_previews, sync_road_type_entry_visibility,
    sync_road_type_popovers, sync_road_waypoint_picker, sync_sign_picker, sync_signal_picker,
    sync_station_cargo_panel, sync_station_catalog_entries, sync_station_spec_entry_previews,
    sync_terraform_picker, sync_toolbar_layout, sync_tree_picker,
    sync_vehicle_breakdowns_button_label, terraform_picker_on_closed, toolbar_group_interaction,
    tree_picker_on_closed, update_build_ghost_preview, update_cursor_tile,
    update_tool_button_visuals, update_toolbar_group_visuals, update_toolbar_tool_visibility,
    update_toolbar_tooltip,
};
use crate::ui::town_authority_window::{
    TownAuthorityEffectWatch, TownAuthorityWindowState, handle_town_authority_buttons,
    observe_town_authority_effects, setup_town_authority_window, sync_town_authority_window,
    town_authority_window_on_closed,
};
use crate::ui::town_window::{
    TownWindowState, handle_town_window_buttons, setup_town_window, sync_town_window,
    town_window_on_closed,
};

pub(crate) struct ToolbarUiPlugin;

impl Plugin for ToolbarUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiToolState>()
            .init_resource::<UiHotkeys>()
            .init_resource::<ToolbarLayoutState>()
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
            .init_resource::<IndustryProductionWindowState>()
            .init_resource::<SaveWindowState>()
            .init_resource::<TownWindowState>()
            .init_resource::<TownAuthorityWindowState>()
            .init_resource::<TownAuthorityEffectWatch>()
            .init_resource::<StationPoolRegistry>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_top_toolbar,
                    setup_build_menu,
                    setup_minimap,
                    setup_order_panel,
                    setup_depot_panel,
                    setup_station_cargo_panel,
                    setup_rail_station_picker,
                    setup_bridge_picker,
                    setup_industry_panel,
                    setup_industry_production_window,
                    setup_save_window,
                    setup_town_window,
                    setup_town_authority_window,
                )
                    .chain()
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_signal_picker,
                    setup_airport_picker,
                    setup_road_stop_picker,
                    setup_object_picker,
                    setup_dock_picker,
                    setup_buoy_picker,
                    setup_rail_waypoint_picker,
                    setup_road_waypoint_picker,
                    setup_tree_picker,
                    setup_terraform_picker,
                    setup_sign_picker,
                    setup_depot_build_picker,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                (
                    dispatch_ui_hotkeys,
                    (
                        handle_toolbar_command_hotkeys,
                        handle_zoom_hotkeys,
                        save_window_keyboard,
                        save_window_editable_keyboard,
                        save_window_name_click_focus,
                        handle_pause_toggle,
                        handle_hud_toggle,
                        cycle_json_save_path_hotkey,
                        handle_tool_hotkeys,
                        rotate_station_with_right_click,
                        close_road_type_picker_on_escape,
                        handle_ingame_escape,
                    ),
                )
                    .chain()
                    .in_set(UpdateSet::Input)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                observe_town_authority_effects
                    .after(handle_town_authority_buttons)
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (handle_toolbar_switch, sync_toolbar_layout)
                    .chain()
                    .in_set(UpdateSet::Ui)
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
                    industry_panel_on_closed,
                    station_view_on_closed,
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
                handle_settings_zoom_buttons
                    .after(handle_settings_menu_buttons)
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    handle_vehicle_breakdowns_menu_button,
                    handle_freight_trains_menu_button,
                    handle_road_driving_side_menu_button,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
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
                (begin_order_list_drag, finish_order_list_drag)
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
                    handle_town_authority_buttons,
                    town_window_on_closed,
                    town_authority_window_on_closed,
                    industry_production_window_on_closed,
                    depot_panel_on_closed,
                    order_panel_on_closed,
                    buoy_picker_on_closed,
                    dock_picker_on_closed,
                    tree_picker_on_closed,
                    terraform_picker_on_closed,
                    rail_waypoint_picker_on_closed,
                    road_waypoint_picker_on_closed,
                    sign_picker_on_closed,
                    depot_build_picker_on_closed,
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
                    handle_dock_picker_buttons,
                    handle_depot_build_picker_buttons,
                    handle_airport_picker_buttons,
                    airport_picker_on_closed,
                    handle_road_stop_picker_buttons,
                    road_stop_picker_on_closed,
                    handle_object_picker_buttons,
                    object_picker_on_closed,
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
                (
                    update_cursor_tile,
                    update_build_ghost_preview,
                    lerp_ghost_previews,
                    handle_tile_click,
                )
                    .chain()
                    .after(sync_build_pointer_modifiers)
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_minimap,
                    sync_order_panel,
                    sync_orders_pick_cursor,
                    sync_depot_panel,
                    sync_station_cargo_panel,
                    sync_industry_panel,
                    sync_industry_production_window,
                    sync_town_window,
                    sync_town_authority_window,
                    sync_rail_station_picker,
                    sync_station_catalog_entries,
                    sync_bridge_picker,
                )
                    .after(crate::i18n::LocalizationSet)
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_signal_picker,
                    sync_airport_picker,
                    sync_buoy_picker,
                    sync_dock_picker,
                    sync_rail_waypoint_picker,
                    sync_road_waypoint_picker,
                    sync_sign_picker,
                    sync_terraform_picker,
                    sync_tree_picker,
                    sync_depot_build_picker,
                    sync_station_spec_entry_previews,
                    sync_action5_gui_toolbar_icons,
                    sync_airport_preview_image,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_road_stop_picker,
                    sync_road_stop_catalog_entries,
                    sync_road_stop_preview_image,
                    sync_object_picker,
                    sync_object_catalog_entries,
                    sync_object_preview_image,
                    sync_rail_type_select_visuals,
                    sync_rail_toolbar_icons,
                    sync_road_type_popovers,
                    sync_road_type_entry_visibility,
                    sync_road_type_catalog_entries,
                    sync_road_type_entry_previews,
                    sync_road_type_class_labels,
                    sync_vehicle_breakdowns_button_label,
                    sync_freight_trains_button_label,
                    sync_road_driving_side_button_label,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
