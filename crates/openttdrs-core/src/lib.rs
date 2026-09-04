//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod ai;
pub mod aircraft_crash;
pub mod aircraft_movement;
pub mod airport;
pub mod airport_class;
pub mod airport_fta;
pub mod airport_tile_action2;
pub mod airport_tile_spec;
pub mod autoreplace;
pub mod badge;
pub mod bridge_spec;
pub mod canal_spec;
pub mod cargo;
pub mod cargo_monitor;
pub mod cargo_packet;
pub mod cargo_spec;
pub mod cargodist;
pub mod cheats;
pub mod command;
pub mod company;
pub mod construction_settings;
pub mod depot;
pub mod depot_leave;
pub mod dev_metrics;
pub mod disaster;
pub mod economy;
pub mod economy_quarterly;
pub mod engine;
pub mod entity_history;
pub mod fleet_index;
mod game_state;
pub mod ground_crash;
pub mod gs;
pub mod house_spec;
pub mod industry;
pub mod industry_spec;
pub mod industry_tile;
pub mod map;
pub mod newgrf_actions;
pub mod newgrf_callback;
mod newgrf_company_ramp;
pub mod newgrf_config;
mod newgrf_palette_data;
pub mod newgrf_sprites;
pub mod newgrf_type_tables;
pub mod newgrf_walk;
pub mod news;
pub mod object_spec;
pub mod ottdmap_extras;
pub mod parity;
pub mod pathfinder;
pub mod pathfinding_settings;
pub mod prelude;
pub mod rail_action2;
pub mod rail_lane;
pub mod rail_pbs;
pub mod rail_signals;
pub mod rail_type;
pub mod refit;
pub mod road_action2;
pub mod road_movement;
pub mod road_stop_action2;
pub mod road_stop_spec;
pub mod road_type;
pub mod sav;
pub mod save;
mod score;
pub mod script_cargo_monitor;
pub mod shared_orders;
pub mod ship_movement;
mod sign;
mod sim_events;
mod sim_step;
pub mod sound_effect;
pub mod sound_id;
pub mod station;
pub mod station_action2;
pub mod station_class;
pub mod subsidy;
pub mod tick;
pub mod timer;
pub mod timetable;
pub mod tnbp_decode;
pub mod town;
pub mod town_action;
pub mod town_authority_serde;
pub mod town_expand;
pub mod townname;
pub mod train_collision;
pub mod train_consist;
pub mod train_movement;
pub mod vehicle;
mod vehicle_ai;
pub mod vehicle_group;
pub mod world_gen;
pub mod world_raw;
pub mod world_semantic;

pub use ai::{
    AI_BUILD_MONEY_THRESHOLD, AiSettings, CompanyAi, DEFAULT_AI_BUILD_MONEY_THRESHOLD,
    DEFAULT_AI_MAX_ROUTES, MAX_AI_ROUTES, TransCargoAi, format_ai_debug_status, tick_ai_companies,
};
pub use aircraft_crash::{
    SHORT_STRIP_JET_CRASH_PROB, crash_airplane, maybe_crash_after_brake_tick,
    should_crash_short_strip_jet,
};
pub use aircraft_movement::{
    aircraft_requires_path, straight_line_path, tick_aircraft_phase,
    tick_aircraft_phase_with_catalog,
};
pub use airport::{
    AIRPORT_SMALL_H, AIRPORT_SMALL_W, AirportPiece, airport_loading_tile, airport_loading_tile_at,
    airport_m6_airport, airport_runway_tile, airport_small_footprint, airport_small_tiles,
    airport_spec_footprint, airport_spec_tiles, airport_station_gfx_animation_frames,
    airport_tile_is_hangar, airport_tile_is_heliport, is_airport_flag_station_gfx,
    is_airport_radar_station_gfx, newgrf_airport_tile_gfx_with_layout,
};
pub use airport_class::{
    AIRPORT_ACTION3_PURCHASE, AirportClassDef, AirportClassId, AirportFtaFlags, AirportLayoutTile,
    AirportSpecDef, AirportSpecId, AirportTileLayout, NEW_AIRPORT_OFFSET, NUM_AIRPORTS,
    NewgrfAirportSpecDef, TOWN_NOISE_POPULATION_DEFAULT, airport_allows_aircraft,
    airport_class_def, airport_noise_for_distance, airport_spec_def, all_airport_class_defs,
    all_airport_spec_defs, list_airport_classes, list_airport_specs, list_newgrf_airport_specs,
    max_town_noise, newgrf_airport_spec_def, next_free_airport_id,
};
pub use airport_fta::{
    AirportHeading, AirportMovingData, CITY_ENTRIES, CITY_MOVING_DATA, CITY_NOF_ELEMENTS,
    COMMUTER_ENTRIES, COMMUTER_MOVING_DATA, COMMUTER_NOF_ELEMENTS, COUNTRY_ENTRIES,
    COUNTRY_MOVING_DATA, COUNTRY_NOF_ELEMENTS, HELIDEPOT_ENTRIES, HELIDEPOT_MOVING_DATA,
    HELIDEPOT_NOF_ELEMENTS, HELIPORT_ENTRIES, HELIPORT_MOVING_DATA, HELIPORT_NOF_ELEMENTS,
    HELISTATION_ENTRIES, HELISTATION_MOVING_DATA, HELISTATION_NOF_ELEMENTS,
    INTERCONTINENTAL_ENTRIES, INTERCONTINENTAL_MOVING_DATA, INTERCONTINENTAL_NOF_ELEMENTS,
    INTERNATIONAL_ENTRIES, INTERNATIONAL_MOVING_DATA, INTERNATIONAL_NOF_ELEMENTS,
    METROPOLITAN_ENTRIES, METROPOLITAN_MOVING_DATA, METROPOLITAN_NOF_ELEMENTS, OILRIG_ENTRIES,
    OILRIG_MOVING_DATA, OILRIG_NOF_ELEMENTS, station_uses_airport_fta, station_uses_country_fta,
};
pub use airport_tile_action2::{
    action2_eval_ctx_for_airport_tile, action2_eval_ctx_for_airport_tile_with_towns,
};
pub use airport_tile_spec::{
    AirportAnimationTrigger, AirportTileGfxId, AirportTileSpecDef, INVALID_AIRPORT_TILE,
    NEW_AIRPORT_TILE_OFFSET, NUM_AIRPORT_TILES, empty_airport_tile_overrides,
    get_translated_airport_tile_id, next_free_airport_tile_gfx_id, resolve_airport_tile_draw_gfx,
    resolve_airport_tile_piece_gfx,
};
pub use autoreplace::{AutoReplaceRule, try_autoreplace_vehicle};
pub use badge::{
    BadgeDef, badge_def, badges_for_spec, empty_badge_catalog, find_badge_by_label, list_badges,
    next_free_badge_id, resolve_badge_labels, resolve_badge_labels_detailed,
};
pub use bridge_spec::{
    BRIDGE_SPECS, BridgePiece, BridgeSpec, BridgeSpecDef, BridgeType, bridge_above_axis_from_mapt,
    bridge_available, bridge_available_at_tick, bridge_available_at_tick_in, bridge_available_in,
    bridge_build_cost, bridge_build_cost_in, bridge_line_tiles, bridge_max_speed_for_tile,
    bridge_middle_length, bridge_spec, bridge_spec_def, bridge_total_length, bridge_type_from_m6,
    calc_bridge_piece, rail_bridge_other_end, road_bridge_other_end, set_bridge_middle_mapt,
    set_bridge_type_m6, tunnel_bridge_rail_reserved, vanilla_bridge_spec_catalog,
};
pub use canal_spec::{
    CANAL_FEATURE_COUNT, CF_BUOY, CF_DIKES, CF_DOCKS, CF_ICON, CF_LOCKS, CF_RIVER_EDGE,
    CF_RIVER_GUI, CF_RIVER_SLOPE, CF_WATERSLOPE, CanalFeatureDef, canal_feature_def,
    vanilla_canal_feature_catalog,
};
pub use cargo::{
    ALL_CARGO_TYPES, ARCTIC_CARGO_TYPES, CUSTOM_CARGO_COUNT, CUSTOM_CARGO_OFFSET, CargoStock,
    CargoType, MAX_CARGO_ID, NUM_ORIGINAL_CARGO, OrderSettings, TEMPERATE_CARGO_TYPES,
    TOYLAND_CARGO_TYPES, TROPIC_CARGO_TYPES, VANILLA_CARGO_COUNT, custom_cargo,
};
pub use cargo_monitor::{
    CargoMonitor, CargoMonitorId, CargoSource, decode_monitor_cargo, decode_monitor_company,
    decode_monitor_industry, decode_monitor_town, encode_cargo_industry_monitor,
    encode_cargo_town_monitor, monitor_monitors_industry,
};
pub use cargo_packet::{
    CargoPacket, CargoUnloadAction, StationCargoList, VehicleCargoList, choose_cargo_action,
    decide_cargo_unload_action, load_unload_speed, prepare_unload,
};
pub use cargo_spec::{
    CARGO_CALLBACK_PROFIT_CALC_MASK, CARGO_CALLBACK_STATION_RATING_CALC_MASK, CargoSpecDef,
    DEFAULT_CARGO_CAPACITY_MULTIPLIER, apply_cargo_capacity_multiplier, cargo_spec_by_label,
    cargo_spec_by_local_id, cargo_spec_def, cargo_spec_display_name, cargo_spec_for_type,
    cargo_type_from_label_with_catalog, cargo_type_label, empty_cargo_spec_catalog,
    payment_spec_for_cargo, payment_spec_for_cargo_climate,
};
pub use cheats::CheatsState;
pub use command::{
    Command, CommandError, LevelMode, MAX_STATION_NAME_CHARS, OrderMoveDirection,
    ROAD_PLACE_FORCE_AXIS, apply_command, check_place_industry_spec,
    check_place_industry_spec_def_layout, command_effects, command_would_fail,
    finalize_road_drag_line, industry_template, infer_road_drag_axis,
    place_industry_spec_def_layout_sandbox, place_industry_spec_def_sandbox, preview_road_bits_at,
    rail_bits_placement_target, rail_station_footprint, rail_station_layout,
    rail_trackbits_from_neighbors, road_bits_for_autoroute, road_drag_line_tiles,
    road_locked_tool_axis,
};
pub use company::{
    COMPANY_COLOUR_SLOTS, COMPANY_LIVERY_FLAG_PRIMARY, COMPANY_LIVERY_FLAG_SECONDARY,
    COMPANY_LIVERY_SCHEME_COUNT, Company, CompanyId, CompanyLivery, FEEDER_SHARE_DEN,
    FEEDER_SHARE_NUM, LIVERY_SCHEME_BUS, LIVERY_SCHEME_DEFAULT, LIVERY_SCHEME_DIESEL,
    LIVERY_SCHEME_DMU, LIVERY_SCHEME_ELECTRIC, LIVERY_SCHEME_EMU, LIVERY_SCHEME_FREIGHT_SHIP,
    LIVERY_SCHEME_FREIGHT_TRAM, LIVERY_SCHEME_FREIGHT_WAGON, LIVERY_SCHEME_HELICOPTER,
    LIVERY_SCHEME_LARGE_PLANE, LIVERY_SCHEME_MAGLEV, LIVERY_SCHEME_MONORAIL,
    LIVERY_SCHEME_PASSENGER_SHIP, LIVERY_SCHEME_PASSENGER_TRAM,
    LIVERY_SCHEME_PASSENGER_WAGON_DIESEL, LIVERY_SCHEME_PASSENGER_WAGON_ELECTRIC,
    LIVERY_SCHEME_PASSENGER_WAGON_MAGLEV, LIVERY_SCHEME_PASSENGER_WAGON_MONORAIL,
    LIVERY_SCHEME_PASSENGER_WAGON_STEAM, LIVERY_SCHEME_SMALL_PLANE, LIVERY_SCHEME_STEAM,
    LIVERY_SCHEME_TRUCK, MAX_COMPANIES, RIVAL_NAME_ROADHAUL, RIVAL_NAME_TRANSCARGO,
    company_colour_taken_by_other, company_id_by_name, company_livery_colours,
    company_livery_primary_colour, company_livery_secondary_colour, default_company_liveries,
    feeder_share_of, first_free_company_colour, tile_owner_colour, tile_with_owner,
    vehicle_livery_scheme,
};
pub use construction_settings::{ConstructionSettings, RoadVehicleDrivingSide, TrainSignalSide};
pub use depot::{
    DEPOT_RESERVATION_M5_BIT, clear_all_depot_reservations, depot_tile_kind_for_vehicle,
    has_depot_reservation, nearest_depot_tile, rail_depot_mouth_dir, set_depot_reservation,
};
pub use depot_leave::{
    TRAIN_DEPOT_LEAVE_WAIT_TICKS, activate_depot_leave_units, tick_train_stay_in_depot,
    ticks_to_leave_depot,
};
pub use disaster::{
    DISASTER_CHECK_INTERVAL, DisasterCraft, UFO_ALTITUDE, UFO_FLIGHT_TICKS, force_disaster,
    tick_disaster_crafts, tick_disasters, trigger_disaster_at,
};
pub use economy::{
    ANNUAL_INTEREST_RATE_PCT, CARGO_AGING_TICKS, CargoPaymentSpec, DEFAULT_MAX_LOAN,
    FluctuationEvent, GlobalEconomy, INFLATION_FRAC_ONE, LOAN_INTERVAL, MAX_INFLATION,
    ORIGINAL_BASE_YEAR, ORIGINAL_MAX_YEAR, OTTD_MILLISECONDS_PER_TICK, SIM_TICKS_PER_SECOND,
    STATION_ACCEPTANCE_TICKS, STATION_RATING_TICKS, TICKS_PER_DAY, TICKS_PER_MONTH, TICKS_PER_YEAR,
    build_object_cost, build_object_cost_factored, buy_land_cost, cargo_current_payment,
    cargo_time_factor, check_bankruptcy, decrease_loan, increase_loan, inflation_income_factor,
    inflation_prices_factor, manhattan_distance, monthly_loan_interest, rail_build_cost_factored,
    road_build_cost_factored, terraform_cost_per_corner, ticks_to_transit_periods,
    transported_goods_income, transported_goods_income_for_climate,
    transported_goods_income_with_spec, vehicle_asset_value_with_catalog, vehicle_purchase_cost,
    vehicle_purchase_cost_with_callbacks, vehicle_running_cost_per_tick, vehicle_sell_refund,
    vehicle_sell_refund_with_catalog,
};
pub use economy_quarterly::{
    ECONOMY_HISTORY_QUARTERS, QuarterlyEconomyEntry, QuarterlyEconomyHistory,
    calculate_company_value, calculate_performance_rating,
};
pub use engine::{
    DepotPurchaseKind, DoUpdateSpeedResult, ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_FOKKER,
    ENGINE_AIRCRAFT_TRICARIO, ENGINE_BUS_MPS, ENGINE_SHIP_COAL, ENGINE_SHIP_FERRY, ENGINE_SHIP_MPS,
    ENGINE_SHIP_OIL, ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_KIRBY, ENGINE_TRAIN_LEV1,
    ENGINE_TRAIN_X2001, ENGINE_TRUCK_MPS, ENGINE_WAGON_COAL, ENGINE_WAGON_GOODS, ENGINE_WAGON_MAIL,
    ENGINE_WAGON_PASSENGER, EngineCatalogSort, EngineDef, NEWGRF_ENGINE_ID_BASE,
    REFERENCE_PROGRESS_STEP, ROAD_ACCEL_ORIGINAL, ROAD_AIR_DRAG_AREA, ROAD_TRACTIVE_EFFORT_DEFAULT,
    RoadEngineFilter, RoadVehicleAccelerationModel, TrainAccelerationModel,
    VEHICLE_VISUAL_EFFECT_DEFAULT, accelerate_train_speed, aircraft_is_helicopter,
    aircraft_is_helicopter_def, aircraft_is_jet, decelerate_road_speed, decelerate_train_speed,
    default_engine_id, do_update_speed, engine_air_drag, engine_available_in_year, engine_by_id,
    engine_catalog, engine_for_vehicle, engine_in_catalog, engine_tractive_effort,
    engines_for_depot_kind, engines_for_depot_kind_in, engines_for_depot_purchase, engines_of_kind,
    get_advance_distance, get_advance_speed, get_curve_speed_limit, next_free_engine_id,
    progress_step_for_speed, road_default_air_drag, road_engine_air_drag,
    road_engine_tractive_effort, road_max_te_n, road_realistic_acceleration, road_rolling_friction,
    scale_train_air_drag, ship_speed_for_tile, ship_speed_for_tile_with_speed,
    tile_progress_length, train_acceleration, train_default_air_drag, train_max_te_n,
    train_realistic_acceleration, train_realistic_station_max_speed, train_smoke_kind,
    train_sprite_group, train_visual_progress_from_motion, train_visual_progress_from_pixel,
    update_road_speed, update_road_vehicle_speed, update_train_speed, vanilla_engine_catalog,
    vanilla_train_tractive_effort,
};
pub use entity_history::{
    ENTITY_HISTORY_MONTHS, INDUSTRY_HISTORY_RECORDS, IndustryAcceptedHistorySample,
    IndustryHistory, IndustryHistorySample, IndustryProducedHistorySample, TownHistory,
    TownHistorySample,
};
pub use fleet_index::{FleetIndex, TerminalSpatialIndex};
pub use ground_crash::{
    CRASHED_CTR_REMOVE, CRASHED_CTR_START, crash_vehicle, maybe_road_train_crash,
    road_veh_check_train_crash, tick_crashed_vehicles,
};
pub use script_cargo_monitor::ScriptCargoMonitor;
// Namespaces de compatibilidad cargodist (sin aplanar tipos en la raíz; #157).
pub mod flow_stat {
    pub use crate::cargodist::legacy::flow_stat::*;
}
pub mod link_graph {
    pub use crate::cargodist::legacy::link_graph::*;
}
pub mod linkgraph_parity {
    pub use crate::cargodist::parity::*;
}
pub mod mcf {
    pub use crate::cargodist::legacy::mcf::*;
}
#[allow(deprecated)]
pub use game_state::CARGO_DELIVERY_PAYMENT;
pub use game_state::IncomePopup;
pub use game_state::{
    BRIDGE_BUILD_COST_PER_TILE, BUY_LAND_BASE_PRICE, CLEAR_TILE_COST, CargoPaymentState,
    CompanyEconomy, DEPOT_BUILD_COST, ECONOMY_HISTORY_MONTHS, EconomyHistory, GameState,
    MonthlyEconomySample, RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, SimStats,
    SimulationRuntime, TERRAFORM_BASE_PRICE, TERRAFORM_COST, TUNNEL_BUILD_COST_PER_TILE,
    WAYPOINT_BUILD_COST, company_net_value,
};
pub use gs::{
    GsGoal, GsGoalKind, GsLeagueRow, GsState, GsStoryPage, league_rows, seed_gs_demo, tick_gs,
};
pub use industry::{
    FACTORY_GRAIN_INPUT, FACTORY_LIVESTOCK_INPUT, FACTORY_STEEL_INPUT, INDUSTRY_PRODUCE_AMOUNT,
    INDUSTRY_PRODUCE_TICKS, Industry, IndustryKind, IndustryLifeType, IndustryProductionAction,
    IndustryProductionChange, IndustrySpec, PERCENT_TRANSPORTED_60, PRODLEVEL_CLOSURE,
    PRODLEVEL_DEFAULT, PRODLEVEL_MAXIMUM, PRODLEVEL_MINIMUM, apply_industry_production_action,
    change_industry_production, industry_produce_period_ticks, remove_closed_industries,
    transport_industry_goods, transport_industry_goods_with_settings,
};
pub use industry_spec::{
    INDUSTRY_BEHAVIOUR_CARGO_TYPES_UNLIMITED_MASK, INDUSTRY_BEHAVIOUR_CUT_TREES_MASK,
    INDUSTRY_BEHAVIOUR_PLANT_FIELDS_MASK, INDUSTRY_BEHAVIOUR_PLANT_ON_BUILD_MASK,
    INDUSTRY_CALLBACK_DECIDE_COLOUR_MASK, INDUSTRY_CALLBACK_INPUT_CARGO_TYPES_MASK,
    INDUSTRY_CALLBACK_LOCATION_MASK, INDUSTRY_CALLBACK_MONTHLY_PROD_CHANGE_MASK,
    INDUSTRY_CALLBACK_OUTPUT_CARGO_TYPES_MASK, INDUSTRY_CALLBACK_PROD_CHANGE_BUILD_MASK,
    INDUSTRY_CALLBACK_PRODUCTION_256_TICKS_MASK, INDUSTRY_CALLBACK_PRODUCTION_CARGO_ARRIVAL_MASK,
    INDUSTRY_CALLBACK_PRODUCTION_CHANGE_MASK, INDUSTRY_CALLBACK_REFUSE_CARGO_MASK,
    INDUSTRY_CALLBACK_SPECIAL_EFFECT_MASK, INDUSTRY_NUM_INPUTS, INDUSTRY_NUM_OUTPUTS,
    INDUSTRY_ORIGINAL_NUM_INPUTS, INDUSTRY_ORIGINAL_NUM_OUTPUTS, INVALID_INDUSTRY,
    IndustryLayoutTile, IndustrySpecDef, IndustryTileLayout, NEW_INDUSTRY_OFFSET,
    NUM_INDUSTRY_TYPES, cargo_type_from_label, empty_industry_overrides,
    empty_industry_spec_catalog, get_cargo_translation, get_cargo_translation_for_climate,
    get_translated_industry_id, industry_spec_def, next_free_industry_id,
};
pub use industry_tile::{
    INDUSTRY_TILE_CALLBACK_ACCEPT_CARGO_MASK, INDUSTRY_TILE_CALLBACK_AUTOSLOPE_MASK,
    INDUSTRY_TILE_CALLBACK_CARGO_ACCEPTANCE_MASK, INDUSTRY_TILE_CALLBACK_DRAW_FOUNDATIONS_MASK,
    INDUSTRY_TILE_CALLBACK_SHAPE_CHECK_MASK, INDUSTRY_TILE_SPECIAL_ACCEPTS_ALL_CARGO_MASK,
    INVALID_INDUSTRY_TILE, IndustryTileGfxId, IndustryTileSpecDef, NEW_INDUSTRY_TILE_OFFSET,
    NUM_INDUSTRY_TILES, empty_industry_tile_overrides, get_clean_industry_gfx,
    get_translated_industry_tile_id, industry_tile_slope_refused, industry_tile_spec_def,
    next_free_industry_tile_gfx_id, resolve_industry_tile_draw_gfx,
};
pub use map::{
    AIRPORT_RADAR_FRAMES, FOUNDATION_ACTION5_SPRITE_BASE, FOUNDATION_INCLINED_X,
    FOUNDATION_INCLINED_Y, FOUNDATION_LEVELED, FOUNDATION_ORIGINAL_SPRITE_BASE, FloodingBehaviour,
    FoundationSpriteBounds, GFX_BUBBLE_GENERATOR, GFX_COAL_MINE_TOWER_ANIMATED,
    GFX_COPPER_MINE_TOWER_ANIMATED, GFX_GOLD_MINE_TOWER_ANIMATED, GFX_OILWELL_ANIMATED_1,
    GFX_OILWELL_ANIMATED_2, GFX_OILWELL_ANIMATED_3, INDUSTRY_RANDOM_TRIGGERS_MASK,
    IndustryAnimationTrigger, IndustryRandomTrigger, IndustryTileLink, MAX_TREE_OR_FIELD_STAGE,
    Map, MapError, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND, OBJECT_TYPE_STATUE_COMPANY,
    OBJECT_TYPE_TRANSMITTER, OTTD_MP_RAILWAY, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE,
    OTTD_TILETYPE_TUNNELBRIDGE, RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER,
    RAIL_TB_RIGHT, RAIL_TB_UPPER, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_DEPOT,
    RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, RAIL_TOUCHING_SIDE_NE, RAIL_TOUCHING_SIDE_NW,
    RAIL_TOUCHING_SIDE_SE, RAIL_TOUCHING_SIDE_SW, RailFoundationDrawPlan, RailFoundationSpriteDraw,
    RailTrackDrawPlan, RailTrackSpritePass, SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_STEEP, SLOPE_SW,
    TILE_PIXEL_HEIGHT, TREE_GROWTH_TICK_INTERVAL, Tile, TileCoord, TileKind, WaterClass,
    action2_eval_ctx_for_industry_tile_with_world,
    action2_eval_ctx_for_industry_tile_with_world_and_parent, advance_industry_animated_tiles,
    advance_industry_construction, advance_industry_construction_tile_loop_at,
    advance_industry_tile_animations, advance_industry_tile_loop_events,
    advance_industry_tile_loop_events_from_visits_with_rng,
    advance_industry_tile_randomisation_from_visits_with_catalog,
    advance_industry_tile_randomisation_from_visits_with_catalog_and_world,
    advance_newgrf_industry_animated_tiles, advance_newgrf_industry_animated_tiles_with_world,
    advance_newgrf_industry_animation_frames, advance_newgrf_industry_animation_frames_with_world,
    airport_radar_frame, apply_desert_transition_from_visits, apply_seasonal_snow,
    bridge_foundation_for_axis, bridge_surface_slope_and_z, bubble_generator_spawns_from_visits,
    clear_tree, coord_from_linear_index, coord_to_dense_index, coord_to_linear_index,
    do_flood_tile, effective_rail_trackbits, effective_road_bits, flood_vehicles,
    foundation_draw_plan, get_flooding_behaviour, inclined_slope_direction,
    industry_animation_frame, industry_construction_counter, industry_construction_stage,
    industry_gfx, industry_instance_id, industry_random_bits, industry_random_triggers,
    industry_tile_anim_state, industry_tile_link, industry_tile_on_water, industry_tile_rng,
    industry_tiles_mergeable, industry_uses_water_ground, init_industry_tile_random,
    is_airport_tower_tile, is_canal_tile, is_industry_completed, is_map_object_tile,
    is_newgrf_object_type, is_newgrf_object_type_id, is_owned_land_tile, is_river_tile,
    is_tropic_desert_zone, is_tunnel_entrance_slope, lift_destination, lift_has_destination,
    lift_position, make_industry_tile_bigger, make_shore_tile, make_water_tile,
    object_footprint_at, object_footprint_tiles, object_id_from_tile, object_origin_from_tile,
    object_spec_id_from_tile, object_type_dims, object_type_dims_id, object_view_index_for_tile,
    object_view_index_for_type, openttd_tile_index_to_coord, opposite_diag_dir, partial_pixel_z,
    plant_tree, process_water_flood_from_visits, rail_bit_for_sides, rail_bits_touching_side,
    rail_foundation_draw_plan, rail_foundation_for_trackbits, rail_signal_diag_dir_offset,
    rail_surface_slope_and_z, rail_tile_is_signals, rail_track_draw_plan,
    rail_trackbits_valid_on_slope, rail_traversal_bits, resolve_tunnel_end,
    river_tile_is_ship_navigable, set_industry_gfx, set_industry_random_bits,
    set_industry_random_triggers, set_water_class_m1, slope_dz_at_subtile, slope_dz_on_tile,
    slope_pixel_z, step_airport_tiles, step_industry_tiles, step_industry_tiles_with_seed,
    step_industry_tiles_with_seed_and_catalog, step_industry_tiles_with_seed_and_catalog_and_world,
    step_newgrf_station_tiles, step_newgrf_station_tiles_with_world,
    step_newgrf_station_tiles_with_world_and_cargo_catalog, step_tree_and_field_growth,
    tick_tree_tile_loop, tick_water_flood, tile_adjacent_to_water, tile_has_water_class,
    tile_loop_clear_desert, tile_loop_water_at, tile_slope_and_z, tree_or_field_stage,
    trigger_industry_randomisation_at, trigger_industry_randomisation_at_with_catalog_and_world,
    trigger_industry_tile_randomisation, trigger_newgrf_industry_animation,
    trigger_newgrf_industry_animation_with_world,
    trigger_newgrf_industry_animation_with_world_and_extra, trigger_newgrf_station_animation,
    trigger_newgrf_station_animation_for_platform,
    trigger_newgrf_station_animation_for_platform_with_world,
    trigger_newgrf_station_animation_for_platform_with_world_and_cargo_catalog,
    trigger_newgrf_station_animation_for_station,
    trigger_newgrf_station_animation_for_station_with_world,
    trigger_newgrf_station_animation_for_station_with_world_and_cargo_catalog,
    trigger_newgrf_station_animation_with_world,
    trigger_newgrf_station_animation_with_world_and_cargo_catalog, tunnel_entrance_m5,
    tunnel_preview_path, water_class, water_class_from_m1,
};
// Runtime NewGRF en raíz; builders/fixtures vía `newgrf_actions` / `newgrf_sprites::fixture` (#157).
pub use house_spec::{
    DEFAULT_HOUSE_AVAILABILITY, DEFAULT_HOUSE_PROBABILITY, HOUSE_CALLBACK_ALLOW_CONSTRUCTION_MASK,
    HOUSE_CALLBACK_DRAW_FOUNDATIONS_MASK, HOUSE_YEAR_MAX, HouseLookup, HouseScopeCounts, HouseSpec,
    HouseSpecDef, INVALID_HOUSE, NEW_HOUSE_OFFSET, NUM_HOUSES, action2_eval_ctx_for_house_tile,
    action2_eval_ctx_for_house_tile_with_counts, action2_eval_ctx_for_house_tile_with_map,
    action2_eval_ctx_for_house_tile_with_towns, empty_house_overrides, empty_house_spec_catalog,
    get_town_radius_group, get_translated_house_id, house_footprint_offsets, house_spec_def,
    next_free_house_id, pick_town_house_id, pick_town_house_id_with_catalog, resolve_house_draw_id,
    vanilla_or_newgrf_house,
};
pub use map::{
    ObjectScopeCounts, action2_eval_ctx_for_object_tile,
    action2_eval_ctx_for_object_tile_with_counts, action2_eval_ctx_for_object_tile_with_map,
    action2_eval_ctx_for_object_tile_with_towns, action2_eval_ctx_for_object_tile_with_world,
    object_origin_from_tile_with_objects,
};
pub use newgrf_actions::{
    ACTION0_FEATURE_BADGES, ACTION0_FEATURE_BRIDGES, ACTION0_FEATURE_CANALS,
    ACTION0_FEATURE_CARGOES, ACTION0_FEATURE_HOUSES, ACTION0_FEATURE_INDUSTRIES,
    ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_OBJECTS, ACTION0_FEATURE_RAILTYPES,
    ACTION0_FEATURE_ROADSTOPS, ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_SOUNDS,
    ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS, ACTION0_FEATURE_TRAMTYPES, Action0Header,
    Action5SlotSummary, GrfInspectReport, ParsedBadgeMeta, ParsedBridgeMeta, ParsedCanalMeta,
    ParsedCargoMeta, ParsedHouseMeta, ParsedIndustryMeta, ParsedIndustryTileMeta, ParsedObjectMeta,
    ParsedRailTypeMeta, ParsedRoadStopMeta, ParsedRoadTypeMeta, ParsedSoundMeta, ParsedStationMeta,
    ParsedTrainMeta, apply_newgrf_action5_airport_preview,
    apply_newgrf_action5_airport_preview_default_dirs, apply_newgrf_action5_all_default_dirs,
    apply_newgrf_action5_bridge_decks, apply_newgrf_action5_bridge_decks_default_dirs,
    apply_newgrf_action5_canals, apply_newgrf_action5_canals_default_dirs,
    apply_newgrf_action5_catenary, apply_newgrf_action5_catenary_default_dirs,
    apply_newgrf_action5_foundations, apply_newgrf_action5_foundations_default_dirs,
    apply_newgrf_action5_oneway, apply_newgrf_action5_oneway_default_dirs,
    apply_newgrf_action5_openttd_gui, apply_newgrf_action5_openttd_gui_default_dirs,
    apply_newgrf_action5_roadstops, apply_newgrf_action5_roadstops_default_dirs,
    apply_newgrf_action5_shore, apply_newgrf_action5_shore_default_dirs,
    apply_newgrf_action5_signals, apply_newgrf_action5_signals_default_dirs, apply_newgrf_badges,
    apply_newgrf_badges_default_dirs, apply_newgrf_bridges, apply_newgrf_bridges_default_dirs,
    apply_newgrf_canals, apply_newgrf_canals_default_dirs, apply_newgrf_cargoes,
    apply_newgrf_cargoes_default_dirs, apply_newgrf_houses, apply_newgrf_houses_default_dirs,
    apply_newgrf_industries, apply_newgrf_industries_default_dirs, apply_newgrf_industry_tiles,
    apply_newgrf_industry_tiles_default_dirs, apply_newgrf_objects,
    apply_newgrf_objects_default_dirs, apply_newgrf_rail_signals,
    apply_newgrf_rail_signals_default_dirs, apply_newgrf_road_types,
    apply_newgrf_road_types_default_dirs, apply_newgrf_roadstops,
    apply_newgrf_roadstops_default_dirs, apply_newgrf_sounds, apply_newgrf_sounds_default_dirs,
    apply_newgrf_stack_catalogs_default_dirs, apply_newgrf_stations,
    apply_newgrf_stations_default_dirs, apply_newgrf_vehicles_trains,
    apply_newgrf_vehicles_trains_default_dirs, inspect_grf_bytes, inspect_grf_file,
    parse_action0_badge_meta, parse_action0_bridge_meta, parse_action0_canal_meta,
    parse_action0_cargo_meta, parse_action0_header, parse_action0_house_meta,
    parse_action0_industry_meta, parse_action0_industry_tile_meta, parse_action0_object_meta,
    parse_action0_railtype_metas, parse_action0_roadstop_meta, parse_action0_roadtype_meta,
    parse_action0_sound_meta, parse_action0_station_meta, parse_action0_train_meta,
};
pub use newgrf_callback::{
    IndustryProductionCallbackResult, IndustryTileCargoAcceptance, RoadStopCallbackWorld,
    Vehicle32DayCallback, VehicleColourMapping, VehicleSoundOverride, VehicleVisualEffectKind,
    action2_eval_ctx_from_station, action2_eval_ctx_from_vehicle, advance_road_stop_animation,
    advance_road_stop_animation_at_with_world, apply_house_construction_callback,
    apply_house_construction_callback_for_build, apply_industry_dynamic_cargo_callbacks,
    apply_industry_dynamic_cargo_callbacks_with_catalog, apply_industry_location_callback,
    apply_industry_location_callback_for_build, apply_industry_production_callback,
    apply_industry_production_callback_with_catalog, apply_industry_tile_anim_callback,
    apply_industry_tile_autoslope_callback, apply_industry_tile_shape_callback_for_build,
    apply_object_slope_callback, apply_object_slope_callback_for_build,
    apply_station_availability_callback, apply_station_availability_callback_for_build,
    apply_vehicle_start_stop_callback, callback_allows_8bit_boolean, callback_allows_location,
    callback_allows_placement, callback_draws_default_foundation,
    effective_vehicle_max_speed_with_catalog, engine_for_vehicle_catalog,
    industry_tile_autoslope_callback_allows, industry_tile_shape_callback_allows,
    resolve_callback_or_failed, resolve_cargo_profit_callback,
    resolve_cargo_station_rating_callback, resolve_industry_decide_colour_callback,
    resolve_industry_production_change_build_callback, resolve_industry_production_change_callback,
    resolve_industry_refuse_cargo_callback, resolve_industry_refuse_cargo_callback_with_catalog,
    resolve_industry_special_effect_callback, resolve_industry_tile_animation_callback,
    resolve_industry_tile_animation_callback_with_world,
    resolve_industry_tile_cargo_acceptance_callback_with_world,
    resolve_industry_tile_cargo_acceptance_callback_with_world_and_cargo_catalog,
    resolve_industry_tile_random_trigger, resolve_vehicle_32day_callback, resolve_vehicle_callback,
    resolve_vehicle_capacity_property_callback, resolve_vehicle_colour_mapping_callback,
    resolve_vehicle_modify_property_callback, resolve_vehicle_sound_callback,
    resolve_vehicle_visual_effect_callback, trigger_road_stop_animation,
    trigger_road_stop_animation_at_with_world, trigger_road_stop_randomisation_at_with_world,
    trigger_vehicle_randomisation, trigger_vehicle_randomisation_chain, vehicle_cost_factor,
    vehicle_max_speed, vehicle_power_hp, vehicle_start_stop_callback_allows,
    vehicle_tractive_effort, vehicle_visual_effect_kind, vehicle_weight_t,
    writeback_industry_persistent_registers, writeback_industry_tile_parent_persistent_registers,
    writeback_station_persistent_registers, writeback_town_persistent_registers,
    writeback_vehicle_persistent_registers,
};
pub use newgrf_config::{
    GrfContainerVersion, GrfFileInfo, GrfParsed, GrfScanError, GrfStackIssue, MAX_NEWGRF_PARAMS,
    NewGrfEntry, default_vanilla_stack, format_grfid, grfid_from_bytes, parse_grf_container,
    parse_grf_full, scan_grf_bytes, scan_grf_file, stack_params_for_grfid, validate_stack,
};
pub use newgrf_sprites::{
    ACTION5_TYPE_AIRPORT_PREVIEW, ACTION5_TYPE_BRIDGE_DECKS, ACTION5_TYPE_CANALS,
    ACTION5_TYPE_CATENARY, ACTION5_TYPE_FOUNDATIONS, ACTION5_TYPE_ONEWAY, ACTION5_TYPE_OPENTTD_GUI,
    ACTION5_TYPE_ROADSTOPS, ACTION5_TYPE_SHORE, ACTION5_TYPE_SIGNALS, ACTION5_TYPE_TRAMWAY,
    ACTION5_TYPE_TWOCC, AIRPORT_PREVIEW_ACTION5_SLOT_COUNT, Action2EvalCtx, Action2RandomEntry,
    Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm, Action5Block,
    Action5LoadContext, BRIDGE_DECKS_ACTION5_SLOT_COUNT, CALLBACK_FAILED, CANALS_ACTION5_LOCK_SLOT,
    CANALS_ACTION5_SLOT_COUNT, CATENARY_ACTION5_SLOT_COUNT, CATENARY_ENTRANCE_SPRITE_BASE,
    CATENARY_PYLON_SPRITE_BASE, CATENARY_WIRE_SPRITE_BASE, CBID_AIRPTILE_ANIMATION_NEXT_FRAME,
    CBID_AIRPTILE_ANIMATION_SPEED, CBID_AIRPTILE_ANIMATION_TRIGGER, CBID_AIRPTILE_DRAW_FOUNDATIONS,
    CBID_CARGO_PROFIT_CALC, CBID_CARGO_STATION_RATING_CALC, CBID_HOUSE_ALLOW_CONSTRUCTION,
    CBID_HOUSE_DRAW_FOUNDATIONS, CBID_INDTILE_ANIM_NEXT_FRAME, CBID_INDTILE_ANIMATION_NEXT_FRAME,
    CBID_INDTILE_ANIMATION_SPEED, CBID_INDTILE_ANIMATION_TRIGGER, CBID_INDTILE_AUTOSLOPE,
    CBID_INDTILE_DRAW_FOUNDATIONS, CBID_INDTILE_SHAPE_CHECK, CBID_INDUSTRY_DECIDE_COLOUR,
    CBID_INDUSTRY_INPUT_CARGO_TYPES, CBID_INDUSTRY_LOCATION, CBID_INDUSTRY_OUTPUT_CARGO_TYPES,
    CBID_INDUSTRY_SPECIAL_EFFECT, CBID_OBJECT_LAND_SLOPE_CHECK, CBID_STATION_ANIMATION_NEXT_FRAME,
    CBID_STATION_ANIMATION_SPEED, CBID_STATION_ANIMATION_TRIGGER, CBID_STATION_AVAILABILITY,
    CBID_STATION_BUILD_TILE_LAYOUT, CBID_STATION_DRAW_TILE_LAYOUT, CBID_STATION_LAND_SLOPE_CHECK,
    CBID_VEHICLE_32DAY_CALLBACK, CBID_VEHICLE_AUTOREPLACE_SELECTION, CBID_VEHICLE_COLOUR_MAPPING,
    CBID_VEHICLE_MODIFY_PROPERTY, CBID_VEHICLE_SOUND_EFFECT, CBID_VEHICLE_START_STOP_CHECK,
    CBID_VEHICLE_VISUAL_EFFECT, DecodedSprite, FOUNDATION_ACTION5_SLOT_COUNT,
    ONEWAY_ACTION5_SLOT_COUNT, OPENTTD_GUI_ACTION5_SLOT_COUNT, ROADSTOP_ACTION5_SLOT_COUNT,
    SHORE_ACTION5_SLOT_COUNT, SHORE_MISSING_BLOCK_SLOTS, SIGNAL_ACTION5_SLOT_COUNT,
    SPR_SIGNALS_ACTION5_BASE, SPRITE_V2_ZOOM_PREFERENCE, TRAMWAY_ACTION5_SLOT_COUNT,
    TWOCC_ACTION5_SLOT_COUNT, TWOCC_PALETTE_BASE, TrainSpriteAssign, TrainSpriteGraphics,
    action5_type_name, airport_preview_action5_slot, apply_company_colour_mask,
    bake_sprite_company_mask, bake_sprite_company_palette, bake_sprite_crash,
    bake_sprite_two_company_palette, bake_sprite_two_company_palette_with_map,
    bridge_decks_action5_base, bridge_decks_action5_slot, catenary_action5_local_slot,
    collect_action5_blocks, collect_active_action5_blocks, collect_airport_sprite_graphics,
    collect_airport_tile_sprite_graphics, collect_canal_sprite_graphics,
    collect_cargo_sprite_graphics, collect_feature_sprite_graphics, collect_house_sprite_graphics,
    collect_industry_sprite_graphics, collect_industry_tile_sprite_graphics,
    collect_object_sprite_graphics, collect_railtype_sprite_graphics,
    collect_roadstop_sprite_graphics, collect_roadtype_sprite_graphics,
    collect_station_sprite_graphics, collect_train_sprite_graphics, disallowed_road_directions,
    foundation_action5_slot_for_sprite_id, merge_action5_offset_block,
    merge_airport_preview_action5_block, merge_bridge_decks_action5_block,
    merge_canals_action5_block, merge_catenary_action5_block, merge_foundation_action5_block,
    merge_oneway_action5_block, merge_openttd_gui_action5_block, merge_roadstop_action5_block,
    merge_shore_action5_block, merge_signals_action5_block, merge_tramway_action5_block,
    merge_twocc_action5_block, oneway_action5_slot, roadstop_action5_slot, signal_action5_slot,
};
pub use newgrf_type_tables::{
    GrfTypeTranslationTables, TypeLabel, cargo_from_local_id_with_catalog,
    collect_type_tables_from_grf, local_cargo_id_with_catalog,
    parse_action0_type_translation_tables, reverse_rail_type, reverse_road_type,
};
pub use news::{
    CALENDAR_BASE_YEAR, NEWS_MAX_AGE_DAYS, NewsDisplayMode, NewsDisplaySettings, NewsItem,
    NewsQueue, NewsReference, NewsType, PendingNewsEvent, VehicleAdviceKind, add_news_item,
    calendar_day_index, calendar_day_index_from_state, calendar_year_day, cargo_display_name,
    default_display_for_type, format_calendar_date, format_calendar_date_from_state,
    format_calendar_day_index, format_money, maybe_purge_old_news, news_display_mode_label,
    news_type_label, poll_vehicle_advice_news, purge_old_news_items, push_cargo_delivery_news,
    push_first_vehicle_running_news, push_rival_achievement_news, push_vehicle_advice_news,
    tick_for_calendar_year, vehicle_kind_label,
};
pub use object_spec::{
    DEFAULT_OBJECT_BUILD_COST_FACTOR, DEFAULT_OBJECT_CLIMATE_MASK, NEW_OBJECT_OFFSET,
    OBJECT_CALLBACK_SLOPE_CHECK_MASK, OBJECT_SIZE_1X1, ObjectSpecDef, empty_object_spec_catalog,
    is_selectable_object_spec, list_1x1_object_specs, list_buildable_object_specs,
    next_free_object_spec_id, object_size_is_1x1, object_spec_def,
};
pub use ottdmap_extras::{OttdmapExtras, dense_payload_end};
pub use pathfinder::{
    PathCache, PathNetwork, TunnelWormholes, diag_dir_offset, find_path, find_path_cached,
    find_path_with_wormholes, find_rail_build_path, find_rail_path_for_engine,
    path_network_for_vehicle, station_entrance_faces_rail, station_entrance_faces_road,
    station_site_adjacent_to_rail, station_site_adjacent_to_transport,
    station_site_tile_allows_build, station_site_tile_needs_clear, tile_allows_rail_build,
    tile_is_path_traversable,
};
pub use pathfinding_settings::{
    DEFAULT_PATH_BACKOFF_INTERVAL, DEFAULT_WAIT_FOR_PBS_PATH_DAYS, DEFAULT_WAIT_ONEWAY_SIGNAL_DAYS,
    DEFAULT_WAIT_TWOWAY_SIGNAL_DAYS, PBS_WAIT_FOREVER, PathfindingSettings,
};
pub use rail_action2::action2_eval_ctx_for_rail_tile;
pub use rail_lane::{
    autorail_drag_uses_x_axis, autorail_trackbit_from_fract, rail_horz_lane_bit, rail_vert_lane_bit,
};
pub use rail_pbs::{
    ChosenTrainTrack, ReservedRailStep, YAPF_RESERVATION_CROSS_PENALTY,
    choose_train_track_on_enter, decode_rail_reservation_m2_hi, encode_rail_reservation_to_m2_hi,
    find_path_to_safe_wait, find_path_to_safe_wait_with_wormholes, follow_train_reservation,
    free_train_track_reservation, is_safe_waiting_position, platform_track_reserved_or_occupied,
    rail_tile_has_pbs_reservation, reservation_ends_at_safe_wait, sync_reservations_to_map,
    tick_pbs_wait_and_maybe_reverse, tick_signal_wait_and_maybe_reverse,
    train_blocked_by_reservation, train_waiting_for_pbs_path, try_path_reserve,
    update_train_reservations, update_train_reservations_with_settings,
    update_train_reservations_with_wormholes,
};
pub use rail_signals::{
    RAIL_REMOVE_REFUND, SEMAPHORE_BUILD_BEFORE_YEAR, SIGNAL_BUILD_COST, SIGTYPE_BLOCK,
    SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH, SIGTYPE_PATH_ONEWAY, SignalTrack,
    YAPF_PBS_BEHIND_PENALTY, YAPF_RED_SIGNAL_PENALTY, YapfSignalRouting, calendar_year_at_tick,
    clear_signal_type_bits_m2, cycle_signal_facing, cycle_signal_side_m3, cycle_signal_type_m2,
    default_signal_variant, is_pbs_signal_type, m2_for_signal, next_placeable_signal_type,
    rail_signal_present_mask, rail_signal_state_mask, resolve_signal_track,
    signal_facing_for_orientation, signal_on_track_mask, signal_placement_for_facing,
    signal_placement_for_track, signal_type_for_track, signal_type_label, signal_variant_for_track,
    tracks_overlap, valid_signal_facings_track, yapf_routing_signal,
};
pub use rail_type::{
    RAIL_CONVERT_COST, RAIL_SPRITE_TYPE_DEPOT, RAIL_SPRITE_TYPE_GROUND,
    RAIL_SPRITE_TYPE_GROUND_COMPLETE, RAIL_SPRITE_TYPE_SIGNALS, RAIL_SPRITE_TYPE_TRACK_OVERLAY,
    RAIL_SPRITE_TYPE_TUNNEL, RAIL_SPRITE_TYPE_TUNNEL_PORTAL, RAIL_SPRITE_TYPE_UNDERLAY,
    RAIL_TYPE_FLAG_NO_SPRITE_COMBINE, RailSignalSpriteSpec, RailType, RailTypeRuntimeProps,
    engine_compatible_with_rail, engine_requires_electric, engine_requires_maglev,
    engine_requires_monorail, powered_railtypes_mask, powered_railtypes_mask_with_props,
    rail_build_cost_multiplier, rail_type_bit, rail_type_from_tile, rail_type_track_speed_cap,
    rail_types_compatible, rail_types_compatible_with_props, railtypes_mask_contains,
    required_rail_type_for_engine, set_rail_type_on_tile, tile_usable_by_rail_type,
};
pub use refit::{
    next_refit_cargo, refit_allowed, refittable_cargo_types, refittable_cargo_types_for_engine,
    refittable_cargo_types_for_engine_with_catalog, refittable_cargo_types_with_catalog,
    vehicle_hidden_from_view, vehicle_hidden_in_tunnel, vehicle_hidden_on_map, vehicle_in_depot,
};
pub use road_action2::action2_eval_ctx_for_road_tile;
pub use road_movement::{
    BayStationTable, VehiclePose, bay_station_table, extrapolate_vehicle_pose,
    retreat_vehicle_pose, road_turn_entry_exit, straight_subtile, train_straight_subtile,
    train_subtile_direction, turn_curve_points, vehicle_render_direction,
    vehicle_render_direction_at, vehicle_render_direction_at_with_map, vehicle_render_progress,
    vehicle_sprite_direction_at, vehicle_sprite_direction_at_with_map, vehicle_subtile,
    vehicle_subtile_at, vehicle_subtile_at_with_map, vehicle_subtile_with_progress,
};
pub use road_stop_action2::{
    RoadStopWorldContext, action2_eval_ctx_for_road_stop_tile,
    action2_eval_ctx_for_road_stop_tile_with_catalog,
    action2_eval_ctx_for_road_stop_tile_with_catalog_and_road_types,
    action2_eval_ctx_for_road_stop_tile_with_catalog_and_world,
};
pub use road_stop_spec::{
    ROADSTOP_ANIMATION_TRIGGER_ACCEPTANCE_TICK, ROADSTOP_ANIMATION_TRIGGER_BUILT,
    ROADSTOP_ANIMATION_TRIGGER_CARGO_TAKEN, ROADSTOP_ANIMATION_TRIGGER_NEW_CARGO,
    ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP, ROADSTOP_ANIMATION_TRIGGER_VEHICLE_ARRIVES,
    ROADSTOP_ANIMATION_TRIGGER_VEHICLE_DEPARTS, ROADSTOP_ANIMATION_TRIGGER_VEHICLE_LOADS,
    ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME, ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED,
    ROADSTOP_CALLBACK_MASK_AVAILABILITY, ROADSTOP_DRAW_MODE_DEFAULT, ROADSTOP_DRAW_MODE_OVERLAY,
    ROADSTOP_DRAW_MODE_ROAD, ROADSTOP_DRAW_MODE_WAYP_GROUND, ROADSTOP_FLAG_CB141_RANDOM_BITS,
    ROADSTOP_FLAG_DRAW_MODE_REGISTER, ROADSTOP_FLAG_DRIVE_THROUGH_ONLY, ROADSTOP_FLAG_NO_CATENARY,
    ROADSTOP_FLAG_ROAD_ONLY, ROADSTOP_FLAG_TRAM_ONLY, ROADSTOP_TYPE_ALL, ROADSTOP_TYPE_BUS,
    ROADSTOP_TYPE_TRUCK, RSV_BAY_NE, RSV_BAY_NW, RSV_BAY_SE, RSV_BAY_SW, RSV_DRIVE_THROUGH_X,
    RSV_DRIVE_THROUGH_Y, RoadStopClassDef, RoadStopSpecDef, drive_through_axis_y,
    empty_road_stop_class_catalog, empty_road_stop_spec_catalog, first_matching_road_stop_spec,
    is_drive_through_orientation, list_road_stop_classes, list_road_stop_specs,
    next_free_road_stop_class_id, next_free_road_stop_spec_id, road_stop_class_def,
    road_stop_spec_by_grf_local, road_stop_spec_def,
};
pub use road_type::{
    RoadTramType, RoadType, RoadTypeDef, all_road_type_defs, list_road_types,
    next_free_road_type_id, road_type_def, road_type_from_tile, set_road_type_on_tile,
    set_tram_road_type_on_tile, set_tram_track_bits_on_tile, tile_has_tram_track,
    tram_road_type_from_tile, tram_track_bits, vanilla_road_type_catalog,
};
pub use sav::{
    EXPORT_SAVE_VERSION, SavCargoPacket, SavContainer, SavError, SavGame, SavIndustry,
    SavIndustryAcceptedCargo, SavIndustryAcceptedHistory, SavIndustryProducedCargo,
    SavIndustryProducedHistory, SavOpaqueChunk, SavPersistentStorage, SavStation, SavStationCargo,
    SavVehicle, SavVehicleKind, house_spec_is_size_1x1, house_spec_population, save as save_sav,
    save_to_bytes as save_sav_to_bytes,
};
pub use save::CURRENT_SAVE_VERSION;
pub use save::SaveError;
pub use save::load_from_str;
pub use score::{
    BANKRUPTCY_STREAK_LIMIT, GameOverReason, GameScore, finish_game, retire_game,
    snapshot_active_score,
};
pub use shared_orders::SharedOrderList;
#[allow(deprecated)]
pub use ship_movement::{
    LOCK_TRANSIT_TICKS, SHIP_ACCELERATION_DEFAULT, SHIP_SUBCOORD, ShipLockOccupancy,
    ShipSubcoordData, choose_ship_track, find_closest_ship_depot, is_water_network_tile,
    is_water_network_tile_at, lock_sprite_level, release_ship_lock, ship_accelerate,
    ship_arrival_ready, ship_controller_tick, ship_controller_tick_with_catalog,
    ship_lock_occupancy_allows, ship_move_up_down_on_lock, ship_requires_path, ship_subcoord,
    try_claim_ship_lock, water_tile_is_lock, water_tiles_connected,
};
pub use sign::{MAX_SIGN_NAME_CHARS, Sign, SignOwner};
pub use sim_events::{
    ConstructionKind, DisasterKind, SimEvent, SimEventQueue, TrainSmokeKind, VehicleRunningPhase,
    VehicleSoundEvent,
};
pub use sim_step::{TickPhaseTimings, step_profiled};
pub use sound_effect::{
    CollectedSoundSamples, PendingNewgrfSound, SoundEffectDef, SoundPlayError, clamp_sound_volume,
    collect_sound_samples_from_grf, effective_volume, empty_sound_effect_catalog,
    play_newgrf_sound, play_sound_or_override, sound_effect_def,
};
pub use sound_id::SoundId;
pub use station::{
    CargoTimeSincePickup, GoodsEntry, INITIAL_STATION_RATING, MAX_TIME_SINCE_PICKUP_DAYS,
    STATION_COVERAGE_RADIUS, STATION_RATING_MAX_STEP, STATION_TILE_PYLONS,
    STATION_TILE_RESERVATION, STATION_TILE_WIRES, STATION_TYPE_DOCK, STATION_TYPE_OILRIG,
    STATION_TYPE_RAIL_WAYPOINT, Station, StationCoverage, StationGoods, StationMapCoherenceReport,
    StationVisit, StopKind, TOWN_CARGO_MIN_OWNER_RATING, can_move_goods_to_station,
    default_station_catenary_flags, industry_in_station_coverage, is_rail_station_type,
    is_rail_waypoint_at, is_rail_waypoint_tile, load_amount_for_rating, move_goods_to_station,
    note_station_load_attempt, on_station_cargo_pickup, pick_stop_tile, platform_past_stop_tiles,
    rail_station_approach_tile, rail_station_axis_y, rail_station_owned_tiles,
    rail_station_platform_tiles, rail_station_platform_track_tiles, rail_station_stop_candidates,
    rail_station_stop_candidates_osl, rail_station_stop_tile, rail_station_stop_tile_for_approach,
    rail_station_stop_tile_for_approach_osl, rail_station_stop_tile_with_osl,
    recompute_station_rating, resolve_order_destination, resolve_order_destination_from,
    road_stop_approach_tile, station_accepts_cargo_with_newgrf,
    station_accepts_cargo_with_newgrf_and_cargo_catalog, station_at_tile, station_catchment_radius,
    station_coverage_at, station_coverage_at_with_newgrf,
    station_coverage_at_with_newgrf_and_cargo_catalog, station_coverage_for, station_covers_tile,
    station_footprint_tiles, station_map_coherence, station_rating_for_cargo,
    station_rating_for_company_cargo, station_tile_can_have_pylons, station_tile_can_have_wires,
    station_tile_has_reservation, station_tile_sets_adjacent, station_type_from_m6,
    stop_kind_from_m6, train_on_rail_platform, update_station_ratings,
    update_station_ratings_with_cargo_callbacks, update_station_waiting, vehicle_at_road_stop,
    vehicle_physically_at_station,
};
pub use station_action2::{
    StationAction2WorldContext, action2_eval_ctx_for_station_tile,
    action2_eval_ctx_for_station_tile_with_grf, action2_eval_ctx_for_station_tile_with_world,
};
pub use station_class::{
    STATION_ANIMATION_TRIGGER_ACCEPTANCE_TICK, STATION_ANIMATION_TRIGGER_BUILT,
    STATION_ANIMATION_TRIGGER_CARGO_TAKEN, STATION_ANIMATION_TRIGGER_NEW_CARGO,
    STATION_ANIMATION_TRIGGER_PATH_RESERVATION, STATION_ANIMATION_TRIGGER_TILE_LOOP,
    STATION_ANIMATION_TRIGGER_VEHICLE_ARRIVES, STATION_ANIMATION_TRIGGER_VEHICLE_DEPARTS,
    STATION_ANIMATION_TRIGGER_VEHICLE_LOADS, STATION_CALLBACK_ANIMATION_NEXT_FRAME_MASK,
    STATION_CALLBACK_ANIMATION_SPEED_MASK, STATION_FLAG_CB141_RANDOM_BITS, StationAnimationTrigger,
    StationClassDef, StationClassId, StationRandomTrigger, StationSpecDef, StationSpecId,
    all_station_class_defs, all_station_spec_defs, apply_station_build_tile_layout_callback,
    apply_station_draw_tile_layout_callback, list_station_classes, list_station_specs,
    next_free_station_class_id, next_free_station_spec_id, station_class_def,
    station_newgrf_view_index, station_platform_info, station_spec_def, station_spec_layout,
    vanilla_station_class_catalog, vanilla_station_spec_catalog,
};
pub use subsidy::{
    SUBSIDY_MAX_DISTANCE, SUBSIDY_OFFER_MONTHS, Subsidy, delivery_income_multiplier,
    subsidy_payment_multiplier_from_index, tick_subsidies, try_award_subsidy, try_create_subsidy,
};
pub use tick::GameTick;
pub use timer::{CalendarTimer, DAY_TICKS, EconomyTimer, TimerTriggers, tick_at_end_of_day};
pub use timetable::{TRAVEL_PRESETS, WAIT_PRESETS, cycle_travel_ticks, cycle_wait_ticks};
pub use town::{
    AUTHORITY_MIN_STATION, FUND_BUILDINGS_COST, FUND_BUILDINGS_MONTHS, HouseZone, MAIL_PER_HOUSE,
    NUM_HOUSE_ZONES, PASSENGERS_PER_HOUSE, STATION_TOWN_CARGO_CAPACITY, TOWN_ADVERTISE_COST,
    TOWN_AUTHORITY_RADIUS, TOWN_GROWTH_DESERT, TOWN_GROWTH_TICKS, TOWN_GROWTH_WINTER,
    TOWN_PRODUCE_TICKS, TOWN_RATING_INITIAL, TOWN_SUPPLIED_HISTORY_MONTHS, Town, TownGrowthEffect,
    TownLayout, authority_allows_new_station, grow_town_if_served, grow_town_if_served_with_ctx,
    process_town_monthly_growth, produce_town_cargo, produce_town_cargo_with_towns,
    town_goal_satisfied, update_town_growth_state, update_town_radius,
};
pub use town_action::{
    TownAction, TownAuthoritySettings, mask_of_town_actions, town_exclusivity_owner,
};
pub use town_expand::{
    TOWN_EXPAND_ATTEMPTS, TOWN_EXPAND_POP_PER_HOUSE, TOWN_EXPAND_SEARCH_RADIUS, TownExpandContext,
    TownExpandResult, expand_town_once, expand_town_physically, place_house_footprint,
};
pub use townname::generate_town_name;
pub use train_collision::{TrainCollision, detect_train_collisions, resolve_train_collisions};
pub use train_consist::{
    TrainUnitPose, VEHICLE_LENGTH, action2_eval_ctx_for_unit, attach_wagon, attach_wagon_chain,
    cargo_class_bits, cargo_type_a_id, consist_changed, consist_changed_with_map,
    consist_changed_with_map_and_catalog, consist_changed_with_map_and_catalog_and_cargo,
    consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier, consist_head_id,
    consist_occupied_tiles, consist_power_hp, consist_tile_span, consist_unit_ids,
    consist_unit_ids_indexed, consist_unit_poses, consist_weight_t, detach_unit,
    detach_unit_keep_tail, engine_is_train_engine, engine_is_wagon,
    enrich_vehicle_track_badge_vars, propagate_consist_unit_poses, reverse_consist_at_stop,
    same_consist, sell_chain_ids,
};
pub use train_movement::{
    ACCEL_SLOWDOWN, AccelSlowdownParams, DELTACOORD_LEAVE_OFFSET, FRACTCOORDS_BEHIND,
    FRACTCOORDS_ENTER, TRAIN_UPDATE_SPEED_ACCEL_MUL, TRAIN_UPDATE_SPEED_BRAKE_MUL,
    TUNNEL_VISIBILITY_FRAME, VEHICLE_INITIAL_X_FRACT, VEHICLE_INITIAL_Y_FRACT, VEHICLE_SUBCOORD,
    VehicleSubcoord, advance_train_pixel, affect_speed_by_z_change, calc_next_vehicle_offset,
    diag_dir_index, dir_difference, is_45_degree_turn, is_diagonal_rail_piece,
    openttd_subcoord_at_entry, rail_track_index, track_bit_for_movement, train_depot_facing,
    train_depot_subtile, train_render_dir_on_rail, train_subtile_on_rail,
    tunnel_hides_train_at_progress,
};
pub use vehicle::reverse_direction;
pub use vehicle::{
    AircraftPhase, BREAKDOWN_DURATION_TICKS, DEFAULT_SERVICE_INTERVAL_DAYS, DIR_E, DIR_N, DIR_NE,
    DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, OrderConditionComparator, OrderConditionKind,
    OrderLoadType, OrderNonStop, OrderStopLocation, OrderUnloadType,
    SERVICING_RELIABILITY_THRESHOLD, TimetableWaitKind, VEHICLE_PROGRESS_STEP, Vehicle,
    VehicleDirection, VehicleIssueDetail, VehicleKind, VehicleOperationalSummary, VehicleOrder,
    direction_from_tile_step,
};
pub use vehicle_group::{MAX_VEHICLE_GROUP_NAME_CHARS, VehicleGroup};
pub use world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, DEF_DESERT_COVERAGE, DEF_SNOW_COVERAGE, DEF_SNOW_LINE_HEIGHT,
    HeightmapData, IndustryDensity, LANDSCAPE_RIVER_TILE_LOOP_PASSES,
    NEW_GAME_RANDOM_WATER_BORDERS, NEW_GAME_STARTUP_RNG_DRAWS, NUM_INITIAL_INDUSTRIES,
    NUM_INITIAL_TOWNS, PopulationGenConfig, PreserveRect, QuantitySeaLakes,
    STARTUP_TILE_LOOP_PASSES, TerrainType, TgenSmoothness, TownDensity, TreePlacement,
    TreePlacementOrigin, WorldGenConfig, WorldGenRng, apply_clear_generation_with_rng,
    apply_heightmap, apply_landscape_with_rng, apply_landscape_with_rng_and_cursor,
    apply_population_gen, apply_population_gen_with_rng, apply_world_gen, apply_world_gen_with_rng,
    ceil_div, clear_ground_m5, effective_clear_ground, effective_new_game_map_height_limit,
    effective_snow_line_height, generate_industries, generate_industries_with_rng,
    generate_objects_with_rng, generate_towns, generate_towns_with_rng, generate_trees,
    generate_trees_with_rng, generate_trees_with_rng_observer,
    generate_trees_with_rng_observer_with_height_limit,
    generate_trees_with_rng_observer_with_map_settings, generate_trees_with_rng_with_map_settings,
    house_beside_road, industry_target_count, initial_clear_ground, parse_hmap,
    road_tiles_are_flat, run_first_regular_game_tick_with_rng, run_generation_tile_loop,
    run_generation_tile_loops_with_rng, run_landscape_river_tile_loops, scale_by_size,
    serialize_heightmap, town_target_count,
};

#[cfg(test)]
pub mod test_fixtures;

#[cfg(test)]
mod tests;
