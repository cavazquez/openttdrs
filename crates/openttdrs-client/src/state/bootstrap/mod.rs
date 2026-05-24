//! Bootstrap del estado simulado: carga de archivos, generacion procedural y resumen de deteccion.

mod demo_layout;
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
pub(crate) use industries::{industry_group_from_gfx, place_industries};
pub(crate) use logging::log_detection_summary;
pub(crate) use terrain::place_tunnel_demo_ridge;
pub(crate) use transport::place_stations;
