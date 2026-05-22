use bevy::prelude::*;

mod handlers;
mod setup;
mod sync;

pub(crate) use handlers::{
    apply_order_edit, handle_order_destination_click, handle_order_panel_buttons,
    start_order_destination_pick,
};
pub(crate) use setup::setup_order_panel;
pub(crate) use sync::sync_order_panel;

pub(crate) const ORDER_PANEL_ROWS: usize = 10;

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRow {
    pub(super) slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRowText {
    pub(super) slot: usize,
}
