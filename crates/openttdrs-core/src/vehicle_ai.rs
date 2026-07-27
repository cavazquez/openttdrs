//! Comportamiento de vehículos sin órdenes manuales (paridad `OpenTTD` `ProcessOrders` +
//! lookahead / `PickRandomBit` en carretera).

use crate::depot::{DepotSpatialIndex, nearest_depot_tile_indexed};
use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::{dir_from_to, rail_neighbors};
use crate::ship_movement::water_tiles_connected;
use crate::tick::GameTick;
use crate::vehicle::{DIR_N, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, VehicleDirection};

fn seeded_index(seed: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let divisor = u64::try_from(len).unwrap_or(1);
    usize::try_from(seed % divisor).unwrap_or(0)
}

fn orderless_seed(vehicle_id: u32, pos: TileCoord, tick: GameTick) -> u64 {
    tick.get()
        .wrapping_add(u64::from(vehicle_id) * 37)
        .wrapping_add(u64::from(pos.x.unsigned_abs()) * 17)
        .wrapping_add(u64::from(pos.y.unsigned_abs()) * 31)
}

#[must_use]
const fn road_bits_toward_neighbor(dx: i32, dy: i32) -> u8 {
    match (dx, dy) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0x0F,
    }
}

#[must_use]
fn road_bits_at(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => {
            let bits = t.m5 & 0x0F;
            if bits == 0 { 0x0F } else { bits }
        }
        TileKind::Station if (t.m6 >> 3) & 0x0F == 2 || (t.m6 >> 3) & 0x0F == 3 => t.m3 & 0x0F,
        _ => 0,
    }
}

#[must_use]
fn tram_bits_at(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Road | TileKind::RoadTunnel | TileKind::RoadBridge => {
            crate::road_type::tram_track_bits(&t)
        }
        TileKind::RoadDepot => {
            let bits = crate::road_type::tram_track_bits(&t);
            if bits != 0 {
                bits
            } else {
                let bits = t.m5 & 0x0F;
                if bits == 0 { 0x0F } else { bits }
            }
        }
        TileKind::Station if (t.m6 >> 3) & 0x0F == 2 || (t.m6 >> 3) & 0x0F == 3 => t.m3 & 0x0F,
        _ => 0,
    }
}

#[must_use]
fn road_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    let exit = road_bits_toward_neighbor(dx, dy);
    let entry = road_bits_toward_neighbor(-dx, -dy);
    road_bits_at(map, cur) & exit != 0 && road_bits_at(map, next) & entry != 0
}

#[must_use]
fn road_neighbors(map: &Map, cur: TileCoord, prev: Option<TileCoord>) -> Vec<TileCoord> {
    let mut out = Vec::new();
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let next = TileCoord::new(cur.x + dx, cur.y + dy);
        if prev == Some(next) {
            continue;
        }
        if road_tiles_connected(map, cur, next) {
            out.push(next);
        }
    }
    out
}

#[must_use]
fn tram_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    let exit = road_bits_toward_neighbor(dx, dy);
    let entry = road_bits_toward_neighbor(-dx, -dy);
    tram_bits_at(map, cur) & exit != 0 && tram_bits_at(map, next) & entry != 0
}

#[must_use]
fn tram_neighbors(map: &Map, cur: TileCoord, prev: Option<TileCoord>) -> Vec<TileCoord> {
    let mut out = Vec::new();
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let next = TileCoord::new(cur.x + dx, cur.y + dy);
        if prev == Some(next) {
            continue;
        }
        if tram_tiles_connected(map, cur, next) {
            out.push(next);
        }
    }
    out
}

#[must_use]
fn water_neighbors(map: &Map, cur: TileCoord, prev: Option<TileCoord>) -> Vec<TileCoord> {
    let mut out = Vec::new();
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let next = TileCoord::new(cur.x + dx, cur.y + dy);
        if prev == Some(next) {
            continue;
        }
        if water_tiles_connected(map, cur, next) {
            out.push(next);
        }
    }
    out
}

#[must_use]
pub(crate) fn vehicle_direction_to_diag(direction: VehicleDirection) -> u8 {
    match direction {
        DIR_SE | DIR_S => 1,
        DIR_SW | DIR_W => 2,
        DIR_N | DIR_NW => 3,
        _ => 0,
    }
}

fn pick_orderless_neighbor(
    neighbors: Vec<TileCoord>,
    cur: TileCoord,
    prev: Option<TileCoord>,
    preferred_diag: u8,
    vehicle_id: u32,
    tick: GameTick,
) -> Option<TileCoord> {
    if neighbors.is_empty() {
        return None;
    }
    if neighbors.len() == 1 {
        return Some(neighbors[0]);
    }
    if let Some(next) = neighbors
        .iter()
        .copied()
        .find(|n| dir_from_to(cur, *n) == Some(preferred_diag))
    {
        return Some(next);
    }
    let mut candidates = neighbors;
    if let Some(previous) = prev
        && candidates.len() > 1
    {
        candidates.retain(|n| *n != previous);
        if candidates.is_empty() {
            return None;
        }
    }
    let seed = orderless_seed(vehicle_id, cur, tick);
    Some(candidates[seeded_index(seed, candidates.len())])
}

/// Siguiente tesela ferroviaria sin destino (`CheckNextTrainTile` simplificado).
#[must_use]
pub(crate) fn orderless_rail_next(
    map: &Map,
    pos: TileCoord,
    prev: Option<TileCoord>,
    preferred_diag: u8,
    vehicle_id: u32,
    tick: GameTick,
) -> Option<TileCoord> {
    pick_orderless_neighbor(
        rail_neighbors(map, pos, prev),
        pos,
        prev,
        preferred_diag,
        vehicle_id,
        tick,
    )
}

/// Siguiente tesela acuática sin destino.
#[must_use]
pub(crate) fn orderless_water_next(
    map: &Map,
    pos: TileCoord,
    prev: Option<TileCoord>,
    preferred_diag: u8,
    vehicle_id: u32,
    tick: GameTick,
) -> Option<TileCoord> {
    pick_orderless_neighbor(
        water_neighbors(map, pos, prev),
        pos,
        prev,
        preferred_diag,
        vehicle_id,
        tick,
    )
}

/// Siguiente tesela de carretera sin destino (`PickRandomBit` en cruces).
#[must_use]
pub(crate) fn orderless_road_next(
    map: &Map,
    pos: TileCoord,
    prev: Option<TileCoord>,
    vehicle_id: u32,
    tick: GameTick,
) -> Option<TileCoord> {
    let neighbors = road_neighbors(map, pos, prev);
    if neighbors.is_empty() {
        return None;
    }
    if neighbors.len() == 1 {
        return Some(neighbors[0]);
    }
    let mut candidates = neighbors;
    if let Some(previous) = prev
        && candidates.len() > 1
    {
        candidates.retain(|n| *n != previous);
        if candidates.is_empty() {
            return None;
        }
    }
    let seed = orderless_seed(vehicle_id, pos, tick);
    Some(candidates[seeded_index(seed, candidates.len())])
}

/// Siguiente tesela de tranvía sin destino (misma lógica, bits m3).
#[must_use]
pub(crate) fn orderless_tram_next(
    map: &Map,
    pos: TileCoord,
    prev: Option<TileCoord>,
    vehicle_id: u32,
    tick: GameTick,
) -> Option<TileCoord> {
    let neighbors = tram_neighbors(map, pos, prev);
    if neighbors.is_empty() {
        return None;
    }
    if neighbors.len() == 1 {
        return Some(neighbors[0]);
    }
    let mut candidates = neighbors;
    if let Some(previous) = prev
        && candidates.len() > 1
    {
        candidates.retain(|n| *n != previous);
        if candidates.is_empty() {
            return None;
        }
    }
    let seed = orderless_seed(vehicle_id, pos, tick);
    Some(candidates[seeded_index(seed, candidates.len())])
}

/// Aeropuerto más cercano para aviones sin órdenes (`HandleMissingAircraftOrders`).
#[must_use]
pub(crate) fn orderless_aircraft_hangar(
    map: &Map,
    pos: TileCoord,
    depot_index: &mut DepotSpatialIndex,
) -> Option<TileCoord> {
    nearest_depot_tile_indexed(map, pos, crate::vehicle::VehicleKind::Aircraft, depot_index)
}

/// Fallback Manhattan para vehículos de carretera sin red adyacente.
pub(crate) fn orderless_wander_destination(
    map: &Map,
    vehicle_id: u32,
    pos: TileCoord,
    previous: TileCoord,
    tick: GameTick,
) -> Option<TileCoord> {
    let (mw, mh) = map.dimensions();
    let mut candidates = [None; 4];
    let mut len = 0usize;
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let c = TileCoord::new(pos.x + dx, pos.y + dy);
        if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
            continue;
        }
        if !road_vehicle_can_wander_on(map.get_kind(c)) {
            continue;
        }
        candidates[len] = Some(c);
        len += 1;
    }
    if len == 0 {
        return None;
    }
    let usable_len = if len > 1 {
        let mut write = 0usize;
        for read in 0..len {
            if candidates[read] != Some(previous) {
                candidates[write] = candidates[read];
                write += 1;
            }
        }
        write.max(1)
    } else {
        len
    };
    let seed = orderless_seed(vehicle_id, pos, tick);
    candidates[seeded_index(seed, usable_len)]
}

fn road_vehicle_can_wander_on(kind: Option<TileKind>) -> bool {
    matches!(
        kind,
        Some(TileKind::Road | TileKind::RoadDepot | TileKind::RoadBridge | TileKind::RoadTunnel)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::tick::GameTick;
    use crate::{Command, GameState, apply_command};

    #[test]
    fn orderless_rail_follows_straight_track() {
        let mut s = GameState::new(12, 6);
        for x in 0..8 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 2))).unwrap();
        }
        let pos = TileCoord::new(2, 2);
        let prev = TileCoord::new(1, 2);
        let next = orderless_rail_next(&s.map, pos, Some(prev), 0, 7, GameTick::new(0)).unwrap();
        assert_eq!(next, TileCoord::new(3, 2));
    }

    #[test]
    fn orderless_road_picks_connected_neighbor() {
        let mut s = GameState::new(8, 8);
        for x in 1..=3 {
            apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(x, 1))).unwrap();
        }
        let next =
            orderless_road_next(&s.map, TileCoord::new(1, 1), None, 9, GameTick::new(4)).unwrap();
        assert_eq!(s.map.get_kind(next), Some(TileKind::Road));
    }
}
