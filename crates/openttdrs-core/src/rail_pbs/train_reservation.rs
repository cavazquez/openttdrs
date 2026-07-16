//! Cálculo y actualización de reservas de trenes.

use std::collections::HashSet;

use crate::map::{Map, TileCoord, TileKind, rail_traversal_bits};
use crate::vehicle::{Vehicle, VehicleKind};

use super::conflicts::{append_platform_reservation, tile_occupied_by_other_train};
use super::model::{
    MAX_TRAIN_RESERVATION_LEN, ReservedRailStep, track_for_rail_step, track_on_departure_tile,
};
use super::search::{
    find_path_to_safe_wait_with_wormholes, is_safe_waiting_position, tile_has_any_pbs_signal,
};

/// Calcula la reserva de un tren sin mutar el mapa global de reservas.
#[must_use]
pub fn compute_train_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    compute_train_reservation_with_settings(
        map,
        vehicles,
        vehicle_idx,
        already_reserved,
        crate::pathfinding_settings::PathfindingSettings::default(),
    )
}

/// Como [`compute_train_reservation`], con settings PBS (look-ahead / `TryReserve`).
#[must_use]
pub fn compute_train_reservation_with_settings(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
    settings: crate::pathfinding_settings::PathfindingSettings,
) -> Vec<ReservedRailStep> {
    compute_train_reservation_with_wormholes(
        map,
        vehicles,
        vehicle_idx,
        already_reserved,
        settings,
        None,
    )
}

/// Como [`compute_train_reservation_with_settings`], con wormholes de túnel en `TryReserve`.
#[must_use]
pub fn compute_train_reservation_with_wormholes(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> Vec<ReservedRailStep> {
    let vehicle = &vehicles[vehicle_idx];
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return Vec::new();
    }

    let path: Vec<TileCoord> = vehicle.path.iter().copied().collect();
    let mut along_path = reserve_along_path(map, vehicles, vehicle, &path, already_reserved);
    append_platform_reservation(map, vehicles, vehicle, already_reserved, &mut along_path);
    if reservation_ends_at_safe_wait_steps(map, vehicle.pos, &path, &along_path) {
        return along_path;
    }
    // TryReservePath: si el path de órdenes no llega a safe wait, buscar alternativa.
    // `path_backoff_interval == 255` desactiva look-ahead; si no, solo reintenta cuando
    // `should_retry_reservation(wait_counter)` (trenes no stuck tienen wait_counter=0 → siempre).
    if !settings.should_retry_reservation(vehicle.wait_counter) {
        return along_path;
    }
    let Some(alt) = find_path_to_safe_wait_with_wormholes(
        map,
        vehicles,
        vehicle.id,
        vehicle.pos,
        &path,
        already_reserved,
        wormholes,
    ) else {
        return along_path;
    };
    let mut alt_res = reserve_along_path(map, vehicles, vehicle, &alt, already_reserved);
    append_platform_reservation(map, vehicles, vehicle, already_reserved, &mut alt_res);
    if alt_res.len() > along_path.len()
        || reservation_ends_at_safe_wait_steps(map, vehicle.pos, &alt, &alt_res)
    {
        alt_res
    } else {
        along_path
    }
}

pub(super) fn reservation_ends_at_safe_wait_steps(
    map: &Map,
    pos: TileCoord,
    path: &[TileCoord],
    reserved: &[ReservedRailStep],
) -> bool {
    let Some(last) = reserved.last() else {
        return false;
    };
    let full: Vec<TileCoord> = std::iter::once(pos).chain(path.iter().copied()).collect();
    let next = full
        .iter()
        .position(|&c| c == last.tile)
        .and_then(|i| full.get(i + 1).copied());
    let passed_path = reserved
        .iter()
        .any(|s| tile_has_any_pbs_signal(map, s.tile));
    is_safe_waiting_position(map, last.tile, next, passed_path)
}

fn reserve_along_path(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    path: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    let mut out = Vec::new();
    let mut cur = vehicle.pos;

    let Some(pos_track) = path
        .first()
        .and_then(|&next| track_on_departure_tile(map, cur, next))
    else {
        let tb = rail_traversal_bits(map, cur);
        let track = (0..6u8)
            .find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
            .unwrap_or(tb & 0x3F);
        if track != 0 {
            out.push(ReservedRailStep::new(cur, track));
        }
        return out;
    };
    out.push(ReservedRailStep::new(cur, pos_track));
    let mut passed_path = tile_has_any_pbs_signal(map, cur);

    for (i, &next) in path.iter().enumerate() {
        if out.len() >= MAX_TRAIN_RESERVATION_LEN {
            break;
        }
        let beyond = path.get(i + 1).copied();
        if !crate::rail_signals::rail_step_signal_allows(map, vehicles, cur, next, beyond) {
            break;
        }
        let Some(track) =
            track_on_departure_tile(map, cur, next).or_else(|| track_for_rail_step(map, cur, next))
        else {
            break;
        };
        let step = ReservedRailStep::new(next, track);
        if already_reserved.contains(&step) {
            break;
        }
        if tile_occupied_by_other_train(map, vehicles, vehicle.id, next, track) {
            break;
        }
        out.push(step);
        cur = next;
        if tile_has_any_pbs_signal(map, cur) {
            passed_path = true;
        }
        if is_safe_waiting_position(map, cur, beyond, passed_path) {
            break;
        }
    }

    out
}

/// Conserva pasos de una reserva previa aún válidos si `TryReserve` falla
/// (`FollowTrainReservation` simplificado de `pbs.cpp`).
///
/// Si hay reserva nueva, se usa. Si queda vacía pero el tren sigue sobre la
/// reserva anterior, se mantienen los pasos bajo el tren y por delante en `path`.
#[must_use]
pub fn follow_train_reservation(
    previous: &[ReservedRailStep],
    newly_reserved: Vec<ReservedRailStep>,
    vehicle: &Vehicle,
) -> Vec<ReservedRailStep> {
    if !newly_reserved.is_empty() {
        return newly_reserved;
    }
    if previous.is_empty() {
        return newly_reserved;
    }
    if !previous.iter().any(|s| s.tile == vehicle.pos) {
        return newly_reserved;
    }
    let path_tiles: HashSet<TileCoord> = vehicle.path.iter().copied().collect();
    previous
        .iter()
        .copied()
        .filter(|s| s.tile == vehicle.pos || path_tiles.contains(&s.tile))
        .collect()
}

/// Recalcula `reserved_steps` de todos los trenes (orden por índice = prioridad).
pub fn update_train_reservations(map: &Map, vehicles: &mut [Vehicle]) {
    update_train_reservations_with_settings(
        map,
        vehicles,
        crate::pathfinding_settings::PathfindingSettings::default(),
    );
}

/// Como [`update_train_reservations`], con settings PBS.
pub fn update_train_reservations_with_settings(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
) {
    update_train_reservations_with_wormholes(map, vehicles, settings, None);
}

/// Como [`update_train_reservations_with_settings`], con wormholes en `TryReserve`.
pub fn update_train_reservations_with_wormholes(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) {
    let mut global = HashSet::new();
    for i in 0..vehicles.len() {
        // Solo cabezas de consist reservan; vagones siguen la huella de la cabeza.
        if vehicles[i].kind != VehicleKind::Train || !vehicles[i].is_consist_head() {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        let previous = vehicles[i].reserved_steps.clone();
        let reserved = compute_train_reservation_with_wormholes(
            map, vehicles, i, &global, settings, wormholes,
        );
        let reserved = follow_train_reservation(&previous, reserved, &vehicles[i]);
        for step in &reserved {
            global.insert(*step);
        }
        vehicles[i].reserved_steps = reserved;
    }
}

/// `true` si el tren no puede avanzar al `movement_target` (sin reserva en esa pista).
///
/// Si la reserva incluye la tesela actual pero aún no el siguiente paso (p. ej. señal
/// block roja cortó la extensión), no bloquea aquí: `train_blocked_by_signal` gobierna.
/// Path exige reserva más allá vía `path_exit_lacks_reservation`.
#[must_use]
pub fn train_blocked_by_reservation(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    if map.get_kind(vehicle.pos) == Some(TileKind::RailDepot) {
        return false;
    }
    if vehicle.reserved_steps.is_empty() {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    if vehicle.path.front() != Some(&next) {
        return false;
    }
    let Some(track) = track_on_departure_tile(map, vehicle.pos, next)
        .or_else(|| track_for_rail_step(map, vehicle.pos, next))
    else {
        return false;
    };
    if vehicle.reserved_steps.iter().any(|s| {
        s.tile == next && (s.track == track || super::conflicts::tracks_overlap(s.track, track))
    }) {
        return false;
    }
    if vehicle.reserved_steps.iter().any(|s| s.tile == vehicle.pos) {
        return false;
    }
    true
}
