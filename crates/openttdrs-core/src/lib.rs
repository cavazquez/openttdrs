//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod cargo;
pub mod command;
pub mod economy;
pub mod engine;
mod game_state;
pub mod industry;
pub mod map;
pub mod ottdmap_extras;
pub mod pathfinder;
pub mod rail_lane;
pub mod rail_signals;
pub mod road_movement;
pub mod sav;
pub mod save;
mod sim_step;
pub mod station;
pub mod tick;
pub mod tnbp_decode;
pub mod town;
pub mod townname;
pub mod vehicle;
mod vehicle_ai;

pub use cargo::{CargoStock, CargoType};
pub use command::{
    Command, CommandError, apply_command, command_error_message, command_would_fail,
    industry_template, rail_station_footprint, rail_trackbits_from_neighbors,
};
pub use economy::{
    CargoPaymentSpec, TICKS_PER_TRANSIT_DAY, TICKS_PER_YEAR, cargo_time_factor,
    inflation_income_factor, manhattan_distance, ticks_to_transit_days, transported_goods_income,
    vehicle_purchase_cost, vehicle_running_cost_per_tick, vehicle_sell_refund,
};
pub use engine::{
    ENGINE_BUS_MPS, ENGINE_TRAIN_KIRBY, ENGINE_TRUCK_MPS, EngineDef, REFERENCE_PROGRESS_STEP,
    ROAD_ACCEL_ORIGINAL, decelerate_road_speed, default_engine_id, engine_by_id, engine_catalog,
    engine_for_vehicle, engines_of_kind, progress_step_for_speed, tile_progress_length,
    update_road_speed,
};
#[allow(deprecated)]
pub use game_state::CARGO_DELIVERY_PAYMENT;
pub use game_state::IncomePopup;
pub use game_state::{
    BRIDGE_BUILD_COST_PER_TILE, CLEAR_TILE_COST, CompanyEconomy, DEPOT_BUILD_COST, GameState,
    RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, SimStats, TUNNEL_BUILD_COST_PER_TILE,
    WAYPOINT_BUILD_COST,
};
pub use industry::{
    FACTORY_COAL_INPUT, FACTORY_WOOD_INPUT, INDUSTRY_PRODUCE_TICKS, Industry, IndustryKind,
    IndustrySpec, industry_produce_period_ticks,
};
pub use map::{
    Map, MapError, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE, OTTD_TILETYPE_TUNNELBRIDGE, SLOPE_NE,
    SLOPE_NW, SLOPE_SE, SLOPE_SW, Tile, TileCoord, TileKind, effective_road_bits,
    inclined_slope_direction, is_tunnel_entrance_slope, openttd_tile_index_to_coord,
    partial_pixel_z, resolve_tunnel_end, slope_dz_at_subtile, slope_dz_on_tile, tile_slope_and_z,
    tunnel_entrance_m5, tunnel_preview_path,
};
pub use ottdmap_extras::{OttdmapExtras, dense_payload_end};
pub use pathfinder::{
    PathCache, PathNetwork, TunnelWormholes, diag_dir_offset, find_path, find_path_cached,
    find_path_with_wormholes, path_network_for_vehicle, station_entrance_faces_rail,
    station_entrance_faces_road, station_site_adjacent_to_rail, station_site_adjacent_to_transport,
    station_site_tile_allows_build, station_site_tile_needs_clear, tile_is_path_traversable,
};
pub use rail_lane::{rail_horz_lane_bit, rail_vert_lane_bit};
pub use rail_signals::{
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SIGNAL_BUILD_COST,
    cycle_signal_facing, rail_tile_is_signals, signal_facing_for_orientation,
    signal_placement_for_facing, valid_signal_facings,
};
pub use road_movement::{
    road_turn_entry_exit, straight_subtile, train_straight_subtile, train_subtile_direction,
    vehicle_render_direction, vehicle_render_progress, vehicle_subtile,
    vehicle_subtile_with_progress,
};
pub use sav::{SavError, SavGame, SavIndustry, SavStation, SavVehicle, SavVehicleKind};
pub use save::CURRENT_SAVE_VERSION;
pub use save::SaveError;
pub use save::load_from_str;
pub use station::{
    STATION_COVERAGE_RADIUS, STATION_TYPE_RAIL_WAYPOINT, Station, StationCoverage, StopKind,
    industry_in_station_coverage, is_rail_waypoint_at, is_rail_waypoint_tile,
    rail_station_approach_tile, resolve_order_destination, station_coverage_at,
    station_covers_tile, station_type_from_m6,
};
pub use tick::GameTick;
pub use tnbp_decode::{
    JgrTunnelRecord, SlPrimitive, SlTableField, TnbpDecodeError, TnbpDecoded, decode_tnbp_blob,
    jgr_tunnels_from_decoded, read_sl_gamma, split_sl_gamma_segments, tnbp_blob_to_json_value,
};
pub use town::{
    MAIL_PER_HOUSE, PASSENGERS_PER_HOUSE, STATION_TOWN_CARGO_CAPACITY, TOWN_PRODUCE_TICKS, Town,
    produce_town_cargo,
};
pub use vehicle::reverse_direction;
pub use vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, VEHICLE_PROGRESS_STEP, Vehicle,
    VehicleDirection, VehicleKind, VehicleOrder, direction_from_tile_step,
};

#[cfg(test)]
mod tests;
