//! Construcción de carretera, paradas bus y depósito para `RoadHaul`.

use crate::GameState;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path};

use super::plan::BusPlan;

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
        // Primero paradas en hierba + boca a carretera; luego corredor (sin pisar paradas).
        if !place_bus_stop_owned(state, stop_a, ai_id) {
            return;
        }
        if !place_bus_stop_owned(state, stop_b, ai_id) {
            return;
        }
        place_road_manhattan_between(state, stop_a, stop_b);
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
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-2, 0), (2, 0), (0, -2), (0, 2)] {
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

/// Corredor Manhattan entre dos paradas **sin** convertir las teselas de parada en road.
fn place_road_manhattan_between(state: &mut GameState, from: TileCoord, to: TileCoord) {
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
    for pair in tiles.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let bits_a = road_bits_toward(a, b);
        let bits_b = road_bits_toward(b, a);
        // Extremos = paradas bus: solo asegurar el vecino (ya tiene boca).
        if a != from && a != to {
            let _ = apply_command(state, &Command::PlaceRoadBits(a, bits_a));
        }
        if b != from && b != to {
            let _ = apply_command(state, &Command::PlaceRoadBits(b, bits_b));
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

fn place_bus_stop_owned(state: &mut GameState, st_pos: TileCoord, ai_id: CompanyId) -> bool {
    if state.stations.iter().any(|s| s.pos == st_pos) {
        if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
            st.owner = ai_id;
        }
        return true;
    }
    // Asegurar carretera adyacente hacia el pueblo / corredor.
    for (dx, dy, dir) in [
        (0_i32, -1, 3u8),
        (0, 1, 1),
        (-1, 0, 0),
        (1, 0, 2),
    ] {
        let road = TileCoord::new(st_pos.x + dx, st_pos.y + dy);
        if state.map.get(road).is_none() {
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
            if find_path(&state.map, depot, stop, PathNetwork::Road).is_some() {
                return Some(depot);
            }
            continue;
        }
        // Boca: carretera hacia la parada.
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
