//! Bootstrap del estado simulado: carga de archivos, generacion procedural y resumen de deteccion.

mod demo_layout;
mod gameplay_showcase;
mod industries;
mod logging;
mod procedural_population;
mod terrain;
mod transport;
pub(crate) mod world;

pub(crate) use world::{
    MapSizePreset, NewGameSettings, PopulationDensity, START_YEARS, STARTING_MONEY_OPTIONS,
    build_procedural_demo_world,
};

pub(crate) use super::stations::{place_stations_from_footer_stxy, place_stations_from_map_tiles};
pub(crate) use demo_layout::log_procedural_demo_zones;
#[cfg(test)]
pub(crate) use demo_layout::{DEMO_ECONOMY_DELIVER_STATION, DEMO_ECONOMY_LOAD_STATION};
pub(crate) use gameplay_showcase::log_gameplay_showcase_zones;
pub(crate) use industries::{industry_group_from_gfx, place_industries, place_industries_from_sav};
pub(crate) use logging::log_detection_summary;
#[cfg(test)]
pub(crate) use terrain::place_tunnel_demo_ridge;
pub(crate) use transport::place_stations;
