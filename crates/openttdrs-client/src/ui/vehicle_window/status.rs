//! Status corto estilo OpenTTD `GetVehicleStatusString` (#174).

use bevy::prelude::Color;
use openttdrs_core::prelude::*;
use openttdrs_core::station::resolve_order_destination;

use crate::state::SimWorld;
use crate::ui::vehicle_details_window::speed_to_kmh;

use super::{STATUS_NO_ROUTE, STATUS_RUNNING, STATUS_STOPPED};

/// Color de avisos (avería / PBS).
const STATUS_WARN: Color = Color::srgb(0.95, 0.55, 0.25);

/// Texto + color de la barra de estado bajo el viewport.
#[must_use]
pub(crate) fn format_vehicle_status(vehicle: &Vehicle, sim: &SimWorld) -> (String, Color) {
    if vehicle.breakdown_ticks_remaining > 0 {
        return ("Averiado".into(), STATUS_WARN);
    }
    if !vehicle.running {
        return ("Detenido".into(), STATUS_STOPPED);
    }
    if vehicle.no_network_route_to_order {
        return ("Sin ruta".into(), STATUS_NO_ROUTE);
    }
    if vehicle.pbs_stuck {
        return ("Esperando señal".into(), STATUS_WARN);
    }
    if vehicle.cargo_loading {
        return ("Cargando".into(), STATUS_RUNNING);
    }
    if vehicle.cargo_unloading {
        return ("Descargando".into(), STATUS_RUNNING);
    }

    let kmh = speed_to_kmh(vehicle.kind, vehicle.cur_speed);
    let dest = active_destination_label(vehicle, sim);
    if let Some(dest) = dest {
        (format!("En marcha a {kmh} km/h → {dest}"), STATUS_RUNNING)
    } else if vehicle.orders.is_empty() {
        (
            format!("En marcha a {kmh} km/h (sin órdenes)"),
            STATUS_RUNNING,
        )
    } else {
        (format!("En marcha a {kmh} km/h"), STATUS_RUNNING)
    }
}

fn active_destination_label(vehicle: &Vehicle, sim: &SimWorld) -> Option<String> {
    if vehicle.orders.is_empty() {
        return None;
    }
    let idx = vehicle
        .current_order
        .min(vehicle.orders.len().saturating_sub(1));
    let order = vehicle.orders[idx];
    let pos = resolve_order_destination(&sim.state.map, vehicle.kind, order);
    if let Some(station) = sim.state.stations.iter().find(|s| s.pos == pos)
        && let Some(name) = station
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
    {
        return Some(name.to_string());
    }
    Some(match order {
        VehicleOrder::Depot { .. } => format!("Depósito ({}, {})", pos.x, pos.y),
        VehicleOrder::Waypoint { .. } => format!("Waypoint ({}, {})", pos.x, pos.y),
        VehicleOrder::Conditional { .. } => format!("Cond. → ord.{}", idx + 1),
        _ => format!("({}, {})", pos.x, pos.y),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::{GameState, TileCoord, Vehicle, VehicleKind, VehicleOrder};

    fn sim_with_vehicle(vehicle: Vehicle) -> (SimWorld, u32) {
        let mut state = GameState::new(16, 16);
        let id = vehicle.id;
        state.vehicles.push(vehicle);
        (
            SimWorld {
                state,
                ..SimWorld::default()
            },
            id,
        )
    }

    #[test]
    fn stopped_when_not_running() {
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.running = false;
        let (sim, _) = sim_with_vehicle(vehicle.clone());
        let (text, _) = format_vehicle_status(&vehicle, &sim);
        assert_eq!(text, "Detenido");
    }

    #[test]
    fn running_includes_speed_and_destination() {
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.running = true;
        vehicle.cur_speed = 88;
        vehicle.orders = vec![VehicleOrder::Tile(TileCoord::new(5, 7))];
        vehicle.current_order = 0;
        let (sim, _) = sim_with_vehicle(vehicle.clone());
        let (text, _) = format_vehicle_status(&vehicle, &sim);
        assert!(text.contains("88 km/h"), "{text}");
        assert!(text.contains("→"), "{text}");
        assert!(text.contains("(5, 7)"), "{text}");
    }

    #[test]
    fn no_route_overrides_running() {
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.running = true;
        vehicle.no_network_route_to_order = true;
        let (sim, _) = sim_with_vehicle(vehicle.clone());
        let (text, _) = format_vehicle_status(&vehicle, &sim);
        assert_eq!(text, "Sin ruta");
    }
}
