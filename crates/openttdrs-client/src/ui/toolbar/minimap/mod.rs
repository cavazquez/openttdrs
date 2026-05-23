mod input;
mod palette;
mod setup;
mod sync;

pub(crate) use input::{handle_minimap_click, minimap_contains_cursor};
pub(crate) use setup::setup_minimap;
pub(crate) use sync::sync_minimap;

use bevy::prelude::*;

pub(crate) const MINIMAP_COLS: u32 = 64;
pub(crate) const MINIMAP_ROWS: u32 = 40;
pub(crate) const MINIMAP_CELL: f32 = 3.0;
pub(crate) const MINIMAP_PAD: f32 = 6.0;
pub(crate) const MINIMAP_RIGHT: f32 = 10.0;
pub(crate) const MINIMAP_BOTTOM: f32 = 10.0;

#[derive(Component)]
pub(crate) struct MinimapRoot;

#[derive(Component)]
pub(crate) struct MinimapCell {
    pub(super) col: u32,
    pub(super) row: u32,
}

#[derive(Component)]
pub(crate) struct MinimapViewport;
