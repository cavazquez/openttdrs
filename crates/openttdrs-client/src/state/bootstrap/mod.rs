//! Bootstrap del estado simulado: carga de archivos, generacion procedural y resumen de deteccion.

mod demo_layout;
mod gameplay_showcase;
mod industries;
mod logging;
mod terrain;
mod transport;

pub(crate) use super::stations::{place_stations_from_footer_stxy, place_stations_from_map_tiles};
#[cfg(test)]
pub(crate) use demo_layout::{DEMO_ECONOMY_DELIVER_STATION, DEMO_ECONOMY_LOAD_STATION};
pub(crate) use demo_layout::{
    fill_flat_grass, log_procedural_demo_zones, place_bridge_demo_gap, place_clean_demo_transport,
    place_demo_economy_loop,
};
pub(crate) use gameplay_showcase::{log_gameplay_showcase_zones, place_gameplay_showcase};
pub(crate) use industries::{industry_group_from_gfx, place_industries, place_industries_from_sav};
pub(crate) use logging::log_detection_summary;
pub(crate) use terrain::place_tunnel_demo_ridge;
pub(crate) use transport::place_stations;
