//! Pathfinder ferroviario estilo YAPF (`OpenTTD` `yapf_rail.cpp` / `yapf_costrail.hpp`).
//!
//! Estado de búsqueda = `(tesela, trackbit, exit_dir)` con convención de dirección de
//! [`crate::rail_signals`] (alineada con `TileOffsByDiagDir` de `OpenTTD`).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::{YapfSignalRouting, yapf_routing_signal};
use crate::station::is_rail_waypoint_tile;

use super::{
    TunnelWormholes, is_rail_network_tile, rail_bit_for_sides, station_entrance_faces_rail,
};

const RAIL_TB_X: u8 = 0x01;
const RAIL_TB_Y: u8 = 0x02;
const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;

/// Lado de entrada libre al iniciar la búsqueda.
const ENTRY_ANY: u8 = 4;

/// Coste base por tesela recta (`YAPF_TILE_LENGTH` normalizado).
const TILE_COST: u32 = 1;
/// Penalización por giro de 45° dentro de una tesela.
const CURVE45_PENALTY: u32 = 1;

/// Dirección de salida + pieza de vía (un solo bit de `m5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RailTrackdir {
    track: u8,
    exit_dir: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NodeKey {
    tile: TileCoord,
    track: u8,
    exit_dir: u8,
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct AstarNode {
    est_total: u32,
    key: NodeKey,
}

impl Ord for AstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est_total
            .cmp(&self.est_total)
            .then_with(|| other.key.tile.x.cmp(&self.key.tile.x))
            .then_with(|| other.key.tile.y.cmp(&self.key.tile.y))
            .then_with(|| other.key.track.cmp(&self.key.track))
            .then_with(|| other.key.exit_dir.cmp(&self.key.exit_dir))
    }
}

impl PartialOrd for AstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Offset de tesela vecina (`TileOffsByDiagDir` / `rail_signals`).
#[must_use]
const fn rail_diag_dir_offset(dir: u8) -> (i32, i32) {
    match dir & 3 {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    }
}

#[must_use]
const fn opposite_dir(d: u8) -> u8 {
    (d + 2) & 3
}

/// Convierte `DiagDir` YAPF → convención del pathfinder legado (solo E/O; N/S iguales).
#[must_use]
const fn yapf_dir_to_pathfinder(d: u8) -> u8 {
    match d & 3 {
        0 => 2,
        2 => 0,
        n => n,
    }
}

#[must_use]
const fn pathfinder_dir_to_yapf(d: u8) -> u8 {
    match d & 3 {
        0 => 2,
        2 => 0,
        n => n,
    }
}

#[must_use]
const fn rail_bits_touching_side(side: u8) -> u8 {
    match side & 3 {
        0 => 0x25,
        1 => 0x2A,
        2 => 0x19,
        _ => 0x16,
    }
}

#[must_use]
fn is_rail_station_tile(tile: &crate::map::Tile) -> bool {
    tile.kind == TileKind::Station && (tile.m6 >> 3).trailing_zeros() >= 4
}

#[must_use]
fn yapf_traversal_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Rail => {
            let tb = t.m5 & 0x3F;
            if tb == 0 { RAIL_TB_X } else { tb }
        }
        TileKind::RailTunnel | TileKind::RailBridge => RAIL_TB_CROSS,
        TileKind::RailDepot => {
            let mouth_pf = t.m5 & 0x03;
            rail_bits_touching_side(mouth_pf)
        }
        TileKind::Station if is_rail_station_tile(&t) || is_rail_waypoint_tile(&t) => {
            if t.m5 & 1 != 0 { RAIL_TB_Y } else { RAIL_TB_X }
        }
        _ => 0,
    }
}

/// Boca del depósito (`m5 & 3`) en convención pathfinder (como el A* legado).
#[must_use]
fn rail_depot_mouth_pf(map: &Map, c: TileCoord) -> Option<u8> {
    map.get(c)
        .filter(|t| t.kind == TileKind::RailDepot)
        .map(|t| t.m5 & 0x03)
}

#[must_use]
fn single_track_bits(tb: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..6 {
        let bit = 1_u8 << i;
        if tb & bit != 0 {
            out.push(bit);
        }
    }
    out
}

#[must_use]
fn trackdir_valid(tb: u8, track: u8, entry: u8, exit: u8) -> bool {
    if tb & track == 0 {
        return false;
    }
    let pf_entry = yapf_dir_to_pathfinder(entry);
    let pf_exit = yapf_dir_to_pathfinder(exit);
    if entry == ENTRY_ANY {
        return rail_bits_touching_side(pf_exit) & track != 0;
    }
    if rail_bit_for_sides(pf_entry, pf_exit) & track != 0 {
        return true;
    }
    pf_entry == opposite_dir(pf_exit)
        && rail_bits_touching_side(pf_entry) & track != 0
        && rail_bits_touching_side(pf_exit) & track != 0
}

#[must_use]
fn possible_trackdirs(tb: u8, entry: u8) -> Vec<RailTrackdir> {
    let mut out = Vec::new();
    for exit in 0..4u8 {
        for track in single_track_bits(tb) {
            if trackdir_valid(tb, track, entry, exit) {
                out.push(RailTrackdir {
                    track,
                    exit_dir: exit,
                });
            }
        }
    }
    out
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
        let (dx, dy) = rail_diag_dir_offset(dir);
        TileCoord::new(station.x + dx, station.y + dy) == track
            && station_entrance_faces_rail(map, station, yapf_dir_to_pathfinder(dir))
    })
}

#[must_use]
fn signal_step_cost(map: &Map, tile: TileCoord, td: RailTrackdir) -> Option<u32> {
    match yapf_routing_signal(map, tile, td.exit_dir) {
        YapfSignalRouting::DeadEnd => None,
        YapfSignalRouting::Clear => Some(TILE_COST),
        YapfSignalRouting::Penalty(p) => Some(TILE_COST + p),
    }
}

#[must_use]
fn curve_penalty(prev: RailTrackdir, next: RailTrackdir) -> u32 {
    if prev.exit_dir != next.exit_dir || prev.track != next.track {
        CURVE45_PENALTY
    } else {
        0
    }
}

#[must_use]
fn manhattan(a: TileCoord, b: TileCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

#[must_use]
fn start_states(map: &Map, from: TileCoord) -> Vec<(TileCoord, RailTrackdir)> {
    if let Some(mouth_pf) = rail_depot_mouth_pf(map, from) {
        let mouth = pathfinder_dir_to_yapf(mouth_pf);
        let tb = yapf_traversal_bits(map, from);
        return possible_trackdirs(tb, ENTRY_ANY)
            .into_iter()
            .filter(|td| td.exit_dir == mouth)
            .map(|td| (from, td))
            .collect();
    }
    if map.get(from).is_some_and(|t| is_rail_station_tile(&t)) {
        let mut out = Vec::new();
        for dir in 0..4u8 {
            let (dx, dy) = rail_diag_dir_offset(dir);
            let rail = TileCoord::new(from.x + dx, from.y + dy);
            if rail_station_entrance_links_track(map, from, rail) {
                let tb = yapf_traversal_bits(map, rail);
                for td in possible_trackdirs(tb, ENTRY_ANY) {
                    out.push((rail, td));
                }
            }
        }
        return out;
    }
    let tb = yapf_traversal_bits(map, from);
    possible_trackdirs(tb, ENTRY_ANY)
        .into_iter()
        .map(|td| (from, td))
        .collect()
}

struct SearchCtx<'a> {
    map: &'a Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&'a TunnelWormholes>,
    g_score: &'a mut HashMap<NodeKey, u32>,
    parent: &'a mut HashMap<NodeKey, NodeKey>,
    heap: &'a mut BinaryHeap<AstarNode>,
}

fn expand_neighbors(ctx: &mut SearchCtx<'_>, key: NodeKey, cur_g: u32, cur_td: RailTrackdir) {
    let map = ctx.map;
    let from = ctx.from;
    let to = ctx.to;
    let wormholes = ctx.wormholes;
    if rail_depot_mouth_pf(map, key.tile).is_some() && key.tile != from {
        return;
    }

    let (dx, dy) = rail_diag_dir_offset(cur_td.exit_dir);
    let next_tile = TileCoord::new(key.tile.x + dx, key.tile.y + dy);
    if map.get_kind(next_tile).is_some() {
        let entry = opposite_dir(cur_td.exit_dir);
        let pf_entry = yapf_dir_to_pathfinder(entry);
        let mut entered_depot_goal = false;
        if next_tile == to
            && let Some(mouth_pf) = rail_depot_mouth_pf(map, next_tile)
        {
            let mouth = pathfinder_dir_to_yapf(mouth_pf);
            if entry == mouth {
                let tentative = cur_g.saturating_add(TILE_COST);
                let next_key = NodeKey {
                    tile: next_tile,
                    track: RAIL_TB_X,
                    exit_dir: mouth,
                };
                if ctx.g_score.get(&next_key).is_none_or(|&g| tentative < g) {
                    ctx.g_score.insert(next_key, tentative);
                    ctx.parent.insert(next_key, key);
                    ctx.heap.push(AstarNode {
                        est_total: tentative + manhattan(next_tile, to),
                        key: next_key,
                    });
                }
                entered_depot_goal = true;
            }
        }
        if !entered_depot_goal {
            let next_tb = yapf_traversal_bits(map, next_tile);
            if next_tb & rail_bits_touching_side(pf_entry) != 0 {
                let can_enter = if let Some(mouth_pf) = rail_depot_mouth_pf(map, next_tile) {
                    entry == pathfinder_dir_to_yapf(mouth_pf)
                } else {
                    true
                };
                if can_enter {
                    for next_td in possible_trackdirs(next_tb, entry) {
                        let Some(step) = signal_step_cost(map, next_tile, next_td) else {
                            continue;
                        };
                        let tentative = cur_g + step + curve_penalty(cur_td, next_td);
                        let next_key = NodeKey {
                            tile: next_tile,
                            track: next_td.track,
                            exit_dir: next_td.exit_dir,
                        };
                        if ctx.g_score.get(&next_key).is_some_and(|&g| tentative >= g) {
                            continue;
                        }
                        ctx.g_score.insert(next_key, tentative);
                        ctx.parent.insert(next_key, key);
                        ctx.heap.push(AstarNode {
                            est_total: tentative + manhattan(next_tile, to),
                            key: next_key,
                        });
                    }
                }
            }
        }
    }

    if let Some(wh) = wormholes
        && map.get_kind(key.tile).is_some_and(is_rail_network_tile)
        && let Some(other) = wh.other_end(key.tile)
    {
        let ok = map.get_kind(other).is_some_and(is_rail_network_tile) || other == to;
        if ok {
            let wh_tb = yapf_traversal_bits(map, other);
            for next_td in possible_trackdirs(wh_tb, ENTRY_ANY) {
                let Some(step) = signal_step_cost(map, other, next_td) else {
                    continue;
                };
                let tentative = cur_g + step;
                let next_key = NodeKey {
                    tile: other,
                    track: next_td.track,
                    exit_dir: next_td.exit_dir,
                };
                if ctx.g_score.get(&next_key).is_none_or(|&g| tentative < g) {
                    ctx.g_score.insert(next_key, tentative);
                    ctx.parent.insert(next_key, key);
                    ctx.heap.push(AstarNode {
                        est_total: tentative + manhattan(other, to),
                        key: next_key,
                    });
                }
            }
        }
    }
}

#[must_use]
pub fn find_rail_path_yapf(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(vec![]);
    }

    let mut g_score: HashMap<NodeKey, u32> = HashMap::new();
    let mut parent: HashMap<NodeKey, NodeKey> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut ctx = SearchCtx {
        map,
        from,
        to,
        wormholes,
        g_score: &mut g_score,
        parent: &mut parent,
        heap: &mut heap,
    };

    let mut closed: HashSet<NodeKey> = HashSet::new();

    for (tile, td) in start_states(map, from) {
        let key = NodeKey {
            tile,
            track: td.track,
            exit_dir: td.exit_dir,
        };
        let Some(cost) = signal_step_cost(map, tile, td) else {
            continue;
        };
        ctx.g_score.insert(key, cost);
        ctx.parent.insert(key, key);
        ctx.heap.push(AstarNode {
            est_total: cost + manhattan(tile, to),
            key,
        });
    }

    while let Some(AstarNode { est_total, key }) = ctx.heap.pop() {
        if closed.contains(&key) {
            continue;
        }
        let g = ctx.g_score[&key];
        if est_total > g.saturating_add(manhattan(key.tile, to)) {
            continue;
        }
        closed.insert(key);

        if key.tile == to {
            return Some(reconstruct(key, ctx.parent));
        }

        let cur_td = RailTrackdir {
            track: key.track,
            exit_dir: key.exit_dir,
        };
        expand_neighbors(&mut ctx, key, g, cur_td);
    }
    None
}

fn reconstruct(goal: NodeKey, parent: &HashMap<NodeKey, NodeKey>) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = goal;
    // Los nodos raíz tienen `parent[key] == k` (p. ej. vía adyacente si `from` es estación).
    while parent.get(&cur) != Some(&cur) {
        path.push(cur.tile);
        cur = parent[&cur];
    }
    path.reverse();
    path
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::map::TileKind;
    use crate::parity::{
        TRAIN_DUAL_STATION_A, TRAIN_DUAL_STATION_B, TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y,
        build_train_supply_dual,
    };
    use crate::rail_signals::{
        RAIL_TILE_SIGNALS, YapfSignalRouting, encode_block_signal_on_track_with_variant,
        yapf_routing_signal,
    };

    #[test]
    fn yapf_one_way_signal_blocks_reverse_direction() {
        let mut map = Map::new_flat(8, 4, 0);
        for x in 0..4 {
            map.set_kind(TileCoord::new(x, 0), TileKind::Rail)
                .expect("rail");
            let mut t = map.get(TileCoord::new(x, 0)).expect("tile");
            t.m5 = RAIL_TB_X;
            map.set_tile(TileCoord::new(x, 0), t).expect("tb");
        }
        let sig = TileCoord::new(1, 0);
        let (m2, m3, m3hi) = encode_block_signal_on_track_with_variant(RAIL_TB_X, 1);
        let mut t = map.get(sig).expect("sig tile");
        t.m5 = RAIL_TB_X | (RAIL_TILE_SIGNALS << 6);
        t.m2 = m2;
        t.m3 = m3;
        t.m3hi = m3hi;
        map.set_tile(sig, t).expect("signal");

        assert_eq!(
            yapf_routing_signal(&map, sig, 0),
            YapfSignalRouting::Clear,
            "+x permitido"
        );
        assert_eq!(
            yapf_routing_signal(&map, sig, 2),
            YapfSignalRouting::DeadEnd,
            "-x bloqueado por señal unidireccional"
        );
    }

    #[test]
    fn yapf_reaches_depot_from_east_station() {
        use crate::GameState;
        use crate::command::{Command, apply_command};
        use crate::pathfinder::{PathNetwork, find_path};

        let mut state = GameState::new(24, 18);
        for x in [12, 13, 15, 16, 17, 18, 19, 20, 22] {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 15))).expect("vía");
        }
        apply_command(
            &mut state,
            &Command::PlaceRailStation(TileCoord::new(14, 15), 2),
        )
        .expect("estación oeste");
        apply_command(
            &mut state,
            &Command::PlaceRailStation(TileCoord::new(21, 15), 0),
        )
        .expect("estación este");
        apply_command(
            &mut state,
            &Command::PlaceRailDepotDir(TileCoord::new(12, 16), 3),
        )
        .expect("depósito");

        let east = TileCoord::new(21, 15);
        let depot = TileCoord::new(12, 16);
        let path = find_path(&state.map, east, depot, PathNetwork::Rail);
        assert!(path.is_some(), "estación este → depósito: {path:?}");
    }

    #[test]
    fn yapf_dual_return_uses_return_track_not_outbound() {
        let state = build_train_supply_dual();
        let path =
            find_rail_path_yapf(&state.map, TRAIN_DUAL_STATION_A, TRAIN_DUAL_STATION_B, None)
                .expect("ruta A → B");
        assert!(
            path.iter()
                .all(|c| c.y != TRAIN_DUAL_TRACK_RET_Y || c.x == 3 || c.x == 10),
            "ida no debe usar carril de vuelta salvo conectores: {path:?}"
        );

        let path =
            find_rail_path_yapf(&state.map, TRAIN_DUAL_STATION_B, TRAIN_DUAL_STATION_A, None)
                .expect("ruta B → A");
        assert!(
            path.iter().any(|c| c.y == TRAIN_DUAL_TRACK_RET_Y),
            "debe usar vía de vuelta y={TRAIN_DUAL_TRACK_RET_Y}: {path:?}"
        );
        assert!(
            !path
                .iter()
                .any(|c| c.y == TRAIN_DUAL_TRACK_OUT_Y && c.x >= 5 && c.x <= 9),
            "no debe cruzar señales unidireccionales de ida en sentido contrario"
        );
    }
}
