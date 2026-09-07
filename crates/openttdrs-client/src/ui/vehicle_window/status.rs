//! Status corto estilo OpenTTD `GetVehicleStatusString` (#174).

use bevy::prelude::Color;
use openttdrs_core::prelude::*;
use openttdrs_core::station::resolve_order_destination;

use crate::i18n::{Locale, localized_text};
use crate::state::SimWorld;
use crate::ui::vehicle_details_window::speed_to_kmh;

use super::{STATUS_NO_ROUTE, STATUS_RUNNING, STATUS_STOPPED};

/// Color de avisos (avería / PBS).
const STATUS_WARN: Color = Color::srgb(0.95, 0.55, 0.25);

/// Texto + color de la barra de estado bajo el viewport.
#[must_use]
pub(crate) fn format_vehicle_status(
    locale: Locale,
    vehicle: &Vehicle,
    sim: &SimWorld,
) -> (String, Color) {
    if vehicle.is_broken_down() {
        return (localized_text(locale, "Averiado"), STATUS_WARN);
    }
    if !vehicle.running {
        return (localized_text(locale, "Detenido"), STATUS_STOPPED);
    }
    if vehicle.no_network_route_to_order {
        return (localized_text(locale, "Sin ruta"), STATUS_NO_ROUTE);
    }
    if vehicle.pbs_stuck {
        return (localized_text(locale, "Esperando señal"), STATUS_WARN);
    }
    if vehicle.cargo_loading {
        return (localized_text(locale, "Cargando"), STATUS_RUNNING);
    }
    if vehicle.cargo_unloading {
        return (localized_text(locale, "Descargando"), STATUS_RUNNING);
    }

    let kmh = speed_to_kmh(vehicle.kind, vehicle.cur_speed);
    let dest = active_destination_label(locale, vehicle, sim);
    let running = localized_text(locale, "En marcha");
    let at = match locale {
        Locale::Es => "a",
        Locale::En => "at",
    };
    if let Some(dest) = dest {
        (
            format!("{running} {at} {kmh} km/h → {dest}"),
            STATUS_RUNNING,
        )
    } else if vehicle.orders.is_empty() {
        (
            format!(
                "{running} {at} {kmh} km/h ({})",
                localized_text(locale, "sin órdenes")
            ),
            STATUS_RUNNING,
        )
    } else {
        (format!("{running} {at} {kmh} km/h"), STATUS_RUNNING)
    }
}

fn active_destination_label(locale: Locale, vehicle: &Vehicle, sim: &SimWorld) -> Option<String> {
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
        VehicleOrder::Depot { .. } => {
            format!(
                "{} ({}, {})",
                localized_text(locale, "Depósito"),
                pos.x,
                pos.y
            )
        }
        VehicleOrder::Waypoint { .. } => format!("Waypoint ({}, {})", pos.x, pos.y),
        VehicleOrder::Conditional { .. } => {
            format!("{} {}", localized_text(locale, "Cond. → ord."), idx + 1)
        }
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
        let (text, _) = format_vehicle_status(Locale::Es, &vehicle, &sim);
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
        let (text, _) = format_vehicle_status(Locale::Es, &vehicle, &sim);
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
        let (text, _) = format_vehicle_status(Locale::Es, &vehicle, &sim);
        assert_eq!(text, "Sin ruta");
    }

    #[test]
    fn status_chrome_follows_english_locale_without_translating_destination_names() {
        let mut stopped = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        stopped.running = false;
        let (text, _) =
            format_vehicle_status(Locale::En, &stopped, &sim_with_vehicle(stopped.clone()).0);
        assert_eq!(text, "Stopped");

        let mut running = Vehicle::new(
            2,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        running.running = true;
        running.cur_speed = 80;
        let sim = sim_with_vehicle(running.clone()).0;
        let (text, _) = format_vehicle_status(Locale::En, &running, &sim);
        assert!(text.contains("Running at 40 km/h (no orders)"), "{text}");

        assert_eq!(localized_text(Locale::En, "Depósito"), "Depot");
        assert_eq!(localized_text(Locale::En, "Cond. → ord."), "Cond. → order");
        assert_eq!(
            localized_text(Locale::En, "Estación Ñandú"),
            "Estación Ñandú"
        );
    }
}
