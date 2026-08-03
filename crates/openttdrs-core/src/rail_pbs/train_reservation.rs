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

/// `true` si delante del tren hay (o habrá) un segmento PBS que exige reserva.
///
/// Aproxima `UpdateSignalsOnSegment == SigSegState::Path`: cualquier path signal
/// en la tesela actual, en el destino inmediato o en las primeras teselas del
/// path de órdenes activa la reserva aunque `reserve_paths` sea `false`.
#[must_use]
pub fn vehicle_segment_requires_path_reserve(map: &Map, vehicle: &Vehicle) -> bool {
    if tile_has_any_pbs_signal(map, vehicle.pos) {
        return true;
    }
    if let Some(next) = vehicle.movement_target()
        && tile_has_any_pbs_signal(map, next)
    {
        return true;
    }
    vehicle
        .path
        .iter()
        .take(16)
        .any(|tile| tile_has_any_pbs_signal(map, *tile))
}

/// Calcula la reserva de un tren sin mutar el mapa global de reservas.
#[must_use]
pub fn compute_train_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    // Helper de tests / callers legacy: fuerza reserva aunque el default vanilla
    // de `pf.reserve_paths` sea `false`.
    let settings = crate::pathfinding_settings::PathfindingSettings {
        reserve_paths: true,
        ..crate::pathfinding_settings::PathfindingSettings::default()
    };
    compute_train_reservation_with_settings(map, vehicles, vehicle_idx, already_reserved, settings)
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
    // OpenTTD: `SIGSEG_PBS || pf.reserve_paths`. Sin path signal delante y con
    // `reserve_paths=false` (default vanilla) las redes de block/entry/exit no
    // crean reservas PBS; gobiernan las señales clásicas.
    if !settings.reserve_paths && !vehicle_segment_requires_path_reserve(map, vehicle) {
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
        || (alt_res.len() >= along_path.len()
            && reservation_ends_at_safe_wait_steps(map, vehicle.pos, &alt, &alt_res))
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
    // Desde depósito el follower PBS empieza en la boca (`path[0]`).
    if map.get_kind(vehicle.pos) == Some(TileKind::RailDepot) {
        return reserve_along_path_from_depot(map, vehicles, vehicle, path, already_reserved);
    }

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
    let start_step = ReservedRailStep::new(cur, pos_track);
    if already_reserved.contains(&start_step)
        || tile_occupied_by_other_train(map, vehicles, vehicle.id, cur, pos_track)
    {
        return out;
    }
    out.push(start_step);
    extend_reservation_along_path(
        map,
        vehicles,
        vehicle.id,
        path,
        already_reserved,
        &mut out,
        &mut cur,
        0,
    );
    out
}

fn reserve_along_path_from_depot(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    path: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    let mut out = Vec::new();
    let Some(&entrance) = path.first() else {
        return out;
    };
    let beyond = path.get(1).copied();
    let Some(track) = beyond
        .and_then(|next| track_on_departure_tile(map, entrance, next))
        .or_else(|| {
            let tb = rail_traversal_bits(map, entrance);
            (0..6u8).find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
        })
    else {
        return out;
    };
    let step = ReservedRailStep::new(entrance, track);
    if already_reserved.contains(&step)
        || tile_occupied_by_other_train(map, vehicles, vehicle.id, entrance, track)
    {
        return out;
    }
    out.push(step);
    let mut cur = entrance;
    if is_safe_waiting_position(map, cur, beyond, tile_has_any_pbs_signal(map, cur)) {
        return out;
    }
    extend_reservation_along_path(
        map,
        vehicles,
        vehicle.id,
        path,
        already_reserved,
        &mut out,
        &mut cur,
        1,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn extend_reservation_along_path(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_id: u32,
    path: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
    out: &mut Vec<ReservedRailStep>,
    cur: &mut TileCoord,
    path_skip: usize,
) {
    let mut passed_path = tile_has_any_pbs_signal(map, *cur);
    for (i, &next) in path.iter().enumerate().skip(path_skip) {
        if out.len() >= MAX_TRAIN_RESERVATION_LEN {
            break;
        }
        let beyond = path.get(i + 1).copied();
        if !crate::rail_signals::rail_step_signal_allows(map, vehicles, *cur, next, beyond) {
            break;
        }
        let Some(track) = track_on_departure_tile(map, *cur, next)
            .or_else(|| track_for_rail_step(map, *cur, next))
        else {
            break;
        };
        let step = ReservedRailStep::new(next, track);
        if already_reserved.contains(&step) {
            break;
        }
        if tile_occupied_by_other_train(map, vehicles, vehicle_id, next, track) {
            break;
        }
        out.push(step);
        *cur = next;
        if tile_has_any_pbs_signal(map, *cur) {
            passed_path = true;
        }
        if is_safe_waiting_position(map, *cur, beyond, passed_path) {
            break;
        }
    }
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

/// Añade a la reserva las teselas aún ocupadas por el consist (vagones detrás
/// de la cabeza). `OpenTTD` mantiene esas pistas reservadas hasta que la cola las
/// abandona.
fn merge_consist_footprint(
    map: &Map,
    vehicles: &[Vehicle],
    head_id: u32,
    mut reserved: Vec<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    let occupied = crate::train_consist::consist_occupied_tiles(vehicles, head_id);
    let existing: HashSet<TileCoord> = reserved.iter().map(|s| s.tile).collect();
    for tile in occupied {
        if existing.contains(&tile) {
            continue;
        }
        // El depósito puede albergar varios consists; no es pista PBS reservable.
        if map.get_kind(tile) == Some(TileKind::RailDepot) {
            continue;
        }
        let tb = rail_traversal_bits(map, tile);
        let track = (0..6u8)
            .find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
            .unwrap_or(tb & 0x3F);
        if track != 0 {
            reserved.push(ReservedRailStep::new(tile, track));
        }
    }
    reserved
}

/// Recalcula `reserved_steps` de todos los trenes (orden por índice = prioridad).
///
/// Helper legacy/tests: `reserve_paths=true`. La simulación real usa
/// [`update_train_reservations_with_settings`] con `GameState.pathfinding`.
pub fn update_train_reservations(map: &Map, vehicles: &mut [Vehicle]) {
    let settings = crate::pathfinding_settings::PathfindingSettings {
        reserve_paths: true,
        ..crate::pathfinding_settings::PathfindingSettings::default()
    };
    update_train_reservations_with_settings(map, vehicles, settings);
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
        // Un tren todavía cerrado dentro del depósito no participa del PBS
        // global. `tick_train_stay_in_depot` reserva atómicamente al autorizar
        // su salida; reservar antes permitía que varios consists apilados se
        // bloquearan entre sí y ocuparan el bloque de la vía principal.
        if map.get_kind(vehicles[i].pos) == Some(TileKind::RailDepot)
            && !vehicles[i].depot_leave_cleared
        {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        let head_id = vehicles[i].id;
        let previous = vehicles[i].reserved_steps.clone();
        let reserved = compute_train_reservation_with_wormholes(
            map, vehicles, i, &global, settings, wormholes,
        );
        let reserved = follow_train_reservation(&previous, reserved, &vehicles[i]);
        let reserved = merge_consist_footprint(map, vehicles, head_id, reserved);
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
