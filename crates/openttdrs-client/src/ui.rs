//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::prelude::*;

mod lifecycle;
mod plugins;

mod ai_settings_window;
pub(crate) mod audio_settings_window;
mod autoreplace_window;
mod buy_window;
mod cargo_dist_settings_window;
mod cargo_payment_window;
mod cheat_window;
pub(crate) mod command_error_text;
mod destination_window;
mod dev_console;
mod display_options_window;
mod endscreen;
mod extra_viewport_window;
mod finances_window;
mod floating_window;
pub(crate) mod font;
mod genland_window;
mod goal_list_window;
mod graph_window;
mod help_window;
mod hotkeys;
mod hud;
mod industry_directory;
mod industry_panel;
mod league_window;
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
mod scrollbar;
mod shared_orders_window;
mod sign_list_window;
mod sparkline;
mod station_directory;
mod statusbar;
mod story_window;
mod subsidy_list;
mod tile_inspector_window;
mod timetable_window;
mod toolbar;
mod town_directory;
mod town_window;
mod ui5_blocked_stubs;
#[cfg(test)]
mod ui_enum_inventory_test;
mod vehicle_details_window;
mod vehicle_list;
mod vehicle_window;
mod window_lifecycle;
mod windows_shot;

pub(crate) use hud::SimHudControls;
pub(crate) use main_menu::{MainMenuCamera, MainMenuUi, leave_main_menu};
pub(crate) use save_window::SaveWindowState;
pub(crate) use toolbar::{BuildMenuAction, OrderEditState, ToolbarState, UiToolState};
pub(crate) use ui5_blocked_stubs::{LinkGraphView, LinkGraphWindowState};
pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            floating_window::FloatingWindowPlugin,
            scrollbar::ClassicScrollbarPlugin,
            windows_shot::WindowsShotPlugin,
            lifecycle::InGameLifecyclePlugin,
            plugins::MainMenuUiPlugin,
            plugins::HudUiPlugin,
            plugins::ToolbarUiPlugin,
            plugins::NavigationUiPlugin,
            plugins::SettingsWindowsPlugin,
            plugins::GameWindowsPlugin,
            plugins::EditorUiPlugin,
        ));
    }
}
