use openttdrs_core::{TileCoord, TileKind, VehicleKind, VehicleOrder};

use crate::state::SimWorld;

/// Destino válido para el vehículo seleccionado (estación compatible o depósito carretera).
#[must_use]
pub(crate) fn order_pick_valid(sim: &SimWorld, vehicle_id: u32, pos: TileCoord) -> bool {
    order_for_clicked_tile(sim, vehicle_id, pos).is_some()
}

pub(crate) fn order_for_clicked_tile(
    sim: &SimWorld,
    vehicle_id: u32,
    pos: TileCoord,
) -> Option<VehicleOrder> {
    let vehicle = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)?;
    if let Some(station) = sim.state.stations.iter().find(|station| station.pos == pos) {
        if station.is_waypoint() {
            return (vehicle.kind == VehicleKind::Train).then_some(VehicleOrder::waypoint(pos));
        }
        return station
            .can_service_vehicle(vehicle.kind)
            .then_some(VehicleOrder::station(pos));
    }
    if sim.state.map.get_kind(pos) == Some(TileKind::RoadDepot)
        && !matches!(vehicle.kind, VehicleKind::Train)
    {
        return Some(VehicleOrder::tile(pos));
    }
    None
}
