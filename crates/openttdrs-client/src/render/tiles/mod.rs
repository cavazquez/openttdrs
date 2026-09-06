//! Render por tesela: land, agua, vías, objetos y batches.

mod batches;
mod bridge;
mod bridge_draw;
mod helpers;
mod land;
mod objects;
mod transport;
mod water;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod spawn_coverage_tests;

pub(crate) use batches::flush_map_batches;
#[cfg(test)]
pub(crate) use bridge::spawn_bridge_middle;
#[cfg(test)]
pub(crate) use bridge::spawn_bridge_middle_with_road_types;
pub(crate) use bridge::spawn_bridge_middle_with_road_types_and_stations;
pub(crate) use bridge_draw::{catenary_under_low_bridge, roadside_detail_visible_under_bridge};
pub(crate) use helpers::{
    FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC, TRAM_OVERLAY_LAYER_FRAC,
    leveled_foundation_overlay_pos, push_water_sprite, sloped_or_flat_image,
    spawn_coast_debug_label, spawn_ground_sprite, spawn_rail_foundation, spawn_road_foundation,
};
#[cfg(test)]
pub(crate) use land::spawn_generic_land_tile;
#[cfg(test)]
pub(crate) use land::spawn_industry_tile;
pub(crate) use land::{
    HouseSpawnResources, push_forest_tree, spawn_generic_land_tile_with_objects, spawn_house_tile,
    spawn_industry_tile_with_world, spawn_void_tile,
};
#[cfg(test)]
pub(crate) use objects::spawn_station_tile;
#[cfg(test)]
pub(crate) use objects::spawn_transport_object_tile;
pub(crate) use objects::{
    spawn_station_tile_with_world_and_road_types, spawn_transport_object_tile_with_road_types,
};
pub(crate) use transport::{spawn_rail_tile, spawn_road_tile};
pub(crate) use water::push_water_tile;
