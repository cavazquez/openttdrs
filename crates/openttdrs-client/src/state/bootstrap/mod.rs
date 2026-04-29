//! Bootstrap del estado simulado: carga de archivos, generacion procedural y resumen de deteccion.

mod industries;
mod logging;
mod terrain;
mod transport;

pub(crate) use super::stations::{place_stations_from_footer_stxy, place_stations_from_map_tiles};
pub(crate) use industries::{industry_group_from_gfx, place_industries};
pub(crate) use logging::log_detection_summary;
pub(crate) use terrain::distribute_tile_kinds;
pub(crate) use transport::{place_roads, place_stations, place_vehicles};
