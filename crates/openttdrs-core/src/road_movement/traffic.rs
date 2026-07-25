//! `RoadVehFindCloseTo` — sincronización de velocidad con el vehículo de delante.

use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleKind};

/// Umbral `OpenTTD`: tras tantos ticks bloqueado se atraviesa (`roadveh_cmd.cpp`).
pub const BLOCKED_CTR_LIMIT: u16 = 1_480;

#[must_use]
pub fn is_road_vehicle_kind(kind: VehicleKind) -> bool {
    matches!(
        kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    )
}

/// Busca el vehículo road más cercano delante en la misma dirección axial.
///
/// Con `blocked_ctr > BLOCKED_CTR_LIMIT` devuelve `None` (permite atravesar).
#[must_use]
pub fn road_veh_find_close_to(vehicles: &[Vehicle], v_idx: usize) -> Option<usize> {
    let v = vehicles.get(v_idx)?;
    if !is_road_vehicle_kind(v.kind) || !v.running {
        return None;
    }
    if matches!(
        v.road_depot_phase,
        crate::vehicle::RoadDepotPhase::InDepot
            | crate::vehicle::RoadDepotPhase::Entering { .. }
            | crate::vehicle::RoadDepotPhase::Exiting { .. }
    ) {
        return None;
    }
    if v.blocked_ctr > BLOCKED_CTR_LIMIT {
        return None;
    }

    let dir = v.direction;
    let pos = v.pos;
    let frame = v.frame;
    let mut best: Option<(u32, usize)> = None;

    for (i, other) in vehicles.iter().enumerate() {
        if i == v_idx || !is_road_vehicle_kind(other.kind) || !other.running {
            continue;
        }
        if other.direction != dir {
            continue;
        }
        if !same_or_adjacent_tile(pos, other.pos) {
            continue;
        }
        if !is_ahead(pos, frame, other.pos, other.frame, dir) {
            continue;
        }
        let dist = axial_distance(pos, frame, other.pos, other.frame, dir);
        if best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, i));
        }
    }

    best.map(|(_, idx)| idx)
}

/// Aplica el resultado de `FindCloseTo`: sync velocidad e incremento de `blocked_ctr`.
pub fn apply_road_veh_close_to(vehicles: &mut [Vehicle], v_idx: usize) -> bool {
    let Some(blocker) = road_veh_find_close_to(vehicles, v_idx) else {
        vehicles[v_idx].blocked_ctr = 0;
        return false;
    };
    let blocker_speed = vehicles[blocker].cur_speed;
    vehicles[v_idx].cur_speed = blocker_speed;
    vehicles[v_idx].blocked_ctr = vehicles[v_idx].blocked_ctr.saturating_add(1);
    if vehicles[v_idx].blocked_ctr > BLOCKED_CTR_LIMIT {
        vehicles[v_idx].blocked_ctr = 0;
        return false;
    }
    true
}

fn same_or_adjacent_tile(a: TileCoord, b: TileCoord) -> bool {
    (a.x - b.x).unsigned_abs() <= 1 && (a.y - b.y).unsigned_abs() <= 1
}

fn is_ahead(
    self_pos: TileCoord,
    self_frame: u8,
    other_pos: TileCoord,
    other_frame: u8,
    dir: u8,
) -> bool {
    use crate::vehicle::{DIR_NE, DIR_NW};
    let self_ord = tile_ordinal(self_pos, self_frame, dir);
    let other_ord = tile_ordinal(other_pos, other_frame, dir);
    if matches!(dir, DIR_NE | DIR_NW) {
        other_ord < self_ord
    } else {
        other_ord > self_ord
    }
}

fn axial_distance(
    self_pos: TileCoord,
    self_frame: u8,
    other_pos: TileCoord,
    other_frame: u8,
    dir: u8,
) -> u32 {
    let a = tile_ordinal(self_pos, self_frame, dir);
    let b = tile_ordinal(other_pos, other_frame, dir);
    a.abs_diff(b)
}

fn tile_ordinal(pos: TileCoord, frame: u8, dir: u8) -> i32 {
    use crate::vehicle::{DIR_NW, DIR_SE};
    let base = if matches!(dir, DIR_SE | DIR_NW) {
        pos.y.saturating_mul(16)
    } else {
        pos.x.saturating_mul(16)
    };
    base + i32::from(frame)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::vehicle::{DIR_SW, Vehicle, VehicleKind};
    use std::collections::VecDeque;

    fn bus_at(id: u32, x: i32, frame: u8, speed: u16) -> Vehicle {
        let mut v = Vehicle::new(
            id,
            VehicleKind::Bus,
            TileCoord::new(x, 0),
            TileCoord::new(x + 5, 0),
        );
        v.direction = DIR_SW;
        v.road_state = 8;
        v.frame = frame;
        v.cur_speed = speed;
        v.running = true;
        v.path = VecDeque::from([TileCoord::new(x + 1, 0)]);
        v
    }

    #[test]
    fn find_close_to_syncs_speed_to_leader() {
        let mut vehicles = vec![bus_at(1, 0, 2, 40), bus_at(2, 1, 4, 10)];
        assert!(apply_road_veh_close_to(&mut vehicles, 0));
        assert_eq!(vehicles[0].cur_speed, 10);
        assert!(vehicles[0].blocked_ctr > 0);
    }

    #[test]
    fn blocked_ctr_above_limit_allows_pass() {
        let mut vehicles = vec![bus_at(1, 0, 2, 40), bus_at(2, 1, 4, 10)];
        vehicles[0].blocked_ctr = BLOCKED_CTR_LIMIT;
        assert!(!apply_road_veh_close_to(&mut vehicles, 0));
        assert_eq!(vehicles[0].blocked_ctr, 0);
    }
}
