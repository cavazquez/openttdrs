//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod cargo;
pub mod command;
pub mod engine;
mod game_state;
pub mod industry;
pub mod map;
pub mod ottdmap_extras;
pub mod pathfinder;
pub mod road_movement;
pub mod save;
mod sim_step;
pub mod station;
pub mod tick;
pub mod tnbp_decode;
pub mod vehicle;
mod vehicle_ai;

pub use cargo::{CargoStock, CargoType};
pub use command::{
    Command, CommandError, apply_command, command_error_message, command_would_fail,
    industry_template,
};
pub use engine::{
    ENGINE_BUS_MPS, ENGINE_TRAIN_KIRBY, ENGINE_TRUCK_MPS, EngineDef, REFERENCE_PROGRESS_STEP,
    ROAD_ACCEL_ORIGINAL, decelerate_road_speed, default_engine_id, engine_for_vehicle,
    progress_step_for_speed, tile_progress_length, update_road_speed,
};
pub use game_state::{
    BRIDGE_BUILD_COST_PER_TILE, CARGO_DELIVERY_PAYMENT, CLEAR_TILE_COST, CompanyEconomy,
    DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, SimStats,
    TUNNEL_BUILD_COST_PER_TILE,
};
pub use industry::{
    INDUSTRY_PRODUCE_TICKS, Industry, IndustryKind, IndustrySpec, industry_produce_period_ticks,
};
pub use map::{
    Map, MapError, OTTD_TILETYPE_TUNNELBRIDGE, SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW, Tile,
    TileCoord, TileKind, inclined_slope_direction, is_tunnel_entrance_slope,
    openttd_tile_index_to_coord, partial_pixel_z, resolve_tunnel_end, slope_dz_at_subtile,
    slope_dz_on_tile, tile_slope_and_z, tunnel_entrance_m5, tunnel_preview_path,
};
pub use ottdmap_extras::{OttdmapExtras, dense_payload_end};
pub use pathfinder::{
    PathNetwork, TunnelWormholes, diag_dir_offset, find_path, find_path_with_wormholes,
    path_network_for_vehicle, station_entrance_faces_rail, station_entrance_faces_road,
    station_site_adjacent_to_rail, station_site_adjacent_to_transport, station_site_tile_allows_build,
    station_site_tile_needs_clear, tile_is_path_traversable,
};
pub use road_movement::{road_turn_entry_exit, straight_subtile, vehicle_subtile};
pub use save::SaveError;
pub use save::load_from_str;
pub use station::{
    STATION_COVERAGE_RADIUS, Station, StationCoverage, StopKind, industry_in_station_coverage,
    station_coverage_at, station_covers_tile,
};
pub use tick::GameTick;
pub use tnbp_decode::{
    JgrTunnelRecord, SlPrimitive, SlTableField, TnbpDecodeError, TnbpDecoded, decode_tnbp_blob,
    jgr_tunnels_from_decoded, read_sl_gamma, split_sl_gamma_segments, tnbp_blob_to_json_value,
};
pub use vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, VEHICLE_PROGRESS_STEP, Vehicle,
    VehicleDirection, VehicleKind, VehicleOrder, direction_from_tile_step,
};

#[cfg(test)]
mod tests;
