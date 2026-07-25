//! Política de espera PBS / señales y reversión automática.

use crate::map::{Map, TileKind};
use crate::rail_signals::{
    rail_signal_present_mask, rail_tile_is_signals, signal_bits_for_exit, signal_exit_dir,
};
use crate::vehicle::{Vehicle, VehicleKind};

/// Espera corta ante deadlock frente-a-frente (sin señal PBS de por medio).
const HEAD_ON_REVERSE_TIMEOUT_TICKS: u32 = 120;

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
/// También cubre deadlock de tráfico frente-a-frente (`also_head_on`), que antes
/// no acumulaba espera porque solo se miraba el bloqueo PBS.
///
/// El look-ahead (`TryReserve`) se reintenta según `path_backoff_interval`
/// en [`crate::rail_pbs::train_reservation::compute_train_reservation_with_settings`]; `255` desactiva look-ahead y giro.
pub fn tick_pbs_wait_and_maybe_reverse(
    map: &Map,
    vehicle: &mut Vehicle,
    settings: crate::pathfinding_settings::PathfindingSettings,
    also_head_on: bool,
) -> bool {
    let waiting_pbs = train_waiting_for_pbs_path(map, vehicle);
    if !waiting_pbs && !also_head_on {
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

    let Some(pbs_timeout) = settings.pbs_reverse_timeout_ticks() else {
        return false;
    };
    // Head-on: acortar el timeout PBS (p. ej. 30 días) y desfasar por id.
    let timeout = if also_head_on {
        let short =
            HEAD_ON_REVERSE_TIMEOUT_TICKS.saturating_add((vehicle.id % 8).saturating_mul(15));
        pbs_timeout.min(short)
    } else {
        pbs_timeout
    };
    if vehicle.wait_counter < timeout {
        return false;
    }
    reverse_train_after_wait(vehicle);
    true
}

/// Espera ante señal de bloque roja (`wait_oneway_signal` / `wait_twoway_signal`).
///
/// Paridad de `TrainController` ante `red_signals` (`train_cmd.cpp:3445-3492`):
/// acumula `wait_counter` y gira si `reverse_at_signals` y se supera el timeout.
/// El llamador debe haber comprobado ya `train_blocked_by_signal`.
pub fn tick_signal_wait_and_maybe_reverse(
    map: &Map,
    vehicle: &mut Vehicle,
    settings: crate::pathfinding_settings::PathfindingSettings,
) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running || vehicle.force_proceed {
        return false;
    }
    // PBS path usa su propio timeout.
    if train_waiting_for_pbs_path(map, vehicle) {
        return false;
    }

    let twoway = facing_twoway_signal(map, vehicle);
    vehicle.wait_counter = vehicle.wait_counter.saturating_add(1);

    let timeout = if twoway {
        settings.twoway_signal_timeout_ticks()
    } else {
        settings.oneway_signal_timeout_ticks()
    };
    let Some(timeout) = timeout else {
        // `reverse_at_signals == false` o 255: quedarse parado.
        return false;
    };
    if vehicle.wait_counter < timeout {
        return false;
    }
    reverse_train_after_wait(vehicle);
    true
}

fn reverse_train_after_wait(vehicle: &mut Vehicle) {
    vehicle.cur_speed = 0;
    vehicle.reverse_heading();
    vehicle.path.clear();
    vehicle.reserved_steps.clear();
    vehicle.wait_counter = 0;
    vehicle.pbs_stuck = false;
    vehicle.no_network_route_to_order = false;
}

/// `true` si la señal que bloquea tiene cara en ambos sentidos (two-way).
fn facing_twoway_signal(map: &Map, vehicle: &Vehicle) -> bool {
    let from = vehicle.pos;
    let Some(to) = vehicle.movement_target() else {
        return false;
    };
    for signal_tile in [to, from] {
        let Some(tile) = map.get(signal_tile) else {
            continue;
        };
        if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
            continue;
        }
        let present = rail_signal_present_mask(tile.m3);
        if present.count_ones() >= 2 {
            // Dos bits presentes en la misma tesela → two-way típico.
            let beyond = if signal_tile == to {
                vehicle.path.get(1).copied().unwrap_or(vehicle.dest)
            } else {
                to
            };
            let bits = signal_bits_for_exit(map, signal_tile, beyond);
            if bits.is_empty() {
                continue;
            }
            let rails = tile.m5 & 0x3F;
            // Hay señal en el sentido contrario del mismo carril.
            for &bit in &bits {
                let exit = signal_exit_dir(rails, bit);
                let rev = crate::map::opposite_diag_dir(exit);
                let has_rev =
                    (0..4u8).any(|b| present & (1 << b) != 0 && signal_exit_dir(rails, b) == rev);
                if has_rev {
                    return true;
                }
            }
            return false;
        }
        return false;
    }
    false
}
