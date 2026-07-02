//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod autoreplace;
pub mod bridge_spec;
pub mod cargo;
pub mod command;
pub mod depot;
pub mod economy;
pub mod engine;
mod game_state;
pub mod industry;
pub mod map;
pub mod news;
pub mod ottdmap_extras;
pub mod parity;
pub mod pathfinder;
pub mod rail_lane;
pub mod rail_signals;
pub mod refit;
pub mod road_movement;
pub mod sav;
pub mod save;
pub mod shared_orders;
mod sim_step;
pub mod station;
pub mod tick;
pub mod timetable;
pub mod tnbp_decode;
pub mod town;
pub mod townname;
pub mod vehicle;
mod vehicle_ai;
pub mod vehicle_group;
pub mod world_gen;

pub use autoreplace::{AutoReplaceRule, try_autoreplace_vehicle};
pub use bridge_spec::{
    BRIDGE_SPECS, BridgePiece, BridgeSpec, BridgeType, bridge_above_axis_from_mapt,
    bridge_available, bridge_available_at_tick, bridge_build_cost, bridge_line_tiles,
    bridge_middle_length, bridge_spec, bridge_total_length, bridge_type_from_m6, calc_bridge_piece,
    set_bridge_middle_mapt, set_bridge_type_m6,
};
pub use cargo::{CargoStock, CargoType};
pub use command::{
    Command, CommandError, LevelMode, OrderMoveDirection, ROAD_PLACE_FORCE_AXIS, apply_command,
    check_place_industry_spec, command_error_message, command_would_fail, finalize_road_drag_line,
    industry_template, infer_road_drag_axis, preview_road_bits_at, rail_bits_placement_target,
    rail_station_footprint, rail_trackbits_from_neighbors, road_bits_for_autoroute,
    road_drag_line_tiles, road_locked_tool_axis,
};
pub use depot::{depot_tile_kind_for_vehicle, nearest_depot_tile};
pub use economy::{
    CargoPaymentSpec, TICKS_PER_TRANSIT_DAY, TICKS_PER_YEAR, buy_land_cost, cargo_time_factor,
    inflation_income_factor, inflation_prices_factor, manhattan_distance,
    terraform_cost_per_corner, ticks_to_transit_days, transported_goods_income,
    vehicle_purchase_cost, vehicle_running_cost_per_tick, vehicle_sell_refund,
};
pub use engine::{
    ENGINE_BUS_MPS, ENGINE_TRAIN_KIRBY, ENGINE_TRUCK_MPS, EngineCatalogSort, EngineDef,
    REFERENCE_PROGRESS_STEP, ROAD_ACCEL_ORIGINAL, RoadEngineFilter, decelerate_road_speed,
    default_engine_id, engine_available_in_year, engine_by_id, engine_catalog, engine_for_vehicle,
    engines_for_depot_purchase, engines_of_kind, progress_step_for_speed, tile_progress_length,
    train_sprite_group, update_road_speed,
};
#[allow(deprecated)]
pub use game_state::CARGO_DELIVERY_PAYMENT;
pub use game_state::IncomePopup;
pub use game_state::{
    BRIDGE_BUILD_COST_PER_TILE, BUY_LAND_BASE_PRICE, CLEAR_TILE_COST, CompanyEconomy,
    DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, SimStats,
    TERRAFORM_BASE_PRICE, TERRAFORM_COST, TUNNEL_BUILD_COST_PER_TILE, WAYPOINT_BUILD_COST,
};
pub use industry::{
    FACTORY_COAL_INPUT, FACTORY_WOOD_INPUT, INDUSTRY_PRODUCE_TICKS, Industry, IndustryKind,
    IndustrySpec, industry_produce_period_ticks,
};
pub use map::{
    GFX_COAL_MINE_TOWER_ANIMATED, GFX_COPPER_MINE_TOWER_ANIMATED, GFX_GOLD_MINE_TOWER_ANIMATED,
    GFX_OILWELL_ANIMATED_1, GFX_OILWELL_ANIMATED_2, GFX_OILWELL_ANIMATED_3, IndustryTileLink, Map,
    MapError, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND, OBJECT_TYPE_TRANSMITTER,
    OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE, OTTD_TILETYPE_TUNNELBRIDGE, SLOPE_NE, SLOPE_NW, SLOPE_SE,
    SLOPE_SW, Tile, TileCoord, TileKind, advance_industry_construction,
    advance_industry_tile_animations, effective_road_bits, inclined_slope_direction,
    industry_animation_frame, industry_construction_counter, industry_construction_stage,
    industry_gfx, industry_instance_id, industry_tile_anim_state, industry_tile_link,
    industry_tiles_mergeable, industry_uses_water_ground, is_industry_completed,
    is_map_object_tile, is_owned_land_tile, is_tunnel_entrance_slope, make_industry_tile_bigger,
    openttd_tile_index_to_coord, partial_pixel_z, rail_foundation_for_trackbits,
    rail_trackbits_valid_on_slope, resolve_tunnel_end, set_industry_gfx, slope_dz_at_subtile,
    slope_dz_on_tile, step_industry_tiles, tile_adjacent_to_water, tile_slope_and_z,
    tunnel_entrance_m5, tunnel_preview_path,
};
pub use news::{
    CALENDAR_BASE_YEAR, NEWS_MAX_AGE_DAYS, NewsDisplayMode, NewsDisplaySettings, NewsItem,
    NewsQueue, NewsReference, NewsType, PendingNewsEvent, VehicleAdviceKind, add_news_item,
    calendar_day_index, calendar_year_day, cargo_display_name, default_display_for_type,
    format_calendar_date, format_money, maybe_purge_old_news, news_display_mode_label,
    news_type_label, poll_vehicle_advice_news, purge_old_news_items, push_cargo_delivery_news,
    push_first_vehicle_running_news, push_vehicle_advice_news, tick_for_calendar_year,
    vehicle_kind_label,
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
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SEMAPHORE_BUILD_BEFORE_YEAR,
    SIGNAL_BUILD_COST, SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SignalTrack,
    calendar_year_at_tick, cycle_signal_facing, cycle_signal_side_m3, default_signal_variant,
    rail_tile_is_signals, resolve_signal_track, signal_facing_for_orientation,
    signal_on_track_mask, signal_placement_for_facing, signal_placement_for_track,
    signal_type_for_track, tracks_overlap, valid_signal_facings_track,
};
pub use refit::{next_refit_cargo, refit_allowed, refittable_cargo_types, vehicle_in_depot};
pub use road_movement::{
    VehiclePose, extrapolate_vehicle_pose, road_turn_entry_exit, straight_subtile,
    train_straight_subtile, train_subtile_direction, turn_curve_points, vehicle_render_direction,
    vehicle_render_direction_at, vehicle_render_progress, vehicle_subtile, vehicle_subtile_at,
    vehicle_subtile_with_progress,
};
pub use sav::{SavError, SavGame, SavIndustry, SavStation, SavVehicle, SavVehicleKind};
pub use save::CURRENT_SAVE_VERSION;
pub use save::SaveError;
pub use save::load_from_str;
pub use shared_orders::SharedOrderList;
pub use station::{
    STATION_COVERAGE_RADIUS, STATION_TYPE_RAIL_WAYPOINT, Station, StationCoverage,
    StationMapCoherenceReport, StopKind, industry_in_station_coverage, is_rail_waypoint_at,
    is_rail_waypoint_tile, rail_station_approach_tile, resolve_order_destination,
    road_stop_approach_tile, station_coverage_at, station_covers_tile, station_map_coherence,
    station_type_from_m6, stop_kind_from_m6, vehicle_at_road_stop, vehicle_physically_at_station,
};
pub use tick::GameTick;
pub use timetable::{TRAVEL_PRESETS, WAIT_PRESETS, cycle_travel_ticks, cycle_wait_ticks};
pub use tnbp_decode::{
    JgrTunnelRecord, SlPrimitive, SlTableField, TnbpDecodeError, TnbpDecoded, decode_tnbp_blob,
    jgr_tunnels_from_decoded, read_sl_gamma, split_sl_gamma_segments, tnbp_blob_to_json_value,
};
pub use town::{
    MAIL_PER_HOUSE, PASSENGERS_PER_HOUSE, STATION_TOWN_CARGO_CAPACITY, TOWN_PRODUCE_TICKS, Town,
    produce_town_cargo,
};
pub use townname::generate_town_name;
pub use vehicle::reverse_direction;
pub use vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, OrderConditionKind,
    TimetableWaitKind, VEHICLE_PROGRESS_STEP, Vehicle, VehicleDirection, VehicleKind, VehicleOrder,
    direction_from_tile_step,
};
pub use vehicle_group::{MAX_VEHICLE_GROUP_NAME_CHARS, VehicleGroup};
pub use world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROUGH, CLEAR_GROUND_SNOW, Climate,
    PreserveRect, WorldGenConfig, apply_world_gen, clear_ground_m5, effective_clear_ground,
};

#[cfg(test)]
mod tests;
