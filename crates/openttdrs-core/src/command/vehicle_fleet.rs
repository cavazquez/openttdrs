//! Comandos de flota avanzados: grupos, horario, autoreemplazo masivo, pool compartido.

use crate::GameState;
use crate::map::TileKind;
use crate::shared_orders::{SharedOrderList, next_shared_order_id, sync_shared_orders_to_vehicles};
use crate::vehicle::{OrderConditionKind, VehicleOrder};
use crate::vehicle_group::{MAX_VEHICLE_GROUP_NAME_CHARS, VehicleGroup, next_vehicle_group_id};

use super::in_bounds;
use super::types::CommandError;
use super::vehicles;

pub(super) fn create_vehicle_group(state: &mut GameState, name: &str) -> Result<(), CommandError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CommandError::VehicleGroupNameInvalid);
    }
    if trimmed.chars().count() > MAX_VEHICLE_GROUP_NAME_CHARS {
        return Err(CommandError::VehicleNameTooLong);
    }
    let id = next_vehicle_group_id(&state.vehicle_groups);
    state
        .vehicle_groups
        .push(VehicleGroup::new(id, trimmed.to_string()));
    Ok(())
}

pub(super) fn rename_vehicle_group(
    state: &mut GameState,
    group_id: u32,
    name: &str,
) -> Result<(), CommandError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CommandError::VehicleGroupNameInvalid);
    }
    if trimmed.chars().count() > MAX_VEHICLE_GROUP_NAME_CHARS {
        return Err(CommandError::VehicleNameTooLong);
    }
    let Some(group) = state.vehicle_groups.iter_mut().find(|g| g.id == group_id) else {
        return Err(CommandError::VehicleGroupNotFound);
    };
    group.name = trimmed.to_string();
    Ok(())
}

pub(super) fn assign_vehicle_to_group(
    state: &mut GameState,
    vehicle_id: u32,
    group_id: Option<u32>,
) -> Result<(), CommandError> {
    if let Some(gid) = group_id
        && !state.vehicle_groups.iter().any(|g| g.id == gid)
    {
        return Err(CommandError::VehicleGroupNotFound);
    }
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.group_id = group_id;
    Ok(())
}

pub(super) fn clear_vehicle_timetable_lateness(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.timetable_lateness = 0;
    Ok(())
}

pub(super) fn set_vehicle_order_wait_ticks(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    wait_ticks: u32,
) -> Result<(), CommandError> {
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let Some(updated) = vehicle.orders[index].with_wait_ticks(wait_ticks) else {
        return Err(CommandError::TimetableNotApplicable);
    };
    vehicle.orders[index] = updated;
    Ok(())
}

pub(super) fn set_vehicle_order_travel_ticks(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    travel_ticks: u32,
) -> Result<(), CommandError> {
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders[index] = vehicle.orders[index].with_travel_ticks(travel_ticks);
    Ok(())
}

pub(super) fn toggle_vehicle_timetable_autofill(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.timetable_autofill = !vehicle.timetable_autofill;
    if !vehicle.timetable_autofill {
        vehicle.timetable_autofill_samples.clear();
    }
    Ok(())
}

pub(super) fn set_autoreplace_only_when_old(
    state: &mut GameState,
    from_engine_id: u16,
    only_when_old: bool,
) -> Result<(), CommandError> {
    let Some(rule) = state
        .autoreplace_rules
        .iter_mut()
        .find(|r| r.from_engine_id == from_engine_id)
    else {
        return Err(CommandError::AutoReplaceRuleNotFound);
    };
    rule.only_when_old = only_when_old;
    Ok(())
}

pub(super) fn set_autoreplace_rule_group(
    state: &mut GameState,
    from_engine_id: u16,
    group_id: Option<u32>,
) -> Result<(), CommandError> {
    if let Some(gid) = group_id
        && !state.vehicle_groups.iter().any(|g| g.id == gid)
    {
        return Err(CommandError::VehicleGroupNotFound);
    }
    let Some(rule) = state
        .autoreplace_rules
        .iter_mut()
        .find(|r| r.from_engine_id == from_engine_id)
    else {
        return Err(CommandError::AutoReplaceRuleNotFound);
    };
    rule.group_id = group_id;
    Ok(())
}

pub(super) fn depot_mass_autoreplace(
    state: &mut GameState,
    depot_pos: crate::TileCoord,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let kind = state.map.get_kind(depot_pos);
    if !matches!(
        kind,
        Some(TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    ) {
        return Err(CommandError::InvalidDepotTile);
    }
    let ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.pos == depot_pos)
        .map(|v| v.id)
        .collect();
    for id in ids {
        if let Some(v) = state.vehicles.iter_mut().find(|v| v.id == id) {
            v.autoreplace_attempted_this_stop = false;
        }
        let _ = crate::autoreplace::try_autoreplace_vehicle(state, id);
    }
    Ok(())
}

pub(super) fn create_shared_orders_from_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if vehicle.orders.is_empty() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let id = next_shared_order_id(&state.shared_order_lists);
    let orders = vehicle.orders.clone();
    state
        .shared_order_lists
        .push(SharedOrderList { id, orders });
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.shared_order_id = Some(id);
    Ok(())
}

pub(super) fn link_vehicle_to_shared_orders(
    state: &mut GameState,
    vehicle_id: u32,
    shared_id: u32,
) -> Result<(), CommandError> {
    if !state.shared_order_lists.iter().any(|l| l.id == shared_id) {
        return Err(CommandError::SharedOrdersNotFound);
    }
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.shared_order_id = Some(shared_id);
    sync_shared_orders_to_vehicles(state, shared_id);
    Ok(())
}

pub(super) fn unlink_vehicle_shared_orders(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.shared_order_id = None;
    Ok(())
}

pub(super) fn set_shared_order_at(
    state: &mut GameState,
    shared_id: u32,
    index: usize,
    order: VehicleOrder,
) -> Result<(), CommandError> {
    in_bounds(&state.map, order.destination())?;
    let Some(list) = state
        .shared_order_lists
        .iter_mut()
        .find(|l| l.id == shared_id)
    else {
        return Err(CommandError::SharedOrdersNotFound);
    };
    if index >= list.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    list.orders[index] = order;
    sync_shared_orders_to_vehicles(state, shared_id);
    Ok(())
}

pub(super) fn set_vehicle_order_conditional(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    condition: OrderConditionKind,
    value: u8,
    jump_to: usize,
) -> Result<(), CommandError> {
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() || jump_to >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders[index] = VehicleOrder::conditional(condition, value, jump_to);
    Ok(())
}

pub(super) fn depot_reorder_vehicle_slot(
    state: &mut GameState,
    depot_pos: crate::TileCoord,
    from_slot: usize,
    to_slot: usize,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let mut ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.pos == depot_pos)
        .map(|v| v.id)
        .collect();
    ids.sort_unstable();
    if from_slot >= ids.len() || to_slot >= ids.len() || from_slot == to_slot {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let id = ids.remove(from_slot);
    ids.insert(to_slot, id);
    for (slot, &vid) in ids.iter().enumerate() {
        if let Some(v) = state.vehicles.iter_mut().find(|v| v.id == vid) {
            v.depot_display_slot = u8::try_from(slot).ok();
        }
    }
    Ok(())
}

/// Reutiliza validación de arranque con horario en depósito.
pub(super) fn can_start_vehicle_from_depot(
    state: &GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if vehicle.timetable_active
        && vehicle.timetable_wait_remaining > 0
        && vehicle
            .orders
            .get(vehicle.current_order)
            .is_some_and(|o| o.is_depot())
    {
        return Err(CommandError::TimetableWaitPending);
    }
    Ok(())
}

pub(super) fn toggle_vehicle_running_checked(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let was_running = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .is_some_and(|v| v.running);
    if !was_running {
        can_start_vehicle_from_depot(state, vehicle_id)?;
    }
    vehicles::toggle_vehicle_running(state, vehicle_id)
}
