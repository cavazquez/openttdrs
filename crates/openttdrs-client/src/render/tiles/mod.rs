//! Render por tesela: land, agua, vías, objetos y batches.

mod batches;
mod bridge;
mod helpers;
mod land;
mod objects;
mod transport;
mod water;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod spawn_coverage_tests;

pub(crate) use batches::flush_map_batches;
pub(crate) use bridge::spawn_bridge_middle;
pub(crate) use helpers::{
    SHORE_LAYER_FRAC, TRAM_OVERLAY_LAYER_FRAC, leveled_foundation_overlay_pos, push_water_sprite,
    sloped_or_flat_image, spawn_coast_debug_label, spawn_ground_sprite, spawn_leveled_foundation,
    spawn_rail_foundation,
};
pub(crate) use land::{
    push_forest_tree, spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile,
};
pub(crate) use objects::{spawn_station_tile, spawn_transport_object_tile};
pub(crate) use transport::{spawn_rail_tile, spawn_road_tile};
pub(crate) use water::push_water_tile;
