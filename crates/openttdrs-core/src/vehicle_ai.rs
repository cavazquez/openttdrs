//! Comportamientos simples para vehículos que no tienen órdenes explícitas.

use crate::map::{Map, TileCoord, TileKind};
use crate::tick::GameTick;

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
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
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
    let seed = tick
        .get()
        .wrapping_add(u64::from(vehicle_id) * 37)
        .wrapping_add(pos.x.unsigned_abs() as u64 * 17)
        .wrapping_add(pos.y.unsigned_abs() as u64 * 31);
    candidates[(seed as usize) % usable_len]
}

fn road_vehicle_can_wander_on(kind: Option<TileKind>) -> bool {
    matches!(
        kind,
        Some(TileKind::Road | TileKind::RoadDepot | TileKind::RoadBridge | TileKind::RoadTunnel)
    )
}
