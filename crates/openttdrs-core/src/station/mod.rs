mod cargo_rating;
mod coverage;
mod destination;
mod geometry;
mod goods_entry;
mod model;
mod move_goods;
mod tests;
mod tile_encoding;

// Re-exports públicos para mantener compatibilidad con imports existentes
pub use cargo_rating::{
    MAX_TIME_SINCE_PICKUP_DAYS, StationVisit, TOWN_ADVERTISE_MEDIUM_RADIUS,
    TOWN_ADVERTISE_MEDIUM_RATING_BOOST, TOWN_CARGO_MIN_OWNER_RATING, load_amount_for_rating,
    modify_station_rating_around, note_station_load_attempt, on_station_cargo_pickup,
    recompute_station_rating, station_is_freight_pickup_stop, station_rating_for_cargo,
    station_rating_for_company_cargo, update_station_ratings,
    update_station_ratings_with_cargo_callbacks,
};
pub use coverage::{
    STATION_COVERAGE_RADIUS, StationCoverage, StationMapCoherenceReport,
    industry_in_station_coverage, industry_in_station_coverage_by_pos, station_catchment_radius,
    station_coverage_at, station_coverage_for, station_covers_tile, station_map_coherence,
};
pub use destination::{
    resolve_aircraft_station_dest, resolve_order_destination, resolve_order_destination_from,
};
pub use geometry::{
    bay_entry_direction, is_connected_bay_road_stop, is_drive_through_road_stop, pick_stop_tile,
    platform_past_stop_tiles, rail_station_approach_tile, rail_station_axis_y,
    rail_station_owned_tiles, rail_station_platform_tiles, rail_station_platform_track_tiles,
    rail_station_stop_candidates, rail_station_stop_candidates_osl, rail_station_stop_tile,
    rail_station_stop_tile_for_approach, rail_station_stop_tile_for_approach_osl,
    rail_station_stop_tile_with_osl, road_stop_approach_tile, station_at_tile,
    station_footprint_tiles, station_tile_sets_adjacent, train_on_rail_platform,
    vehicle_at_road_stop, vehicle_physically_at_station,
};
pub use goods_entry::{GoodsEntry, INITIAL_STATION_RATING, STATION_RATING_MAX_STEP, StationGoods};
pub use model::{CargoTimeSincePickup, Station, StopKind};
pub use move_goods::{can_move_goods_to_station, move_goods_to_station, update_station_waiting};
pub use tile_encoding::{
    STATION_TILE_PYLONS, STATION_TILE_WIRES, STATION_TYPE_BUOY, STATION_TYPE_DOCK,
    STATION_TYPE_OILRIG, STATION_TYPE_RAIL_WAYPOINT, STATION_TYPE_ROAD_WAYPOINT,
    default_station_catenary_flags, is_rail_waypoint_at, is_rail_waypoint_tile,
    station_tile_can_have_pylons, station_tile_can_have_wires, station_type_from_m6,
    stop_kind_from_m6,
};
