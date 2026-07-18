//! A* genérico (road/tram) y primitivas compartidas.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::bridge_spec::road_bridge_other_end;
use crate::map::{Map, TileCoord, TileKind};

use super::network::{
    PathNetwork, TunnelWormholes, is_network_tile, is_road_stop_station_tile, tiles_connected,
};

#[derive(Copy, Clone, Eq, PartialEq)]
pub(super) struct AstarNode {
    pub(super) est_total: u32,
    pub(super) pos: TileCoord,
}

impl Ord for AstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est_total
            .cmp(&self.est_total)
            .then_with(|| other.pos.x.cmp(&self.pos.x))
            .then_with(|| other.pos.y.cmp(&self.pos.y))
    }
}

impl PartialOrd for AstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[must_use]
pub(super) fn manhattan(a: TileCoord, b: TileCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

#[must_use]
pub(super) const fn step_cost(_from: TileCoord, _to: TileCoord) -> u32 {
    1
}

pub(super) fn reconstruct(
    from: TileCoord,
    to: TileCoord,
    parent: &HashMap<TileCoord, TileCoord>,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = to;
    while cur != from {
        path.push(cur);
        cur = parent[&cur];
    }
    path.reverse();
    path
}

/// A* road/tram (con wormholes opcionales). Rail/air/water se despachan en `mod`.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub(super) fn find_road_or_tram_path_with_wormholes(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    debug_assert!(matches!(network, PathNetwork::Road | PathNetwork::Tram));

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
            let reachable = if is_network_tile(map, next, next_kind, network) {
                tiles_connected(map, cur, next, network)
                    // Destino parada road: entrar desde cualquier tesela de red adyacente
                    // (la boca m3 puede mirar a otro lado tras el corredor IA).
                    || (network == PathNetwork::Road
                        && next == to
                        && is_road_stop_station_tile(map, next)
                        && is_network_tile(map, cur, cur_kind, network))
            } else if network == PathNetwork::Road && next == to {
                // Paradas bus/camión: la tesela de destino puede no ser carretera pura.
                is_network_tile(map, cur, cur_kind, network)
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
            let f = tentative + manhattan(next, to);
            heap.push(AstarNode {
                est_total: f,
                pos: next,
            });
        }
        if let Some(wh) = wormholes {
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            if is_network_tile(map, cur, cur_kind, network)
                && let Some(other) = wh.other_end(cur)
            {
                let other_kind = map.get_kind(other).unwrap_or(TileKind::Grass);
                let reachable = is_network_tile(map, other, other_kind, network) || other == to;
                let tentative = cur_g + step_cost(cur, other);
                if reachable && g_score.get(&other).is_none_or(|&g| tentative < g) {
                    g_score.insert(other, tentative);
                    parent.insert(other, cur);
                    heap.push(AstarNode {
                        est_total: tentative + manhattan(other, to),
                        pos: other,
                    });
                }
            }
        }
        // Puente road: salto rampa→rampa sobre vano Water (#187).
        if network == PathNetwork::Road
            && map.get_kind(cur) == Some(TileKind::RoadBridge)
            && let Some(other) = road_bridge_other_end(map, cur)
        {
            let tentative = cur_g + step_cost(cur, other);
            if g_score.get(&other).is_none_or(|&g| tentative < g) {
                g_score.insert(other, tentative);
                parent.insert(other, cur);
                heap.push(AstarNode {
                    est_total: tentative + manhattan(other, to),
                    pos: other,
                });
            }
        }
    }
    None
}
