//! A* sobre teselas de agua.

use std::collections::{BinaryHeap, HashMap};

use crate::map::{Map, TileCoord, TileKind};
use crate::ship_movement::water_tiles_connected;

use super::astar::{AstarNode, manhattan, reconstruct, step_cost};
use super::network::{PathNetwork, is_network_tile};

/// A* sobre teselas de agua (vecinos ortogonales conectados).
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub(super) fn find_water_path(map: &Map, from: TileCoord, to: TileCoord) -> Option<Vec<TileCoord>> {
    let (mw, mh) = map.dimensions();
    let mut g_score: HashMap<TileCoord, u32> = HashMap::new();
    let mut parent: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut heap = BinaryHeap::new();

    g_score.insert(from, 0);
    parent.insert(from, from);
    heap.push(AstarNode {
        est_total: manhattan(from, to),
        pos: from,
    });

    let dirs = [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)];

    while let Some(AstarNode {
        est_total: _,
        pos: cur,
    }) = heap.pop()
    {
        if cur == to {
            return Some(reconstruct(from, to, &parent));
        }

        let cur_g = g_score[&cur];
        for (dx, dy) in dirs {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw as i32 || next.y >= mh as i32 {
                continue;
            }
            let next_kind = map.get_kind(next).unwrap_or(TileKind::Grass);
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            let reachable = if is_network_tile(map, next, next_kind, PathNetwork::Water) {
                water_tiles_connected(map, cur, next)
            } else if next == to {
                is_network_tile(map, cur, cur_kind, PathNetwork::Water)
            } else {
                false
            };
            if !reachable {
                continue;
            }

            let tentative = cur_g + step_cost(cur, next);
            if g_score.get(&next).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(next, tentative);
            parent.insert(next, cur);
            heap.push(AstarNode {
                est_total: tentative + manhattan(next, to),
                pos: next,
            });
        }
    }
    None
}
