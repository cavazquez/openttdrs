use bevy::prelude::*;

mod handlers;
mod setup;
mod sync;

pub(crate) use handlers::{
    apply_order_edit, handle_order_destination_click, handle_order_panel_buttons,
    open_order_edit_for_vehicle, start_order_destination_pick, try_append_order_at_tile,
};
pub(crate) use setup::setup_order_panel;
pub(crate) use sync::sync_order_panel;

pub(crate) const ORDER_PANEL_ROWS: usize = 32;
/// Altura visible de la lista (~10 filas) con scroll para el resto.
pub(crate) const ORDER_PANEL_LIST_MAX_HEIGHT: f32 = 240.0;

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRow {
    pub(super) slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRowText {
    pub(super) slot: usize,
}
