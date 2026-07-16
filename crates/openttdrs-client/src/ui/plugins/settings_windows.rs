//! Plugin de UI para ventanas de configuración y herramientas de desarrollo.

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;
use crate::ui::ai_settings_window::{
    AiSettingsWindowState, ai_settings_on_closed, handle_ai_settings_buttons,
    setup_ai_settings_window, sync_ai_settings_window,
};
use crate::ui::audio_settings_window::{
    SoundMusicWindowState, handle_audio_settings_buttons, handle_music_window_buttons,
    handle_sound_music_toolbar_button, handle_volume_sliders, setup_sound_music_window,
    sound_music_window_on_closed, sync_sound_music_window,
};
use crate::ui::cargo_dist_settings_window::{
    CargoDistSettingsWindowState, cargo_dist_settings_on_closed,
    handle_cargo_dist_settings_buttons, setup_cargo_dist_settings_window,
    sync_cargo_dist_settings_window,
};
use crate::ui::cheat_window::{
    CheatWindowState, cheat_window_on_closed, handle_cheat_window_buttons,
    handle_cheat_window_hotkey, setup_cheat_window, sync_cheat_window,
};
use crate::ui::dev_console::{
    DevConsoleState, dev_console_window_on_closed, handle_dev_console_buttons,
    handle_dev_console_keyboard, setup_dev_console, sync_dev_console,
};
use crate::ui::display_options_window::{
    DisplayOptionsWindowState, display_options_window_on_closed, handle_display_options_buttons,
    setup_display_options_window, sync_display_options_window,
};
use crate::ui::extra_viewport_window::{
    ExtraViewportWindowState, extra_viewport_window_on_closed, setup_extra_viewport_window,
    sync_extra_viewport_window,
};
use crate::ui::help_window::{
    HelpWindowState, handle_help_hotkey, help_window_on_closed, setup_help_window, sync_help_window,
};
use crate::ui::newgrf_window::{
    NewGrfWindowState, handle_newgrf_window_buttons, newgrf_window_on_closed, setup_newgrf_window,
    sync_newgrf_window,
};
use crate::ui::news_settings_window::{
    NewsSettingsWindowState, handle_news_settings_buttons, news_settings_on_closed,
    setup_news_settings_window, sync_news_settings_window,
};
use crate::ui::pathfinding_settings_window::{
    PathfindingSettingsWindowState, handle_pathfinding_settings_buttons,
    pathfinding_settings_on_closed, setup_pathfinding_settings_window,
    sync_pathfinding_settings_window,
};
use crate::ui::sign_list_window::{
    SignListWindowState, handle_sign_list_body_click, handle_sign_list_buttons,
    open_sign_list_from_routes, setup_sign_list_window, sign_list_rename_keyboard,
    sign_list_window_on_closed, sync_sign_list_window,
};
use crate::ui::tile_inspector_window::{
    TileInspectorWindowState, draw_selected_tile_bounds, handle_tile_inspector_hotkey,
    setup_tile_inspector_window, sync_tile_inspector_window, tile_inspector_window_on_closed,
};
use crate::ui::ui5_blocked_stubs::{
    LinkGraphWindowState, handle_link_graph_filter_button, handle_link_graph_view_button,
    link_graph_window_on_closed, open_link_graph_from_routes, setup_link_graph_window,
    sync_link_graph_window,
};

pub(crate) struct SettingsWindowsPlugin;

impl Plugin for SettingsWindowsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NewsSettingsWindowState>()
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
            .init_resource::<SoundMusicWindowState>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_news_settings_window,
                    setup_pathfinding_settings_window,
                    setup_sound_music_window,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (setup_ai_settings_window, setup_cargo_dist_settings_window).in_set(StartupSet::Ui),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_display_options_window,
                    setup_extra_viewport_window,
                    setup_sign_list_window,
                    setup_link_graph_window,
                    setup_newgrf_window,
                    setup_help_window,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_dev_console,
                    setup_tile_inspector_window,
                    setup_cheat_window,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                (
                    handle_help_hotkey,
                    handle_tile_inspector_hotkey,
                    handle_dev_console_keyboard,
                    handle_cheat_window_hotkey,
                )
                    .in_set(UpdateSet::Input)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
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
                draw_selected_tile_bounds
                    .in_set(UpdateSet::Visuals)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
