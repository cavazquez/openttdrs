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
    let Some(orders) = state
        .shared_order_lists
        .iter()
        .find(|l| l.id == shared_id)
        .map(|l| l.orders.as_slice())
    else {
        return;
    };
    for vehicle in &mut state.vehicles {
        if vehicle.shared_order_id == Some(shared_id) {
            vehicle.orders.clear();
            vehicle.orders.extend_from_slice(orders);
            if vehicle.current_order >= vehicle.orders.len() && !vehicle.orders.is_empty() {
                vehicle.current_order = vehicle.orders.len() - 1;
            }
            vehicle.sync_order_destination(&state.map);
        }
    }
}
