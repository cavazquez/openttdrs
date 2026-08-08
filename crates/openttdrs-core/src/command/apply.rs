use crate::bridge_spec::BridgeType;
use crate::map::TileKind;
use crate::{GameState, StopKind};

use super::error::CommandError;
use super::metadata::command_effects;
use super::types::Command;
use super::{
    build_object, buy_land, company, economy, industry, newgrf, sign, terraform, town, transport,
    vehicles,
};

/// Aplica `cmd` a `state` o devuelve error sin mutar.
///
/// # Errors
///
/// Ver variantes de [`CommandError`].
pub fn apply_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    state.prepare_player_command();
    state.runtime.fleet_index.rebuild(&state.vehicles);
    state
        .runtime
        .terminal_spatial_index
        .rebuild(&state.stations);
    let money_before = state.economy.money;
    let result = apply_command_inner(state, cmd);
    if result.is_ok() {
        let effects = command_effects(cmd);
        // Editar el mapa invalida los caminos cacheados: un tren con ruta vieja
        // seguiría cruzando vía recién desconectada. Se recalculan el próximo tick.
        if effects.modifies_map {
            invalidate_vehicle_paths(state);
            state.runtime.depot_spatial_index.invalidate();
        }
        if let Some((kind, at)) = effects.construction_event {
            state
                .runtime
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Construction { kind, at });
            mark_landscape_tiles_dirty_for_action(state, at);
        }
        if let Some(at) = effects.demolition_event {
            state
                .runtime
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Demolition { at });
            mark_landscape_tiles_dirty_for_action(state, at);
        }
        if state.cheats.infinite_money_active() && state.economy.money < money_before {
            state.economy.money = money_before;
        }
        // Comandos mutan el espejo `economy`; sincronizar pool.
        state.sync_active_from_mirrors();
        state.runtime.fleet_index.rebuild(&state.vehicles);
        state
            .runtime
            .terminal_spatial_index
            .rebuild(&state.stations);
        if let Some(rec) = state.runtime.command_recorder.as_mut() {
            rec.push_back(cmd.clone());
        }
    }
    result
}

fn mark_landscape_tiles_dirty_for_action(state: &mut GameState, at: crate::map::TileCoord) {
    let (map_w, map_h) = state.map.dimensions();
    let Ok(map_w) = i32::try_from(map_w) else {
        return;
    };
    let Ok(map_h) = i32::try_from(map_h) else {
        return;
    };
    let add = |runtime: &mut crate::game_state::SimulationRuntime, x: i32, y: i32| {
        if x >= 0 && y >= 0 && x < map_w && y < map_h {
            let coord = crate::map::TileCoord::new(x, y);
            if state.map.get(coord).is_none() {
                return;
            }
            if !runtime.landscape_tile_dirty.contains(&coord) {
                runtime.landscape_tile_dirty.push(coord);
            }
        }
    };

    let runtime = &mut state.runtime;
    add(runtime, at.x, at.y);
    for (dx, dy) in [(-1_i32, 0), (1, 0), (0, -1), (0, 1)] {
        add(runtime, at.x + dx, at.y + dy);
    }
}

fn invalidate_vehicle_paths(state: &mut GameState) {
    for v in &mut state.vehicles {
        v.path.clear();
        v.no_network_route_to_order = false;
    }
}

#[allow(clippy::too_many_lines)]
fn apply_vehicle_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    match cmd {
        Command::SetVehicleOrders(id, orders) => {
            vehicles::set_vehicle_orders(state, *id, orders.clone())
        }
        Command::SetVehicleStationOrders(id, stations) => {
            vehicles::set_vehicle_station_orders(state, *id, stations.clone())
        }
        Command::SetVehicleOrderList(id, orders) => {
            vehicles::set_vehicle_order_list(state, *id, orders.clone())
        }
        Command::BuildRoadVehicleAtDepot(c, kind) => {
            vehicles::build_road_vehicle_at_depot(state, *c, *kind)
        }
        Command::BuildVehicleAtDepot(c, engine_id) => {
            vehicles::build_vehicle_at_depot(state, *c, *engine_id)
        }
        Command::AttachWagonToConsist { head_id, wagon_id } => {
            vehicles::attach_wagon_to_consist(state, *head_id, *wagon_id)
        }
        Command::DetachConsistUnit(id) => vehicles::detach_consist_unit(state, *id),
        Command::MoveRailVehicle {
            head_id,
            unit_id,
            after_id,
            move_chain,
        } => vehicles::move_rail_vehicle(state, *head_id, *unit_id, *after_id, *move_chain),
        Command::SellVehicle(id) => vehicles::sell_vehicle(state, *id),
        Command::ToggleVehicleRunning(id) => {
            super::vehicle_fleet::toggle_vehicle_running_checked(state, *id)
        }
        Command::CloneVehicleOrders {
            from_vehicle_id,
            to_vehicle_id,
        } => vehicles::clone_vehicle_orders(state, *from_vehicle_id, *to_vehicle_id),
        Command::CloneVehicleAtDepot {
            source_vehicle_id,
            depot_pos,
        } => vehicles::clone_vehicle_at_depot(state, *source_vehicle_id, *depot_pos),
        Command::SellAllVehiclesAtDepot(depot_pos) => {
            vehicles::sell_all_vehicles_at_depot(state, *depot_pos)
        }
        Command::RemoveVehicleOrderAt { vehicle_id, index } => {
            vehicles::remove_vehicle_order_at(state, *vehicle_id, *index)
        }
        Command::SkipVehicleOrder(id) => vehicles::skip_vehicle_order(state, *id),
        Command::ToggleVehicleOrderFullLoad { vehicle_id, index } => {
            vehicles::toggle_vehicle_order_full_load(state, *vehicle_id, *index)
        }
        Command::ToggleVehicleOrderNoUnload { vehicle_id, index } => {
            vehicles::toggle_vehicle_order_no_unload(state, *vehicle_id, *index)
        }
        Command::AppendGotoNearestDepot(id) => vehicles::append_goto_nearest_depot(state, *id),
        Command::RenameVehicle { vehicle_id, name } => {
            vehicles::rename_vehicle(state, *vehicle_id, name.clone())
        }
        Command::RenameStation { station_pos, name } => {
            transport::rename_station(state, *station_pos, name.clone())
        }
        Command::SetDepotVehiclesRunning { depot_pos, running } => {
            vehicles::set_depot_vehicles_running(state, *depot_pos, *running)
        }
        Command::MoveVehicleOrder {
            vehicle_id,
            index,
            direction,
        } => vehicles::move_vehicle_order(state, *vehicle_id, *index, *direction),
        Command::ToggleVehicleOrderDepotStop { vehicle_id, index } => {
            vehicles::toggle_vehicle_order_depot_stop(state, *vehicle_id, *index)
        }
        Command::ToggleVehicleOrderDepotUnbunch { vehicle_id, index } => {
            vehicles::toggle_vehicle_order_depot_unbunch(state, *vehicle_id, *index)
        }
        Command::SetVehicleOrderMaxSpeed {
            vehicle_id,
            index,
            max_speed,
        } => vehicles::set_vehicle_order_max_speed(state, *vehicle_id, *index, *max_speed),
        Command::TurnAroundVehicle(id) => vehicles::turn_around_vehicle(state, *id),
        Command::ForceVehicleProceed(id) => vehicles::force_vehicle_proceed(state, *id),
        Command::RefitVehicle {
            vehicle_id,
            cargo,
            unit_ids,
        } => vehicles::refit_vehicle(state, *vehicle_id, *cargo, unit_ids),
        Command::CycleVehicleOrderDepotRefit { vehicle_id, index } => {
            vehicles::cycle_vehicle_order_depot_refit(state, *vehicle_id, *index)
        }
        Command::ToggleVehicleTimetable(id) => vehicles::toggle_vehicle_timetable(state, *id),
        Command::CycleVehicleOrderWait { vehicle_id, index } => {
            vehicles::cycle_vehicle_order_wait(state, *vehicle_id, *index)
        }
        Command::CycleVehicleOrderTravel { vehicle_id, index } => {
            vehicles::cycle_vehicle_order_travel(state, *vehicle_id, *index)
        }
        Command::SetAutoReplaceRule {
            from_engine_id,
            to_engine_id,
        } => vehicles::set_autoreplace_rule(state, *from_engine_id, *to_engine_id),
        Command::ClearAutoReplaceRule { from_engine_id } => {
            vehicles::clear_autoreplace_rule(state, *from_engine_id)
        }
        Command::ToggleAutoReplaceRule { from_engine_id } => {
            vehicles::toggle_autoreplace_rule(state, *from_engine_id)
        }
        Command::CreateVehicleGroup { name } => {
            super::vehicle_fleet::create_vehicle_group(state, name)
        }
        Command::RenameVehicleGroup { group_id, name } => {
            super::vehicle_fleet::rename_vehicle_group(state, *group_id, name)
        }
        Command::AssignVehicleToGroup {
            vehicle_id,
            group_id,
        } => super::vehicle_fleet::assign_vehicle_to_group(state, *vehicle_id, *group_id),
        Command::SetVehicleGroupRunning { group_id, running } => {
            super::vehicle_fleet::set_vehicle_group_running(state, *group_id, *running)
        }
        Command::ClearVehicleTimetableLateness(id) => {
            super::vehicle_fleet::clear_vehicle_timetable_lateness(state, *id)
        }
        Command::SetVehicleOrderWaitTicks {
            vehicle_id,
            index,
            wait_ticks,
        } => super::vehicle_fleet::set_vehicle_order_wait_ticks(
            state,
            *vehicle_id,
            *index,
            *wait_ticks,
        ),
        Command::SetVehicleOrderTravelTicks {
            vehicle_id,
            index,
            travel_ticks,
        } => super::vehicle_fleet::set_vehicle_order_travel_ticks(
            state,
            *vehicle_id,
            *index,
            *travel_ticks,
        ),
        Command::ToggleVehicleTimetableAutofill(id) => {
            super::vehicle_fleet::toggle_vehicle_timetable_autofill(state, *id)
        }
        Command::SetVehicleTimetableStart {
            vehicle_id,
            start_tick,
        } => super::vehicle_fleet::set_vehicle_timetable_start(state, *vehicle_id, *start_tick),
        Command::ToggleAutoReplaceOnlyWhenOld { from_engine_id } => {
            let only_when_old = state
                .autoreplace_rules
                .iter()
                .find(|r| r.from_engine_id == *from_engine_id)
                .map(|r| r.only_when_old)
                .ok_or(CommandError::AutoReplaceRuleNotFound)?;
            super::vehicle_fleet::set_autoreplace_only_when_old(
                state,
                *from_engine_id,
                !only_when_old,
            )
        }
        Command::SetAutoReplaceRuleGroup {
            from_engine_id,
            group_id,
        } => super::vehicle_fleet::set_autoreplace_rule_group(state, *from_engine_id, *group_id),
        Command::DepotMassAutoreplace { depot_pos } => {
            super::vehicle_fleet::depot_mass_autoreplace(state, *depot_pos)
        }
        Command::CreateSharedOrdersFromVehicle(id) => {
            super::vehicle_fleet::create_shared_orders_from_vehicle(state, *id)
        }
        Command::LinkVehicleToSharedOrders {
            vehicle_id,
            shared_id,
        } => super::vehicle_fleet::link_vehicle_to_shared_orders(state, *vehicle_id, *shared_id),
        Command::UnlinkVehicleSharedOrders(id) => {
            super::vehicle_fleet::unlink_vehicle_shared_orders(state, *id)
        }
        Command::SetSharedOrderAt {
            shared_id,
            index,
            order,
        } => super::vehicle_fleet::set_shared_order_at(state, *shared_id, *index, *order),
        Command::SetVehicleOrderConditional {
            vehicle_id,
            index,
            condition,
            value,
            jump_to,
        } => super::vehicle_fleet::set_vehicle_order_conditional(
            state,
            *vehicle_id,
            *index,
            *condition,
            *value,
            *jump_to,
        ),
        Command::DepotReorderVehicleSlot {
            depot_pos,
            from_slot,
            to_slot,
        } => super::vehicle_fleet::depot_reorder_vehicle_slot(
            state, *depot_pos, *from_slot, *to_slot,
        ),
        _ => Err(CommandError::VehicleNotFound),
    }
}

#[allow(clippy::too_many_lines)]
fn apply_command_inner(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    match cmd {
        Command::PlaceRoad(c) => transport::place_road(state, *c),
        Command::PlaceRoadBits(c, bits) => transport::place_road_bits(state, *c, *bits),
        Command::PlaceTramBits(c, bits) => transport::place_tram_bits(state, *c, *bits),
        Command::RemoveTramBits(c) => transport::remove_tram_bits(state, *c),
        Command::SetRoadBits(c, bits) => transport::set_road_bits(state, *c, *bits),
        Command::PlaceRail(c) => transport::place_rail(state, *c),
        Command::PlaceRailBits(c, bits) => transport::place_rail_bits(state, *c, *bits),
        Command::SetRailBits(c, bits) => transport::set_rail_bits(state, *c, *bits),
        Command::PlaceRailWaypoint(c) => transport::place_rail_waypoint(state, *c),
        Command::PlaceRoadWaypoint(c) => transport::place_road_waypoint(state, *c),
        Command::RemoveRailBits(c, bits) => transport::remove_rail_bits(state, *c, *bits),
        Command::RemoveRail(c) => transport::remove_rail(state, *c),
        Command::ConvertRail(c, to) => {
            transport::convert_rail(state, *c, crate::rail_type::RailType::from_u8(*to))
        }
        Command::PlaceRailSignal(c, face, fx, fy, sig_type) => {
            transport::place_rail_signal(state, *c, *face, *fx, *fy, *sig_type, u8::MAX)
        }
        Command::PlaceRailSignalWithVariant(c, face, fx, fy, sig_type, variant) => {
            transport::place_rail_signal(state, *c, *face, *fx, *fy, *sig_type, *variant)
        }
        Command::CycleRailSignalType(c, fx, fy) => {
            transport::cycle_rail_signal_type(state, *c, *fx, *fy)
        }
        Command::CycleRailSignalVariant(c, fx, fy) => {
            transport::cycle_rail_signal_variant(state, *c, *fx, *fy)
        }
        Command::RemoveRailSignal(c, fx, fy) => transport::remove_rail_signal(state, *c, *fx, *fy),
        Command::PlaceRoadDepot(c) => transport::place_road_depot_dir(state, *c, 0),
        Command::PlaceRoadDepotDir(c, dir) => transport::place_road_depot_dir(state, *c, *dir),
        Command::PlaceRailDepot(c) => transport::place_rail_depot_dir(state, *c, 0),
        Command::PlaceRailDepotDir(c, dir) => transport::place_rail_depot_dir(state, *c, *dir),
        Command::PlaceShipDepotDir(c, dir) => transport::place_ship_depot_dir(state, *c, *dir),
        Command::PlaceDock(c, dir) => transport::place_dock(state, *c, *dir),
        Command::PlaceAirport(c) => transport::place_airport(state, *c),
        Command::PlaceAirportArea {
            origin,
            axis_y,
            spec,
        } => transport::place_airport_area(state, *origin, *axis_y, *spec),
        Command::PlaceCanal(c) => transport::place_canal(state, *c),
        Command::PlaceRiver(c) => transport::place_river(state, *c),
        Command::PlaceBuoy(c) => transport::place_buoy(state, *c),
        Command::PlaceAqueduct(a, b) => transport::place_aqueduct(state, *a, *b),
        Command::PlaceLock(c, axis_y) => transport::place_lock(state, *c, *axis_y),
        Command::PlaceRoadTunnel(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadTunnel,
            0x90,
            0x04,
            BridgeType::Wooden,
        ),
        Command::PlaceRailTunnel(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailTunnel,
            0x90,
            0x00,
            BridgeType::Wooden,
        ),
        Command::PlaceRoadBridge(a, b, bt) => {
            transport::place_tunnel_or_bridge(state, *a, *b, TileKind::RoadBridge, 0x90, 0x84, *bt)
        }
        Command::PlaceRailBridge(a, b, bt) => {
            transport::place_tunnel_or_bridge(state, *a, *b, TileKind::RailBridge, 0x90, 0x80, *bt)
        }
        Command::PlaceHouse(c) => {
            transport::place_single_transport_tile(state, *c, TileKind::House, 0x30, 0x00, 50)
        }
        Command::PlaceIndustry(c) => industry::place_industry_sandbox(state, *c),
        Command::PlaceIndustryKind(c, kind) => {
            industry::place_industry_kind_sandbox(state, *c, *kind)
        }
        Command::PlaceIndustrySpec(c, spec) => {
            industry::place_industry_spec_sandbox(state, *c, *spec)
        }
        Command::PlaceForest(c) => {
            transport::place_single_transport_tile(state, *c, TileKind::Forest, 0x40, 0x00, 30)
        }
        Command::PlaceStation(c) => transport::place_station(state, *c),
        Command::PlaceStationDir(c, dir) => transport::place_station_dir(state, *c, *dir),
        Command::PlaceBusStop(c, dir) => {
            transport::place_stop_kind(state, *c, *dir, StopKind::BusStop)
        }
        Command::PlaceTruckStop(c, dir) => {
            transport::place_stop_kind(state, *c, *dir, StopKind::TruckStop)
        }
        Command::PlaceRailStation(c, dir) => transport::place_rail_station(state, *c, *dir),
        Command::PlaceRailStationArea {
            origin,
            axis_y,
            platforms,
            length,
        } => transport::place_rail_station_area(state, *origin, *axis_y, *platforms, *length),
        Command::SetVehicleOrders(..)
        | Command::SetVehicleStationOrders(..)
        | Command::SetVehicleOrderList(..)
        | Command::BuildRoadVehicleAtDepot(..)
        | Command::BuildVehicleAtDepot(..)
        | Command::AttachWagonToConsist { .. }
        | Command::DetachConsistUnit(..)
        | Command::MoveRailVehicle { .. }
        | Command::SellVehicle(..)
        | Command::ToggleVehicleRunning(..)
        | Command::CloneVehicleOrders { .. }
        | Command::CloneVehicleAtDepot { .. }
        | Command::SellAllVehiclesAtDepot(..)
        | Command::RemoveVehicleOrderAt { .. }
        | Command::SkipVehicleOrder(..)
        | Command::ToggleVehicleOrderFullLoad { .. }
        | Command::ToggleVehicleOrderNoUnload { .. }
        | Command::AppendGotoNearestDepot(..)
        | Command::RenameVehicle { .. }
        | Command::RenameStation { .. }
        | Command::SetDepotVehiclesRunning { .. }
        | Command::MoveVehicleOrder { .. }
        | Command::ToggleVehicleOrderDepotStop { .. }
        | Command::ToggleVehicleOrderDepotUnbunch { .. }
        | Command::SetVehicleOrderMaxSpeed { .. }
        | Command::TurnAroundVehicle(..)
        | Command::ForceVehicleProceed(..)
        | Command::RefitVehicle { .. }
        | Command::CycleVehicleOrderDepotRefit { .. }
        | Command::ToggleVehicleTimetable(..)
        | Command::CycleVehicleOrderWait { .. }
        | Command::CycleVehicleOrderTravel { .. }
        | Command::SetAutoReplaceRule { .. }
        | Command::ClearAutoReplaceRule { .. }
        | Command::ToggleAutoReplaceRule { .. }
        | Command::CreateVehicleGroup { .. }
        | Command::RenameVehicleGroup { .. }
        | Command::AssignVehicleToGroup { .. }
        | Command::SetVehicleGroupRunning { .. }
        | Command::ClearVehicleTimetableLateness(..)
        | Command::SetVehicleOrderWaitTicks { .. }
        | Command::SetVehicleOrderTravelTicks { .. }
        | Command::ToggleVehicleTimetableAutofill(..)
        | Command::SetVehicleTimetableStart { .. }
        | Command::ToggleAutoReplaceOnlyWhenOld { .. }
        | Command::SetAutoReplaceRuleGroup { .. }
        | Command::DepotMassAutoreplace { .. }
        | Command::CreateSharedOrdersFromVehicle(..)
        | Command::LinkVehicleToSharedOrders { .. }
        | Command::UnlinkVehicleSharedOrders(..)
        | Command::SetSharedOrderAt { .. }
        | Command::SetVehicleOrderConditional { .. }
        | Command::DepotReorderVehicleSlot { .. } => apply_vehicle_command(state, cmd),
        Command::ClearTile(c) => transport::clear_tile(state, *c),
        Command::RaiseLand(c) => terraform::raise_land(state, *c),
        Command::LowerLand(c) => terraform::lower_land(state, *c),
        Command::LevelLand { from, to, mode } => terraform::level_land(state, *from, *to, *mode),
        Command::BuyLand(c) => buy_land::buy_land(state, *c),
        Command::BuyLandArea { from, to } => buy_land::buy_land_area(state, *from, *to),
        Command::BuildObject { pos, object_type } => {
            build_object::build_object(state, *pos, *object_type)
        }
        Command::IncreaseLoan => economy::increase_company_loan(state),
        Command::DecreaseLoan => economy::decrease_company_loan(state),
        Command::BuyCompany(id) => company::buy_company(state, *id),
        Command::TownAdvertise(town_id) => town::town_advertise(state, *town_id),
        Command::TownFundBuildings(town_id) => town::town_fund_buildings(state, *town_id),
        Command::DoTownAction { town_id, action } => town::do_town_action(state, *town_id, *action),
        Command::FoundTown(c) => town::found_town(state, *c),
        Command::CheatSetEnabled(on) => {
            state.cheats.enabled = *on;
            Ok(())
        }
        Command::CheatAddMoney(amount) => {
            if !state.cheats.enabled {
                return Err(CommandError::CheatsDisabled);
            }
            state.economy.money = state.economy.money.saturating_add(*amount);
            Ok(())
        }
        Command::CheatToggleInfiniteMoney => {
            if !state.cheats.enabled {
                return Err(CommandError::CheatsDisabled);
            }
            state.cheats.infinite_money = !state.cheats.infinite_money;
            Ok(())
        }
        Command::CheatToggleMagicBulldozer => {
            if !state.cheats.enabled {
                return Err(CommandError::CheatsDisabled);
            }
            state.cheats.magic_bulldozer = !state.cheats.magic_bulldozer;
            Ok(())
        }
        Command::CheatSetYear(year) => {
            if !state.cheats.enabled {
                return Err(CommandError::CheatsDisabled);
            }
            if !crate::cheats::year_in_range(*year) {
                return Err(CommandError::InvalidCheatYear);
            }
            state.tick = crate::news::tick_for_calendar_year(*year);
            Ok(())
        }
        Command::CheatSwitchCompany(id) => {
            if !state.cheats.enabled {
                return Err(CommandError::CheatsDisabled);
            }
            if !state.set_active_company(*id) {
                return Err(CommandError::CompanyNotFound);
            }
            Ok(())
        }
        Command::PlantTree(c) => crate::map::tree_tile_loop::plant_tree(state, *c),
        Command::ClearTree(c) => crate::map::tree_tile_loop::clear_tree(state, *c),
        Command::PlaceSign { pos, name } => sign::place_sign(state, *pos, name.clone()),
        Command::RemoveSign { sign_id } => sign::remove_sign(state, *sign_id),
        Command::RenameSign { sign_id, name } => sign::rename_sign(state, *sign_id, name.clone()),
        Command::JoinStations { keep, merge } => transport::join_stations(state, *keep, *merge),
        Command::SetNewGrfEnabled { index, enabled } => {
            newgrf::set_newgrf_enabled(state, *index, *enabled)
        }
        Command::MoveNewGrfInStack { from, to } => newgrf::move_newgrf_in_stack(state, *from, *to),
        Command::RemoveNewGrfFromStack { index } => newgrf::remove_newgrf_from_stack(state, *index),
        Command::AddNewGrfToStack { entry } => newgrf::add_newgrf_to_stack(state, entry.clone()),
        Command::SetNewGrfParam {
            index,
            param_index,
            value,
        } => newgrf::set_newgrf_param(state, *index, *param_index, *value),
        Command::SetPathfindingSettings(settings) => {
            let mut next = *settings;
            next.wait_for_pbs_path = next.wait_for_pbs_path.max(2);
            next.path_backoff_interval = next.path_backoff_interval.max(1);
            next.wait_oneway_signal = next.wait_oneway_signal.max(2);
            next.wait_twoway_signal = next.wait_twoway_signal.max(2);
            if state.pathfinding == next {
                return Ok(());
            }
            state.pathfinding = next;
            Ok(())
        }
        Command::SetConstructionSettings(settings) => {
            if state.construction == *settings {
                return Ok(());
            }
            state.construction = *settings;
            Ok(())
        }
        Command::SetVehicleBreakdowns(level) => {
            let level = (*level).min(2);
            state.vehicle_breakdowns = level;
            if level == 0 {
                // El cambio es efectivo en el acto: no deja una avería ya
                // sorteada esperando varios ticks para detener el vehículo.
                for vehicle in &mut state.vehicles {
                    vehicle.breakdown_ctr = 0;
                    vehicle.breakdown_delay = 0;
                    vehicle.breakdown_chance = 0;
                }
            }
            Ok(())
        }
        Command::SetCargoDistDistribution(mode) => {
            if state.cargo_dist.distribution == *mode {
                return Ok(());
            }
            state.cargo_dist.distribution = *mode;
            state.rebuild_station_flows();
            Ok(())
        }
        Command::SetCompanyColour(colour) => {
            let colour = *colour % crate::company::COMPANY_COLOUR_SLOTS;
            if state.company_colour == colour {
                return Ok(());
            }
            if crate::company::company_colour_taken_by_other(
                &state.companies,
                state.active_company,
                colour,
            ) {
                return Err(CommandError::CompanyColourTaken);
            }
            state.company_colour = colour;
            Ok(())
        }
        Command::SetCurrentRailType(rt) => {
            if state.current_rail_type == *rt {
                return Ok(());
            }
            state.current_rail_type = *rt;
            Ok(())
        }
        Command::SetCurrentRoadType(rt) => {
            if state.current_road_type == *rt {
                return Ok(());
            }
            state.current_road_type = *rt;
            Ok(())
        }
        Command::SetCurrentTramType(rt) => {
            if state.current_tram_type == *rt {
                return Ok(());
            }
            state.current_tram_type = *rt;
            Ok(())
        }
        Command::SetCurrentStationClass(class) => {
            state.current_station_class = *class;
            if let Some(first) =
                crate::station_class::list_station_specs(&state.station_spec_catalog, *class, "")
                    .first()
            {
                state.current_station_spec = first.id;
            }
            Ok(())
        }
        Command::SetCurrentStationSpec(spec) => {
            if state.current_station_spec == *spec {
                return Ok(());
            }
            state.current_station_spec = *spec;
            Ok(())
        }
        Command::SetCurrentRoadStopClass(class) => {
            state.current_road_stop_class = Some(*class);
            state.current_road_stop_spec = state
                .road_stop_spec_catalog
                .iter()
                .find(|s| s.class == *class)
                .map(|s| s.id);
            Ok(())
        }
        Command::SetCurrentRoadStopSpec(spec) => {
            if state.current_road_stop_spec == Some(*spec) {
                return Ok(());
            }
            if let Some(def) =
                crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, *spec)
            {
                state.current_road_stop_class = Some(def.class);
                state.current_road_stop_spec = Some(*spec);
            } else {
                state.current_road_stop_class = None;
                state.current_road_stop_spec = None;
            }
            Ok(())
        }
        Command::SetCurrentAirportClass(class) => {
            state.current_airport_class = *class;
            state.current_airport_newgrf_id = None;
            if let Some(first) = crate::airport_class::list_airport_specs(*class, "").first() {
                state.current_airport_spec = first.id;
            }
            Ok(())
        }
        Command::SetCurrentAirportSpec(spec) => {
            if let Some(def) = crate::airport_class::airport_spec_def(*spec) {
                state.current_airport_class = def.class;
                state.current_airport_spec = *spec;
                state.current_airport_newgrf_id = None;
            }
            Ok(())
        }
        Command::SetCurrentAirportNewgrfSpec(id) => {
            if let Some(def) =
                crate::airport_class::newgrf_airport_spec_def(&state.airport_spec_catalog, *id)
            {
                state.current_airport_class = def.class;
                state.current_airport_spec = def.subst_id;
                state.current_airport_newgrf_id = Some(*id);
            }
            Ok(())
        }
        Command::SetCurrentObjectSpec(spec) => {
            if state.current_object_spec == *spec {
                return Ok(());
            }
            if crate::object_spec::is_selectable_object_spec(&state.object_spec_catalog, *spec) {
                state.current_object_spec = *spec;
            }
            Ok(())
        }
        Command::SetAiSettings(settings) => {
            let next = settings.clamped();
            if state.ai == next {
                return Ok(());
            }
            state.ai = next;
            Ok(())
        }
        Command::FinalizeRoadDragLine { tiles, axis } => {
            transport::finalize_road_drag_line(state, tiles, *axis)
        }
        Command::RegenerateLandscape {
            climate,
            seed,
            island,
            height_span,
        } => {
            let seed = if *seed == 0 { 0xDEAD_BEEF } else { *seed };
            let cfg = crate::world_gen::WorldGenConfig {
                climate: *climate,
                seed,
                sea_level: 1,
                island: *island,
                ..crate::world_gen::WorldGenConfig::default().with_height_span(*height_span)
            };
            crate::world_gen::apply_world_gen(&mut state.map, &cfg, &[])
                .map_err(|_| CommandError::OutOfBounds)?;
            state.climate = *climate;
            state.world_seed = seed;
            state.towns.clear();
            state.industries.clear();
            state.stations.clear();
            state.vehicles.clear();
            state.signs.clear();
            // Orden genworld: terreno → pueblos → industrias (P3.1).
            crate::world_gen::apply_population_gen(
                state,
                &crate::world_gen::PopulationGenConfig {
                    town_density: crate::world_gen::TownDensity::Normal,
                    industry_density: crate::world_gen::IndustryDensity::Normal,
                    seed,
                },
                &[],
            );
            Ok(())
        }
    }
}
