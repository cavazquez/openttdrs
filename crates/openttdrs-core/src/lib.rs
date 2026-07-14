//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod ai;
pub mod aircraft_movement;
pub mod airport;
pub mod airport_class;
pub mod autoreplace;
pub mod bridge_spec;
pub mod cargo;
pub mod cargo_packet;
pub mod cheats;
pub mod command;
pub mod company;
pub mod depot;
pub mod dev_metrics;
pub mod disaster;
pub mod economy;
pub mod economy_quarterly;
pub mod engine;
pub mod entity_history;
mod game_state;
pub mod industry;
pub mod link_graph;
pub mod map;
pub mod newgrf_actions;
mod newgrf_company_ramp;
pub mod newgrf_config;
mod newgrf_palette_data;
pub mod newgrf_sprites;
pub mod newgrf_type_tables;
pub mod news;
pub mod ottdmap_extras;
pub mod parity;
pub mod pathfinder;
pub mod pathfinding_settings;
pub mod rail_lane;
pub mod rail_pbs;
pub mod rail_signals;
pub mod rail_type;
pub mod refit;
pub mod road_action2;
pub mod road_movement;
pub mod road_type;
pub mod sav;
pub mod save;
mod score;
pub mod shared_orders;
pub mod ship_movement;
mod sign;
mod sim_events;
mod sim_step;
pub mod sound_id;
pub mod station;
pub mod station_action2;
pub mod station_class;
pub mod subsidy;
pub mod tick;
pub mod timetable;
pub mod tnbp_decode;
pub mod town;
pub mod town_expand;
pub mod townname;
pub mod train_collision;
pub mod train_consist;
pub mod train_movement;
pub mod vehicle;
mod vehicle_ai;
pub mod vehicle_group;
pub mod world_gen;

pub use aircraft_movement::{aircraft_requires_path, straight_line_path};
pub use airport::{
    AIRPORT_SMALL_H, AIRPORT_SMALL_W, AirportPiece, airport_loading_tile, airport_loading_tile_at,
    airport_m6_airport, airport_runway_tile, airport_small_footprint, airport_small_tiles,
    airport_spec_footprint, airport_spec_tiles, airport_tile_is_hangar, airport_tile_is_heliport,
};
pub use airport_class::{
    AirportClassDef, AirportClassId, AirportSpecDef, AirportSpecId, airport_class_def,
    airport_spec_def, all_airport_class_defs, all_airport_spec_defs, list_airport_classes,
    list_airport_specs,
};
pub use autoreplace::{AutoReplaceRule, try_autoreplace_vehicle};
pub use bridge_spec::{
    BRIDGE_SPECS, BridgePiece, BridgeSpec, BridgeType, bridge_above_axis_from_mapt,
    bridge_available, bridge_available_at_tick, bridge_build_cost, bridge_line_tiles,
    bridge_max_speed_for_tile, bridge_middle_length, bridge_spec, bridge_total_length,
    bridge_type_from_m6, calc_bridge_piece, set_bridge_middle_mapt, set_bridge_type_m6,
};
pub use cargo::{ALL_CARGO_TYPES, CargoStock, CargoType, OrderSettings, TEMPERATE_CARGO_TYPES};
pub use cargo_packet::{CargoPacket, StationCargoList, VehicleCargoList, load_unload_speed};
pub use cheats::CheatsState;
pub use command::{
    Command, CommandError, LevelMode, MAX_STATION_NAME_CHARS, OrderMoveDirection,
    ROAD_PLACE_FORCE_AXIS, apply_command, check_place_industry_spec, command_error_message,
    command_would_fail, finalize_road_drag_line, industry_template, infer_road_drag_axis,
    preview_road_bits_at, rail_bits_placement_target, rail_station_footprint, rail_station_layout,
    rail_trackbits_from_neighbors, road_bits_for_autoroute, road_drag_line_tiles,
    road_locked_tool_axis,
};
pub use company::{
    Company, CompanyId, FEEDER_SHARE_DEN, FEEDER_SHARE_NUM, feeder_share_of, tile_with_owner,
};
pub use depot::{depot_tile_kind_for_vehicle, nearest_depot_tile, rail_depot_mouth_dir};
pub use dev_metrics::{CargoProbeOptions, VehicleCargoReport, probe_vehicle_cargo_cycle};
pub use disaster::{DISASTER_CHECK_INTERVAL, force_disaster, tick_disasters, trigger_disaster_at};
pub use economy::{
    ANNUAL_INTEREST_RATE_PCT, CargoPaymentSpec, DEFAULT_MAX_LOAN, LOAN_INTERVAL,
    OTTD_MILLISECONDS_PER_TICK, SIM_TICKS_PER_SECOND, TICKS_PER_MONTH, TICKS_PER_TRANSIT_DAY,
    TICKS_PER_YEAR, build_object_cost, buy_land_cost, cargo_time_factor, check_bankruptcy,
    decrease_loan, increase_loan, inflation_income_factor, inflation_prices_factor,
    manhattan_distance, monthly_loan_interest, terraform_cost_per_corner, ticks_to_transit_days,
    transported_goods_income, vehicle_purchase_cost, vehicle_running_cost_per_tick,
    vehicle_sell_refund,
};
pub use economy_quarterly::{
    ECONOMY_HISTORY_QUARTERS, QuarterlyEconomyEntry, QuarterlyEconomyHistory,
    calculate_company_value, calculate_performance_rating,
};
pub use engine::{
    DepotPurchaseKind, ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_FOKKER, ENGINE_AIRCRAFT_TRICARIO,
    ENGINE_BUS_MPS, ENGINE_SHIP_COAL, ENGINE_SHIP_FERRY, ENGINE_SHIP_MPS, ENGINE_SHIP_OIL,
    ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_KIRBY, ENGINE_TRAIN_LEV1, ENGINE_TRAIN_X2001,
    ENGINE_TRUCK_MPS, ENGINE_WAGON_COAL, ENGINE_WAGON_GOODS, ENGINE_WAGON_MAIL,
    ENGINE_WAGON_PASSENGER, EngineCatalogSort, EngineDef, NEWGRF_ENGINE_ID_BASE,
    REFERENCE_PROGRESS_STEP, ROAD_ACCEL_ORIGINAL, RoadEngineFilter, accelerate_train_speed,
    aircraft_is_helicopter, decelerate_road_speed, decelerate_train_speed, default_engine_id,
    engine_available_in_year, engine_by_id, engine_catalog, engine_for_vehicle, engine_in_catalog,
    engines_for_depot_kind, engines_for_depot_kind_in, engines_for_depot_purchase, engines_of_kind,
    next_free_engine_id, progress_step_for_speed, tile_progress_length, train_acceleration,
    train_smoke_kind, train_sprite_group, update_road_speed, vanilla_engine_catalog,
};
pub use entity_history::{
    ENTITY_HISTORY_MONTHS, IndustryHistory, IndustryHistorySample, TownHistory, TownHistorySample,
};
#[allow(deprecated)]
pub use game_state::CARGO_DELIVERY_PAYMENT;
pub use game_state::IncomePopup;
pub use game_state::{
    BRIDGE_BUILD_COST_PER_TILE, BUY_LAND_BASE_PRICE, CLEAR_TILE_COST, CompanyEconomy,
    DEPOT_BUILD_COST, ECONOMY_HISTORY_MONTHS, EconomyHistory, GameState, MonthlyEconomySample,
    RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, SimStats, TERRAFORM_BASE_PRICE,
    TERRAFORM_COST, TUNNEL_BUILD_COST_PER_TILE, WAYPOINT_BUILD_COST, company_net_value,
};
pub use industry::{
    FACTORY_COAL_INPUT, FACTORY_WOOD_INPUT, INDUSTRY_PRODUCE_TICKS, Industry, IndustryKind,
    IndustrySpec, industry_produce_period_ticks,
};
pub use link_graph::{LinkEdgeKey, LinkFlowSample, LinkGraphStats};
pub use map::{
    AIRPORT_RADAR_FRAMES, GFX_COAL_MINE_TOWER_ANIMATED, GFX_COPPER_MINE_TOWER_ANIMATED,
    GFX_GOLD_MINE_TOWER_ANIMATED, GFX_OILWELL_ANIMATED_1, GFX_OILWELL_ANIMATED_2,
    GFX_OILWELL_ANIMATED_3, IndustryTileLink, MAX_TREE_OR_FIELD_STAGE, Map, MapError,
    OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND, OBJECT_TYPE_TRANSMITTER, OTTD_MP_ROAD,
    OTTD_MP_TUNNELBRIDGE, OTTD_TILETYPE_TUNNELBRIDGE, SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW,
    TREE_GROWTH_TICK_INTERVAL, Tile, TileCoord, TileKind, WaterClass,
    advance_industry_construction, advance_industry_tile_animations, airport_radar_frame,
    apply_seasonal_snow, clear_tree, effective_road_bits, inclined_slope_direction,
    industry_animation_frame, industry_construction_counter, industry_construction_stage,
    industry_gfx, industry_instance_id, industry_tile_anim_state, industry_tile_link,
    industry_tile_on_water, industry_tiles_mergeable, industry_uses_water_ground,
    is_airport_tower_tile, is_canal_tile, is_industry_completed, is_map_object_tile,
    is_owned_land_tile, is_river_tile, is_tunnel_entrance_slope, make_industry_tile_bigger,
    make_water_tile, openttd_tile_index_to_coord, partial_pixel_z, plant_tree,
    rail_foundation_for_trackbits, rail_trackbits_valid_on_slope, resolve_tunnel_end,
    river_tile_is_ship_navigable, set_industry_gfx, set_water_class_m1, slope_dz_at_subtile,
    slope_dz_on_tile, step_airport_tiles, step_industry_tiles, step_tree_and_field_growth,
    tick_tree_tile_loop, tile_adjacent_to_water, tile_has_water_class, tile_slope_and_z,
    tree_or_field_stage, tunnel_entrance_m5, tunnel_preview_path, water_class, water_class_from_m1,
};
pub use newgrf_actions::{
    ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS, Action0Header,
    Action5SlotSummary, GrfInspectReport, ParsedRoadTypeMeta, ParsedStationMeta, ParsedTrainMeta,
    apply_newgrf_action5_catenary, apply_newgrf_action5_catenary_default_dirs,
    apply_newgrf_action5_shore, apply_newgrf_action5_shore_default_dirs, apply_newgrf_road_types,
    apply_newgrf_road_types_default_dirs, apply_newgrf_stack_catalogs_default_dirs,
    apply_newgrf_stations, apply_newgrf_stations_default_dirs, apply_newgrf_vehicles_trains,
    apply_newgrf_vehicles_trains_default_dirs, build_action0_roadtype_payload,
    build_action0_station_payload, build_action0_train_payload,
    build_grf_v2_with_action0_and_action8, collect_roadtype_metas_from_grf,
    collect_station_metas_from_grf, collect_train_metas_from_grf, default_newgrf_search_dirs,
    for_each_pseudo_payload, inspect_grf_bytes, inspect_grf_file, parse_action0_header,
    parse_action0_roadtype_meta, parse_action0_station_meta, parse_action0_train_meta,
};
pub use newgrf_config::{
    GrfContainerVersion, GrfFileInfo, GrfParsed, GrfScanError, GrfStackIssue, NewGrfEntry,
    build_minimal_grf_v2, default_vanilla_stack, format_grfid, grfid_from_bytes,
    parse_grf_container, parse_grf_full, scan_grf_bytes, scan_grf_file, validate_stack,
};
pub use newgrf_sprites::{
    ACTION5_TYPE_CATENARY, ACTION5_TYPE_SHORE, Action2EvalCtx, Action2RandomEntry,
    Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm, Action5Block,
    CATENARY_ACTION5_SLOT_COUNT, CATENARY_ENTRANCE_SPRITE_BASE, CATENARY_PYLON_SPRITE_BASE,
    CATENARY_WIRE_SPRITE_BASE, DecodedSprite, SHORE_ACTION5_SLOT_COUNT, SHORE_MISSING_BLOCK_SLOTS,
    SPRITE_V2_ZOOM_PREFERENCE, TrainSpriteAssign, TrainSpriteGraphics, action5_type_name,
    apply_company_colour_mask, bake_sprite_company_mask, build_action1_feature_payload,
    build_action1_trains_payload, build_action2_single_set_payload, build_action2_stations_payload,
    build_action2_trains_payload, build_action2_trains_random, build_action2_trains_random_consist,
    build_action2_trains_variational_default, build_action2_variational_advanced_add_literal,
    build_action2_variational_default_payload, build_action2_variational_divmod_payload,
    build_action2_variational_payload, build_action2_vehicle_payload,
    build_action3_feature_payload, build_action3_trains_payload, build_grf_v2_action5_with_sprite,
    build_grf_v2_feature_with_action2_chain, build_grf_v2_roadtype_with_action2_chain,
    build_grf_v2_roadtype_with_preview_sprite, build_grf_v2_station_with_action2_chain,
    build_grf_v2_station_with_preview_sprite, build_grf_v2_train_with_action2_chain,
    build_grf_v2_train_with_chunked_sprite, build_grf_v2_train_with_compressed_sprite,
    build_grf_v2_train_with_fd_rgba_sprite, build_grf_v2_train_with_fd_sprite,
    build_grf_v2_train_with_preview_sprite, build_grf_v2_train_with_variational_chain,
    build_grf_v2_with_preview_sprite, build_real_sprite_v1_chunked,
    build_real_sprite_v1_chunked_payload, build_real_sprite_v1_compressed,
    build_real_sprite_v1_compressed_payload, build_real_sprite_v1_dims,
    build_real_sprite_v1_uncompressed, build_real_sprite_v1_uncompressed_payload,
    build_sprite_section_palette_entry, build_sprite_section_rgba_chunked_entry,
    build_sprite_section_rgba_entry, build_sprite_section_rgba_mask_entry,
    catenary_action5_local_slot, collect_action5_blocks, collect_feature_sprite_graphics,
    collect_roadtype_sprite_graphics, collect_station_sprite_graphics,
    collect_train_sprite_graphics, compress_grf_lz77_literals, decode_chunked_8bpp,
    decode_chunked_pixels, decode_real_sprite_v1, decode_real_sprite_v1_uncompressed,
    decode_real_sprite_v2_section, decode_real_sprite_v2_section_zoom, decompress_grf_lz77,
    encode_chunked_8bpp_full_rows, encode_chunked_pixels_full_rows, index_sprite_section,
    indices_to_rgba, merge_catenary_action5_block, merge_shore_action5_block, resolve_fd_sprite,
    sprite_v2_bpp,
};
pub use newgrf_type_tables::{
    GrfTypeTranslationTables, TypeLabel, collect_type_tables_from_grf,
    parse_action0_type_translation_tables, reverse_rail_type, reverse_road_type,
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
    find_path_with_wormholes, find_rail_path_for_engine, path_network_for_vehicle,
    rail_bit_for_sides, station_entrance_faces_rail, station_entrance_faces_road,
    station_site_adjacent_to_rail, station_site_adjacent_to_transport,
    station_site_tile_allows_build, station_site_tile_needs_clear, tile_is_path_traversable,
};
pub use pathfinding_settings::{
    DEFAULT_PATH_BACKOFF_INTERVAL, DEFAULT_WAIT_FOR_PBS_PATH_DAYS, PBS_WAIT_FOREVER,
    PathfindingSettings,
};
pub use rail_lane::{rail_horz_lane_bit, rail_vert_lane_bit};
pub use rail_pbs::{
    ReservedRailStep, YAPF_RESERVATION_CROSS_PENALTY, decode_rail_reservation_m2_hi,
    encode_rail_reservation_to_m2_hi, find_path_to_safe_wait,
    find_path_to_safe_wait_with_wormholes, follow_train_reservation, is_safe_waiting_position,
    rail_tile_has_pbs_reservation, reservation_ends_at_safe_wait, sync_reservations_to_map,
    tick_pbs_wait_and_maybe_reverse, train_blocked_by_reservation, train_waiting_for_pbs_path,
    update_train_reservations, update_train_reservations_with_settings,
    update_train_reservations_with_wormholes,
};
pub use rail_signals::{
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SEMAPHORE_BUILD_BEFORE_YEAR,
    SIGNAL_BUILD_COST, SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH,
    SIGTYPE_PATH_ONEWAY, SignalTrack, YAPF_PBS_BEHIND_PENALTY, YAPF_RED_SIGNAL_PENALTY,
    YapfSignalRouting, calendar_year_at_tick, clear_signal_type_bits_m2, cycle_signal_facing,
    cycle_signal_side_m3, cycle_signal_type_m2, default_signal_variant, is_pbs_signal_type,
    next_placeable_signal_type, rail_tile_is_signals, resolve_signal_track,
    signal_facing_for_orientation, signal_on_track_mask, signal_placement_for_facing,
    signal_placement_for_track, signal_type_for_track, signal_type_label, tracks_overlap,
    valid_signal_facings_track, yapf_routing_signal,
};
pub use rail_type::{
    RAIL_CONVERT_COST, RailType, engine_compatible_with_rail, engine_requires_electric,
    engine_requires_maglev, engine_requires_monorail, rail_type_from_tile, rail_types_compatible,
    required_rail_type_for_engine, set_rail_type_on_tile, tile_usable_by_rail_type,
};
pub use refit::{
    next_refit_cargo, refit_allowed, refittable_cargo_types, vehicle_hidden_from_view,
    vehicle_hidden_in_tunnel, vehicle_hidden_on_map, vehicle_in_depot,
};
pub use road_action2::action2_eval_ctx_for_road_tile;
pub use road_movement::{
    BayStationTable, VehiclePose, bay_station_table, extrapolate_vehicle_pose,
    retreat_vehicle_pose, road_turn_entry_exit, straight_subtile, train_straight_subtile,
    train_subtile_direction, turn_curve_points, vehicle_render_direction,
    vehicle_render_direction_at, vehicle_render_direction_at_with_map, vehicle_render_progress,
    vehicle_subtile, vehicle_subtile_at, vehicle_subtile_at_with_map,
    vehicle_subtile_with_progress,
};
pub use road_type::{
    RoadTramType, RoadType, RoadTypeDef, all_road_type_defs, list_road_types,
    next_free_road_type_id, road_type_def, road_type_from_tile, set_road_type_on_tile,
    set_tram_road_type_on_tile, set_tram_track_bits_on_tile, tile_has_tram_track,
    tram_road_type_from_tile, tram_track_bits, vanilla_road_type_catalog,
};
pub use sav::{
    EXPORT_SAVE_VERSION, SavContainer, SavError, SavGame, SavIndustry, SavStation, SavVehicle,
    SavVehicleKind, save as save_sav, save_to_bytes as save_sav_to_bytes,
};
pub use save::CURRENT_SAVE_VERSION;
pub use save::SaveError;
pub use save::load_from_str;
pub use score::{
    BANKRUPTCY_STREAK_LIMIT, GameOverReason, GameScore, finish_game, retire_game,
    snapshot_active_score,
};
pub use shared_orders::SharedOrderList;
pub use ship_movement::{
    LOCK_TRANSIT_TICKS, is_water_network_tile, is_water_network_tile_at, lock_sprite_level,
    maybe_start_lock_transit, ship_requires_path, tick_ship_lock_wait, water_tile_is_lock,
    water_tiles_connected,
};
pub use sign::{MAX_SIGN_NAME_CHARS, Sign};
pub use sim_events::{
    ConstructionKind, DisasterKind, SimEvent, SimEventQueue, TrainSmokeKind, VehicleRunningPhase,
};
pub use sound_id::SoundId;
pub use station::{
    CargoTimeSincePickup, MAX_TIME_SINCE_PICKUP_DAYS, STATION_COVERAGE_RADIUS, STATION_TILE_PYLONS,
    STATION_TILE_WIRES, STATION_TYPE_RAIL_WAYPOINT, Station, StationCoverage,
    StationMapCoherenceReport, StopKind, TOWN_CARGO_MIN_OWNER_RATING,
    default_station_catenary_flags, industry_in_station_coverage, is_rail_waypoint_at,
    is_rail_waypoint_tile, load_amount_for_rating, on_station_cargo_pickup,
    rail_station_approach_tile, rail_station_axis_y, rail_station_owned_tiles,
    rail_station_platform_tiles, rail_station_stop_tile, recompute_station_rating,
    resolve_order_destination, road_stop_approach_tile, station_at_tile, station_coverage_at,
    station_covers_tile, station_footprint_tiles, station_map_coherence, station_rating_for_cargo,
    station_rating_for_company_cargo, station_tile_can_have_pylons, station_tile_can_have_wires,
    station_tile_sets_adjacent, station_type_from_m6, stop_kind_from_m6, tick_station_cargo_age,
    train_on_rail_platform, vehicle_at_road_stop, vehicle_physically_at_station,
};
pub use station_action2::action2_eval_ctx_for_station_tile;
pub use station_class::{
    StationClassDef, StationClassId, StationSpecDef, StationSpecId, all_station_class_defs,
    all_station_spec_defs, list_station_classes, list_station_specs, next_free_station_class_id,
    next_free_station_spec_id, station_class_def, station_spec_def, station_spec_layout,
    vanilla_station_class_catalog, vanilla_station_spec_catalog,
};
pub use subsidy::{
    SUBSIDY_AWARDED_YEARS, SUBSIDY_OFFER_MONTHS, SUBSIDY_PAYMENT_MULTIPLIER, Subsidy,
    delivery_income_multiplier, tick_subsidies, try_award_subsidy, try_create_subsidy,
};
pub use tick::GameTick;
pub use timetable::{TRAVEL_PRESETS, WAIT_PRESETS, cycle_travel_ticks, cycle_wait_ticks};
pub use tnbp_decode::{
    JgrTunnelRecord, SlPrimitive, SlTableField, TnbpDecodeError, TnbpDecoded, decode_tnbp_blob,
    jgr_tunnels_from_decoded, read_sl_gamma, split_sl_gamma_segments, tnbp_blob_to_json_value,
};
pub use town::{
    AUTHORITY_MIN_STATION, FUND_BUILDINGS_COST, FUND_BUILDINGS_MONTHS, MAIL_PER_HOUSE,
    PASSENGERS_PER_HOUSE, STATION_TOWN_CARGO_CAPACITY, TOWN_ADVERTISE_COST, TOWN_AUTHORITY_RADIUS,
    TOWN_GROWTH_DESERT, TOWN_GROWTH_TICKS, TOWN_GROWTH_WINTER, TOWN_PRODUCE_TICKS, Town,
    TownGrowthEffect, authority_allows_new_station, grow_town_if_served,
    process_town_monthly_growth, produce_town_cargo, town_goal_satisfied, update_town_growth_state,
};
pub use town_expand::{
    TOWN_EXPAND_ATTEMPTS, TOWN_EXPAND_POP_PER_HOUSE, TOWN_EXPAND_SEARCH_RADIUS, TownExpandResult,
    expand_town_once, expand_town_physically,
};
pub use townname::generate_town_name;
pub use train_collision::{TrainCollision, detect_train_collisions, resolve_train_collisions};
pub use train_consist::{
    VEHICLE_LENGTH, action2_eval_ctx_for_unit, attach_wagon, cargo_class_bits, cargo_type_a_id,
    consist_changed, consist_head_id, consist_occupied_tiles, consist_power_hp, consist_tile_span,
    consist_unit_ids, consist_weight_t, detach_unit, engine_is_train_engine, engine_is_wagon,
    same_consist, sell_chain_ids,
};
pub use train_movement::{
    ACCEL_SLOWDOWN, AccelSlowdownParams, DELTACOORD_LEAVE_OFFSET, FRACTCOORDS_BEHIND,
    FRACTCOORDS_ENTER, RAIL_TOUCHING_SIDE_NE, RAIL_TOUCHING_SIDE_NW, RAIL_TOUCHING_SIDE_SE,
    RAIL_TOUCHING_SIDE_SW, TRAIN_UPDATE_SPEED_ACCEL_MUL, TRAIN_UPDATE_SPEED_BRAKE_MUL,
    TUNNEL_VISIBILITY_FRAME, VEHICLE_INITIAL_X_FRACT, VEHICLE_INITIAL_Y_FRACT, VEHICLE_SUBCOORD,
    VehicleSubcoord, diag_dir_index, dir_difference, is_45_degree_turn, is_diagonal_rail_piece,
    openttd_subcoord_at_entry, rail_track_index, track_bit_for_movement, train_depot_facing,
    train_depot_subtile, train_render_dir_on_rail, train_subtile_on_rail,
    tunnel_hides_train_at_progress,
};
pub use vehicle::reverse_direction;
pub use vehicle::{
    AircraftPhase, BREAKDOWN_DURATION_TICKS, DEFAULT_SERVICE_INTERVAL_DAYS, DIR_E, DIR_N, DIR_NE,
    DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, OrderConditionKind, SERVICING_RELIABILITY_THRESHOLD,
    TimetableWaitKind, VEHICLE_PROGRESS_STEP, Vehicle, VehicleDirection, VehicleKind, VehicleOrder,
    direction_from_tile_step,
};
pub use vehicle_group::{MAX_VEHICLE_GROUP_NAME_CHARS, VehicleGroup};
pub use world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, HeightmapData, PreserveRect, WorldGenConfig, apply_heightmap,
    apply_world_gen, clear_ground_m5, effective_clear_ground, initial_clear_ground, parse_hmap,
};

#[cfg(test)]
mod tests;
