use crate::map::TileKind;
use crate::{BRIDGE_BUILD_COST_PER_TILE, GameState, StopKind, TUNNEL_BUILD_COST_PER_TILE};

use super::types::{Command, CommandError};
use super::{industry, transport, vehicles};

/// Aplica `cmd` a `state` o devuelve error sin mutar.
///
/// # Errors
///
/// Ver variantes de [`CommandError`].
pub fn apply_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    let result = apply_command_inner(state, cmd);
    // Editar el mapa invalida los caminos cacheados: un tren con ruta vieja
    // seguiría cruzando vía recién desconectada. Se recalculan el próximo tick.
    if result.is_ok() && command_modifies_map(cmd) {
        invalidate_vehicle_paths(state);
    }
    result
}

const fn command_modifies_map(cmd: &Command) -> bool {
    !matches!(
        cmd,
        Command::SetVehicleOrders(..)
            | Command::SetVehicleStationOrders(..)
            | Command::SetVehicleOrderList(..)
            | Command::BuildRoadVehicleAtDepot(..)
            | Command::BuildVehicleAtDepot(..)
            | Command::SellVehicle(..)
            | Command::ToggleVehicleRunning(..)
            | Command::CloneVehicleOrders { .. }
    )
}

fn invalidate_vehicle_paths(state: &mut GameState) {
    for v in &mut state.vehicles {
        v.path.clear();
        v.no_network_route_to_order = false;
    }
}

fn apply_command_inner(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    match cmd {
        Command::PlaceRoad(c) => transport::place_road(state, *c),
        Command::PlaceRoadBits(c, bits) => transport::place_road_bits(state, *c, *bits),
        Command::SetRoadBits(c, bits) => transport::set_road_bits(state, *c, *bits),
        Command::PlaceRail(c) => transport::place_rail(state, *c),
        Command::PlaceRailBits(c, bits) => transport::place_rail_bits(state, *c, *bits),
        Command::SetRailBits(c, bits) => transport::set_rail_bits(state, *c, *bits),
        Command::PlaceRailWaypoint(c) => transport::place_rail_waypoint(state, *c),
        Command::PlaceRoadDepot(c) => transport::place_road_depot_dir(state, *c, 0),
        Command::PlaceRoadDepotDir(c, dir) => transport::place_road_depot_dir(state, *c, *dir),
        Command::PlaceRailDepot(c) => transport::place_rail_depot_dir(state, *c, 0),
        Command::PlaceRailDepotDir(c, dir) => transport::place_rail_depot_dir(state, *c, *dir),
        Command::PlaceRoadTunnel(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadTunnel,
            0x90,
            0x04,
            TUNNEL_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRailTunnel(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailTunnel,
            0x90,
            0x00,
            TUNNEL_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRoadBridge(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadBridge,
            0x90,
            0x84,
            BRIDGE_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRailBridge(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailBridge,
            0x90,
            0x80,
            BRIDGE_BUILD_COST_PER_TILE,
        ),
        Command::SetVehicleOrders(id, orders) => {
            vehicles::set_vehicle_orders(state, *id, orders.clone())
        }
        Command::SetVehicleStationOrders(id, stations) => {
            vehicles::set_vehicle_station_orders(state, *id, stations.clone())
        }
        Command::SetVehicleOrderList(id, orders) => {
            vehicles::set_vehicle_order_list(state, *id, orders.clone())
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
        Command::BuildRoadVehicleAtDepot(c, kind) => {
            vehicles::build_road_vehicle_at_depot(state, *c, *kind)
        }
        Command::BuildVehicleAtDepot(c, engine_id) => {
            vehicles::build_vehicle_at_depot(state, *c, *engine_id)
        }
        Command::SellVehicle(id) => vehicles::sell_vehicle(state, *id),
        Command::ToggleVehicleRunning(id) => vehicles::toggle_vehicle_running(state, *id),
        Command::CloneVehicleOrders {
            from_vehicle_id,
            to_vehicle_id,
        } => vehicles::clone_vehicle_orders(state, *from_vehicle_id, *to_vehicle_id),
        Command::ClearTile(c) => transport::clear_tile(state, *c),
    }
}
