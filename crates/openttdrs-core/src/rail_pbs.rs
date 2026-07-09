//! Reserva de ruta ferroviaria (`PBS` fase 2).
//!
//! Cada tren reserva **pistas** (`TrackBits`) a lo largo de su `path` hasta la siguiente
//! **posición segura de espera** (delante de señal, depósito o fin de vía), o hasta el
//! primer conflicto (otra reserva, ocupación o señal block cerrada). Las vías paralelas
//! en la misma tesela (`Horz`/`Vert`) pueden reservarse de forma independiente.

#![allow(clippy::implicit_hasher)]

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::{
    SIGTYPE_BLOCK, YapfSignalRouting, dir_from_to, is_pbs_signal_type, rail_signal_present_mask,
    rail_tile_is_signals, rail_traversal_bits, signal_exit_dir, signal_track_for_bit,
    signal_type_for_track, yapf_routing_signal,
};
use crate::train_movement::track_bit_for_movement;
use crate::vehicle::{Vehicle, VehicleKind};

/// Máscara de reserva PBS en el byte alto de `m2` (`m2_hi`: bits 8–11 del `m2()` de 16 bits).
pub const RAIL_RESERVATION_M2_HI_MASK: u8 = 0x0F;

/// Vía doble horizontal / vertical.
const RAIL_TB_HORZ: u8 = 0x0C;
const RAIL_TB_VERT: u8 = 0x30;

/// Tope de pasos reservados por tren (paridad con límites PBS del original).
pub const MAX_TRAIN_RESERVATION_LEN: usize = 64;

/// Penalización YAPF por cruzar una pista ya reservada por otro tren.
pub const YAPF_RESERVATION_CROSS_PENALTY: u32 = 80;

/// Un paso de reserva: tesela + un único `TrackBit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ReservedRailStep {
    pub tile: TileCoord,
    pub track: u8,
}

impl ReservedRailStep {
    #[must_use]
    pub const fn new(tile: TileCoord, track: u8) -> Self {
        Self { tile, track }
    }
}

#[must_use]
const fn opposite_dir(d: u8) -> u8 {
    (d + 2) & 3
}

/// `true` si `tile` tiene una señal no-PBS que controla la salida `exit_dir`.
#[must_use]
fn has_block_signal_on_exit(map: &Map, tile: TileCoord, exit_dir: u8) -> bool {
    let Some(t) = map.get(tile) else {
        return false;
    };
    if t.kind != TileKind::Rail || !rail_tile_is_signals(t.m5) {
        return false;
    }
    let rails = t.m5 & 0x3F;
    let present = rail_signal_present_mask(t.m3);
    (0..4u8).any(|bit| {
        if present & (1 << bit) == 0 {
            return false;
        }
        if signal_exit_dir(rails, bit) != exit_dir {
            return false;
        }
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(t.m2, track));
        !is_pbs_signal_type(sig_type)
    })
}

/// `true` si `tile` tiene una path signal cuya salida es `exit_dir`.
#[must_use]
fn has_pbs_signal_on_exit(map: &Map, tile: TileCoord, exit_dir: u8) -> bool {
    let Some(t) = map.get(tile) else {
        return false;
    };
    if t.kind != TileKind::Rail || !rail_tile_is_signals(t.m5) {
        return false;
    }
    let rails = t.m5 & 0x3F;
    let present = rail_signal_present_mask(t.m3);
    (0..4u8).any(|bit| {
        if present & (1 << bit) == 0 {
            return false;
        }
        if signal_exit_dir(rails, bit) != exit_dir {
            return false;
        }
        signal_track_for_bit(rails, bit)
            .is_some_and(|track| is_pbs_signal_type(signal_type_for_track(t.m2, track)))
    })
}

/// Posición segura de espera (`IsSafeWaitingPosition` en `pbs.cpp`).
///
/// - Depósito
/// - Señal block/presignal en esta tesela (sentido `tile` → `next`)
/// - Delante de una path signal (la siguiente tesela tiene PBS a favor)
/// - Fin de vía / fin de path (`next == None`)
///
/// `after_path_signal`: si es `false`, no cortar “delante de path” (hay que reservar
/// *a través* de la primera path; el safe wait es la siguiente).
#[must_use]
pub fn is_safe_waiting_position(
    map: &Map,
    tile: TileCoord,
    next: Option<TileCoord>,
    after_path_signal: bool,
) -> bool {
    if map.get_kind(tile) == Some(TileKind::RailDepot) {
        return true;
    }
    let Some(next) = next else {
        return true;
    };
    let Some(exit_dir) = dir_from_to(tile, next) else {
        return true;
    };
    if has_block_signal_on_exit(map, tile, exit_dir) {
        return true;
    }
    if after_path_signal && has_pbs_signal_on_exit(map, next, exit_dir) {
        return true;
    }
    // PathOneWay en contra en la siguiente tesela ≈ fin de vía usable.
    matches!(
        yapf_routing_signal(map, next, exit_dir),
        YapfSignalRouting::DeadEnd
    )
}

/// `true` si `tile` es una tesela con path signal.
#[must_use]
pub(crate) fn tile_has_any_pbs_signal(map: &Map, tile: TileCoord) -> bool {
    let Some(t) = map.get(tile) else {
        return false;
    };
    if t.kind != TileKind::Rail || !rail_tile_is_signals(t.m5) {
        return false;
    }
    let rails = t.m5 & 0x3F;
    let present = rail_signal_present_mask(t.m3);
    (0..4u8).any(|bit| {
        present & (1 << bit) != 0
            && signal_track_for_bit(rails, bit)
                .is_some_and(|track| is_pbs_signal_type(signal_type_for_track(t.m2, track)))
    })
}

/// Coste base por tesela en `TryReserve` (alineado con YAPF `TILE_COST`).
const TRY_RESERVE_TILE_COST: u32 = 1;
/// Sesgo si la tesela no está en el path de órdenes (prioriza YAPF sobre BFS ciego).
const TRY_RESERVE_OFF_PATH_PENALTY: u32 = 50;

#[derive(Clone, Eq, PartialEq)]
struct TryReserveNode {
    cost: u32,
    cur: TileCoord,
    path: Vec<TileCoord>,
    passed_path: bool,
}

impl Ord for TryReserveNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| other.path.len().cmp(&self.path.len()))
    }
}

impl PartialOrd for TryReserveNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Dijkstra con costes YAPF hasta la siguiente posición segura (`TryPathReserve`).
///
/// Bloqueos duros PBS (reserva ajena, ocupación, señales `DeadEnd`/block rojo) +
/// costes nativos: tesela, cruce de reserva en mapa, sesgo hacia el path de órdenes.
/// Elige el safe wait de **menor coste** (no el primero en BFS).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn find_path_to_safe_wait(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    from: TileCoord,
    preferred: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
) -> Option<Vec<TileCoord>> {
    use crate::rail_signals::rail_neighbors;
    use std::collections::BinaryHeap;

    if is_safe_waiting_position(map, from, preferred.first().copied(), false) {
        return Some(vec![]);
    }

    let preferred_set: HashSet<TileCoord> = preferred.iter().copied().collect();
    let mut heap = BinaryHeap::from([TryReserveNode {
        cost: 0,
        cur: from,
        path: Vec::new(),
        passed_path: false,
    }]);
    let mut best_g: HashMap<TileCoord, u32> = HashMap::from([(from, 0)]);
    let mut best_goal: Option<(u32, Vec<TileCoord>)> = None;
    let mut end_of_line: Option<(u32, Vec<TileCoord>)> = None;

    while let Some(TryReserveNode {
        cost,
        cur,
        path: path_so_far,
        passed_path,
    }) = heap.pop()
    {
        if best_g.get(&cur).is_some_and(|&g| cost > g) {
            continue;
        }
        if path_so_far.len() >= MAX_TRAIN_RESERVATION_LEN {
            continue;
        }
        if let Some((best_c, _)) = best_goal
            && cost >= best_c
        {
            continue;
        }

        let prev = path_so_far.last().copied();
        for next in rail_neighbors(map, cur, prev) {
            if !crate::rail_signals::rail_step_signal_allows(map, vehicles, cur, next, None) {
                continue;
            }
            let Some(track) = track_on_departure_tile(map, cur, next)
                .or_else(|| track_for_rail_step(map, cur, next))
            else {
                continue;
            };
            let step = ReservedRailStep::new(next, track);
            if already_reserved.contains(&step) {
                continue;
            }
            if tile_occupied_by_other_train(map, vehicles, self_id, next, track) {
                continue;
            }
            let mut step_cost = TRY_RESERVE_TILE_COST;
            if tile_track_reserved_by_map(map, next, track) {
                step_cost += YAPF_RESERVATION_CROSS_PENALTY;
            }
            if !preferred_set.contains(&next) {
                step_cost += TRY_RESERVE_OFF_PATH_PENALTY;
            }
            let new_cost = cost.saturating_add(step_cost);
            if best_g.get(&next).is_some_and(|&g| new_cost >= g) {
                continue;
            }
            best_g.insert(next, new_cost);
            let mut new_path = path_so_far.clone();
            new_path.push(next);
            let new_passed = passed_path || tile_has_any_pbs_signal(map, next);
            let next_beyond = preferred
                .iter()
                .position(|&c| c == next)
                .and_then(|i| preferred.get(i + 1).copied())
                .or_else(|| {
                    rail_neighbors(map, next, Some(cur))
                        .into_iter()
                        .find(|&n| n != cur)
                });
            if is_safe_waiting_position(map, next, next_beyond, new_passed) {
                if next_beyond.is_none() && !new_passed {
                    if end_of_line.as_ref().is_none_or(|(c, _)| new_cost < *c) {
                        end_of_line = Some((new_cost, new_path));
                    }
                    continue;
                }
                if best_goal.as_ref().is_none_or(|(c, _)| new_cost < *c) {
                    best_goal = Some((new_cost, new_path));
                }
                continue;
            }
            heap.push(TryReserveNode {
                cost: new_cost,
                cur: next,
                path: new_path,
                passed_path: new_passed,
            });
        }
    }
    best_goal
        .map(|(_, p)| p)
        .or_else(|| end_of_line.map(|(_, p)| p))
}

/// `true` si el último paso de `reserved` es una posición segura de espera.
#[must_use]
pub fn reservation_ends_at_safe_wait(map: &Map, vehicle: &Vehicle) -> bool {
    let Some(last) = vehicle.reserved_steps.last() else {
        return false;
    };
    let path: Vec<TileCoord> = std::iter::once(vehicle.pos)
        .chain(vehicle.path.iter().copied())
        .collect();
    let next = path
        .iter()
        .position(|&c| c == last.tile)
        .and_then(|i| path.get(i + 1).copied());
    let passed_path = vehicle
        .reserved_steps
        .iter()
        .any(|s| tile_has_any_pbs_signal(map, s.tile));
    is_safe_waiting_position(map, last.tile, next, passed_path)
}

/// Decodifica `m2_hi` → `TrackBits` reservados (`GetRailReservationTrackBits`).
#[must_use]
pub fn decode_rail_reservation_m2_hi(m2_hi: u8) -> u8 {
    let encoded = m2_hi & RAIL_RESERVATION_M2_HI_MASK;
    let track_idx = (encoded & 0x07).wrapping_sub(1);
    if track_idx > 5 {
        return 0;
    }
    let primary = 1_u8 << track_idx;
    if encoded & (1 << 3) != 0 {
        return primary | opposite_parallel_track(primary);
    }
    primary
}

#[must_use]
const fn opposite_parallel_track(track: u8) -> u8 {
    match track {
        0x04 => 0x08,
        0x08 => 0x04,
        0x10 => 0x20,
        0x20 => 0x10,
        _ => 0,
    }
}

/// Codifica `TrackBits` reservados en `m2_hi` (sin tocar el byte bajo de `m2`).
#[must_use]
pub fn encode_rail_reservation_to_m2_hi(track_bits: u8) -> u8 {
    if track_bits == 0 {
        return 0;
    }
    let Some(first_track) = (0..6u8).find(|i| track_bits & (1 << i) != 0) else {
        return 0;
    };
    let mut out = first_track + 1;
    if track_bits == RAIL_TB_HORZ || track_bits == RAIL_TB_VERT {
        out |= 1 << 3;
    }
    out
}

/// `true` si la tesela tiene alguna pista reservada en `m2_hi`.
#[must_use]
pub fn rail_tile_has_pbs_reservation(m2_hi: u8) -> bool {
    decode_rail_reservation_m2_hi(m2_hi) != 0
}

/// `true` si `track` choca con la reserva ya escrita en el mapa.
#[must_use]
pub fn tile_track_reserved_by_map(map: &Map, tile: TileCoord, track: u8) -> bool {
    let Some(t) = map.get(tile) else {
        return false;
    };
    if t.kind != TileKind::Rail {
        return false;
    }
    let reserved = decode_rail_reservation_m2_hi(t.m2_hi);
    reserved != 0 && reserved & track != 0
}

/// Pista usada en `tile` al avanzar `from` → `to`.
#[must_use]
pub fn track_for_rail_step(map: &Map, from: TileCoord, to: TileCoord) -> Option<u8> {
    let exit_dir = dir_from_to(from, to)?;
    let entry = opposite_dir(exit_dir);
    let tb = rail_traversal_bits(map, to);
    track_bit_for_movement(entry, tb)
}

/// Pista usada en `from` al salir hacia `to`.
#[must_use]
pub fn track_on_departure_tile(map: &Map, from: TileCoord, to: TileCoord) -> Option<u8> {
    let exit_dir = dir_from_to(from, to)?;
    let entry = opposite_dir(exit_dir);
    let tb = rail_traversal_bits(map, from);
    track_bit_for_movement(entry, tb)
}

fn tracks_overlap(a: u8, b: u8) -> bool {
    a & b != 0
}

/// `true` si otro tren ocupa `tile` en una pista que solapa con `track`.
#[must_use]
fn tile_occupied_by_other_train(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    tile: TileCoord,
    track: u8,
) -> bool {
    if map.get_kind(tile) == Some(TileKind::RailDepot) {
        return false;
    }
    vehicles.iter().any(|v| {
        if v.id == self_id || v.kind != VehicleKind::Train {
            return false;
        }
        if v.pos != tile {
            return false;
        }
        // Sin dirección de salida conocida: ocupa toda la tesela (parado / sin path).
        let Some(other) = track_on_departure_tile(map, tile, v.movement_target().unwrap_or(tile))
            .or_else(|| {
                v.path
                    .front()
                    .and_then(|&next| track_on_departure_tile(map, tile, next))
            })
        else {
            return true;
        };
        tracks_overlap(other, track)
    })
}

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
/// en [`compute_train_reservation_with_settings`]; `255` desactiva look-ahead y giro.
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
    vehicle.reverse_heading();
    vehicle.path.clear();
    vehicle.reserved_steps.clear();
    vehicle.wait_counter = 0;
    vehicle.pbs_stuck = false;
    vehicle.no_network_route_to_order = false;
    true
}

/// `true` si algún tren tiene reserva que sale de `signal_tile` por `exit_dir` y
/// termina en posición segura (`TryReservePath` exitoso).
#[must_use]
pub fn pbs_exit_has_complete_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    signal_tile: TileCoord,
    exit_dir: u8,
    block: &[TileCoord],
) -> bool {
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    let (dx, dy) = crate::map::diag_dir_offset(exit_dir);
    let first_beyond = TileCoord::new(signal_tile.x + dx, signal_tile.y + dy);
    vehicles.iter().any(|v| {
        if v.kind != VehicleKind::Train || !v.running {
            return false;
        }
        if !v.reserved_steps.iter().any(|s| s.tile == first_beyond) {
            return false;
        }
        if !v
            .reserved_steps
            .iter()
            .any(|s| block_set.contains(&s.tile) || s.tile == first_beyond)
        {
            return false;
        }
        reservation_ends_at_safe_wait(map, v)
    })
}

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
    let vehicle = &vehicles[vehicle_idx];
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return Vec::new();
    }

    let path: Vec<TileCoord> = vehicle.path.iter().copied().collect();
    let along_path = reserve_along_path(map, vehicles, vehicle, &path, already_reserved);
    if reservation_ends_at_safe_wait_steps(map, vehicle.pos, &path, &along_path) {
        return along_path;
    }
    // TryReservePath: si el path de órdenes no llega a safe wait, buscar alternativa.
    // `path_backoff_interval == 255` desactiva look-ahead; si no, solo reintenta cuando
    // `should_retry_reservation(wait_counter)` (trenes no stuck tienen wait_counter=0 → siempre).
    if !settings.should_retry_reservation(vehicle.wait_counter) {
        return along_path;
    }
    let Some(alt) = find_path_to_safe_wait(
        map,
        vehicles,
        vehicle.id,
        vehicle.pos,
        &path,
        already_reserved,
    ) else {
        return along_path;
    };
    let alt_res = reserve_along_path(map, vehicles, vehicle, &alt, already_reserved);
    if alt_res.len() > along_path.len()
        || reservation_ends_at_safe_wait_steps(map, vehicle.pos, &alt, &alt_res)
    {
        alt_res
    } else {
        along_path
    }
}

fn reservation_ends_at_safe_wait_steps(
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
    let mut global = HashSet::new();
    for i in 0..vehicles.len() {
        if vehicles[i].kind != VehicleKind::Train {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        let reserved = compute_train_reservation_with_settings(map, vehicles, i, &global, settings);
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
    if vehicle
        .reserved_steps
        .iter()
        .any(|s| s.tile == next && (s.track == track || tracks_overlap(s.track, track)))
    {
        return false;
    }
    if vehicle.reserved_steps.iter().any(|s| s.tile == vehicle.pos) {
        return false;
    }
    true
}

/// Bit de reserva PBS en cruces a nivel (`HasCrossingReservation` / `m5` bit 4).
pub const CROSSING_RESERVATION_M5_BIT: u8 = 1 << 4;

/// Escribe reservas PBS en `m2_hi` (vía plana) y `m5` bit 4 (cruces); marca `dirty`.
pub fn sync_reservations_to_map(
    map: &mut Map,
    vehicles: &[Vehicle],
    prev_active: &mut HashSet<TileCoord>,
    dirty: &mut Vec<TileCoord>,
) {
    let mut next_tracks: HashMap<TileCoord, u8> = HashMap::new();
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        for step in &v.reserved_steps {
            match map.get_kind(step.tile) {
                Some(TileKind::Rail) => {
                    next_tracks
                        .entry(step.tile)
                        .and_modify(|bits| *bits |= step.track)
                        .or_insert(step.track);
                }
                Some(TileKind::Road) => {
                    let Some(tile) = map.get(step.tile) else {
                        continue;
                    };
                    if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
                        next_tracks.entry(step.tile).or_insert(step.track);
                    }
                }
                _ => {}
            }
        }
    }

    let mut touch = HashSet::new();
    for c in prev_active.iter().chain(next_tracks.keys()) {
        touch.insert(*c);
    }

    for c in touch {
        let Some(mut tile) = map.get(c) else {
            continue;
        };
        let want = next_tracks.get(&c).copied().unwrap_or(0);
        let changed = if tile.kind == TileKind::Rail {
            let had = decode_rail_reservation_m2_hi(tile.m2_hi);
            tile.m2_hi = (tile.m2_hi & !RAIL_RESERVATION_M2_HI_MASK)
                | encode_rail_reservation_to_m2_hi(want);
            had != want
        } else if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
            let had = tile.m5 & CROSSING_RESERVATION_M5_BIT != 0;
            let want_flag = want != 0;
            if want_flag {
                tile.m5 |= CROSSING_RESERVATION_M5_BIT;
            } else {
                tile.m5 &= !CROSSING_RESERVATION_M5_BIT;
            }
            had != want_flag
        } else {
            false
        };
        if changed {
            let _ = map.set_tile(c, tile);
            dirty.push(c);
        }
    }

    *prev_active = next_tracks.keys().copied().collect();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::GameState;
    use crate::command::{Command, apply_command};
    use crate::map::{OTTD_MP_ROAD, TileKind, is_road_level_crossing};
    use crate::parity::{
        TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y, TRAIN_DUAL_VEHICLE_2_ID,
        TRAIN_DUAL_VEHICLE_ID, build_train_supply_dual,
    };
    use crate::rail_signals::{RAIL_TILE_NORMAL, SIGTYPE_PATH, update_rail_signal_states};
    use crate::vehicle::VehicleKind;

    #[test]
    fn encode_decode_roundtrip_horz_and_single() {
        assert_eq!(decode_rail_reservation_m2_hi(0), 0);
        assert_eq!(
            decode_rail_reservation_m2_hi(encode_rail_reservation_to_m2_hi(0x04)),
            0x04
        );
        assert_eq!(
            decode_rail_reservation_m2_hi(encode_rail_reservation_to_m2_hi(RAIL_TB_HORZ)),
            RAIL_TB_HORZ
        );
    }

    #[test]
    fn parallel_tracks_get_disjoint_reservations() {
        let mut state = build_train_supply_dual();
        {
            let t2 = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            t2.pos = TileCoord::new(7, TRAIN_DUAL_TRACK_RET_Y);
            t2.path = VecDeque::from([
                TileCoord::new(6, TRAIN_DUAL_TRACK_RET_Y),
                TileCoord::new(5, TRAIN_DUAL_TRACK_RET_Y),
            ]);
            t2.running = true;
        }
        {
            let t1 = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            t1.pos = TileCoord::new(5, TRAIN_DUAL_TRACK_OUT_Y);
            t1.path = VecDeque::from([
                TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y),
            ]);
            t1.running = true;
        }

        let mut dirty = Vec::new();
        crate::rail_signals::update_rail_signal_states(
            &mut state.map,
            &state.vehicles,
            &mut dirty,
            true,
        );
        update_train_reservations(&state.map, &mut state.vehicles);
        let t1 = state.vehicles.iter().find(|v| v.id == 1).expect("tren 1");
        let t2 = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            t1.reserved_steps
                .iter()
                .all(|s| s.tile.y == TRAIN_DUAL_TRACK_OUT_Y)
        );
        assert!(
            t2.reserved_steps
                .iter()
                .all(|s| s.tile.y == TRAIN_DUAL_TRACK_RET_Y)
        );
        assert!(
            t1.reserved_steps.len() >= 3,
            "tren 1 reserva ida: {:?}",
            t1.reserved_steps
        );
        assert!(
            t2.reserved_steps.len() >= 3,
            "tren 2 reserva vuelta: {:?}",
            t2.reserved_steps
        );
    }

    #[test]
    fn disjoint_tracks_on_same_tile_do_not_conflict() {
        let tile = TileCoord::new(5, 4);
        let upper = 0x04;
        let lower = 0x08;
        let mut reserved = HashSet::from([ReservedRailStep::new(tile, upper)]);
        let lower_step = ReservedRailStep::new(tile, lower);
        assert!(!reserved.contains(&lower_step));
        reserved.insert(lower_step);
        assert_eq!(reserved.len(), 2);
    }

    #[test]
    fn follower_reservation_stops_before_leader_on_same_track() {
        let mut state = build_train_supply_dual();
        let leader_pos = TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y);
        let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);
        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            leader.pos = leader_pos;
            leader.path.clear();
            leader.running = true;
        }
        let mut follower = crate::vehicle::Vehicle::new(
            2,
            VehicleKind::Train,
            follower_pos,
            TileCoord::new(13, TRAIN_DUAL_TRACK_OUT_Y),
        );
        follower.path = VecDeque::from([
            TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
            leader_pos,
            TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y),
        ]);
        follower.running = true;
        state.vehicles.push(follower);

        update_train_reservations(&state.map, &mut state.vehicles);
        let follower = state.vehicles.iter().find(|v| v.id == 2).expect("tren 2");
        assert!(
            follower
                .reserved_steps
                .iter()
                .all(|s| s.tile.x <= follower_pos.x),
            "no debe reservar más allá del líder: {:?}",
            follower.reserved_steps
        );
        assert!(
            !follower
                .reserved_steps
                .iter()
                .any(|s| s.tile.x > follower_pos.x),
            "reserva cortada antes del líder: {:?}",
            follower.reserved_steps
        );
    }

    #[test]
    fn connector_tile_stays_reserved_while_train_turns() {
        let mut state = build_train_supply_dual();
        let connector = TileCoord::new(10, 5);
        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            leader.pos = connector;
            leader.path = VecDeque::from([TileCoord::new(10, TRAIN_DUAL_TRACK_RET_Y)]);
            leader.running = true;
        }

        update_train_reservations(&state.map, &mut state.vehicles);
        let leader = state.vehicles.iter().find(|v| v.id == 1).expect("tren 1");
        assert!(
            leader.reserved_steps.iter().any(|s| s.tile == connector),
            "conector ocupado: {:?}",
            leader.reserved_steps
        );
    }

    #[test]
    fn sync_sets_m2_reservation_bits_on_rail() {
        let mut state = build_train_supply_dual();
        let tile = TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y);
        let rails_before = state.map.get(tile).expect("vía").m5 & 0x3F;
        let track =
            track_on_departure_tile(&state.map, tile, TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y))
                .expect("pista");
        state.vehicles[0].reserved_steps = vec![ReservedRailStep::new(tile, track)];
        let mut prev = HashSet::new();
        let mut dirty = Vec::new();
        sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
        let t = state.map.get(tile).expect("vía");
        assert_eq!(
            t.m5 & 0x3F,
            rails_before,
            "reserva no debe alterar TrackBits"
        );
        assert_ne!(decode_rail_reservation_m2_hi(t.m2_hi), 0);
        assert!(!dirty.is_empty());

        state.vehicles[0].reserved_steps.clear();
        sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
        let t = state.map.get(tile).expect("vía");
        assert_eq!(decode_rail_reservation_m2_hi(t.m2_hi), 0);
    }

    #[test]
    fn sync_sets_crossing_m5_reservation_bit() {
        let mut state = GameState::new(8, 4);
        let c = TileCoord::new(2, 1);
        state.map.set_kind(c, TileKind::Road).expect("road");
        let mut t = state.map.get(c).expect("tile");
        t.mapt = OTTD_MP_ROAD << 4;
        t.m5 = 1 << 6; // RoadTileType::Crossing
        state.map.set_tile(c, t).expect("crossing");
        assert!(is_road_level_crossing(
            state.map.get(c).unwrap().mapt,
            state.map.get(c).unwrap().m5,
            TileKind::Road
        ));

        let mut train = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(3, 1),
        );
        train.running = true;
        train.reserved_steps = vec![ReservedRailStep::new(c, 0x01)];
        state.vehicles = vec![train];

        let mut prev = HashSet::new();
        let mut dirty = Vec::new();
        sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
        let t = state.map.get(c).expect("crossing");
        assert_ne!(t.m5 & CROSSING_RESERVATION_M5_BIT, 0);
        assert!(dirty.contains(&c));

        state.vehicles[0].reserved_steps.clear();
        sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
        let t = state.map.get(c).expect("crossing");
        assert_eq!(t.m5 & CROSSING_RESERVATION_M5_BIT, 0);
    }

    /// Path rojo no debe impedir extender la reserva (rompe deadlock reserva↔verde).
    #[test]
    fn path_signal_allows_reservation_while_red() {
        const RAIL_TB_X: u8 = 0x01;
        let mut state = GameState::new(12, 4);
        let y = 1;
        for x in 0..=8 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
            let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
            t.m5 = RAIL_TB_X | (RAIL_TILE_NORMAL << 6);
            state.map.set_tile(TileCoord::new(x, y), t).expect("x");
        }
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(6, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path 2");

        let mut train = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, y),
            TileCoord::new(8, y),
        );
        train.path = VecDeque::from([
            TileCoord::new(2, y),
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
            TileCoord::new(7, y),
            TileCoord::new(8, y),
        ]);
        train.running = true;
        state.vehicles = vec![train];

        let mut dirty = Vec::new();
        // Primera pasada: path queda rojo (sin reserva aún).
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        let sig = state.map.get(TileCoord::new(3, y)).expect("sig");
        assert!(
            crate::rail_signals::rail_signal_state_mask(sig.m3hi) == 0
                || !crate::rail_signals::signal_is_green(sig.m3hi, 0)
                    && !crate::rail_signals::signal_is_green(sig.m3hi, 2),
            "path debería estar rojo antes de reservar: m3hi={:#x}",
            sig.m3hi
        );

        update_train_reservations(&state.map, &mut state.vehicles);
        let reserved = &state.vehicles[0].reserved_steps;
        assert!(
            reserved.iter().any(|s| s.tile.x >= 4),
            "reserva debe cruzar path roja: {reserved:?}"
        );

        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, false);
        let sig = state.map.get(TileCoord::new(3, y)).expect("sig");
        let present = crate::rail_signals::rail_signal_present_mask(sig.m3);
        let any_green = (0..4u8).any(|bit| {
            present & (1 << bit) != 0 && crate::rail_signals::signal_is_green(sig.m3hi, bit)
        });
        assert!(
            any_green,
            "path debe ponerse verde con reserva: m3hi={:#x}",
            sig.m3hi
        );
    }

    /// Dos corredores paralelos con path signals: ambos reservan sin deadlock (rojo↔reserva).
    #[test]
    fn path_signals_parallel_corridors_reserve() {
        let mut state = GameState::new(16, 8);
        for &y in &[2, 4] {
            for x in 1..=10 {
                apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
                let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
                t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6); // TRACK_X
                state.map.set_tile(TileCoord::new(x, y), t).expect("x");
            }
            for &x in &[3, 7] {
                apply_command(
                    &mut state,
                    &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
                )
                .expect("path");
            }
        }

        let mut t1 = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, 2),
            TileCoord::new(9, 2),
        );
        t1.path = VecDeque::from([
            TileCoord::new(3, 2),
            TileCoord::new(4, 2),
            TileCoord::new(5, 2),
            TileCoord::new(6, 2),
            TileCoord::new(7, 2),
            TileCoord::new(8, 2),
            TileCoord::new(9, 2),
        ]);
        t1.running = true;

        let mut t2 = crate::vehicle::Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(2, 4),
            TileCoord::new(9, 4),
        );
        t2.path = VecDeque::from([
            TileCoord::new(3, 4),
            TileCoord::new(4, 4),
            TileCoord::new(5, 4),
            TileCoord::new(6, 4),
            TileCoord::new(7, 4),
            TileCoord::new(8, 4),
            TileCoord::new(9, 4),
        ]);
        t2.running = true;
        state.vehicles = vec![t1, t2];

        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        update_train_reservations(&state.map, &mut state.vehicles);

        let r1 = &state.vehicles[0].reserved_steps;
        let r2 = &state.vehicles[1].reserved_steps;
        assert!(r1.len() >= 4, "tren norte reserva: {r1:?}");
        assert!(r2.len() >= 4, "tren sur reserva: {r2:?}");
        assert!(
            r1.iter().all(|s| s.tile.y == 2) && r2.iter().all(|s| s.tile.y == 4),
            "reservas disjuntas por corredor: {r1:?} / {r2:?}"
        );
        assert!(
            r1.iter().any(|s| s.tile.x >= 4) && r2.iter().any(|s| s.tile.x >= 4),
            "ambos cruzan path: {r1:?} / {r2:?}"
        );
        // Safe wait: cortar delante de la 2.ª path (x=7) → último paso x=6.
        assert!(
            r1.iter().map(|s| s.tile.x).max() == Some(6),
            "debe cortar en safe wait delante de path x=7: {r1:?}"
        );
        assert!(
            reservation_ends_at_safe_wait(&state.map, &state.vehicles[0]),
            "reserva completa hasta safe wait"
        );
    }

    #[test]
    fn depot_is_safe_waiting_position() {
        let mut state = GameState::new(12, 6);
        let y = 2;
        for x in 1..=6 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        }
        apply_command(
            &mut state,
            &Command::PlaceRailDepotDir(TileCoord::new(1, 3), 3),
        )
        .expect("depósito");
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(4, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");

        let depot = TileCoord::new(1, 3);
        assert!(
            is_safe_waiting_position(&state.map, depot, Some(TileCoord::new(1, y)), false),
            "depósito es safe wait"
        );

        let mut train =
            crate::vehicle::Vehicle::new(1, VehicleKind::Train, TileCoord::new(2, y), depot);
        // Path hacia el depósito vía (1,y) → (1,3).
        train.path = VecDeque::from([
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
        ]);
        train.running = true;
        state.vehicles = vec![train];
        update_train_reservations(&state.map, &mut state.vehicles);
        let reserved = &state.vehicles[0].reserved_steps;
        // Sin depósito en el path: corta en fin de path (x=6) o delante de nada.
        assert!(
            reservation_ends_at_safe_wait(&state.map, &state.vehicles[0]),
            "fin de path es safe wait: {reserved:?}"
        );
    }

    #[test]
    fn reservation_stops_before_next_path_signal() {
        let mut state = GameState::new(14, 4);
        let y = 1;
        for x in 0..=10 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
            let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
            t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
            state.map.set_tile(TileCoord::new(x, y), t).expect("x");
        }
        for &x in &[3, 7] {
            apply_command(
                &mut state,
                &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
            )
            .expect("path");
        }

        let mut train = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, y),
            TileCoord::new(10, y),
        );
        train.path = VecDeque::from([
            TileCoord::new(2, y),
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
            TileCoord::new(7, y),
            TileCoord::new(8, y),
            TileCoord::new(9, y),
            TileCoord::new(10, y),
        ]);
        train.running = true;
        state.vehicles = vec![train];
        update_train_reservations(&state.map, &mut state.vehicles);
        let reserved = &state.vehicles[0].reserved_steps;
        let max_x = reserved.iter().map(|s| s.tile.x).max();
        assert_eq!(
            max_x,
            Some(6),
            "reserva hasta delante de path x=7, no más allá: {reserved:?}"
        );
        assert!(
            !reserved.iter().any(|s| s.tile.x >= 7),
            "no debe incluir la 2.ª path ni más allá: {reserved:?}"
        );
        assert!(reservation_ends_at_safe_wait(
            &state.map,
            &state.vehicles[0]
        ));
    }

    #[test]
    fn wait_for_pbs_path_marks_stuck_and_reverses_on_timeout() {
        use crate::pathfinding_settings::PathfindingSettings;
        use crate::vehicle::DIR_SW;

        let mut state = GameState::new(12, 4);
        let y = 1;
        for x in 0..=8 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
            let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
            t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
            state.map.set_tile(TileCoord::new(x, y), t).expect("x");
        }
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");

        // Bloqueador en el bloque: impide reserva completa.
        let mut blocker = crate::vehicle::Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(5, y),
            TileCoord::new(5, y),
        );
        blocker.running = true;
        blocker.path.clear();

        let mut train = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, y),
            TileCoord::new(8, y),
        );
        train.path = VecDeque::from([
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
            TileCoord::new(7, y),
            TileCoord::new(8, y),
        ]);
        train.running = true;
        train.direction = DIR_SW; // hacia +x
        state.vehicles = vec![train, blocker];
        update_train_reservations(&state.map, &mut state.vehicles);

        assert!(
            train_waiting_for_pbs_path(&state.map, &state.vehicles[0]),
            "debe esperar path sin reserva completa"
        );

        let settings = PathfindingSettings {
            wait_for_pbs_path: 2, // 2 días → 148 ticks
            ..Default::default()
        };
        let timeout = settings.pbs_reverse_timeout_ticks().expect("timeout");
        let dir_before = state.vehicles[0].direction;

        for _ in 0..timeout.saturating_sub(1) {
            let reversed =
                tick_pbs_wait_and_maybe_reverse(&state.map, &mut state.vehicles[0], settings);
            assert!(!reversed);
            assert!(state.vehicles[0].pbs_stuck);
        }
        let reversed =
            tick_pbs_wait_and_maybe_reverse(&state.map, &mut state.vehicles[0], settings);
        assert!(reversed, "debe girar al timeout");
        assert_ne!(state.vehicles[0].direction, dir_before);
        assert!(!state.vehicles[0].pbs_stuck);
        assert!(state.vehicles[0].path.is_empty());
    }

    #[test]
    fn wait_for_pbs_path_255_never_reverses() {
        use crate::pathfinding_settings::{PBS_WAIT_FOREVER, PathfindingSettings};

        let mut state = GameState::new(10, 4);
        let y = 1;
        for x in 0..=6 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        }
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(2, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");
        let mut blocker = crate::vehicle::Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(4, y),
            TileCoord::new(4, y),
        );
        blocker.running = true;
        let mut train = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, y),
            TileCoord::new(6, y),
        );
        train.path = VecDeque::from([
            TileCoord::new(2, y),
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
        ]);
        train.running = true;
        state.vehicles = vec![train, blocker];
        update_train_reservations(&state.map, &mut state.vehicles);

        let settings = PathfindingSettings {
            wait_for_pbs_path: PBS_WAIT_FOREVER,
            ..Default::default()
        };
        for _ in 0..500 {
            assert!(!tick_pbs_wait_and_maybe_reverse(
                &state.map,
                &mut state.vehicles[0],
                settings
            ));
        }
        assert!(state.vehicles[0].pbs_stuck);
    }

    #[test]
    fn find_path_to_safe_wait_reaches_next_path() {
        let mut state = GameState::new(14, 4);
        let y = 1;
        for x in 0..=10 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
            let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
            t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
            state.map.set_tile(TileCoord::new(x, y), t).expect("x");
        }
        for &x in &[3, 7] {
            apply_command(
                &mut state,
                &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
            )
            .expect("path");
        }
        let from = TileCoord::new(1, y);
        let preferred = [
            TileCoord::new(2, y),
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
            TileCoord::new(7, y),
        ];
        let path = find_path_to_safe_wait(&state.map, &[], 1, from, &preferred, &HashSet::new())
            .expect("safe wait path");
        assert!(
            path.iter().any(|c| c.x == 6),
            "debe llegar delante de path x=7: {path:?}"
        );
        assert!(
            !path.iter().any(|c| c.x >= 7),
            "no debe incluir la 2.ª path: {path:?}"
        );
    }

    /// Línea principal bloqueada + desvío libre: `TryReserve` solo corre en ticks de backoff.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn path_backoff_interval_throttles_try_reserve() {
        use crate::pathfinding_settings::PathfindingSettings;

        let mut state = GameState::new(12, 6);
        let y = 2;
        // Vía principal X.
        for x in 0..=8 {
            apply_command(
                &mut state,
                &Command::SetRailBits(TileCoord::new(x, y), 0x01),
            )
            .expect("vía X");
        }
        // Cruce + desvío hacia y=0 (fin de vía = safe wait).
        apply_command(
            &mut state,
            &Command::SetRailBits(TileCoord::new(4, y), 0x03),
        )
        .expect("cruce");
        apply_command(
            &mut state,
            &Command::SetRailBits(TileCoord::new(4, 1), 0x02),
        )
        .expect("desvío");
        apply_command(
            &mut state,
            &Command::SetRailBits(TileCoord::new(4, 0), 0x02),
        )
        .expect("fin desvío");
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");

        let mut blocker = crate::vehicle::Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(6, y),
            TileCoord::new(6, y),
        );
        blocker.running = true;
        blocker.path.clear();

        let mut train = crate::vehicle::Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, y),
            TileCoord::new(8, y),
        );
        train.path = VecDeque::from([
            TileCoord::new(3, y),
            TileCoord::new(4, y),
            TileCoord::new(5, y),
            TileCoord::new(6, y),
            TileCoord::new(7, y),
            TileCoord::new(8, y),
        ]);
        train.running = true;
        train.pbs_stuck = true;
        state.vehicles = vec![train, blocker];

        let settings = PathfindingSettings {
            path_backoff_interval: 20,
            ..Default::default()
        };

        // Tick intermedio: no TryReserve → no usa el desvío.
        state.vehicles[0].wait_counter = 7;
        let mid = compute_train_reservation_with_settings(
            &state.map,
            &state.vehicles,
            0,
            &HashSet::new(),
            settings,
        );
        assert!(
            !mid.iter().any(|s| s.tile.y != y),
            "sin backoff no debe desviarse: {mid:?}"
        );
        assert!(
            !reservation_ends_at_safe_wait_steps(
                &state.map,
                state.vehicles[0].pos,
                &state.vehicles[0].path.iter().copied().collect::<Vec<_>>(),
                &mid
            ),
            "reserva intermedia incompleta: {mid:?}"
        );

        // Múltiplo del intervalo: TryReserve encuentra el desvío hasta safe wait.
        state.vehicles[0].wait_counter = 40;
        let on_backoff = compute_train_reservation_with_settings(
            &state.map,
            &state.vehicles,
            0,
            &HashSet::new(),
            settings,
        );
        assert!(
            on_backoff.iter().any(|s| s.tile == TileCoord::new(4, 0)),
            "con backoff debe reservar el desvío: {on_backoff:?}"
        );
        assert!(
            reservation_ends_at_safe_wait_steps(
                &state.map,
                state.vehicles[0].pos,
                &state.vehicles[0].path.iter().copied().collect::<Vec<_>>(),
                &on_backoff
            ),
            "desvío debe terminar en safe wait: {on_backoff:?}"
        );

        // 255: look-ahead off aunque wait_counter sea múltiplo.
        let off = PathfindingSettings {
            path_backoff_interval: crate::pathfinding_settings::PBS_WAIT_FOREVER,
            ..Default::default()
        };
        state.vehicles[0].wait_counter = 40;
        let forever = compute_train_reservation_with_settings(
            &state.map,
            &state.vehicles,
            0,
            &HashSet::new(),
            off,
        );
        assert!(
            !forever.iter().any(|s| s.tile.y != y),
            "255 no debe hacer TryReserve: {forever:?}"
        );
    }
}
