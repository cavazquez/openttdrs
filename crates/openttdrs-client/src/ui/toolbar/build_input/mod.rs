//! Entrada de construcción: clic en mapa, drag, órdenes y comandos core.

mod click;
pub(crate) mod commands;
pub(crate) mod drag;
pub(crate) mod orders;
mod placement;

pub(crate) use click::handle_tile_click;
pub(crate) use placement::cancel_placement;
