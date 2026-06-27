//! Barra inferior (fecha, ticker, dinero) y cartel de noticias.

mod setup;
mod sync;

pub(crate) use setup::setup_status_bar;
pub(crate) use sync::{
    drain_news_events, handle_news_popup_close, handle_news_popup_focus,
    handle_status_bar_center_click, sync_status_bar, update_news_playback,
};

use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;

/// Altura de la barra de estado (encima del borde inferior).
pub(crate) const STATUS_BAR_HEIGHT: f32 = 34.0;
pub(crate) const STATUS_BAR_Z: i32 = 2000;

/// Scroll máximo del ticker (referencia OpenTTD `TICKER_STOP`).
pub(crate) const TICKER_SCROLL_MAX: f32 = 1640.0;
pub(crate) const TICKER_SCROLL_SPEED: f32 = 90.0;

pub(crate) const POPUP_SLIDE_SPEED: f32 = 140.0;
pub(crate) const POPUP_HOLD_MS: f32 = 10_000.0;
pub(crate) const POPUP_WIDTH: f32 = 460.0;

pub(crate) const COMPANY_DISPLAY_NAME: &str = "Tu compañía";

#[derive(Resource, Default)]
pub(crate) struct NewsUiState {
    pub ticker: Option<TickerState>,
    pub popup: Option<PopupState>,
    pub waiting_full: VecDeque<u64>,
    pub waiting_ticker: VecDeque<u64>,
    pub shown_full: HashSet<u64>,
    pub reminder_until_secs: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct TickerState {
    pub item_id: u64,
    pub scroll: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct PopupState {
    #[allow(dead_code)]
    pub item_id: u64,
    pub bottom: f32,
    pub target_bottom: f32,
    pub hold_remaining_ms: f32,
    pub sliding_in: bool,
    pub entity: Entity,
}

#[derive(Component)]
pub(crate) struct StatusBarRoot;

#[derive(Component)]
pub(crate) struct StatusBarDateText;

#[derive(Component)]
pub(crate) struct StatusBarCenterButton;

#[derive(Component)]
pub(crate) struct StatusBarTickerText;

#[derive(Component)]
pub(crate) struct StatusBarDefaultText;

#[derive(Component)]
pub(crate) struct StatusBarMoneyText;

#[derive(Component)]
pub(crate) struct StatusBarReminderDot;

#[derive(Component)]
pub(crate) struct NewsPopupRoot;

#[derive(Component)]
pub(crate) struct NewsPopupDateText;

#[derive(Component)]
pub(crate) struct NewsPopupHeadlineText;

#[derive(Component)]
pub(crate) struct NewsPopupBodyText;

#[derive(Component)]
pub(crate) struct NewsPopupCloseButton;

#[derive(Component)]
pub(crate) struct NewsPopupFocusButton;
