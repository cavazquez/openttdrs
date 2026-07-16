//! Entrada de construcción: clic en mapa, drag, órdenes y comandos core.

pub(super) mod apply_intent;
mod click;
pub(super) mod click_intent;
pub(crate) mod commands;
pub(crate) mod cursor;
pub(crate) mod drag;
pub(crate) mod orders;
pub(super) mod placement;
pub(crate) mod rail_lane;
pub(super) mod remap_plan;
pub(super) mod selection;

pub(crate) use click::{handle_tile_click, sync_build_pointer_modifiers};
pub(crate) use cursor::update_cursor_tile;
pub(crate) use placement::cancel_placement;
