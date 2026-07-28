//! Orquestación compartida para aplicar `NewGRF` stack.

use std::path::PathBuf;

use crate::GameState;

pub mod action5;
pub mod industry;
pub mod rail;
pub mod road;
pub mod station;
pub mod train;

#[must_use]
pub fn default_newgrf_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("assets/opengfx/opengfx2-32ez"),
        PathBuf::from("assets/newgrf"),
    ];
    if let Ok(extra) = std::env::var("OPENTTDRS_NEWGRF_DIR")
        && !extra.trim().is_empty()
    {
        dirs.push(PathBuf::from(extra));
    }
    dirs
}

/// Refresco completo de catálogos Action0 tras cambiar el stack.
pub fn apply_newgrf_stack_catalogs_default_dirs(state: &mut GameState) {
    road::apply_newgrf_road_types_default_dirs(state);
    station::apply_newgrf_stations_default_dirs(state);
    train::apply_newgrf_vehicles_trains_default_dirs(state);
    industry::apply_newgrf_industry_tiles_default_dirs(state);
    rail::apply_newgrf_rail_signals_default_dirs(state);
    action5::apply_newgrf_action5_all_default_dirs(state);
}
