//! Política de espera PBS y reversión automática.

use crate::map::Map;
use crate::vehicle::{Vehicle, VehicleKind};

/// `true` si el tren está detenido ante una path signal sin reserva completa.
#[must_use]
pub fn train_waiting_for_pbs_path(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running || vehicle.force_proceed {
        return false;
    }
    crate::rail_signals::train_blocked_by_pbs_path(map, vehicle)
}

/// Actualiza `wait_counter` / `pbs_stuck` y, si toca, gira el tren.
///
/// Paridad simplificada de stuck + `wait_for_pbs_path` en `train_cmd.cpp`.
/// El look-ahead (`TryReserve`) se reintenta según `path_backoff_interval`
/// en [`crate::rail_pbs::train_reservation::compute_train_reservation_with_settings`]; `255` desactiva look-ahead y giro.
pub fn tick_pbs_wait_and_maybe_reverse(
    map: &Map,
    vehicle: &mut Vehicle,
    settings: crate::pathfinding_settings::PathfindingSettings,
) -> bool {
    if !train_waiting_for_pbs_path(map, vehicle) {
        if vehicle.pbs_stuck || vehicle.wait_counter > 0 {
            vehicle.pbs_stuck = false;
            vehicle.wait_counter = 0;
        }
        return false;
    }

    if !vehicle.pbs_stuck {
        vehicle.pbs_stuck = true;
        vehicle.wait_counter = 0;
    }
    vehicle.wait_counter = vehicle.wait_counter.saturating_add(1);

    // `path_backoff_interval == 255`: no look-ahead / no giro automático.
    if settings.path_backoff_interval == crate::pathfinding_settings::PBS_WAIT_FOREVER {
        return false;
    }

    let Some(timeout) = settings.pbs_reverse_timeout_ticks() else {
        return false;
    };
    if vehicle.wait_counter < timeout {
        return false;
    }
    // Timeout: girar y limpiar reserva/path (como ReverseTrainDirection stuck).
    vehicle.cur_speed = 0;
    vehicle.reverse_heading();
    vehicle.path.clear();
    vehicle.reserved_steps.clear();
    vehicle.wait_counter = 0;
    vehicle.pbs_stuck = false;
    vehicle.no_network_route_to_order = false;
    true
}
