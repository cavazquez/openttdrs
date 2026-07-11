use crate::bridge_spec::BridgeType;
use crate::map::TileKind;
use crate::{GameState, StopKind};

use super::types::{Command, CommandError};
use super::{buy_land, economy, industry, sign, terraform, town, transport, vehicles};

/// Aplica `cmd` a `state` o devuelve error sin mutar.
///
/// # Errors
///
/// Ver variantes de [`CommandError`].
pub fn apply_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    state.prepare_player_command();
    let result = apply_command_inner(state, cmd);
    // Editar el mapa invalida los caminos cacheados: un tren con ruta vieja
    // seguiría cruzando vía recién desconectada. Se recalculan el próximo tick.
    if result.is_ok() && command_modifies_map(cmd) {
        invalidate_vehicle_paths(state);
    }
    if result.is_ok() {
        if let Some((kind, at)) = construction_event_for(cmd) {
            state
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Construction { kind, at });
        }
        if let Some(at) = demolition_event_for(cmd) {
            state
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Demolition { at });
        }
        // Comandos mutan el espejo `economy`; sincronizar pool.
        state.sync_active_from_mirrors();
    }
    result
}

fn construction_event_for(
    cmd: &Command,
) -> Option<(crate::sim_events::ConstructionKind, crate::map::TileCoord)> {
    use crate::sim_events::ConstructionKind;
    match cmd {
        Command::PlaceRail(c)
        | Command::PlaceRailBits(c, _)
        | Command::SetRailBits(c, _)
        | Command::PlaceRailWaypoint(c)
        | Command::PlaceRailDepot(c)
        | Command::PlaceRailDepotDir(c, _)
        | Command::PlaceRailSignal(c, _, _, _, _)
        | Command::CycleRailSignalType(c, _, _)
        | Command::RemoveRailSignal(c, _, _)
        | Command::PlaceRailStation(c, _)
        | Command::PlaceRailTunnel(c, _) => Some((ConstructionKind::Rail, *c)),
        Command::PlaceRailBridge(c, _, _)
        | Command::PlaceRoadBridge(c, _, _)
        | Command::PlaceAqueduct(c, _) => Some((ConstructionKind::Bridge, *c)),
        Command::PlaceRoad(c)
        | Command::PlaceRoadBits(c, _)
        | Command::PlaceTramBits(c, _)
        | Command::SetRoadBits(c, _)
        | Command::PlaceRoadDepot(c)
        | Command::PlaceRoadDepotDir(c, _)
        | Command::PlaceShipDepotDir(c, _)
        | Command::PlaceDock(c, _)
        | Command::PlaceAirport(c)
        | Command::PlaceAirportArea { origin: c, .. }
        | Command::PlaceCanal(c)
        | Command::PlaceRiver(c)
        | Command::PlaceBuoy(c)
        | Command::PlaceLock(c, _)
        | Command::PlaceStation(c)
        | Command::PlaceStationDir(c, _)
        | Command::PlaceBusStop(c, _)
        | Command::PlaceTruckStop(c, _)
        | Command::PlaceRoadTunnel(c, _) => Some((ConstructionKind::Road, *c)),
        Command::PlaceRailStationArea { origin, .. } => Some((ConstructionKind::Rail, *origin)),
        // Quitar vía suena al SFX de rail (como en OpenTTD), no a explosión.
        Command::RemoveRail(c) | Command::RemoveRailBits(c, _) => {
            Some((ConstructionKind::Rail, *c))
        }
        Command::BuyLand(c)
        | Command::RaiseLand(c)
        | Command::LowerLand(c)
        | Command::PlaceIndustry(c)
        | Command::PlaceIndustryKind(c, _)
        | Command::PlaceIndustrySpec(c, _)
        | Command::PlaceHouse(c)
        | Command::PlaceForest(c) => Some((ConstructionKind::Other, *c)),
        Command::BuyLandArea { from, .. } | Command::LevelLand { from, .. } => {
            Some((ConstructionKind::Other, *from))
        }
        _ => None,
    }
}

fn demolition_event_for(cmd: &Command) -> Option<crate::map::TileCoord> {
    match cmd {
        // Solo la dinamita/limpieza de tesela suena a explosión.
        Command::ClearTile(c) => Some(*c),
        _ => None,
    }
}

const fn command_modifies_map(cmd: &Command) -> bool {
    !matches!(
        cmd,
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
            | Command::RenameSign { .. }
            | Command::PlaceSign { .. }
            | Command::RemoveSign { .. }
            | Command::JoinStations { .. }
            | Command::SetDepotVehiclesRunning { .. }
            | Command::MoveVehicleOrder { .. }
            | Command::ToggleVehicleOrderDepotStop { .. }
            | Command::TurnAroundVehicle(..)
            | Command::ForceVehicleProceed(..)
            | Command::RefitVehicle { .. }
            | Command::ToggleVehicleTimetable(..)
            | Command::CycleVehicleOrderWait { .. }
            | Command::CycleVehicleOrderTravel { .. }
            | Command::SetAutoReplaceRule { .. }
            | Command::ClearAutoReplaceRule { .. }
            | Command::ToggleAutoReplaceRule { .. }
            | Command::CreateVehicleGroup { .. }
            | Command::RenameVehicleGroup { .. }
            | Command::AssignVehicleToGroup { .. }
            | Command::ClearVehicleTimetableLateness(..)
            | Command::SetVehicleOrderWaitTicks { .. }
            | Command::SetVehicleOrderTravelTicks { .. }
            | Command::ToggleVehicleTimetableAutofill(..)
            | Command::ToggleAutoReplaceOnlyWhenOld { .. }
            | Command::SetAutoReplaceRuleGroup { .. }
            | Command::DepotMassAutoreplace { .. }
            | Command::CreateSharedOrdersFromVehicle(..)
            | Command::LinkVehicleToSharedOrders { .. }
            | Command::UnlinkVehicleSharedOrders(..)
            | Command::SetSharedOrderAt { .. }
            | Command::SetVehicleOrderConditional { .. }
            | Command::DepotReorderVehicleSlot { .. }
            | Command::IncreaseLoan
            | Command::DecreaseLoan
            | Command::TownAdvertise(..)
            | Command::TownFundBuildings(..)
    )
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
        } => vehicles::move_rail_vehicle(state, *head_id, *unit_id, *after_id),
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
        Command::TurnAroundVehicle(id) => vehicles::turn_around_vehicle(state, *id),
        Command::ForceVehicleProceed(id) => vehicles::force_vehicle_proceed(state, *id),
        Command::RefitVehicle { vehicle_id, cargo } => {
            vehicles::refit_vehicle(state, *vehicle_id, *cargo)
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
        Command::SetRoadBits(c, bits) => transport::set_road_bits(state, *c, *bits),
        Command::PlaceRail(c) => transport::place_rail(state, *c),
        Command::PlaceRailBits(c, bits) => transport::place_rail_bits(state, *c, *bits),
        Command::SetRailBits(c, bits) => transport::set_rail_bits(state, *c, *bits),
        Command::PlaceRailWaypoint(c) => transport::place_rail_waypoint(state, *c),
        Command::RemoveRailBits(c, bits) => transport::remove_rail_bits(state, *c, *bits),
        Command::RemoveRail(c) => transport::remove_rail(state, *c),
        Command::ConvertRail(c, to) => {
            transport::convert_rail(state, *c, crate::rail_type::RailType::from_u8(*to))
        }
        Command::PlaceRailSignal(c, face, fx, fy, sig_type) => {
            transport::place_rail_signal(state, *c, *face, *fx, *fy, *sig_type)
        }
        Command::CycleRailSignalType(c, fx, fy) => {
            transport::cycle_rail_signal_type(state, *c, *fx, *fy)
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
        | Command::TurnAroundVehicle(..)
        | Command::ForceVehicleProceed(..)
        | Command::RefitVehicle { .. }
        | Command::ToggleVehicleTimetable(..)
        | Command::CycleVehicleOrderWait { .. }
        | Command::CycleVehicleOrderTravel { .. }
        | Command::SetAutoReplaceRule { .. }
        | Command::ClearAutoReplaceRule { .. }
        | Command::ToggleAutoReplaceRule { .. }
        | Command::CreateVehicleGroup { .. }
        | Command::RenameVehicleGroup { .. }
        | Command::AssignVehicleToGroup { .. }
        | Command::ClearVehicleTimetableLateness(..)
        | Command::SetVehicleOrderWaitTicks { .. }
        | Command::SetVehicleOrderTravelTicks { .. }
        | Command::ToggleVehicleTimetableAutofill(..)
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
        Command::IncreaseLoan => economy::increase_company_loan(state),
        Command::DecreaseLoan => economy::decrease_company_loan(state),
        Command::TownAdvertise(town_id) => town::town_advertise(state, *town_id),
        Command::TownFundBuildings(town_id) => town::town_fund_buildings(state, *town_id),
        Command::PlantTree(c) => crate::map::tree_tile_loop::plant_tree(state, *c),
        Command::ClearTree(c) => crate::map::tree_tile_loop::clear_tree(state, *c),
        Command::PlaceSign { pos, name } => sign::place_sign(state, *pos, name.clone()),
        Command::RemoveSign { sign_id } => sign::remove_sign(state, *sign_id),
        Command::RenameSign { sign_id, name } => sign::rename_sign(state, *sign_id, name.clone()),
        Command::JoinStations { keep, merge } => transport::join_stations(state, *keep, *merge),
    }
}
