//! `RoadVehCheckOvertake` — adelantamiento por el carril opuesto.

use crate::map::{Map, TileCoord, TileKind};
use crate::road_movement::rvsb::{RVSB_DRIVE_SIDE, RVSB_IN_ROAD_STOP, RVSB_TRACKDIR_MASK};
use crate::road_movement::traffic::is_road_vehicle_kind;
use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleKind};

/// Timeout de adelantamiento (`roadveh.h` `RV_OVERTAKE_TIMEOUT`).
pub const RV_OVERTAKE_TIMEOUT: u8 = 35;
/// Aceleración original mientras se adelanta (`roadveh_cmd.cpp` `UpdateSpeed`).
pub const ROAD_ACCEL_OVERTAKE: u16 = 512;

/// ¿Trackdir recto (sin curva)?
#[must_use]
pub fn is_straight_road_trackdir(trackdir: u8) -> bool {
    matches!(trackdir & RVSB_TRACKDIR_MASK, 0 | 1 | 8 | 9)
}

/// Intenta iniciar adelantamiento sobre el vehículo `blocker` delante.
///
/// Paridad reducida con `RoadVehCheckOvertake`: mismo rumbo diagonal, recta,
/// sin estación, sin tranvía, carril opuesto libre en la tesela actual y la
/// siguiente; `overtaking = RVSB_DRIVE_SIDE` y timeout 35 (mitad si parado).
pub fn road_veh_check_overtake(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    blocker_idx: usize,
    map: Option<&Map>,
) {
    let Some(v) = vehicles.get(v_idx) else {
        return;
    };
    if v.overtaking != 0 || !is_road_vehicle_kind(v.kind) {
        return;
    }
    if v.kind == VehicleKind::Tram {
        return;
    }
    if v.road_state >= RVSB_IN_ROAD_STOP {
        return;
    }
    if !is_straight_road_trackdir(v.road_state) {
        return;
    }
    // Solo diagonales de sprite (`direction & 1` en OpenTTD Direction 0..7).
    if !matches!(v.direction, DIR_NE | DIR_SE | DIR_SW | DIR_NW) {
        return;
    }
    let Some(u) = vehicles.get(blocker_idx) else {
        return;
    };
    if v.direction != u.direction {
        return;
    }
    if map.is_some_and(|m| tile_is_station(m, v.pos) || tile_is_station(m, u.pos)) {
        return;
    }
    let v_max = v.effective_engine().max_speed;
    let u_max = u.effective_engine().max_speed;
    let u_speed = if u.running && u.cur_speed != 0 {
        u_max
    } else {
        u.cur_speed
    };
    if u_speed >= v_max && u.running && u.cur_speed != 0 {
        return;
    }
    if map.is_some_and(|m| {
        road_blocked_for_overtaking(m, vehicles, v_idx, blocker_idx, v.pos, v.direction)
            || next_tile(v.pos, v.direction).is_some_and(|n| {
                road_blocked_for_overtaking(m, vehicles, v_idx, blocker_idx, n, v.direction)
            })
    }) {
        return;
    }

    let half = u.cur_speed == 0 || !u.running;
    let v = &mut vehicles[v_idx];
    v.overtaking = RVSB_DRIVE_SIDE;
    v.overtaking_ctr = if half { RV_OVERTAKE_TIMEOUT / 2 } else { 0 };
}

/// Avanza / aborta el adelantamiento en curso (`IndividualRoadVehicleController`).
pub fn tick_overtaking(v: &mut Vehicle, map: Option<&Map>) {
    if v.overtaking == 0 {
        return;
    }
    if map.is_some_and(|m| tile_is_station(m, v.pos)) {
        v.overtaking = 0;
        v.overtaking_ctr = 0;
        return;
    }
    v.overtaking_ctr = v.overtaking_ctr.saturating_add(1);
    if v.overtaking_ctr >= RV_OVERTAKE_TIMEOUT
        && v.road_state < RVSB_IN_ROAD_STOP
        && is_straight_road_trackdir(v.road_state)
    {
        v.overtaking = 0;
        v.overtaking_ctr = 0;
    }
}

/// Índice de tabla `_road_drive_data` con carril opuesto si `overtaking`.
#[must_use]
pub fn drive_state_with_overtake(road_state: u8, overtaking: u8) -> u8 {
    (road_state & RVSB_TRACKDIR_MASK) ^ (overtaking & RVSB_DRIVE_SIDE)
}

fn tile_is_station(map: &Map, pos: TileCoord) -> bool {
    map.get_kind(pos) == Some(TileKind::Station)
}

fn next_tile(pos: TileCoord, dir: u8) -> Option<TileCoord> {
    match dir {
        DIR_NE => Some(TileCoord::new(pos.x.saturating_sub(1), pos.y)),
        DIR_SE => Some(TileCoord::new(pos.x, pos.y.saturating_add(1))),
        DIR_SW => Some(TileCoord::new(pos.x.saturating_add(1), pos.y)),
        DIR_NW => Some(TileCoord::new(pos.x, pos.y.saturating_sub(1))),
        _ => None,
    }
}

fn road_blocked_for_overtaking(
    map: &Map,
    vehicles: &[Vehicle],
    v_idx: usize,
    blocker_idx: usize,
    tile: TileCoord,
    dir: u8,
) -> bool {
    let Some(t) = map.get(tile) else {
        return true;
    };
    if matches!(
        t.kind,
        TileKind::Station | TileKind::RoadDepot | TileKind::RailDepot | TileKind::Water
    ) {
        return true;
    }
    if crate::map::is_road_level_crossing(t.mapt, t.m5, t.kind) {
        return true;
    }
    // Otros vehículos en la tesela (salvo nosotros y el adelantado).
    vehicles.iter().enumerate().any(|(i, other)| {
        i != v_idx
            && i != blocker_idx
            && is_road_vehicle_kind(other.kind)
            && other.pos == tile
            && (other.direction == dir || other.direction == opposite_dir(dir))
    })
}

fn opposite_dir(dir: u8) -> u8 {
    match dir {
        DIR_NE => DIR_SW,
        DIR_SE => DIR_NW,
        DIR_SW => DIR_NE,
        DIR_NW => DIR_SE,
        _ => dir,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::vehicle::{DIR_SW, VehicleKind};
    use std::collections::VecDeque;

    fn bus_at(id: u32, x: i32, speed: u16) -> Vehicle {
        let mut v = Vehicle::new(
            id,
            VehicleKind::Bus,
            TileCoord::new(x, 0),
            TileCoord::new(x + 5, 0),
        );
        v.direction = DIR_SW;
        v.road_state = 8;
        v.cur_speed = speed;
        v.running = true;
        v.path = VecDeque::from([TileCoord::new(x + 1, 0)]);
        v
    }

    #[test]
    fn starts_overtake_on_slower_leader() {
        // Con AM_ORIGINAL OpenTTD compara max_speed: mismo motor no adelanta
        // si el líder se mueve. Solo si está parado (o más lento de techo).
        let mut vehicles = vec![bus_at(1, 0, 40), bus_at(2, 1, 0)];
        vehicles[1].running = false;
        road_veh_check_overtake(&mut vehicles, 0, 1, None);
        assert_eq!(vehicles[0].overtaking, RVSB_DRIVE_SIDE);
    }

    #[test]
    fn same_max_speed_moving_leader_blocks_overtake() {
        let mut vehicles = vec![bus_at(1, 0, 40), bus_at(2, 1, 5)];
        road_veh_check_overtake(&mut vehicles, 0, 1, None);
        assert_eq!(vehicles[0].overtaking, 0);
    }

    #[test]
    fn half_timeout_when_leader_stopped() {
        let mut vehicles = vec![bus_at(1, 0, 40), bus_at(2, 1, 0)];
        vehicles[1].running = false;
        road_veh_check_overtake(&mut vehicles, 0, 1, None);
        assert_eq!(vehicles[0].overtaking_ctr, RV_OVERTAKE_TIMEOUT / 2);
    }

    #[test]
    fn timeout_clears_overtaking_on_straight() {
        let mut v = bus_at(1, 0, 40);
        v.overtaking = RVSB_DRIVE_SIDE;
        v.overtaking_ctr = RV_OVERTAKE_TIMEOUT - 1;
        tick_overtaking(&mut v, None);
        assert_eq!(v.overtaking, 0);
    }
}
