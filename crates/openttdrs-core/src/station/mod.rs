mod cargo_rating;
mod coverage;
mod destination;
mod geometry;
mod model;
mod tests;
mod tile_encoding;

// Re-exports públicos para mantener compatibilidad con imports existentes
pub use cargo_rating::{
    MAX_TIME_SINCE_PICKUP_DAYS, TOWN_CARGO_MIN_OWNER_RATING, load_amount_for_rating,
    on_station_cargo_pickup, recompute_station_rating, station_is_freight_pickup_stop,
    station_rating_for_cargo, station_rating_for_company_cargo, tick_station_cargo_age,
};
pub use coverage::{
    STATION_COVERAGE_RADIUS, StationCoverage, StationMapCoherenceReport,
    industry_in_station_coverage, industry_in_station_coverage_by_pos, station_coverage_at,
    station_covers_tile, station_map_coherence,
};
pub use destination::{
    resolve_aircraft_station_dest, resolve_order_destination, resolve_order_destination_from,
};
pub use geometry::{
    bay_entry_direction, is_connected_bay_road_stop, rail_station_approach_tile,
    rail_station_axis_y, rail_station_owned_tiles, rail_station_platform_tiles,
    rail_station_stop_tile, rail_station_stop_tile_for_approach, road_stop_approach_tile,
    station_at_tile, station_footprint_tiles, station_tile_sets_adjacent, train_on_rail_platform,
    vehicle_at_road_stop, vehicle_physically_at_station,
};
pub use model::{CargoTimeSincePickup, Station, StopKind};
pub use tile_encoding::{
    STATION_TILE_PYLONS, STATION_TILE_WIRES, STATION_TYPE_BUOY, STATION_TYPE_RAIL_WAYPOINT,
    STATION_TYPE_ROAD_WAYPOINT, default_station_catenary_flags, is_rail_waypoint_at,
    is_rail_waypoint_tile, station_tile_can_have_pylons, station_tile_can_have_wires,
    station_type_from_m6, stop_kind_from_m6,
};
