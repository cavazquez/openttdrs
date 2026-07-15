//! A* direccional ferroviario legacy (pre-YAPF). Conservado para referencia/tests futuros.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::map::{
    Map, TileCoord, TileKind, opposite_diag_dir as opposite_dir, rail_bit_for_sides,
    rail_bits_touching_side, rail_traversal_bits,
};

use super::astar::manhattan;
use super::diag_dir_offset;
use super::network::{TunnelWormholes, is_rail_network_tile, is_rail_station_tile};
use super::station_entrance_faces_rail;

/// Lado de entrada «libre»: el tren parte (o se rematerializa) sin restricción de giro.
const SIDE_ANY: u8 = 4;

/// Boca del depósito de vía (`m5 & 3`) si la tesela es un depósito.
#[must_use]
fn rail_depot_mouth(map: &Map, c: TileCoord) -> Option<u8> {
    map.get(c)
        .filter(|t| t.kind == TileKind::RailDepot)
        .map(|t| t.m5 & 0x03)
}

#[must_use]
fn rail_station_entrance_links_track(map: &Map, station: TileCoord, track: TileCoord) -> bool {
    let Some(tile) = map.get(station) else {
        return false;
    };
    if !is_rail_station_tile(&tile) {
        return false;
    }
    if !map
        .get_kind(track)
        .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
    {
        return false;
    }
    if station.x.abs_diff(track.x) + station.y.abs_diff(track.y) != 1 {
        return false;
    }
    (0..4).any(|dir| {
        let (dx, dy) = diag_dir_offset(dir);
        TileCoord::new(station.x + dx, station.y + dy) == track
            && station_entrance_faces_rail(map, station, dir)
    })
}

/// A* direccional para vía: estado = (tesela, lado de entrada). Un giro dentro
/// de una tesela solo es válido si existe el trackbit que conecta el lado de
/// entrada con el de salida (piezas X/Y/curvas de `OpenTTD`). Los depósitos solo
/// conectan por su boca y las plataformas solo se usan como origen.
#[allow(dead_code)]
pub(super) fn find_rail_path_legacy_astar(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    let mut g_score: HashMap<(TileCoord, u8), u32> = HashMap::new();
    let mut parent: HashMap<(TileCoord, u8), (TileCoord, u8)> = HashMap::new();
    let mut heap = BinaryHeap::new();

    let start = (from, SIDE_ANY);
    g_score.insert(start, 0);
    parent.insert(start, start);
    heap.push(RailAstarNode {
        est_total: manhattan(from, to),
        pos: from,
        in_side: SIDE_ANY,
    });

    while let Some(RailAstarNode {
        est_total: _,
        pos: cur,
        in_side,
    }) = heap.pop()
    {
        if cur == to {
            return Some(reconstruct_rail(from, (cur, in_side), &parent));
        }
        let cur_g = g_score[&(cur, in_side)];
        let cur_is_start = cur == from && in_side == SIDE_ANY;

        // Un tren que llega a un depósito termina ahí; solo se sale por la boca.
        let depot_mouth = rail_depot_mouth(map, cur);
        if depot_mouth.is_some() && !cur_is_start {
            continue;
        }
        let station_start = cur_is_start && map.get(cur).is_some_and(|t| is_rail_station_tile(&t));
        let cur_bits = rail_traversal_bits(map, cur);

        for out in 0..4u8 {
            let exit_allowed = if let Some(mouth) = depot_mouth {
                out == mouth
            } else if station_start {
                let (dx, dy) = diag_dir_offset(out);
                rail_station_entrance_links_track(map, cur, TileCoord::new(cur.x + dx, cur.y + dy))
            } else if in_side == SIDE_ANY {
                cur_bits & rail_bits_touching_side(out) != 0
            } else {
                cur_bits & rail_bit_for_sides(in_side, out) != 0
            };
            if !exit_allowed {
                continue;
            }
            let (dx, dy) = diag_dir_offset(out);
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if map.get_kind(next).is_none() {
                continue;
            }
            let entry = opposite_dir(out);
            let next_in = if let Some(mouth) = rail_depot_mouth(map, next) {
                if entry != mouth {
                    continue; // al depósito solo se entra por la boca
                }
                entry
            } else if station_start {
                // El enlace plataforma → vía deja al tren sobre la vía sin
                // restricción de giro (abstracción de estación).
                SIDE_ANY
            } else if rail_traversal_bits(map, next) & rail_bits_touching_side(entry) != 0 {
                entry
            } else {
                continue;
            };
            let tentative = cur_g + 1;
            let key = (next, next_in);
            if g_score.get(&key).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(key, tentative);
            parent.insert(key, (cur, in_side));
            heap.push(RailAstarNode {
                est_total: tentative + manhattan(next, to),
                pos: next,
                in_side: next_in,
            });
        }

        // Wormholes (túneles JGR): el túnel es recto, conserva el lado de entrada.
        if let Some(wh) = wormholes
            && map.get_kind(cur).is_some_and(is_rail_network_tile)
            && let Some(other) = wh.other_end(cur)
        {
            let ok = map.get_kind(other).is_some_and(is_rail_network_tile) || other == to;
            let tentative = cur_g + 1;
            let key = (other, in_side);
            if ok && g_score.get(&key).is_none_or(|&g| tentative < g) {
                g_score.insert(key, tentative);
                parent.insert(key, (cur, in_side));
                heap.push(RailAstarNode {
                    est_total: tentative + manhattan(other, to),
                    pos: other,
                    in_side,
                });
            }
        }
    }
    None
}

fn reconstruct_rail(
    from: TileCoord,
    goal: (TileCoord, u8),
    parent: &HashMap<(TileCoord, u8), (TileCoord, u8)>,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = goal;
    while cur.0 != from {
        path.push(cur.0);
        cur = parent[&cur];
    }
    path.reverse();
    path
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct RailAstarNode {
    est_total: u32,
    pos: TileCoord,
    in_side: u8,
}

impl Ord for RailAstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est_total
            .cmp(&self.est_total)
            .then_with(|| other.pos.x.cmp(&self.pos.x))
            .then_with(|| other.pos.y.cmp(&self.pos.y))
            .then_with(|| other.in_side.cmp(&self.in_side))
    }
}

impl PartialOrd for RailAstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
