use std::collections::{HashSet, VecDeque};

use crate::map::{Map, TileCoord, TileKind};
use crate::{GameState, Vehicle, VehicleKind, VehicleOrder};

use super::transport::road_depot_exit_for_dir;
use super::{CommandError, in_bounds};

pub(super) fn set_vehicle_order_list(
    state: &mut GameState,
    id: u32,
    orders: Vec<VehicleOrder>,
) -> Result<(), CommandError> {
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle_kind = state.vehicles[vehicle_idx].kind;
    for order in &orders {
        in_bounds(&state.map, order.destination())?;
        match order {
            VehicleOrder::Station { station, .. } => {
                let Some(st) = state.stations.iter().find(|s| s.pos == *station) else {
                    return Err(CommandError::StationNotFound);
                };
                if !st.can_service_vehicle(vehicle_kind) || st.is_waypoint() {
                    return Err(CommandError::IncompatibleStopForVehicle);
                }
            }
            VehicleOrder::Waypoint { waypoint } => {
                if vehicle_kind != VehicleKind::Train {
                    return Err(CommandError::IncompatibleStopForVehicle);
                }
                let Some(st) = state.stations.iter().find(|s| s.pos == *waypoint) else {
                    return Err(CommandError::StationNotFound);
                };
                if !st.is_waypoint() {
                    return Err(CommandError::IncompatibleStopForVehicle);
                }
            }
            VehicleOrder::Tile(_) => {}
        }
    }
    let vehicle = &mut state.vehicles[vehicle_idx];
    vehicle.set_vehicle_orders(orders);
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn set_vehicle_orders(
    state: &mut GameState,
    id: u32,
    orders: Vec<TileCoord>,
) -> Result<(), CommandError> {
    for order in &orders {
        in_bounds(&state.map, *order)?;
    }
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.set_orders(orders);
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn set_vehicle_station_orders(
    state: &mut GameState,
    id: u32,
    stations: Vec<TileCoord>,
) -> Result<(), CommandError> {
    set_vehicle_order_list(
        state,
        id,
        stations.into_iter().map(VehicleOrder::station).collect(),
    )
}

/// Comando viejo: compra el motor por defecto del tipo (solo depósito de carretera).
pub(super) fn build_road_vehicle_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
    kind: VehicleKind,
) -> Result<(), CommandError> {
    if matches!(kind, VehicleKind::Train) {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    build_vehicle_at_depot(state, depot_pos, crate::engine::default_engine_id(kind))
}

/// Compra el modelo `engine_id` en un depósito compatible, validando fondos.
pub(super) fn build_vehicle_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
    engine_id: u16,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let Some(tile) = state.map.get(depot_pos) else {
        return Err(CommandError::OutOfBounds);
    };
    let Some(engine) = crate::engine::engine_by_id(engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    let depot_ok = match engine.kind {
        VehicleKind::Bus | VehicleKind::Truck => tile.kind == TileKind::RoadDepot,
        VehicleKind::Train => tile.kind == TileKind::RailDepot,
    };
    if !depot_ok {
        return Err(CommandError::InvalidDepotTile);
    }
    if state.economy.money < engine.price {
        return Err(CommandError::InsufficientFunds);
    }
    let next_id = state
        .vehicles
        .iter()
        .map(|v| v.id)
        .max()
        .map_or(1, |v| v.saturating_add(1));
    let mut vehicle = Vehicle::new(next_id, engine.kind, depot_pos, depot_pos);
    vehicle.running = false;
    vehicle.engine_id = Some(engine.id);
    // Locomotoras sin capacidad propia: hasta que existan vagones, conservan
    // la capacidad genérica para que el transporte siga funcionando.
    if engine.capacity > 0 {
        vehicle.capacity = engine.capacity;
    }
    state.vehicles.push(vehicle);
    state.economy.money -= engine.price;
    Ok(())
}

pub(super) fn sell_vehicle(state: &mut GameState, vehicle_id: u32) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let in_depot = matches!(
        state.map.get_kind(vehicle.pos),
        Some(TileKind::RoadDepot | TileKind::RailDepot)
    );
    if !in_depot {
        return Err(CommandError::VehicleNotInDepot);
    }
    let refund = crate::economy::vehicle_sell_refund(vehicle);
    let Some(idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    state.vehicles.remove(idx);
    state.economy.money += refund;
    Ok(())
}

pub(super) fn toggle_vehicle_running(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let road_dest = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .and_then(|v| road_depot_exit_tile(state, v.pos))
        .and_then(|exit| farthest_reachable_road_tile(&state.map, exit).or(Some(exit)));
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.running = !vehicle.running;
    if vehicle.running
        && vehicle.pos == vehicle.dest
        && let Some(dest) = road_dest
    {
        vehicle.dest = dest;
        vehicle.path.clear();
    }
    Ok(())
}

fn road_depot_exit_tile(state: &GameState, depot_pos: TileCoord) -> Option<TileCoord> {
    let tile = state.map.get(depot_pos)?;
    if tile.kind != TileKind::RoadDepot {
        return None;
    }
    if let Some((exit, _)) = road_depot_exit_for_dir(&state.map, depot_pos, tile.m5 & 0x03)
        && traversable_road_kind(state.map.get_kind(exit))
    {
        return Some(exit);
    }
    let (mw, mh) = state.map.dimensions();
    [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .map(|(dx, dy)| TileCoord::new(depot_pos.x + dx, depot_pos.y + dy))
        .find(|c| {
            c.x >= 0
                && c.y >= 0
                && c.x < mw.cast_signed()
                && c.y < mh.cast_signed()
                && traversable_road_kind(state.map.get_kind(*c))
        })
}

fn traversable_road_kind(kind: Option<TileKind>) -> bool {
    matches!(
        kind,
        Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel)
    )
}

fn farthest_reachable_road_tile(map: &Map, start: TileCoord) -> Option<TileCoord> {
    let (mw, mh) = map.dimensions();
    if !traversable_road_kind(map.get_kind(start)) {
        return None;
    }
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([start]);
    let mut farthest = start;
    seen.insert(start);

    while let Some(cur) = queue.pop_front() {
        if tile_distance(cur, start) > tile_distance(farthest, start) {
            farthest = cur;
        }
        for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw.cast_signed() || next.y >= mh.cast_signed()
            {
                continue;
            }
            if seen.insert(next) && traversable_road_kind(map.get_kind(next)) {
                queue.push_back(next);
            }
        }
    }

    Some(farthest)
}

fn tile_distance(a: TileCoord, b: TileCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

pub(super) fn clone_vehicle_orders(
    state: &mut GameState,
    from_vehicle_id: u32,
    to_vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(src_idx) = state.vehicles.iter().position(|v| v.id == from_vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let Some(dst_idx) = state.vehicles.iter().position(|v| v.id == to_vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let src_orders = state.vehicles[src_idx].orders.clone();
    state.vehicles[dst_idx].set_vehicle_orders(src_orders);
    Ok(())
}
