//! Plugin de UI para HUD (tile info, statusbar, news, popups).

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;
use crate::ui::hud::{
    HoveredTileCoord, HudBuildFeedback, HudSfxHandles, PlayHudSfx, SelectedTileInfo,
    SimHudControls, animate_build_place_flash, animate_income_popups, flush_hud_sfx,
    load_hud_sfx, play_hud_sfx, setup_tile_info_ui, spawn_build_place_flash, spawn_income_popups,
    update_tile_info_text,
};
use crate::ui::statusbar::{
    NewsHistoryState, NewsUiState, drain_news_events, handle_news_history_row_click,
    handle_news_popup_close, handle_news_popup_focus, handle_open_news_history,
    handle_status_bar_center_click, news_history_on_closed, setup_news_history_window,
    setup_status_bar, sync_news_history_window, sync_status_bar, update_news_playback,
};
use crate::ui::toolbar::{toolbar_click_beep, RailSignalGhostState};

pub(crate) struct HudUiPlugin;

impl Plugin for HudUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NewsUiState>()
            .init_resource::<NewsHistoryState>()
            .init_resource::<crate::news_prefs::NewsDisplayPrefs>()
            .init_resource::<SelectedTileInfo>()
            .init_resource::<HoveredTileCoord>()
            .init_resource::<SimHudControls>()
            .init_resource::<HudBuildFeedback>()
            .init_resource::<HudSfxHandles>()
            .init_resource::<RailSignalGhostState>()
            .add_message::<PlayHudSfx>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_tile_info_ui,
                    setup_status_bar,
                    setup_news_history_window,
                    load_hud_sfx,
                )
                    .in_set(StartupSet::Ui),
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
                    handle_news_history_row_click,
                    news_history_on_closed,
                    sync_news_history_window,
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
                    spawn_build_place_flash,
                    flush_hud_sfx,
                )
                    .chain()
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
