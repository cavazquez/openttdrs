//! Pathfinder ferroviario estilo YAPF (`OpenTTD` `yapf_rail.cpp` / `yapf_costrail.hpp`).
//!
//! Estado de búsqueda = `(tesela, trackbit, exit_dir)` con convención de dirección de
//! [`crate::rail_signals`] (alineada con `TileOffsByDiagDir` de `OpenTTD`).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::map::{
    Map, RAIL_TB_X, RAIL_TB_Y, TileCoord, TileKind, opposite_diag_dir as opposite_dir,
    rail_bit_for_sides, rail_bits_touching_side,
    rail_signal_diag_dir_offset as rail_diag_dir_offset, rail_traversal_bits,
};
use crate::rail_pbs::{YAPF_RESERVATION_CROSS_PENALTY, tile_track_reserved_by_map};
use crate::rail_signals::{YapfSignalRouting, yapf_routing_signal};

use super::{
    TunnelWormholes, is_rail_network_tile, is_rail_station_tile, station_entrance_faces_rail,
};

/// Reexport de la escala `OpenTTD` (`pathfinder_type.h`).
pub use crate::rail_pbs::{YAPF_TILE_CORNER_LENGTH, YAPF_TILE_LENGTH};

/// Lado de entrada libre al iniciar la búsqueda.
const ENTRY_ANY: u8 = 4;

/// Penalización por giro de 45° (`rail_curve45_penalty` = 1×tesela).
const CURVE45_PENALTY: u32 = YAPF_TILE_LENGTH;

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
fn yapf_traversal_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    if t.kind == TileKind::RailDepot {
        let mouth_pf = t.m5 & 0x03;
        return rail_bits_touching_side(mouth_pf);
    }
    rail_traversal_bits(map, c)
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

/// `IsDiagonalTrack`: X/Y son diagonales; Upper/Lower/Left/Right son esquinas.
#[must_use]
const fn is_diagonal_track(track: u8) -> bool {
    matches!(track, RAIL_TB_X | RAIL_TB_Y)
}

#[must_use]
const fn tile_base_cost(track: u8) -> u32 {
    if is_diagonal_track(track) {
        YAPF_TILE_LENGTH
    } else {
        YAPF_TILE_CORNER_LENGTH
    }
}

#[must_use]
fn reservation_step_penalty(map: &Map, tile: TileCoord, track: u8) -> u32 {
    if !tile_track_reserved_by_map(map, tile, track) {
        return 0;
    }
    let base = YAPF_RESERVATION_CROSS_PENALTY;
    if is_diagonal_track(track) {
        base
    } else {
        (base * YAPF_TILE_CORNER_LENGTH) / YAPF_TILE_LENGTH
    }
}

#[must_use]
fn signal_step_cost(map: &Map, tile: TileCoord, td: RailTrackdir) -> Option<u32> {
    let base = tile_base_cost(td.track);
    match yapf_routing_signal(map, tile, td.exit_dir) {
        YapfSignalRouting::DeadEnd => None,
        YapfSignalRouting::Clear => Some(base + reservation_step_penalty(map, tile, td.track)),
        YapfSignalRouting::Penalty(p) => {
            Some(base + p + reservation_step_penalty(map, tile, td.track))
        }
    }
}

/// Coste de paso con caché de segmento básica (por búsqueda; el mapa no muda).
fn cached_signal_step_cost(
    map: &Map,
    tile: TileCoord,
    td: RailTrackdir,
    cache: &mut HashMap<NodeKey, Option<u32>>,
) -> Option<u32> {
    let key = NodeKey {
        tile,
        track: td.track,
        exit_dir: td.exit_dir,
    };
    if let Some(&cached) = cache.get(&key) {
        return cached;
    }
    let cost = signal_step_cost(map, tile, td);
    cache.insert(key, cost);
    cost
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
    (a.x.abs_diff(b.x) + a.y.abs_diff(b.y)).saturating_mul(YAPF_TILE_LENGTH)
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
    /// Si `Some`, solo expandir a teselas compatibles con este tipo.
    required: Option<crate::rail_type::RailType>,
    g_score: &'a mut HashMap<NodeKey, u32>,
    parent: &'a mut HashMap<NodeKey, NodeKey>,
    heap: &'a mut BinaryHeap<AstarNode>,
    /// Caché de coste de segmento/paso por `(tile, track, exit_dir)` dentro de la búsqueda.
    step_cache: &'a mut HashMap<NodeKey, Option<u32>>,
}

fn tile_ok_for_required(
    map: &Map,
    tile: TileCoord,
    required: Option<crate::rail_type::RailType>,
) -> bool {
    let Some(req) = required else {
        return true;
    };
    map.get(tile)
        .is_some_and(|t| crate::rail_type::tile_usable_by_rail_type(t, req))
}

#[allow(clippy::too_many_lines)]
fn expand_neighbors(ctx: &mut SearchCtx<'_>, key: NodeKey, cur_g: u32, cur_td: RailTrackdir) {
    let map = ctx.map;
    let from = ctx.from;
    let to = ctx.to;
    let wormholes = ctx.wormholes;
    let required = ctx.required;
    if rail_depot_mouth_pf(map, key.tile).is_some() && key.tile != from {
        return;
    }

    let (dx, dy) = rail_diag_dir_offset(cur_td.exit_dir);
    let next_tile = TileCoord::new(key.tile.x + dx, key.tile.y + dy);
    if map.get_kind(next_tile).is_some() {
        if !tile_ok_for_required(map, next_tile, required) && next_tile != to {
            return;
        }
        let entry = opposite_dir(cur_td.exit_dir);
        let pf_entry = yapf_dir_to_pathfinder(entry);
        let mut entered_depot_goal = false;
        if next_tile == to
            && let Some(mouth_pf) = rail_depot_mouth_pf(map, next_tile)
        {
            let mouth = pathfinder_dir_to_yapf(mouth_pf);
            if entry == mouth {
                let tentative = cur_g.saturating_add(YAPF_TILE_LENGTH);
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
                        let Some(step) =
                            cached_signal_step_cost(map, next_tile, next_td, ctx.step_cache)
                        else {
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
        let ok = other == to
            || (map.get_kind(other).is_some_and(is_rail_network_tile)
                && tile_ok_for_required(map, other, required));
        if ok {
            let wh_tb = yapf_traversal_bits(map, other);
            for next_td in possible_trackdirs(wh_tb, ENTRY_ANY) {
                let Some(step) = cached_signal_step_cost(map, other, next_td, ctx.step_cache)
                else {
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

/// Extiende un path parcial hacia `to` (YAPF incremental).
///
/// Si `from` ya tiene un path parcial, busca solo desde el último tile hacia el
/// destino y concatena. Útil cuando el tren vacía `path` cerca del destino.
#[must_use]
pub fn extend_rail_path_yapf(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    existing: &[TileCoord],
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(existing.to_vec());
    }
    let start = existing.last().copied().unwrap_or(from);
    if start == to {
        return Some(existing.to_vec());
    }
    let extension = find_rail_path_yapf(map, start, to, wormholes)?;
    let mut out = existing.to_vec();
    for tile in extension {
        if out.last() == Some(&tile) {
            continue;
        }
        out.push(tile);
    }
    Some(out)
}

/// Siguiente trackdir sugerido desde `from` hacia `to` (estado YAPF, no solo tesela).
#[must_use]
pub fn next_rail_trackdir_yapf(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<(TileCoord, u8, u8)> {
    let path = find_rail_path_yapf(map, from, to, wormholes)?;
    let next = path.first().copied()?;
    let track = crate::rail_pbs::track_on_departure_tile(map, from, next)
        .or_else(|| crate::rail_pbs::track_for_rail_step(map, from, next))?;
    let exit_dir = crate::rail_signals::dir_from_to(from, next)?;
    Some((next, track, exit_dir))
}

#[must_use]
pub fn find_rail_path_yapf(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    find_rail_path_yapf_for_type(map, from, to, wormholes, None)
}

/// Como [`find_rail_path_yapf`], restringiendo aristas al `RailType` del motor.
#[must_use]
pub fn find_rail_path_yapf_for_type(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
    required: Option<crate::rail_type::RailType>,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(vec![]);
    }

    let mut g_score: HashMap<NodeKey, u32> = HashMap::new();
    let mut parent: HashMap<NodeKey, NodeKey> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut step_cache: HashMap<NodeKey, Option<u32>> = HashMap::new();
    let mut ctx = SearchCtx {
        map,
        from,
        to,
        wormholes,
        required,
        g_score: &mut g_score,
        parent: &mut parent,
        heap: &mut heap,
        step_cache: &mut step_cache,
    };

    let mut closed: HashSet<NodeKey> = HashSet::new();

    for (tile, td) in start_states(map, from) {
        if !tile_ok_for_required(map, tile, required) && tile != from && tile != to {
            continue;
        }
        let key = NodeKey {
            tile,
            track: td.track,
            exit_dir: td.exit_dir,
        };
        let Some(cost) = cached_signal_step_cost(map, tile, td, ctx.step_cache) else {
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
            return Some(reconstruct(key, ctx.parent, from));
        }

        let cur_td = RailTrackdir {
            track: key.track,
            exit_dir: key.exit_dir,
        };
        expand_neighbors(&mut ctx, key, g, cur_td);
    }
    None
}

fn reconstruct(
    goal: NodeKey,
    parent: &HashMap<NodeKey, NodeKey>,
    from: TileCoord,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = goal;
    // Los nodos raíz tienen `parent[key] == key` (p. ej. vía/andén adyacente si
    // `from` es estación: el seed no está en `from`).
    while parent.get(&cur) != Some(&cur) {
        path.push(cur.tile);
        cur = parent[&cur];
    }
    // Incluir el hop raíz cuando la búsqueda arrancó fuera de `from`.
    if cur.tile != from {
        path.push(cur.tile);
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
    fn yapf_station_adjacent_platform_includes_dest_tile() {
        let mut map = Map::new_flat(8, 8, 0);
        for y in 1..=4 {
            let c = TileCoord::new(2, y);
            map.set_kind(c, TileKind::Station).expect("station");
            let mut t = map.get(c).expect("tile");
            t.m5 = 0x01; // eje Y
            t.m6 = 0; // StationType::Rail
            map.set_tile(c, t).expect("set");
        }
        let path = find_rail_path_yapf(&map, TileCoord::new(2, 2), TileCoord::new(2, 3), None)
            .expect("andén adyacente");
        assert_eq!(path, vec![TileCoord::new(2, 3)]);
        let path = find_rail_path_yapf(&map, TileCoord::new(2, 2), TileCoord::new(2, 4), None)
            .expect("dos teselas de andén");
        assert_eq!(
            path,
            vec![TileCoord::new(2, 3), TileCoord::new(2, 4)],
            "no debe saltar el andén intermedio"
        );
    }

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
    fn next_rail_trackdir_returns_first_step() {
        let mut state = crate::GameState::new(12, 4);
        for x in 0..=6 {
            crate::command::apply_command(
                &mut state,
                &crate::command::Command::PlaceRail(TileCoord::new(x, 1)),
            )
            .expect("vía");
        }
        let step =
            next_rail_trackdir_yapf(&state.map, TileCoord::new(1, 1), TileCoord::new(5, 1), None)
                .expect("trackdir");
        assert_eq!(step.0, TileCoord::new(2, 1));
        assert_ne!(step.1, 0);
    }

    #[test]
    fn extend_rail_path_concatenates() {
        let mut state = crate::GameState::new(12, 4);
        for x in 0..=8 {
            crate::command::apply_command(
                &mut state,
                &crate::command::Command::PlaceRail(TileCoord::new(x, 1)),
            )
            .expect("vía");
        }
        let partial = vec![TileCoord::new(2, 1), TileCoord::new(3, 1)];
        let full = extend_rail_path_yapf(
            &state.map,
            TileCoord::new(1, 1),
            TileCoord::new(7, 1),
            &partial,
            None,
        )
        .expect("extend");
        assert!(full.starts_with(&partial));
        assert!(full.iter().any(|c| *c == TileCoord::new(7, 1)));
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

    #[test]
    fn yapf_penalizes_crossing_foreign_reservation() {
        use crate::map::RAIL_TB_UPPER;
        use crate::rail_pbs::{
            ReservedRailStep, YAPF_RESERVATION_CROSS_PENALTY, encode_rail_reservation_to_m2_hi,
        };

        let mut map = Map::new_flat(10, 4, 0);
        for x in 0..8 {
            map.set_kind(TileCoord::new(x, 1), TileKind::Rail)
                .expect("rail");
            let mut t = map.get(TileCoord::new(x, 1)).expect("tile");
            t.m5 = RAIL_TB_X;
            map.set_tile(TileCoord::new(x, 1), t).expect("tb");
        }
        // Reserva ajena en el medio del corredor.
        let mid = TileCoord::new(4, 1);
        let mut t = map.get(mid).expect("mid");
        t.m2_hi = encode_rail_reservation_to_m2_hi(RAIL_TB_X);
        map.set_tile(mid, t).expect("res");

        assert_eq!(
            reservation_step_penalty(&map, mid, RAIL_TB_X),
            YAPF_RESERVATION_CROSS_PENALTY
        );
        assert_eq!(
            reservation_step_penalty(&map, TileCoord::new(3, 1), RAIL_TB_X),
            0
        );
        assert_eq!(tile_base_cost(RAIL_TB_X), YAPF_TILE_LENGTH);
        assert_eq!(tile_base_cost(RAIL_TB_UPPER), YAPF_TILE_CORNER_LENGTH);
        assert_eq!(
            reservation_step_penalty(&map, mid, RAIL_TB_UPPER),
            0,
            "sin reserva en UPPER"
        );
        let _ = ReservedRailStep::new(mid, RAIL_TB_X);
    }

    #[test]
    fn yapf_cost_scale_matches_openttd_defaults() {
        assert_eq!(YAPF_TILE_LENGTH, 100);
        assert_eq!(YAPF_TILE_CORNER_LENGTH, 71);
        assert_eq!(crate::rail_signals::YAPF_RED_SIGNAL_PENALTY, 1000);
        assert_eq!(crate::rail_pbs::YAPF_RESERVATION_CROSS_PENALTY, 300);
        assert_eq!(CURVE45_PENALTY, 100);
    }
}
