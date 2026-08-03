use openttdrs_core::prelude::*;

use crate::state::SimWorld;

/// Destino válido para el vehículo seleccionado (estación compatible o depósito).
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
            return station
                .can_service_vehicle(vehicle.kind)
                .then_some(VehicleOrder::waypoint(pos));
        }
        return station
            .can_service_vehicle(vehicle.kind)
            .then_some(VehicleOrder::station(pos));
    }
    match (vehicle.kind, sim.state.map.get_kind(pos)) {
        (VehicleKind::Train, Some(TileKind::RailDepot)) => Some(VehicleOrder::depot(pos)),
        (VehicleKind::Ship, Some(TileKind::ShipDepot)) => Some(VehicleOrder::depot(pos)),
        (VehicleKind::Aircraft, Some(TileKind::Airport)) => Some(VehicleOrder::depot(pos)),
        (VehicleKind::Bus | VehicleKind::Truck, Some(TileKind::RoadDepot)) => {
            Some(VehicleOrder::depot(pos))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::{Station, StopKind, Vehicle};

    #[test]
    fn road_vehicle_can_add_a_road_waypoint_order() {
        let pos = TileCoord::new(3, 3);
        let mut sim = SimWorld::default();
        sim.state
            .vehicles
            .push(Vehicle::new(1, VehicleKind::Bus, pos, pos));
        sim.state
            .stations
            .push(Station::new_with_kind(pos, StopKind::RoadWaypoint));

        assert_eq!(
            order_for_clicked_tile(&sim, 1, pos),
            Some(VehicleOrder::waypoint(pos))
        );
        assert!(order_pick_valid(&sim, 1, pos));
    }

    #[test]
    fn incompatible_waypoint_is_not_offered_as_an_order() {
        let pos = TileCoord::new(3, 3);
        let mut sim = SimWorld::default();
        sim.state
            .vehicles
            .push(Vehicle::new(1, VehicleKind::Train, pos, pos));
        sim.state
            .stations
            .push(Station::new_with_kind(pos, StopKind::RoadWaypoint));

        assert_eq!(order_for_clicked_tile(&sim, 1, pos), None);
    }
}
