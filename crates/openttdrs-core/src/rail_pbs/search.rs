//! Búsqueda de rutas PBS hasta posición segura de espera.

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::{
    SIGTYPE_BLOCK, YapfSignalRouting, dir_from_to, is_pbs_signal_type, rail_signal_present_mask,
    rail_tile_is_signals, signal_exit_dir, signal_track_for_bit, signal_type_for_track,
    yapf_routing_signal,
};
use crate::vehicle::Vehicle;

use super::conflicts::tile_occupied_by_other_train;
use super::model::{
    MAX_TRAIN_RESERVATION_LEN, ReservedRailStep, YAPF_RESERVATION_CROSS_PENALTY, YAPF_TILE_LENGTH,
    track_for_rail_step, track_on_departure_tile,
};

/// Coste base por tesela en `TryReserve` (alineado con YAPF `YAPF_TILE_LENGTH`).
const TRY_RESERVE_TILE_COST: u32 = YAPF_TILE_LENGTH;
/// Sesgo si la tesela no está en el path de órdenes (prioriza YAPF sobre BFS ciego).
const TRY_RESERVE_OFF_PATH_PENALTY: u32 = 50 * YAPF_TILE_LENGTH;

#[derive(Clone, Eq, PartialEq)]
pub(super) struct TryReserveNode {
    pub cost: u32,
    pub cur: TileCoord,
    pub path: Vec<TileCoord>,
    pub passed_path: bool,
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
/// `after_path_signal`: si es `false`, no cortar "delante de path" (hay que reservar
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
        // El vano de un puente conserva su terreno original y el path contiene
        // el enlace lógico rampa→rampa. No es un fin de vía ni una posición de
        // espera segura: la reserva debe alcanzar la rampa opuesta.
        return crate::rail_bridge_other_end(map, tile) != Some(next);
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
pub fn tile_has_any_pbs_signal(map: &Map, tile: TileCoord) -> bool {
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

/// Dijkstra con costes YAPF hasta la siguiente posición segura (`TryPathReserve`).
///
/// Bloqueos duros PBS (reserva ajena, ocupación, señales `DeadEnd`/block rojo) +
/// costes nativos: tesela, cruce de reserva en mapa, sesgo hacia el path de órdenes.
/// Elige el safe wait de **menor coste** (no el primero en BFS).
///
/// `wormholes`: enlaces túnel JGR (misma semántica que YAPF).
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
    find_path_to_safe_wait_with_wormholes(
        map,
        vehicles,
        self_id,
        from,
        preferred,
        already_reserved,
        None,
    )
}

/// Como [`find_path_to_safe_wait`], con wormholes de túnel.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn find_path_to_safe_wait_with_wormholes(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    from: TileCoord,
    preferred: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    use super::conflicts::tile_track_reserved_by_map;
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
        let mut neighbors = rail_neighbors(map, cur, prev);
        if let Some(wh) = wormholes
            && let Some(other) = wh.other_end(cur)
            && !neighbors.contains(&other)
        {
            neighbors.push(other);
        }
        for next in neighbors {
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
