//! Clasificación de estado operacional de vehículos.
//!
//! Este módulo proporciona análisis estructurado del estado de vehículos,
//! separando la lógica de dominio de la presentación en el HUD.

use crate::vehicle::Vehicle;
use crate::GameState;

/// Resumen de vehículos con problemas operacionales.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VehicleOperationalSummary {
    /// Vehículos sin ruta por red a su orden actual.
    pub no_network_route: VehicleIssueDetail,
    /// Vehículos en ejecución sin órdenes.
    pub no_orders: VehicleIssueDetail,
    /// Vehículos en parada incompatible con su tipo.
    pub incompatible_stop: VehicleIssueDetail,
    /// Vehículos esperando carga que no está disponible.
    pub waiting_cargo: VehicleIssueDetail,
    /// Trenes esperando señal PBS.
    pub pbs_stuck: VehicleIssueDetail,
}

/// Detalle de un tipo de problema operacional.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VehicleIssueDetail {
    /// Número total de vehículos afectados.
    pub count: usize,
    /// IDs de vehículos afectados (limitado a ejemplos para evitar overhead).
    pub example_vehicle_ids: Vec<u32>,
}

impl VehicleIssueDetail {
    /// Crea un nuevo detalle vacío.
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: 0,
            example_vehicle_ids: Vec::new(),
        }
    }

    /// Registra un vehículo con este problema.
    pub fn add_vehicle(&mut self, vehicle_id: u32) {
        self.count += 1;
        // Solo guardamos el primer ejemplo para mensajes simples
        if self.example_vehicle_ids.is_empty() {
            self.example_vehicle_ids.push(vehicle_id);
        }
    }
}

impl VehicleOperationalSummary {
    /// Analiza el estado de todos los vehículos en el juego.
    #[must_use]
    pub fn analyze(state: &GameState) -> Self {
        let mut summary = Self::default();

        for vehicle in &state.vehicles {
            if vehicle.running && vehicle.no_network_route_to_order {
                summary.no_network_route.add_vehicle(vehicle.id);
            }

            if vehicle.running && vehicle.orders.is_empty() {
                summary.no_orders.add_vehicle(vehicle.id);
            }

            if vehicle_has_incompatible_stop(state, vehicle) {
                summary.incompatible_stop.add_vehicle(vehicle.id);
            }

            if vehicle_waiting_for_cargo(state, vehicle) {
                summary.waiting_cargo.add_vehicle(vehicle.id);
            }

            if vehicle.running && vehicle.pbs_stuck {
                summary.pbs_stuck.add_vehicle(vehicle.id);
            }
        }

        summary
    }

    /// Verifica si hay algún problema operacional.
    #[must_use]
    pub fn has_any_issues(&self) -> bool {
        self.no_network_route.count > 0
            || self.no_orders.count > 0
            || self.incompatible_stop.count > 0
            || self.waiting_cargo.count > 0
            || self.pbs_stuck.count > 0
    }
}

/// Verifica si un vehículo tiene una parada incompatible con su tipo.
fn vehicle_has_incompatible_stop(state: &GameState, v: &Vehicle) -> bool {
    use crate::vehicle::VehicleOrder;

    if !v.running || v.orders.is_empty() {
        return false;
    }
    let Some(order) = v.orders.get(v.current_order) else {
        return false;
    };
    match order {
        VehicleOrder::Station { station, .. } => state
            .stations
            .iter()
            .find(|s| s.pos == *station)
            .is_some_and(|st| !st.can_service_vehicle(v.kind) || st.is_waypoint()),
        VehicleOrder::Waypoint { waypoint, .. } => state
            .stations
            .iter()
            .find(|s| s.pos == *waypoint)
            .is_none_or(|st| !st.can_service_vehicle(v.kind)),
        VehicleOrder::Depot { .. } | VehicleOrder::Tile(_) | VehicleOrder::Conditional { .. } => {
            false
        }
    }
}

/// Verifica si un vehículo está esperando carga que no está disponible.
fn vehicle_waiting_for_cargo(state: &GameState, v: &Vehicle) -> bool {
    use crate::station::station_covers_tile;
    use crate::vehicle::VehicleOrder;
    use crate::STATION_COVERAGE_RADIUS;

    if !v.running || v.cargo > 0 || v.no_network_route_to_order || v.orders.is_empty() {
        return false;
    }
    let Some(VehicleOrder::Station { station, .. }) = v.orders.get(v.current_order).copied() else {
        return false;
    };
    if !station_covers_tile(station, v.pos, 1) && v.pos != station {
        return false;
    }
    let Some(st) = state.stations.iter().find(|s| s.pos == station) else {
        return false;
    };
    if !st.can_service_vehicle(v.kind) {
        return false;
    }
    let industry_has = state.industries.iter().any(|ind| {
        ind.stock > 0
            && crate::station::industry_in_station_coverage(ind, station, STATION_COVERAGE_RADIUS)
            && st.accepts_cargo(ind.output_cargo())
    });
    let station_has = match v.kind {
        crate::vehicle::VehicleKind::Bus | crate::vehicle::VehicleKind::Tram => {
            st.cargo_stock.passengers > 0 || st.cargo_stock.mail > 0
        }
        crate::vehicle::VehicleKind::Truck
        | crate::vehicle::VehicleKind::Train
        | crate::vehicle::VehicleKind::Ship => {
            st.stock > 0 || st.cargo_stock.pick_freight_to_load(v.cargo_type).is_some()
        }
        crate::vehicle::VehicleKind::Aircraft => false,
    };
    !industry_has && !station_has
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::{Vehicle, VehicleKind};
    use crate::{GameState, Industry, IndustryKind, Station, StopKind};

    #[test]
    fn summary_detects_no_network_route() {
        let origin = TileCoord::new(0, 0);
        let mut v1 = Vehicle::new(1, VehicleKind::Bus, origin, origin);
        v1.running = true;
        v1.set_orders(vec![TileCoord::new(1, 0)]);
        v1.no_network_route_to_order = true;

        let mut state = GameState::new(4, 4);
        state.vehicles = vec![v1];

        let summary = VehicleOperationalSummary::analyze(&state);
        assert_eq!(summary.no_network_route.count, 1);
        assert_eq!(summary.no_network_route.example_vehicle_ids, vec![1]);
        assert_eq!(summary.no_orders.count, 0);
    }

    #[test]
    fn summary_detects_no_orders() {
        let origin = TileCoord::new(0, 0);
        let mut v2 = Vehicle::new(2, VehicleKind::Truck, origin, origin);
        v2.running = true;

        let mut state = GameState::new(4, 4);
        state.vehicles = vec![v2];

        let summary = VehicleOperationalSummary::analyze(&state);
        assert_eq!(summary.no_orders.count, 1);
        assert_eq!(summary.no_orders.example_vehicle_ids, vec![2]);
        assert_eq!(summary.no_network_route.count, 0);
    }

    #[test]
    fn summary_detects_incompatible_stop() {
        let stop = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        state
            .stations
            .push(Station::new_with_kind(stop, StopKind::BusStop));

        let mut truck = Vehicle::new(3, VehicleKind::Truck, stop, stop);
        truck.running = true;
        truck.set_station_orders(vec![stop]);
        state.vehicles.push(truck);

        let summary = VehicleOperationalSummary::analyze(&state);
        assert_eq!(summary.incompatible_stop.count, 1);
        assert_eq!(summary.incompatible_stop.example_vehicle_ids, vec![3]);
    }

    #[test]
    fn summary_detects_waiting_cargo() {
        let stop = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        state
            .stations
            .push(Station::new_with_kind(stop, StopKind::TruckStop));
        state
            .industries
            .push(Industry::new(TileCoord::new(2, 4), IndustryKind::CoalMine));

        let mut truck = Vehicle::new(4, VehicleKind::Truck, stop, stop);
        truck.running = true;
        truck.set_station_orders(vec![stop]);
        state.vehicles.push(truck);

        let summary = VehicleOperationalSummary::analyze(&state);
        assert_eq!(summary.waiting_cargo.count, 1);
        assert_eq!(summary.waiting_cargo.example_vehicle_ids, vec![4]);
    }

    #[test]
    fn summary_detects_pbs_stuck() {
        let origin = TileCoord::new(0, 0);
        let mut train = Vehicle::new(5, VehicleKind::Train, origin, origin);
        train.running = true;
        train.pbs_stuck = true;
        train.set_orders(vec![TileCoord::new(3, 0)]);

        let mut state = GameState::new(4, 4);
        state.vehicles = vec![train];

        let summary = VehicleOperationalSummary::analyze(&state);
        assert_eq!(summary.pbs_stuck.count, 1);
        assert_eq!(summary.pbs_stuck.example_vehicle_ids, vec![5]);
    }

    #[test]
    fn summary_detects_multiple_issues() {
        let origin = TileCoord::new(0, 0);
        let mut v1 = Vehicle::new(1, VehicleKind::Bus, origin, origin);
        v1.running = true;
        v1.set_orders(vec![TileCoord::new(1, 0)]);
        v1.no_network_route_to_order = true;

        let mut v2 = Vehicle::new(2, VehicleKind::Truck, origin, origin);
        v2.running = true;

        let mut state = GameState::new(4, 4);
        state.vehicles = vec![v1, v2];

        let summary = VehicleOperationalSummary::analyze(&state);
        assert_eq!(summary.no_network_route.count, 1);
        assert_eq!(summary.no_orders.count, 1);
        assert!(summary.has_any_issues());
    }

    #[test]
    fn summary_empty_when_no_issues() {
        let origin = TileCoord::new(0, 0);
        let mut v = Vehicle::new(1, VehicleKind::Bus, origin, origin);
        v.running = true;
        v.set_orders(vec![TileCoord::new(1, 0)]);

        let mut state = GameState::new(4, 4);
        state.vehicles = vec![v];

        let summary = VehicleOperationalSummary::analyze(&state);
        assert!(!summary.has_any_issues());
    }
}
