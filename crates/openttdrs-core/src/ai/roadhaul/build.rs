//! Construcción de carretera, paradas bus y depósito para `RoadHaul`.

use std::collections::{HashMap, HashSet};

use crate::GameState;
use crate::ROAD_PLACE_FORCE_AXIS;
use crate::bridge_spec::BridgeType;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path, find_road_build_path};

use super::plan::BusPlan;
use crate::ai::build_queue::{AiBuildFinish, AiBuildQueue, record_build_commands};

/// Planifica en un clon (grabando comandos) y devuelve la cola sin mutar `state`.
pub(super) fn plan_bus_line_queue(
    state: &GameState,
    ai_id: CompanyId,
    plan: BusPlan,
) -> Option<AiBuildQueue> {
    let mut endpoints = None;
    let commands = record_build_commands(state, |tmp| {
        endpoints = build_bus_line(tmp, ai_id, plan);
    });
    let (stop_a, stop_b, depot) = endpoints?;
    if commands.is_empty() {
        return None;
    }
    Some(AiBuildQueue {
        company: ai_id,
        commands,
        finish: AiBuildFinish::RoadHaul {
            stop_a,
            stop_b,
            depot,
        },
    })
}

pub(super) fn build_bus_line(
    state: &mut GameState,
    ai_id: CompanyId,
    plan: BusPlan,
) -> Option<(TileCoord, TileCoord, TileCoord)> {
    let stop_a = pick_stop_tile(state, plan.town_a, plan.town_b)?;
    let stop_b = pick_stop_tile(state, plan.town_b, plan.town_a)?;
    if stop_a == stop_b {
        return None;
    }

    let mut depot_out = None;
    with_ai_active(state, ai_id, |state| {
        // Corredor primero (extremos aún hierba); luego paradas mirando la calzada (#190/#187).
        place_road_corridor(state, stop_a, stop_b);
        if !place_bus_stop_facing_road(state, stop_a, ai_id) {
            return;
        }
        if !place_bus_stop_facing_road(state, stop_b, ai_id) {
            return;
        }
        link_stops_to_corridor(state, stop_a, stop_b);
        depot_out = try_place_road_depot_near(state, stop_a);
    });

    let depot = depot_out?;
    let has_a = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == stop_a);
    let has_b = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == stop_b);
    if has_a && has_b {
        Some((stop_a, stop_b, depot))
    } else {
        None
    }
}

fn with_ai_active(state: &mut GameState, ai_id: CompanyId, f: impl FnOnce(&mut GameState)) {
    let prev_active = state.active_company;
    state.active_company = ai_id;
    if let Some(c) = state.companies.get(ai_id.index()) {
        state.economy = c.economy;
        state.company_colour = c.colour;
    }
    f(state);
    state.active_company = prev_active;
    state.sync_mirrors_from_active();
}

fn pick_stop_tile(state: &GameState, near: TileCoord, toward: TileCoord) -> Option<TileCoord> {
    let mut cands = Vec::new();
    for (dx, dy) in [
        (-1, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (-2, 0),
        (2, 0),
        (0, -2),
        (0, 2),
    ] {
        let c = TileCoord::new(near.x + dx, near.y + dy);
        if state.map.get(c).is_none() {
            continue;
        }
        if !matches!(
            state.map.get_kind(c),
            Some(TileKind::Grass | TileKind::Forest | TileKind::Road)
        ) {
            continue;
        }
        cands.push(c);
    }
    cands.sort_by_key(|c| c.x.abs_diff(toward.x) + c.y.abs_diff(toward.y));
    cands.into_iter().next()
}

/// A* de construcción (evita agua) o L Manhattan + puentes. Extremos no se convierten en Road.
fn place_road_corridor(state: &mut GameState, from: TileCoord, to: TileCoord) {
    if let Some(path) = find_road_build_path(&state.map, from, to)
        && path.len() >= 2
    {
        place_road_along_tiles(state, &path, from, to);
        if corridor_mids_connected(state, &path) {
            return;
        }
    }
    let tiles = manhattan_polyline(from, to);
    place_road_along_tiles(state, &tiles, from, to);
    if !corridor_mids_connected(state, &tiles) {
        repair_road_stop_neighbors(state, from, to);
        place_road_along_tiles(state, &tiles, from, to);
    }
}

fn corridor_mids_connected(state: &GameState, path: &[TileCoord]) -> bool {
    if path.len() < 2 {
        return false;
    }
    let a = path[1];
    let b = path[path.len() - 2];
    if a == b {
        return matches!(
            state.map.get_kind(a),
            Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel)
        );
    }
    find_path(&state.map, a, b, PathNetwork::Road).is_some()
}

fn manhattan_polyline(from: TileCoord, to: TileCoord) -> Vec<TileCoord> {
    let step_x = (to.x - from.x).signum();
    let step_y = (to.y - from.y).signum();
    let mut c = from;
    let mut tiles = vec![from];
    while c.x != to.x {
        c = TileCoord::new(c.x + step_x, c.y);
        tiles.push(c);
    }
    while c.y != to.y {
        c = TileCoord::new(c.x, c.y + step_y);
        tiles.push(c);
    }
    tiles
}

fn place_road_along_tiles(
    state: &mut GameState,
    tiles: &[TileCoord],
    from: TileCoord,
    to: TileCoord,
) {
    if tiles.len() < 2 {
        return;
    }
    let skip = place_bridges_over_water_spans(state, tiles);
    let mut bits_acc: HashMap<TileCoord, u8> = HashMap::new();
    for pair in tiles.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let bits_a = road_bits_toward(a, b);
        let bits_b = road_bits_toward(b, a);
        if a != from && a != to && bits_a != 0 && !tile_skips_road_bits(state, a, &skip) {
            *bits_acc.entry(a).or_default() |= bits_a;
        }
        if b != from && b != to && bits_b != 0 && !tile_skips_road_bits(state, b, &skip) {
            *bits_acc.entry(b).or_default() |= bits_b;
        }
    }
    for (c, bits) in bits_acc {
        let cmd = if bits == 0x0A || bits == 0x05 {
            bits | ROAD_PLACE_FORCE_AXIS
        } else {
            bits
        };
        let _ = apply_command(state, &Command::PlaceRoadBits(c, cmd));
    }
}

fn tile_skips_road_bits(state: &GameState, c: TileCoord, skip: &HashSet<TileCoord>) -> bool {
    if skip.contains(&c) {
        return true;
    }
    matches!(
        state.map.get_kind(c),
        Some(
            TileKind::Water
                | TileKind::RoadBridge
                | TileKind::RoadTunnel
                | TileKind::Station
                | TileKind::RoadDepot
        )
    )
}

fn place_bridges_over_water_spans(
    state: &mut GameState,
    tiles: &[TileCoord],
) -> HashSet<TileCoord> {
    let mut skip = HashSet::new();
    let mut i = 0usize;
    while i < tiles.len() {
        if state.map.get_kind(tiles[i]) != Some(TileKind::Water) {
            i += 1;
            continue;
        }
        let start = i;
        while i < tiles.len() && state.map.get_kind(tiles[i]) == Some(TileKind::Water) {
            i += 1;
        }
        let end = i;
        if start == 0 || end >= tiles.len() {
            continue;
        }
        let a = tiles[start - 1];
        let b = tiles[end];
        if a.x != b.x && a.y != b.y {
            continue;
        }
        if apply_command(state, &Command::PlaceRoadBridge(a, b, BridgeType::Wooden)).is_ok() {
            for t in &tiles[start - 1..=end] {
                skip.insert(*t);
            }
        }
    }
    skip
}

fn link_stops_to_corridor(state: &mut GameState, from: TileCoord, to: TileCoord) {
    for stop in [from, to] {
        for (dx, dy) in [(-1_i32, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(stop.x + dx, stop.y + dy);
            if state.map.get_kind(n) != Some(TileKind::Road) {
                continue;
            }
            // Cruce completo en la boca: une parada + corredor en cualquier eje.
            let _ = apply_command(state, &Command::PlaceRoadBits(n, 0x0F));
            for (dx2, dy2) in [(-1_i32, 0), (1, 0), (0, -1), (0, 1)] {
                let n2 = TileCoord::new(n.x + dx2, n.y + dy2);
                if state.map.get_kind(n2) == Some(TileKind::Road) {
                    let back = road_bits_toward(n2, n);
                    if back != 0 {
                        let _ = apply_command(state, &Command::PlaceRoadBits(n2, back));
                    }
                }
            }
        }
    }
}

fn repair_road_stop_neighbors(state: &mut GameState, from: TileCoord, to: TileCoord) {
    for stop in [from, to] {
        for (dx, dy) in [(-1_i32, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(stop.x + dx, stop.y + dy);
            if state.map.get(n).is_none() {
                continue;
            }
            if !matches!(
                state.map.get_kind(n),
                Some(TileKind::Grass | TileKind::Forest | TileKind::Road)
            ) {
                continue;
            }
            let toward_stop = road_bits_toward(n, stop);
            let along = match (dx, dy) {
                (-1, 0) | (1, 0) => 0x0A,
                _ => 0x05,
            };
            let bits = toward_stop | along;
            let cmd = if bits == 0x0A || bits == 0x05 {
                bits | ROAD_PLACE_FORCE_AXIS
            } else {
                bits
            };
            let _ = apply_command(state, &Command::PlaceRoadBits(n, cmd));
        }
    }
}

const fn road_bits_toward(from: TileCoord, to: TileCoord) -> u8 {
    match (to.x - from.x, to.y - from.y) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0,
    }
}

/// Parada con boca hacia una carretera/puente ya tendido.
fn place_bus_stop_facing_road(
    state: &mut GameState,
    st_pos: TileCoord,
    ai_id: CompanyId,
) -> bool {
    if state.stations.iter().any(|s| s.pos == st_pos) {
        if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
            st.owner = ai_id;
        }
        return true;
    }
    let dirs = [
        (0_i32, -1, 3u8),
        (0, 1, 1),
        (-1, 0, 0),
        (1, 0, 2),
    ];
    for (dx, dy, dir) in dirs {
        let road = TileCoord::new(st_pos.x + dx, st_pos.y + dy);
        if !matches!(
            state.map.get_kind(road),
            Some(TileKind::Road | TileKind::RoadBridge)
        ) {
            continue;
        }
        if apply_command(state, &Command::PlaceBusStop(st_pos, dir)).is_ok() {
            if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
                st.owner = ai_id;
            }
            return true;
        }
    }
    // Fallback: crear boca y parada.
    for (dx, dy, dir) in dirs {
        let road = TileCoord::new(st_pos.x + dx, st_pos.y + dy);
        if state.map.get(road).is_none() || state.map.get_kind(road) == Some(TileKind::Water) {
            continue;
        }
        let _ = apply_command(state, &Command::PlaceRoad(road));
        if apply_command(state, &Command::PlaceBusStop(st_pos, dir)).is_ok() {
            if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
                st.owner = ai_id;
            }
            return true;
        }
    }
    false
}

fn try_place_road_depot_near(state: &mut GameState, stop: TileCoord) -> Option<TileCoord> {
    let ai_id = state.active_company;
    let preferred = [
        (TileCoord::new(stop.x + 1, stop.y), 0u8),
        (TileCoord::new(stop.x - 1, stop.y), 2u8),
        (TileCoord::new(stop.x, stop.y + 1), 3u8),
        (TileCoord::new(stop.x, stop.y - 1), 1u8),
        (TileCoord::new(stop.x + 1, stop.y + 1), 3u8),
        (TileCoord::new(stop.x - 1, stop.y - 1), 1u8),
    ];
    for (depot, dir) in preferred {
        if state.map.get(depot).is_none() {
            continue;
        }
        if state.map.get_kind(depot) == Some(TileKind::RoadDepot) {
            if let Some(tile) = state.map.get(depot) {
                let owner = crate::company::CompanyId::from_tile_m1(tile.m1, state.companies.len());
                if owner != ai_id {
                    continue;
                }
            }
            if find_path(&state.map, depot, stop, PathNetwork::Road).is_some() {
                return Some(depot);
            }
            continue;
        }
        let (mx, my) = match dir {
            0 => (-1_i32, 0),
            1 => (0, 1),
            2 => (1, 0),
            _ => (0, -1),
        };
        let mouth = TileCoord::new(depot.x + mx, depot.y + my);
        if state.map.get(mouth).is_some() {
            let _ = apply_command(state, &Command::PlaceRoad(mouth));
        }
        if apply_command(state, &Command::PlaceRoadDepotDir(depot, dir)).is_err() {
            continue;
        }
        if find_path(&state.map, depot, stop, PathNetwork::Road).is_some() {
            return Some(depot);
        }
        let _ = apply_command(state, &Command::ClearTile(depot));
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::company::CompanyId;

    fn build_stops_and_corridor(state: &mut GameState, a: TileCoord, b: TileCoord) {
        place_road_corridor(state, a, b);
        assert!(place_bus_stop_facing_road(state, a, CompanyId::PLAYER));
        assert!(place_bus_stop_facing_road(state, b, CompanyId::PLAYER));
        link_stops_to_corridor(state, a, b);
    }

    #[test]
    fn manhattan_l_corner_has_turn_bits_and_path() {
        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(8, 6);
        build_stops_and_corridor(&mut state, a, b);

        let corner_xy = TileCoord::new(8, 2);
        let corner_yx = TileCoord::new(2, 6);
        let corner = [corner_xy, corner_yx]
            .into_iter()
            .find(|&c| state.map.get_kind(c) == Some(TileKind::Road))
            .expect("debe existir codo L");
        let bits = state.map.get(corner).unwrap().m5 & 0x0F;
        assert!(
            bits.count_ones() >= 2,
            "codo L debe unir ambos ejes: m5={bits:#04x} at {corner:?}"
        );
        assert!(
            find_path(&state.map, a, b, PathNetwork::Road).is_some(),
            "paradas deben quedar unidas por path road"
        );
    }

    #[test]
    fn road_corridor_avoids_water_without_flat_road_on_water() {
        let mut state = GameState::new(16, 10);
        state.economy.money = 500_000;
        for x in 4..8 {
            state
                .map
                .set_kind(TileCoord::new(x, 4), TileKind::Water)
                .unwrap();
        }
        let a = TileCoord::new(2, 4);
        let b = TileCoord::new(10, 4);
        build_stops_and_corridor(&mut state, a, b);

        for x in 4..8 {
            let c = TileCoord::new(x, 4);
            assert_ne!(
                state.map.get_kind(c),
                Some(TileKind::Road),
                "no debe quedar Road plana sobre agua en {c:?}"
            );
        }
        assert!(
            find_path(&state.map, a, b, PathNetwork::Road).is_some(),
            "debe rodear el canal o cruzarlo con puente"
        );
    }

    #[test]
    fn road_corridor_bridges_when_water_blocks_all_detours() {
        let mut state = GameState::new(14, 8);
        state.economy.money = 500_000;
        for y in 0..8 {
            state
                .map
                .set_kind(TileCoord::new(6, y), TileKind::Water)
                .unwrap();
        }
        let a = TileCoord::new(3, 3);
        let b = TileCoord::new(9, 3);
        build_stops_and_corridor(&mut state, a, b);

        assert_eq!(
            state.map.get_kind(TileCoord::new(5, 3)),
            Some(TileKind::RoadBridge),
            "rampa oeste del puente"
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(7, 3)),
            Some(TileKind::RoadBridge),
            "rampa este del puente"
        );
        assert_ne!(
            state.map.get_kind(TileCoord::new(6, 3)),
            Some(TileKind::Road),
            "no Road plana sobre el vano"
        );
        assert!(
            find_path(&state.map, a, b, PathNetwork::Road).is_some(),
            "puente (salto rampa→rampa) debe conectar las paradas"
        );
    }
}
