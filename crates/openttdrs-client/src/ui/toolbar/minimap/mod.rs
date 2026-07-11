mod input;
mod palette;
mod setup;
mod sync;

pub(crate) use input::{handle_minimap_click, minimap_contains_cursor};
pub(crate) use setup::{handle_minimap_layer_buttons, setup_minimap};
pub(crate) use sync::sync_minimap;

use bevy::prelude::*;

pub(crate) const MINIMAP_COLS: u32 = 64;
pub(crate) const MINIMAP_ROWS: u32 = 40;
pub(crate) const MINIMAP_CELL: f32 = 3.0;
/// Tamaño de celda en modo ExtraLargeMap.
pub(crate) const MINIMAP_CELL_EXPANDED: f32 = 8.0;
pub(crate) const MINIMAP_PAD: f32 = 6.0;
pub(crate) const MINIMAP_RIGHT: f32 = 10.0;
pub(crate) const MINIMAP_BOTTOM: f32 = 44.0;
/// Altura de la fila de toggles de capas bajo la rejilla.
pub(crate) const MINIMAP_CONTROLS_H: f32 = 22.0;

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MinimapLayerState {
    pub(crate) industries: bool,
    pub(crate) owners: bool,
    pub(crate) vehicles: bool,
    /// ExtraLargeMap: rejilla centrada con celdas más grandes.
    pub(crate) expanded: bool,
}

impl Default for MinimapLayerState {
    fn default() -> Self {
        Self {
            industries: true,
            owners: false,
            vehicles: true,
            expanded: false,
        }
    }
}

impl MinimapLayerState {
    #[must_use]
    pub(crate) fn cell_px(self) -> f32 {
        if self.expanded {
            MINIMAP_CELL_EXPANDED
        } else {
            MINIMAP_CELL
        }
    }

    #[must_use]
    pub(crate) fn grid_size(self) -> (f32, f32) {
        let cell = self.cell_px();
        (MINIMAP_COLS as f32 * cell, MINIMAP_ROWS as f32 * cell)
    }

    #[must_use]
    pub(crate) fn root_size(self) -> (f32, f32) {
        let (gw, gh) = self.grid_size();
        (
            gw + MINIMAP_PAD * 2.0,
            gh + MINIMAP_PAD * 2.0 + MINIMAP_CONTROLS_H,
        )
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MinimapLayerToggle {
    Industries,
    Owners,
    Vehicles,
    Expand,
}

#[derive(Component)]
pub(crate) struct MinimapRoot;

#[derive(Component)]
pub(crate) struct MinimapCell {
    pub(super) col: u32,
    pub(super) row: u32,
}

#[derive(Component)]
pub(crate) struct MinimapViewport;

#[derive(Component)]
pub(crate) struct MinimapLegendText;
