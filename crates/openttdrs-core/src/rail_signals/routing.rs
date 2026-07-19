//! Costes YAPF y predicates de tráfico / espera ante señales.

use crate::map::{Map, TileCoord, TileKind, opposite_diag_dir as opposite_dir};
use crate::vehicle::{Vehicle, VehicleKind};

use super::encoding::{
    SIGTYPE_BLOCK, SIGTYPE_ENTRY, SIGTYPE_PATH, SIGTYPE_PATH_ONEWAY, is_pbs_signal_type,
    rail_signal_present_mask, signal_is_green, signal_type_for_track,
};
use super::encoding::{signal_exit_dir, signal_track_for_bit};
use super::rail_tile_is_signals;
use super::topology::{block_is_occupied_by_trains, dir_from_to, rail_block_ahead, rail_neighbors};

/// Resultado de evaluar señales al planificar ruta (YAPF `SignalCost` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YapfSignalRouting {
    /// Sin señal relevante en la dirección de marcha.
    Clear,
    /// Penalización por señal roja (el camino sigue siendo válido).
    Penalty(u32),
    /// Sentido único en contra: rama inválida (`EndSegmentReason::DeadEnd`).
    DeadEnd,
}

/// Penalización YAPF por señal de bloque roja (aprox. `rail_firstred_penalty`).
pub const YAPF_RED_SIGNAL_PENALTY: u32 = 100;
/// Penalización YAPF por cruzar una señal path por detrás (`yapf_costrail.hpp`).
pub const YAPF_PBS_BEHIND_PENALTY: u32 = 100;

/// Evalúa señales al planificar salida de `tile` en `exit_dir` (convención `OpenTTD` / `rail_signals`).
///
/// Replica la regla central de `CYapfCostRailT::SignalCost`: señal unidireccional solo en
/// sentido contrario → callejón sin salida; roja a favor → penalización.
#[must_use]
pub fn yapf_routing_signal(map: &Map, tile: TileCoord, exit_dir: u8) -> YapfSignalRouting {
    let Some(t) = map.get(tile) else {
        return YapfSignalRouting::Clear;
    };
    if t.kind != TileKind::Rail || !rail_tile_is_signals(t.m5) {
        return YapfSignalRouting::Clear;
    }
    let rails = t.m5 & 0x3F;
    let present = rail_signal_present_mask(t.m3);
    if present == 0 {
        return YapfSignalRouting::Clear;
    }

    let mut along = false;
    let mut against = false;
    let mut red_penalty = 0u32;
    for bit in 0..4u8 {
        if present & (1 << bit) == 0 {
            continue;
        }
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(t.m2, track));
        let sig_exit = signal_exit_dir(rails, bit);
        if sig_exit == exit_dir {
            along = true;
            if signal_track_for_bit(rails, bit)
                .is_some_and(|track| signal_type_for_track(t.m2, track) == SIGTYPE_ENTRY)
            {
                continue;
            }
            if is_pbs_signal_type(sig_type) {
                if !signal_is_green(t.m3hi, bit) {
                    red_penalty = red_penalty.saturating_add(YAPF_RED_SIGNAL_PENALTY);
                }
                continue;
            }
            if !signal_is_green(t.m3hi, bit) {
                red_penalty = red_penalty.saturating_add(YAPF_RED_SIGNAL_PENALTY);
            }
        } else if sig_exit == opposite_dir(exit_dir) {
            against = true;
            if sig_type == SIGTYPE_PATH {
                red_penalty = red_penalty.saturating_add(YAPF_PBS_BEHIND_PENALTY);
            }
        }
    }

    // One-way real: PathOneWay, o señal de bloque convencional con un solo lado.
    // Un Path "two-way" suele tener un solo bit presente; ir en contra es
    // `YAPF_PBS_BEHIND_PENALTY`, no callejón sin salida (OpenTTD `SignalCost`).
    let is_oneway = (0..4).any(|bit| {
        if present & (1 << bit) == 0 {
            return false;
        }
        let Some(track) = signal_track_for_bit(rails, bit) else {
            return false;
        };
        let ty = signal_type_for_track(t.m2, track);
        ty == SIGTYPE_PATH_ONEWAY || (present.is_power_of_two() && !is_pbs_signal_type(ty))
    });
    if against && !along && is_oneway {
        return YapfSignalRouting::DeadEnd;
    }
    if red_penalty > 0 {
        YapfSignalRouting::Penalty(red_penalty)
    } else {
        YapfSignalRouting::Clear
    }
}

/// Bits de señal presentes que controlan la salida `from` → `to`.
#[must_use]
pub(super) fn signal_bits_for_exit(map: &Map, from: TileCoord, to: TileCoord) -> Vec<u8> {
    let Some(tile) = map.get(from) else {
        return Vec::new();
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return Vec::new();
    }
    let exit_dir = dir_from_to(from, to).unwrap_or(0);
    let rails = tile.m5 & 0x3F;
    let present = rail_signal_present_mask(tile.m3);
    (0..4u8)
        .filter(|&bit| present & (1 << bit) != 0 && signal_exit_dir(rails, bit) == exit_dir)
        .collect()
}

/// Siguiente tesela tras `via` según el path del tren (o `dest` si es el último salto).
#[must_use]
fn path_continuation_after(vehicle: &Vehicle, via: TileCoord) -> Option<TileCoord> {
    if vehicle.path.front() != Some(&via) {
        return None;
    }
    if let Some(&next) = vehicle.path.get(1) {
        return Some(next);
    }
    if vehicle.dest != via {
        return Some(vehicle.dest);
    }
    None
}

/// `true` si la salida `signal_tile` → `beyond` está prohibida (rojo u ocupado).
///
/// Path / `PathOneWay`: el verde se deriva de la reserva PBS; no se exige verde previa
/// ni se usa ocupación de bloque (evita deadlock reserva↔rojo). `PathOneWay` en sentido
/// contrario ya es `DeadEnd` vía `yapf_routing_signal`.
#[must_use]
fn signal_exit_denied(
    map: &Map,
    vehicles: &[Vehicle],
    signal_tile: TileCoord,
    beyond: TileCoord,
    tile: &crate::map::Tile,
) -> bool {
    let exit_dir = dir_from_to(signal_tile, beyond).unwrap_or(0);
    if matches!(
        yapf_routing_signal(map, signal_tile, exit_dir),
        YapfSignalRouting::DeadEnd
    ) {
        return true;
    }
    let mut checked = false;
    for bit in signal_bits_for_exit(map, signal_tile, beyond) {
        checked = true;
        let rails = tile.m5 & 0x3F;
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
        // Path: el verde se deriva de la reserva; no bloquear por rojo/ocupación de bloque.
        if is_pbs_signal_type(sig_type) {
            continue;
        }
        if sig_type == SIGTYPE_ENTRY {
            if !signal_is_green(tile.m3hi, bit) {
                return true;
            }
            continue;
        }
        if !signal_is_green(tile.m3hi, bit) {
            return true;
        }
        let block = rail_block_ahead(map, signal_tile, exit_dir);
        if block_is_occupied_by_trains(vehicles, signal_tile, &block) {
            return true;
        }
    }
    // Ningún bit controla esta salida: ocupación de bloque solo para señales no-PBS
    // (path permite pasar por detrás; PathOneWay ya es DeadEnd arriba).
    if !checked && tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
        let rails = tile.m5 & 0x3F;
        let present = rail_signal_present_mask(tile.m3);
        let has_non_pbs = (0..4u8).any(|bit| {
            present & (1 << bit) != 0
                && !is_pbs_signal_type(
                    signal_track_for_bit(rails, bit)
                        .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track)),
                )
        });
        if has_non_pbs {
            let block = rail_block_ahead(map, signal_tile, exit_dir);
            if block_is_occupied_by_trains(vehicles, signal_tile, &block) {
                return true;
            }
        }
    }
    false
}

/// `true` si un paso `from` → `to` puede incluirse en una reserva PBS.
#[must_use]
pub(crate) fn rail_step_signal_allows(
    map: &Map,
    vehicles: &[Vehicle],
    from: TileCoord,
    to: TileCoord,
    beyond: Option<TileCoord>,
) -> bool {
    if let Some(sig_tile) = map.get(to)
        && sig_tile.kind == TileKind::Rail
        && rail_tile_is_signals(sig_tile.m5)
        && let Some(beyond) = beyond
        && signal_exit_denied(map, vehicles, to, beyond, &sig_tile)
    {
        return false;
    }
    let Some(from_tile) = map.get(from) else {
        return true;
    };
    if from_tile.kind != TileKind::Rail || !rail_tile_is_signals(from_tile.m5) {
        return true;
    }
    !signal_exit_denied(map, vehicles, from, to, &from_tile)
}

/// `true` si el avance sub-tesela de este tick completaría la tesela actual.
#[must_use]
fn train_would_complete_current_tile(vehicle: &Vehicle) -> bool {
    if vehicle.depart_turn > 0 {
        let step = u16::from(vehicle.progress_step().max(1));
        return u16::from(vehicle.depart_turn).saturating_add(step) >= 255;
    }
    if vehicle.progress == 255 && vehicle.needs_depart_turnaround() {
        return true;
    }
    // Usa el modelo físico (2× loco handler + píxeles); el setting de partida
    // realista se aplica al importar SAV; aquí asumimos realista si hay caché TE.
    let model = if vehicle.cached_max_te_n > 0 {
        crate::engine::TrainAccelerationModel::Realistic
    } else {
        crate::engine::TrainAccelerationModel::Original
    };
    vehicle.train_would_leave_tile_this_tick(model)
}

/// `true` si la salida path en `signal_tile` → `beyond` carece de reserva completa.
#[must_use]
fn path_exit_lacks_reservation(
    map: &Map,
    vehicle: &Vehicle,
    signal_tile: TileCoord,
    beyond: TileCoord,
    tile: &crate::map::Tile,
) -> bool {
    let exit_dir = dir_from_to(signal_tile, beyond).unwrap_or(0);
    if matches!(
        yapf_routing_signal(map, signal_tile, exit_dir),
        YapfSignalRouting::DeadEnd
    ) {
        return true;
    }
    let bits = signal_bits_for_exit(map, signal_tile, beyond);
    if bits.is_empty() {
        return false;
    }
    let rails = tile.m5 & 0x3F;
    let any_pbs = bits.iter().any(|&bit| {
        signal_track_for_bit(rails, bit)
            .is_some_and(|track| is_pbs_signal_type(signal_type_for_track(tile.m2, track)))
    });
    if !any_pbs {
        return false;
    }
    // Debe reservar `beyond` y llegar a una posición segura (TryReservePath).
    let has_beyond = vehicle.reserved_steps.iter().any(|s| s.tile == beyond);
    !(has_beyond && crate::rail_pbs::reservation_ends_at_safe_wait(map, vehicle))
}

/// `true` si el tren debe esperar ante la tesela de señal `to` (sin entrar al bloque).
#[must_use]
fn train_held_before_signal_tile(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    to: TileCoord,
    signal_tile: &crate::map::Tile,
) -> bool {
    let Some(beyond) = path_continuation_after(vehicle, to) else {
        return false;
    };
    let denied = signal_exit_denied(map, vehicles, to, beyond, signal_tile)
        || path_exit_lacks_reservation(map, vehicle, to, beyond, signal_tile);
    if !denied {
        return false;
    }
    if vehicle.pos == to {
        return true;
    }
    // Detenido avanzando hacia la señal: mantener espera (usar píxeles, no el
    // remanente físico `progress`, que suele ser >0 en el modelo rail exacto).
    if vehicle.cur_speed == 0 && vehicle.rail_pixel > 0 {
        return true;
    }
    train_would_complete_current_tile(vehicle)
}

/// `true` si el tren no puede avanzar por falta de reserva PBS completa en path.
#[must_use]
pub fn train_blocked_by_pbs_path(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    let from = vehicle.pos;
    let Some(to) = vehicle.movement_target() else {
        return false;
    };

    if let Some(signal_tile) = map.get(to)
        && signal_tile.kind == TileKind::Rail
        && rail_tile_is_signals(signal_tile.m5)
        && let Some(beyond) = path_continuation_after(vehicle, to)
        && path_exit_lacks_reservation(map, vehicle, to, beyond, &signal_tile)
    {
        return true;
    }

    let Some(tile) = map.get(from) else {
        return false;
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return false;
    }
    path_exit_lacks_reservation(map, vehicle, from, to, &tile)
}

/// `true` si el tren no puede avanzar al siguiente paso por señal en rojo.
#[must_use]
pub fn train_blocked_by_signal(map: &Map, vehicles: &[Vehicle], vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    let from = vehicle.pos;
    let Some(to) = vehicle.movement_target() else {
        return false;
    };

    if let Some(signal_tile) = map.get(to)
        && signal_tile.kind == TileKind::Rail
        && rail_tile_is_signals(signal_tile.m5)
        && train_held_before_signal_tile(map, vehicles, vehicle, to, &signal_tile)
    {
        return true;
    }

    let Some(tile) = map.get(from) else {
        return false;
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return false;
    }
    signal_exit_denied(map, vehicles, from, to, &tile)
        || path_exit_lacks_reservation(map, vehicle, from, to, &tile)
}

/// `true` si otro tren ocupa la vía delante (misma dirección o frente a frente).
#[must_use]
pub fn train_blocked_by_traffic(map: &Map, vehicles: &[Vehicle], vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    // Solo la cabeza del consist se mueve / bloquea.
    if !vehicle.is_consist_head() {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    let self_id = vehicle.id;
    let foreign = |v: &Vehicle| {
        v.kind == VehicleKind::Train
            && v.is_consist_head()
            && !crate::train_consist::same_consist(vehicles, self_id, v.id)
    };

    if vehicles.iter().any(|v| foreign(v) && v.pos == next) {
        return true;
    }

    // Varios trenes pueden compartir la misma tesela de depósito (OpenTTD).
    if map.get_kind(vehicle.pos) != Some(crate::map::TileKind::RailDepot)
        && vehicles.iter().any(|v| foreign(v) && v.pos == vehicle.pos)
    {
        return true;
    }

    // Colisión con huella multi-tesela de otro consist.
    let self_tiles = crate::train_consist::consist_occupied_tiles(vehicles, self_id);
    for other in vehicles.iter().filter(|v| foreign(v)) {
        let other_tiles = crate::train_consist::consist_occupied_tiles(vehicles, other.id);
        if other_tiles.contains(&next)
            || self_tiles
                .iter()
                .any(|t| other_tiles.contains(t) && *t != vehicle.pos)
        {
            // Solape de huellas (excepto compartir depósito ya filtrado).
            if map.get_kind(vehicle.pos) != Some(crate::map::TileKind::RailDepot) {
                return true;
            }
        }
    }

    let mut prev = vehicle.pos;
    let mut cur = next;
    for _ in 0..64 {
        if let Some(other) = vehicles.iter().find(|v| foreign(v) && v.pos == cur) {
            if !other.running {
                return true;
            }
            if let Some(other_next) = other.movement_target() {
                if other_next == prev || other_next == vehicle.pos {
                    return true;
                }
            } else {
                return true;
            }
            return true;
        }

        let neighbors = rail_neighbors(map, cur, Some(prev));
        let continuations: Vec<_> = neighbors.into_iter().filter(|n| *n != prev).collect();
        if continuations.len() != 1 {
            break;
        }
        prev = cur;
        cur = continuations[0];
    }
    false
}
