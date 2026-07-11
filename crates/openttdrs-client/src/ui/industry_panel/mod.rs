//! Panel flotante al clicar una industria (sin herramienta activa), con vista previa renderizada a textura.

use bevy::prelude::*;
use openttdrs_core::TileCoord;

mod logic;
mod setup;
mod systems;

#[cfg(test)]
mod tests;

pub(crate) use logic::kind_label;
pub(crate) use setup::setup_industry_panel;
pub(crate) use systems::{industry_panel_close_interaction, sync_industry_panel};

#[derive(Resource, Default)]
pub(crate) struct IndustryPanelState {
    pub(crate) open: bool,
    pub(crate) focus_tile: Option<TileCoord>,
}

#[derive(Component)]
pub(crate) struct IndustryPanelRoot;

#[derive(Component)]
pub(crate) struct IndustryPanelTitle;

#[derive(Component)]
pub(crate) struct IndustryPanelDetails;

#[derive(Component)]
pub(crate) struct IndustryPanelCloseButton;
