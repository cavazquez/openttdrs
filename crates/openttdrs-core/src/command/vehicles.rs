use crate::GameState;
use crate::depot::nearest_depot_tile;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, farthest_reachable_tile};
use crate::vehicle::{MAX_VEHICLE_NAME_CHARS, Vehicle, VehicleKind, VehicleOrder};

use super::error::OrderMoveDirection;
use super::transport::road_depot_exit_for_dir;
use super::{CommandError, in_bounds, require_tile_owned_by_active, require_vehicle_owned_by_active};

pub(super) fn set_vehicle_order_list(
    state: &mut GameState,
    id: u32,
    orders: Vec<VehicleOrder>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, id)?;
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
            VehicleOrder::Waypoint { waypoint, .. } => {
                let Some(st) = state.stations.iter().find(|s| s.pos == *waypoint) else {
                    return Err(CommandError::StationNotFound);
                };
                if !st.is_waypoint() || !st.can_service_vehicle(vehicle_kind) {
                    return Err(CommandError::IncompatibleStopForVehicle);
                }
            }
            VehicleOrder::Depot { depot, .. } => {
                in_bounds(&state.map, *depot)?;
                let kind = state.map.get_kind(*depot);
                let ok = match vehicle_kind {
                    VehicleKind::Train => kind == Some(TileKind::RailDepot),
                    VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
                        kind == Some(TileKind::RoadDepot)
                    }
                    VehicleKind::Ship => kind == Some(TileKind::ShipDepot),
                    VehicleKind::Aircraft => kind == Some(TileKind::Airport),
                };
                if !ok {
                    return Err(CommandError::InvalidDepotTile);
                }
            }
            VehicleOrder::Tile(_) | VehicleOrder::Conditional { .. } => {}
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
    require_vehicle_owned_by_active(state, id)?;
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
    if matches!(
        kind,
        VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft
    ) {
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
    // Validar ownership del depósito antes de comprar.
    require_tile_owned_by_active(state, depot_pos)?;
    let Some(engine) = crate::engine::engine_in_catalog(&state.engine_catalog, engine_id)
        .cloned()
        .or_else(|| crate::engine::engine_by_id(engine_id).cloned())
    else {
        return Err(CommandError::EngineNotFound);
    };
    let depot_ok = match engine.kind {
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            tile.kind == TileKind::RoadDepot
        }
        VehicleKind::Train => tile.kind == TileKind::RailDepot,
        VehicleKind::Ship => tile.kind == TileKind::ShipDepot,
        VehicleKind::Aircraft => {
            tile.kind == TileKind::Airport
                && crate::airport::airport_tile_is_hangar(&state.map, depot_pos)
        }
    };
    if !depot_ok {
        return Err(CommandError::InvalidDepotTile);
    }
    if engine.kind == VehicleKind::Aircraft {
        let heli_tile = crate::airport::airport_tile_is_heliport(&state.map, depot_pos);
        let heli_engine = crate::engine::aircraft_is_helicopter(engine_id);
        if heli_tile != heli_engine {
            return Err(CommandError::VehicleKindNotAllowed);
        }
    }
    // Compatibilidad motor↔vía adyacente (eléctrico / mono / maglev).
    if engine.kind == VehicleKind::Train {
        let required = crate::rail_type::required_rail_type_for_engine(engine_id);
        if required != crate::rail_type::RailType::Rail {
            let neighbor_ok = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let n = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
                state.map.get(n).is_some_and(|t| {
                    t.kind == TileKind::Rail && crate::rail_type::rail_type_from_tile(t) == required
                })
            });
            if !neighbor_ok {
                return Err(match required {
                    crate::rail_type::RailType::Electric => {
                        CommandError::EngineRequiresElectricRail
                    }
                    crate::rail_type::RailType::Monorail => CommandError::EngineRequiresMonorail,
                    crate::rail_type::RailType::Maglev => CommandError::EngineRequiresMaglev,
                    crate::rail_type::RailType::Rail => CommandError::EngineRequiresElectricRail,
                });
            }
        }
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
    if engine.capacity > 0 {
        vehicle.capacity = engine.capacity;
    } else if engine.kind == VehicleKind::Train && engine.is_train_engine() {
        // Loco sola: capacidad placeholder hasta enganchar vagones.
        vehicle.capacity = crate::vehicle::VEHICLE_CAPACITY;
    }
    if engine.is_wagon() {
        vehicle.cargo_type = engine.cargo;
        vehicle.capacity = engine.capacity;
    }
    vehicle.build_tick = state.tick.get();
    vehicle.owner = state.active_company;
    if engine.kind == VehicleKind::Train
        && let Some(mouth) = crate::depot::rail_depot_mouth_dir(&state.map, depot_pos)
    {
        vehicle.direction = crate::train_movement::train_depot_facing(mouth);
        vehicle.progress = 0;
    }
    state.vehicles.push(vehicle);
    if engine.kind == VehicleKind::Train && engine.is_dual_headed() {
        spawn_dual_headed_rear(state, next_id, depot_pos, &engine);
    }
    if engine.kind == VehicleKind::Train {
        crate::train_consist::consist_changed(&mut state.vehicles, next_id);
    }
    state.economy.money -= engine.price;
    Ok(())
}

/// Cabina trasera multihead (`AddRearEngineToMultiheadedTrain`).
fn spawn_dual_headed_rear(
    state: &mut GameState,
    front_id: u32,
    depot_pos: TileCoord,
    engine: &crate::engine::EngineDef,
) {
    let rear_id = front_id.saturating_add(1);
    let mut rear = Vehicle::new(rear_id, engine.kind, depot_pos, depot_pos);
    rear.running = false;
    rear.engine_id = Some(engine.id);
    rear.capacity = engine.capacity;
    rear.cargo_type = engine.cargo;
    rear.build_tick = state.tick.get();
    rear.owner = state.active_company;
    rear.direction = state
        .vehicles
        .iter()
        .find(|v| v.id == front_id)
        .map_or(crate::DIR_NE, |v| v.direction);
    rear.prev_unit = Some(front_id);
    rear.other_multiheaded_part = Some(front_id);
    if let Some(front) = state.vehicles.iter_mut().find(|v| v.id == front_id) {
        front.next_unit = Some(rear_id);
        front.other_multiheaded_part = Some(rear_id);
        if engine.capacity > 0 {
            front.capacity = engine.capacity;
            front.cargo_type = engine.cargo;
        }
    }
    state.vehicles.push(rear);
}

pub(super) fn attach_wagon_to_consist(
    state: &mut GameState,
    head_id: u32,
    wagon_id: u32,
) -> Result<(), CommandError> {
    attach_wagon_to_consist_ex(state, head_id, wagon_id, false)
}

fn attach_wagon_to_consist_ex(
    state: &mut GameState,
    head_id: u32,
    wagon_id: u32,
    keep_chain: bool,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, head_id)?;
    require_vehicle_owned_by_active(state, wagon_id)?;
    let head = state
        .vehicles
        .iter()
        .find(|v| v.id == head_id)
        .ok_or(CommandError::VehicleNotFound)?;
    let wagon = state
        .vehicles
        .iter()
        .find(|v| v.id == wagon_id)
        .ok_or(CommandError::VehicleNotFound)?;
    if head.kind != VehicleKind::Train || wagon.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if !head.is_consist_head() {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    let in_depot = |pos| matches!(state.map.get_kind(pos), Some(TileKind::RailDepot));
    if !in_depot(head.pos) || !in_depot(wagon.pos) || head.pos != wagon.pos {
        return Err(CommandError::VehicleNotInDepot);
    }
    let wagon_eng = wagon
        .engine_id
        .and_then(crate::engine::engine_by_id)
        .ok_or(CommandError::EngineNotFound)?;
    if !wagon_eng.is_wagon() && wagon.prev_unit.is_some() {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    let attach = if keep_chain {
        crate::train_consist::attach_wagon_chain
    } else {
        crate::train_consist::attach_wagon
    };
    attach(&mut state.vehicles, head_id, wagon_id)
        .map_err(|()| CommandError::VehicleKindNotAllowed)?;
    Ok(())
}

pub(super) fn detach_consist_unit(state: &mut GameState, unit_id: u32) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, unit_id)?;
    let unit = state
        .vehicles
        .iter()
        .find(|v| v.id == unit_id)
        .ok_or(CommandError::VehicleNotFound)?;
    if unit.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if !matches!(state.map.get_kind(unit.pos), Some(TileKind::RailDepot)) {
        return Err(CommandError::VehicleNotInDepot);
    }
    crate::train_consist::detach_unit(&mut state.vehicles, unit_id)
        .map_err(|()| CommandError::VehicleNotFound)?;
    Ok(())
}

pub(super) fn move_rail_vehicle(
    state: &mut GameState,
    head_id: u32,
    unit_id: u32,
    after_id: Option<u32>,
    move_chain: bool,
) -> Result<(), CommandError> {
    // Validar como detach; luego cortar con o sin cola.
    require_vehicle_owned_by_active(state, unit_id)?;
    let unit = state
        .vehicles
        .iter()
        .find(|v| v.id == unit_id)
        .ok_or(CommandError::VehicleNotFound)?;
    if unit.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if !matches!(state.map.get_kind(unit.pos), Some(TileKind::RailDepot)) {
        return Err(CommandError::VehicleNotInDepot);
    }
    if move_chain {
        crate::train_consist::detach_unit_keep_tail(&mut state.vehicles, unit_id)
            .map_err(|()| CommandError::VehicleNotFound)?;
    } else {
        crate::train_consist::detach_unit(&mut state.vehicles, unit_id)
            .map_err(|()| CommandError::VehicleNotFound)?;
    }
    let attach_head = after_id
        .and_then(|aid| crate::train_consist::consist_head_id(&state.vehicles, aid))
        .unwrap_or(head_id);
    // Enganchar al final del consist (MVP: no inserta en medio).
    attach_wagon_to_consist_ex(state, attach_head, unit_id, move_chain)
}

pub(super) fn sell_vehicle(state: &mut GameState, vehicle_id: u32) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let owner = vehicle.owner;
    let in_depot = matches!(
        state.map.get_kind(vehicle.pos),
        Some(TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    );
    if !in_depot {
        return Err(CommandError::VehicleNotInDepot);
    }
    let chain = crate::train_consist::sell_chain_ids(&state.vehicles, vehicle_id);
    if chain.is_empty() {
        return Err(CommandError::VehicleNotFound);
    }
    // Si vendemos un vagón del medio, desenganchar primero.
    if chain.len() == 1
        && state
            .vehicles
            .iter()
            .find(|v| v.id == vehicle_id)
            .is_some_and(|v| v.prev_unit.is_some() || v.next_unit.is_some())
    {
        let _ = crate::train_consist::detach_unit(&mut state.vehicles, vehicle_id);
    }
    let mut refund_total = 0_i64;
    for id in &chain {
        if let Some(v) = state.vehicles.iter().find(|x| x.id == *id) {
            refund_total += crate::economy::vehicle_sell_refund(v);
        }
    }
    state.vehicles.retain(|v| !chain.contains(&v.id));
    // Recalcular cabezas restantes tocadas.
    let heads: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        .map(|v| v.id)
        .collect();
    for hid in heads {
        crate::train_consist::consist_changed(&mut state.vehicles, hid);
    }
    state.credit_company(owner, refund_total);
    Ok(())
}

pub(super) fn toggle_vehicle_running(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let kind = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .map(|v| v.kind);
    let road_dest = match kind {
        Some(VehicleKind::Bus | VehicleKind::Truck) => state
            .vehicles
            .iter()
            .find(|v| v.id == vehicle_id)
            .and_then(|v| road_depot_exit_tile(state, v.pos))
            .and_then(|exit| {
                farthest_reachable_tile(&state.map, exit, PathNetwork::Road).or(Some(exit))
            }),
        Some(VehicleKind::Tram) => {
            if let Some(v) = state.vehicles.iter().find(|v| v.id == vehicle_id)
                && let Some(exit) = road_depot_exit_tile(state, v.pos)
            {
                ensure_tram_bits_for_depot_exit(state, v.pos, exit);
                farthest_reachable_tile(&state.map, exit, PathNetwork::Tram)
            } else {
                None
            }
        }
        _ => None,
    };
    let depot_mouth = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .filter(|v| v.kind == VehicleKind::Train)
        .and_then(|v| crate::depot::rail_depot_mouth_dir(&state.map, v.pos));
    let (was_running, vehicle_pos, vehicle_kind, now_running) = {
        let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
            return Err(CommandError::VehicleNotFound);
        };
        let was_running = vehicle.running;
        let vehicle_pos = vehicle.pos;
        let vehicle_kind = vehicle.kind;
        vehicle.running = !vehicle.running;
        let now_running = vehicle.running;
        if now_running && let Some(mouth) = depot_mouth {
            vehicle.direction = crate::train_movement::train_depot_facing(mouth);
            vehicle.progress = 0;
            vehicle.depart_turn = 0;
            vehicle.wait_counter = 0;
            vehicle.depot_leave_cleared = false;
            vehicle.pbs_stuck = false;
        }
        if now_running
            && vehicle.pos == vehicle.dest
            && let Some(dest) = road_dest
        {
            vehicle.dest = dest;
            vehicle.path.clear();
        }
        (was_running, vehicle_pos, vehicle_kind, now_running)
    };
    if now_running && !was_running && !state.news_first_vehicle_running_sent {
        state.news_first_vehicle_running_sent = true;
        crate::news::push_first_vehicle_running_news(state, vehicle_id, vehicle_pos, vehicle_kind);
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

/// Asegura overlay de tranvía en depósito y boca al salir (copia bits de carretera si falta).
fn ensure_tram_bits_for_depot_exit(state: &mut GameState, depot: TileCoord, exit: TileCoord) {
    use crate::road_type::{
        RoadType, set_tram_road_type_on_tile, set_tram_track_bits_on_tile, tram_track_bits,
    };
    let Some(exit_tile) = state.map.get(exit) else {
        return;
    };
    if tram_track_bits(&exit_tile) == 0 {
        let road_bits = exit_tile.m5 & 0x0F;
        let bits = if road_bits == 0 { 0x0F } else { road_bits };
        let mut t = exit_tile;
        t = set_tram_track_bits_on_tile(t, bits);
        t = set_tram_road_type_on_tile(t, Some(RoadType::Tram));
        let _ = state.map.set_tile(exit, t);
    }
    let Some(depot_tile) = state.map.get(depot) else {
        return;
    };
    if tram_track_bits(&depot_tile) == 0 {
        let road_bits = depot_tile.m5 & 0x0F;
        let bits = if road_bits == 0 { 0x0F } else { road_bits };
        let mut t = depot_tile;
        t = set_tram_track_bits_on_tile(t, bits);
        t = set_tram_road_type_on_tile(t, Some(RoadType::Tram));
        let _ = state.map.set_tile(depot, t);
    }
}

pub(super) fn clone_vehicle_orders(
    state: &mut GameState,
    from_vehicle_id: u32,
    to_vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, from_vehicle_id)?;
    require_vehicle_owned_by_active(state, to_vehicle_id)?;
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

/// Compra una copia del vehículo origen (motor + órdenes) en el mismo depósito.
pub(super) fn clone_vehicle_at_depot(
    state: &mut GameState,
    source_vehicle_id: u32,
    depot_pos: TileCoord,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, source_vehicle_id)?;
    let (engine_id, orders) = {
        let Some(source) = state.vehicles.iter().find(|v| v.id == source_vehicle_id) else {
            return Err(CommandError::VehicleNotFound);
        };
        if source.pos != depot_pos {
            return Err(CommandError::VehicleNotInDepot);
        }
        (
            source
                .engine_id
                .unwrap_or_else(|| crate::engine::default_engine_id(source.kind)),
            source.orders.clone(),
        )
    };
    build_vehicle_at_depot(state, depot_pos, engine_id)?;
    let Some(new_vehicle) = state.vehicles.last_mut() else {
        return Err(CommandError::VehicleNotFound);
    };
    new_vehicle.set_vehicle_orders(orders);
    Ok(())
}

/// Vende todos los vehículos en la tesela de depósito.
pub(super) fn sell_all_vehicles_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
) -> Result<(), CommandError> {
    let owner = state.active_company;
    let ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.pos == depot_pos && v.owner == owner)
        .map(|v| v.id)
        .collect();
    for id in ids {
        sell_vehicle(state, id)?;
    }
    Ok(())
}

pub(super) fn refit_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
    cargo: crate::cargo::CargoType,
    unit_ids: &[u32],
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let targets: Vec<u32> = if unit_ids.is_empty() {
        vec![vehicle_id]
    } else {
        unit_ids.to_vec()
    };
    for &tid in &targets {
        require_vehicle_owned_by_active(state, tid)?;
        if tid != vehicle_id && !crate::same_consist(&state.vehicles, vehicle_id, tid) {
            return Err(CommandError::RefitNotAllowed);
        }
    }
    for &tid in &targets {
        let Some(vehicle) = state.vehicles.iter().find(|v| v.id == tid) else {
            return Err(CommandError::VehicleNotFound);
        };
        if vehicle.cargo > 0 {
            return Err(CommandError::RefitNotAllowed);
        }
        if !crate::refit::vehicle_in_depot(&state.map, vehicle.pos) {
            return Err(CommandError::RefitNotAllowed);
        }
        if !crate::refit::refittable_cargo_types(vehicle).contains(&cargo) {
            return Err(CommandError::RefitNotAllowed);
        }
    }
    for &tid in &targets {
        let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == tid) else {
            return Err(CommandError::VehicleNotFound);
        };
        vehicle.cargo_type = Some(cargo);
    }
    Ok(())
}

pub(super) fn cycle_vehicle_order_depot_refit(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if index >= state.vehicles[vehicle_idx].orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let options = crate::refit::refittable_cargo_types(&state.vehicles[vehicle_idx]).to_vec();
    let Some(updated) = state.vehicles[vehicle_idx].orders[index].with_cycled_depot_refit(&options)
    else {
        return Err(CommandError::OrderIndexOutOfRange);
    };
    state.vehicles[vehicle_idx].orders[index] = updated;
    Ok(())
}

pub(super) fn remove_vehicle_order_at(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders.remove(index);
    if vehicle.orders.is_empty() {
        vehicle.current_order = 0;
        vehicle.dest = vehicle.pos;
        vehicle.path.clear();
        vehicle.no_network_route_to_order = false;
        return Ok(());
    }
    if vehicle.current_order > index {
        vehicle.current_order -= 1;
    } else if vehicle.current_order == index {
        vehicle.current_order = vehicle.current_order.min(vehicle.orders.len() - 1);
    }
    vehicle.path.clear();
    vehicle.depart_turn = 0;
    vehicle.no_network_route_to_order = false;
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn skip_vehicle_order(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if vehicle.orders.is_empty() {
        return Ok(());
    }
    vehicle.path.clear();
    vehicle.depart_turn = 0;
    vehicle.no_network_route_to_order = false;
    vehicle.progress = 0;
    vehicle.current_order = (vehicle.current_order + 1) % vehicle.orders.len();
    vehicle.origin = vehicle.pos;
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn toggle_vehicle_order_full_load(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(
        state,
        vehicle_id,
        index,
        VehicleOrder::with_toggled_full_load,
    )
}

pub(super) fn toggle_vehicle_order_no_unload(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(
        state,
        vehicle_id,
        index,
        VehicleOrder::with_toggled_no_unload,
    )
}

fn toggle_vehicle_order_flag(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    toggle: impl FnOnce(VehicleOrder) -> Option<VehicleOrder>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let Some(updated) = toggle(vehicle.orders[index]) else {
        return Err(CommandError::OrderFlagNotApplicable);
    };
    vehicle.orders[index] = updated;
    if index == vehicle.current_order {
        vehicle.sync_order_destination(&state.map);
    }
    Ok(())
}

pub(super) fn append_goto_nearest_depot(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let (kind, pos) = {
        let v = &state.vehicles[vehicle_idx];
        (v.kind, v.pos)
    };
    let Some(depot) = nearest_depot_tile(&state.map, pos, kind) else {
        return Err(CommandError::DepotNotFound);
    };
    in_bounds(&state.map, depot)?;
    let vehicle = &mut state.vehicles[vehicle_idx];
    vehicle.append_order(VehicleOrder::depot(depot), &state.map);
    Ok(())
}

pub(super) fn rename_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
    name: Option<String>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let normalized = name.and_then(|n| {
        let trimmed = n.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    if normalized
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_VEHICLE_NAME_CHARS)
    {
        return Err(CommandError::VehicleNameTooLong);
    }
    vehicle.name = normalized;
    Ok(())
}

pub(super) fn set_depot_vehicles_running(
    state: &mut GameState,
    depot_pos: TileCoord,
    running: bool,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let kind = state.map.get_kind(depot_pos);
    if !matches!(
        kind,
        Some(TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    ) {
        return Err(CommandError::InvalidDepotTile);
    }
    let owner = state.active_company;
    let ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.pos == depot_pos && v.owner == owner)
        .map(|v| v.id)
        .collect();
    for id in ids {
        if running && super::vehicle_fleet::can_start_vehicle_from_depot(state, id).is_err() {
            continue;
        }
        let Some(idx) = state.vehicles.iter().position(|v| v.id == id) else {
            continue;
        };
        let vehicle_pos = state.vehicles[idx].pos;
        let was_at_dest = state.vehicles[idx].pos == state.vehicles[idx].dest;
        let depot_mouth = if running && state.vehicles[idx].kind == VehicleKind::Train {
            crate::depot::rail_depot_mouth_dir(&state.map, vehicle_pos)
        } else {
            None
        };
        if state.vehicles[idx].running != running {
            if running && let Some(mouth) = depot_mouth {
                state.vehicles[idx].direction = crate::train_movement::train_depot_facing(mouth);
                state.vehicles[idx].progress = 0;
                state.vehicles[idx].depart_turn = 0;
                state.vehicles[idx].wait_counter = 0;
                state.vehicles[idx].depot_leave_cleared = false;
                state.vehicles[idx].pbs_stuck = false;
            }
            state.vehicles[idx].running = running;
            if running
                && was_at_dest
                && let Some(dest) = match state.vehicles[idx].kind {
                    VehicleKind::Tram => {
                        road_depot_exit_tile(state, vehicle_pos).and_then(|exit| {
                            ensure_tram_bits_for_depot_exit(state, vehicle_pos, exit);
                            farthest_reachable_tile(&state.map, exit, PathNetwork::Tram)
                        })
                    }
                    VehicleKind::Bus | VehicleKind::Truck => {
                        road_depot_exit_tile(state, vehicle_pos).and_then(|exit| {
                            farthest_reachable_tile(&state.map, exit, PathNetwork::Road)
                                .or(Some(exit))
                        })
                    }
                    _ => None,
                }
            {
                state.vehicles[idx].dest = dest;
                state.vehicles[idx].path.clear();
            }
        }
    }
    Ok(())
}

pub(super) fn move_vehicle_order(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    direction: OrderMoveDirection,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let other = match direction {
        OrderMoveDirection::Up if index > 0 => index - 1,
        OrderMoveDirection::Down if index + 1 < vehicle.orders.len() => index + 1,
        _ => return Err(CommandError::OrderIndexOutOfRange),
    };
    vehicle.orders.swap(index, other);
    if vehicle.current_order == index {
        vehicle.current_order = other;
    } else if vehicle.current_order == other {
        vehicle.current_order = index;
    }
    Ok(())
}

pub(super) fn toggle_vehicle_order_depot_stop(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(
        state,
        vehicle_id,
        index,
        VehicleOrder::with_toggled_depot_stop,
    )
}

pub(super) fn turn_around_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if state.vehicles[idx].kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if state.vehicles[idx].breakdown_ticks_remaining > 0 {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    // `CmdReverseTrainDirection`: velocidad a 0 + liberar reserva delante.
    state.vehicles[idx].cur_speed = 0;
    state.vehicles[idx].force_proceed = false;
    state.vehicles[idx].reverse_heading();
    state.vehicles[idx].path.clear();
    state.vehicles[idx].reserved_steps.clear();
    state.vehicles[idx].wait_counter = 0;
    state.vehicles[idx].pbs_stuck = false;
    state.vehicles[idx].no_network_route_to_order = false;
    state.vehicles[idx].sync_order_destination(&state.map);
    crate::rail_pbs::update_train_reservations(&state.map, &mut state.vehicles);
    if crate::rail_pbs::train_waiting_for_pbs_path(&state.map, &state.vehicles[idx]) {
        state.vehicles[idx].pbs_stuck = true;
        crate::news::push_vehicle_advice_news(
            state,
            vehicle_id,
            state.vehicles[idx].current_order,
            state.vehicles[idx].pos,
            crate::news::VehicleAdviceKind::PbsStuck,
        );
    }
    Ok(())
}

pub(super) fn force_vehicle_proceed(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if vehicle.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    vehicle.force_proceed = true;
    Ok(())
}

pub(super) fn toggle_vehicle_timetable(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.timetable_active = !vehicle.timetable_active;
    if !vehicle.timetable_active {
        vehicle.timetable_wait_remaining = 0;
        vehicle.timetable_wait_kind = crate::vehicle::TimetableWaitKind::None;
    }
    Ok(())
}

pub(super) fn cycle_vehicle_order_wait(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(state, vehicle_id, index, VehicleOrder::with_cycled_wait)
}

pub(super) fn cycle_vehicle_order_travel(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders[index] = vehicle.orders[index].with_cycled_travel();
    Ok(())
}

pub(super) fn set_autoreplace_rule(
    state: &mut GameState,
    from_engine_id: u16,
    to_engine_id: u16,
) -> Result<(), CommandError> {
    if from_engine_id == to_engine_id {
        return Err(CommandError::AutoreplaceNotAllowed);
    }
    let Some(from) = crate::engine::engine_by_id(from_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    let Some(to) = crate::engine::engine_by_id(to_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    if from.kind != to.kind {
        return Err(CommandError::AutoreplaceNotAllowed);
    }
    if let Some(rule) = state
        .autoreplace_rules
        .iter_mut()
        .find(|r| r.from_engine_id == from_engine_id)
    {
        rule.to_engine_id = to_engine_id;
        rule.enabled = true;
    } else {
        state
            .autoreplace_rules
            .push(crate::autoreplace::AutoReplaceRule::new(
                from_engine_id,
                to_engine_id,
            ));
    }
    Ok(())
}

pub(super) fn clear_autoreplace_rule(
    state: &mut GameState,
    from_engine_id: u16,
) -> Result<(), CommandError> {
    let len_before = state.autoreplace_rules.len();
    state
        .autoreplace_rules
        .retain(|r| r.from_engine_id != from_engine_id);
    if state.autoreplace_rules.len() == len_before {
        return Err(CommandError::AutoReplaceRuleNotFound);
    }
    Ok(())
}

pub(super) fn toggle_autoreplace_rule(
    state: &mut GameState,
    from_engine_id: u16,
) -> Result<(), CommandError> {
    let Some(rule) = state
        .autoreplace_rules
        .iter_mut()
        .find(|r| r.from_engine_id == from_engine_id)
    else {
        return Err(CommandError::AutoReplaceRuleNotFound);
    };
    rule.enabled = !rule.enabled;
    Ok(())
}
