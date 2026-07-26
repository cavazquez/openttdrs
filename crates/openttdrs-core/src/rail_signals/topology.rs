//! Topología de vía, bloques protegidos y ocupación consultiva.

use std::collections::HashSet;

use crate::map::{
    Map, TileCoord, TileKind, opposite_diag_dir as opposite_dir, rail_bits_touching_side,
    rail_signal_diag_dir_offset as diag_dir_offset,
};
use crate::vehicle::{Vehicle, VehicleKind};

use super::rail_tile_is_signals;
use super::rail_traversal_bits;

#[must_use]
pub(crate) fn rail_neighbors(map: &Map, cur: TileCoord, prev: Option<TileCoord>) -> Vec<TileCoord> {
    let tb = rail_traversal_bits(map, cur);
    if tb == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for dir in 0..4u8 {
        let (dx, dy) = diag_dir_offset(dir);
        let next = TileCoord::new(cur.x + dx, cur.y + dy);
        if prev == Some(next) {
            continue;
        }
        if tb & rail_bits_touching_side(dir) == 0 {
            continue;
        }
        let entry = opposite_dir(dir);
        if rail_traversal_bits(map, next) & rail_bits_touching_side(entry) != 0 {
            out.push(next);
        }
    }
    out
}

#[must_use]
pub(crate) fn dir_from_to(from: TileCoord, to: TileCoord) -> Option<u8> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    match (dx, dy) {
        (1, 0) => Some(0),
        (0, 1) => Some(1),
        (-1, 0) => Some(2),
        (0, -1) => Some(3),
        _ => None,
    }
}

#[must_use]
fn rail_continuation_along(
    map: &Map,
    cur: TileCoord,
    prev: TileCoord,
    preferred_dir: u8,
) -> Option<TileCoord> {
    let neighbors: Vec<_> = rail_neighbors(map, cur, Some(prev))
        .into_iter()
        .filter(|n| *n != prev)
        .collect();
    match neighbors.len() {
        0 => None,
        1 => Some(neighbors[0]),
        _ => neighbors
            .into_iter()
            .find(|n| dir_from_to(cur, *n) == Some(preferred_dir)),
    }
}

/// Teselas de conector en un cruce: ramas perpendiculares a la vía del bloque.
///
/// Sin esto, un tren que gira a la vía perpendicular (p. ej. `(10,5)` en el escenario
/// dual) deja de contar como ocupación y la señal pasa a verde con el bloque aún en uso.
fn junction_spur_tiles(map: &Map, block: &[TileCoord], exit_dir: u8) -> Vec<TileCoord> {
    if block.is_empty() {
        return Vec::new();
    }
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    let mut spurs = Vec::new();
    for (i, &tile) in block.iter().enumerate() {
        let forward = if i == 0 {
            exit_dir
        } else {
            dir_from_to(block[i - 1], tile).unwrap_or(exit_dir)
        };
        let back = opposite_dir(forward);
        for n in rail_neighbors(map, tile, None) {
            if block_set.contains(&n) || spurs.contains(&n) {
                continue;
            }
            let Some(d) = dir_from_to(tile, n) else {
                continue;
            };
            if d != forward && d != back {
                spurs.push(n);
            }
        }
    }
    spurs
}

/// Teselas del bloque protegido al salir de `signal_tile` hacia `exit_dir`.
#[must_use]
pub fn rail_block_ahead(map: &Map, signal_tile: TileCoord, exit_dir: u8) -> Vec<TileCoord> {
    rail_block_ahead_with_wormholes(map, signal_tile, exit_dir, None)
}

/// Como [`rail_block_ahead`], saltando wormholes JGR (`tile_n` ↔ `tile_s`).
#[must_use]
pub fn rail_block_ahead_with_wormholes(
    map: &Map,
    signal_tile: TileCoord,
    exit_dir: u8,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> Vec<TileCoord> {
    let (dx, dy) = diag_dir_offset(exit_dir);
    let start = TileCoord::new(signal_tile.x + dx, signal_tile.y + dy);
    if map.get(start).is_none() {
        return Vec::new();
    }
    let mut block = vec![start];
    let mut cur = start;
    let mut prev = signal_tile;
    let mut forward = exit_dir;
    loop {
        let next = rail_continuation_along(map, cur, prev, forward).or_else(|| {
            wormholes
                .and_then(|wh| wh.other_end(cur))
                .filter(|&o| o != prev && rail_traversal_bits(map, o) != 0)
        });
        let Some(next) = next else {
            break;
        };
        if map
            .get(next)
            .is_some_and(|t| t.kind == TileKind::Rail && rail_tile_is_signals(t.m5))
        {
            break;
        }
        block.push(next);
        if let Some(dir) = dir_from_to(cur, next) {
            forward = dir;
        }
        prev = cur;
        cur = next;
    }
    block.extend(junction_spur_tiles(map, &block, exit_dir));
    block
}

/// `true` si algún tren ocupa el bloque protegido por la señal en `signal_tile`.
///
/// Un tren que está sobre `signal_tile` aún no entró al bloque, así que su
/// `movement_target` no lo reserva: de lo contrario un tren detenido sobre su
/// propia señal la pondría en rojo y no podría salir (deadlock).
#[must_use]
pub(super) fn block_is_occupied_by_trains(
    map: &Map,
    vehicles: &[Vehicle],
    signal_tile: TileCoord,
    block: &[TileCoord],
) -> bool {
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        // Varios consists pueden estar apilados en Track::Depot. Mientras el
        // gate de salida siga cerrado no ocupan el bloque ferroviario exterior.
        if map.get_kind(v.pos) == Some(TileKind::RailDepot) && !v.depot_leave_cleared {
            continue;
        }
        if block_set.contains(&v.pos) {
            return true;
        }
        if !v.running || v.pos == signal_tile {
            continue;
        }
        if let Some(next) = v.movement_target()
            && block_set.contains(&next)
        {
            return true;
        }
    }
    false
}
