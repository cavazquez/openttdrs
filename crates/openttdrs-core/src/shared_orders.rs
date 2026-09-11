//! Pool de órdenes compartidas entre vehículos (`OpenTTD` shared orders).

use crate::GameState;
use crate::vehicle::VehicleOrder;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SharedOrderList {
    pub id: u32,
    pub orders: Vec<VehicleOrder>,
}

#[must_use]
pub fn next_shared_order_id(lists: &[SharedOrderList]) -> u32 {
    lists
        .iter()
        .map(|l| l.id)
        .max()
        .map_or(1, |id| id.saturating_add(1))
}

pub fn sync_shared_orders_to_vehicles(state: &mut GameState, shared_id: u32) {
    let Some(list_idx) = state
        .shared_order_lists
        .iter()
        .position(|l| l.id == shared_id)
    else {
        return;
    };
    // Un save legacy puede contener la sección sur de un depósito naval en
    // una lista compartida. Normalizar la lista antes de copiarla mantiene
    // iguales el writer persistente, los nuevos vínculos y `vehicle.dest`.
    let orders: Vec<_> = state.shared_order_lists[list_idx]
        .orders
        .iter()
        .copied()
        .map(|order| {
            order.with_depot_tile(crate::depot::canonical_depot_command_tile(
                &state.map,
                order.destination(),
            ))
        })
        .collect();
    state.shared_order_lists[list_idx]
        .orders
        .clone_from(&orders);
    for vehicle in &mut state.vehicles {
        if vehicle.shared_order_id == Some(shared_id) {
            vehicle.orders.clear();
            vehicle.orders.extend_from_slice(&orders);
            if vehicle.current_order >= vehicle.orders.len() && !vehicle.orders.is_empty() {
                vehicle.current_order = vehicle.orders.len() - 1;
            }
            vehicle.sync_order_destination_with_stations(&state.map, &state.stations);
        }
    }
}
