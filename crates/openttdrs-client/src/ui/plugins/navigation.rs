//! Plugin de UI para navegación (menús de toolbar y directorios).

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;
use crate::ui::goal_list_window::{
    GoalListWindowState, goal_list_window_on_closed, open_goal_list_from_routes,
    setup_goal_list_window, sync_goal_list_window,
};
use crate::ui::industry_directory::{
    IndustryDirectoryState, handle_industry_directory_buttons, industry_directory_on_closed,
    industry_directory_search_keyboard, open_industry_directory_from_routes,
    setup_industry_directory, sync_industry_directory,
};
use crate::ui::league_window::{
    LeagueWindowState, league_window_on_closed, open_league_from_routes, setup_league_window,
    sync_league_window,
};
use crate::ui::navigation::{
    OpenUiRoute, ToolbarContext, ToolbarMenuState, dismiss_toolbar_menu_on_outside_click,
    handle_toolbar_menu_entries, handle_toolbar_menu_keyboard, handle_toolbar_navigation_button,
    open_file_actions_from_routes, open_help_windows_from_routes, open_message_windows_from_routes,
    open_settings_windows_from_routes, refresh_toolbar_context, sync_toolbar_localized_labels,
    sync_toolbar_navigation_menu,
};
use crate::ui::station_directory::{
    StationDirectoryState, handle_station_directory_buttons, open_station_directory_from_routes,
    setup_station_directory, station_directory_on_closed, sync_station_directory,
};
use crate::ui::story_window::{
    StoryWindowState, handle_story_nav_buttons, open_story_from_routes, setup_story_window,
    story_window_on_closed, sync_story_window,
};
use crate::ui::subsidy_list::{
    SubsidyListState, handle_subsidy_list_buttons, open_subsidy_list_from_routes,
    setup_subsidy_list, subsidy_list_on_closed, sync_subsidy_list,
};
use crate::ui::toolbar::handle_editor_file_routes;
use crate::ui::town_directory::{
    TownDirectoryState, handle_town_directory_buttons, open_town_directory_from_routes,
    setup_town_directory, sync_town_directory, town_directory_on_closed,
    town_directory_search_keyboard,
};
use crate::ui::vehicle_list::{
    VehicleListState, handle_vehicle_group_rename_buttons, handle_vehicle_list_buttons,
    open_vehicle_list_from_routes, setup_vehicle_list, sync_vehicle_group_rename_row,
    sync_vehicle_list, vehicle_list_group_rename_keyboard, vehicle_list_on_closed,
};

pub(crate) struct NavigationUiPlugin;

impl Plugin for NavigationUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IndustryDirectoryState>()
            .init_resource::<TownDirectoryState>()
            .init_resource::<StationDirectoryState>()
            .init_resource::<VehicleListState>()
            .init_resource::<SubsidyListState>()
            .init_resource::<GoalListWindowState>()
            .init_resource::<StoryWindowState>()
            .init_resource::<LeagueWindowState>()
            .init_resource::<ToolbarMenuState>()
            .init_resource::<ToolbarContext>()
            .add_message::<OpenUiRoute>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_goal_list_window,
                    setup_story_window,
                    setup_league_window,
                    setup_town_directory,
                    setup_industry_directory,
                    setup_station_directory,
                    setup_vehicle_list,
                    setup_subsidy_list,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                (
                    open_file_actions_from_routes,
                    handle_editor_file_routes,
                    open_settings_windows_from_routes,
                    open_message_windows_from_routes,
                    open_help_windows_from_routes,
                )
                    .after(handle_toolbar_menu_entries)
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    (
                        refresh_toolbar_context,
                        handle_toolbar_navigation_button,
                        handle_toolbar_menu_entries,
                        handle_toolbar_menu_keyboard,
                        dismiss_toolbar_menu_on_outside_click,
                        sync_toolbar_navigation_menu,
                        sync_toolbar_localized_labels,
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
                    open_vehicle_list_from_routes
                        .after(handle_toolbar_menu_entries)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    handle_vehicle_list_buttons
                        .after(open_vehicle_list_from_routes)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    vehicle_list_on_closed
                        .after(handle_vehicle_list_buttons)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    sync_vehicle_list
                        .after(vehicle_list_on_closed)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    handle_vehicle_group_rename_buttons
                        .after(handle_vehicle_list_buttons)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    vehicle_list_group_rename_keyboard
                        .after(handle_vehicle_list_buttons)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    sync_vehicle_group_rename_row
                        .after(handle_vehicle_group_rename_buttons)
                        .in_set(UpdateSet::Ui)
                        .run_if(in_state(ClientScreen::InGame)),
                    (
                        open_subsidy_list_from_routes,
                        handle_subsidy_list_buttons,
                        subsidy_list_on_closed,
                        sync_subsidy_list,
                    )
                        .chain()
                        .after(handle_toolbar_menu_entries),
                    (
                        open_goal_list_from_routes,
                        goal_list_window_on_closed,
                        sync_goal_list_window,
                    )
                        .chain()
                        .after(handle_toolbar_menu_entries),
                    (
                        open_story_from_routes,
                        handle_story_nav_buttons,
                        story_window_on_closed,
                        sync_story_window,
                    )
                        .chain()
                        .after(handle_toolbar_menu_entries),
                    (
                        open_league_from_routes,
                        league_window_on_closed,
                        sync_league_window,
                    )
                        .chain()
                        .after(handle_toolbar_menu_entries),
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
