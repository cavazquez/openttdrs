//! `RoadVehFindCloseTo` — sincronización de velocidad con el vehículo de delante.

use std::collections::HashMap;

use crate::map::{Map, TileCoord};
use crate::vehicle::{Vehicle, VehicleKind};

/// Umbral `OpenTTD`: tras tantos ticks bloqueado se atraviesa (`roadveh_cmd.cpp`).
pub const BLOCKED_CTR_LIMIT: u16 = 1_480;

/// Índice mutable de roadveh por tesela durante un tick.
///
/// `RoadVehFindCloseTo` sólo consulta la misma tesela y sus ocho vecinas. Sin
/// este índice cada subpaso recorría toda la flota, lo que se vuelve prohibitivo
/// al restaurar partidas con miles de vehículos de carretera.
#[derive(Debug, Clone, Default)]
pub struct RoadTrafficIndex {
    by_tile: HashMap<TileCoord, Vec<usize>>,
}

impl RoadTrafficIndex {
    pub fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.by_tile.clear();
        for (index, vehicle) in vehicles.iter().enumerate() {
            if is_road_vehicle_kind(vehicle.kind) && !vehicle.is_articulated_unit() {
                self.by_tile.entry(vehicle.pos).or_default().push(index);
            }
        }
    }

    /// Refleja la nueva posición después de procesar un roadveh en el orden
    /// secuencial del tick. Los vehículos todavía no procesados conservan su
    /// posición de inicio, igual que antes del índice.
    pub fn update_vehicle(&mut self, vehicles: &[Vehicle], index: usize, previous: TileCoord) {
        let Some(vehicle) = vehicles.get(index) else {
            return;
        };
        if !is_road_vehicle_kind(vehicle.kind)
            || vehicle.is_articulated_unit()
            || vehicle.pos == previous
        {
            return;
        }
        if let Some(indices) = self.by_tile.get_mut(&previous) {
            indices.retain(|&other| other != index);
            if indices.is_empty() {
                self.by_tile.remove(&previous);
            }
        }
        self.by_tile.entry(vehicle.pos).or_default().push(index);
    }

    fn at(&self, pos: TileCoord) -> &[usize] {
        self.by_tile.get(&pos).map_or(&[], Vec::as_slice)
    }
}

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
    if !is_road_vehicle_kind(v.kind) || v.is_articulated_unit() || !v.running {
        return None;
    }
    // Dentro de una bahía la exclusión de boca y la asignación far/near son
    // autoritativas; el orden axial por tesela no representa ese lazo.
    if crate::road_movement::rvsb::is_bay_road_state(v.road_state) {
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
        if i == v_idx
            || !is_road_vehicle_kind(other.kind)
            || other.is_articulated_unit()
            || !other.running
        {
            continue;
        }
        if crate::road_movement::rvsb::is_bay_road_state(other.road_state) {
            continue;
        }
        // El carril opuesto se representa con RVSB_DRIVE_SIDE. OpenTTD lo
        // descarta por coordenadas sub-tesela; el modelo lógico debe hacerlo
        // explícitamente para no frenar detrás de un vehículo adelantando.
        if other.overtaking != v.overtaking {
            continue;
        }
        if other.direction != dir {
            continue;
        }
        if !same_or_adjacent_tile(pos, other.pos) {
            continue;
        }
        let overlaps = pos == other.pos && frame == other.frame;
        if overlaps && other.id > v.id
            || !overlaps && !is_ahead(pos, frame, other.pos, other.frame, dir)
        {
            continue;
        }
        let dist = axial_distance(pos, frame, other.pos, other.frame, dir);
        if best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, i));
        }
    }

    best.map(|(_, idx)| idx)
}

/// Variante indexada de [`road_veh_find_close_to`] para el hot path del tick.
#[must_use]
pub fn road_veh_find_close_to_indexed(
    vehicles: &[Vehicle],
    v_idx: usize,
    index: &RoadTrafficIndex,
) -> Option<usize> {
    let v = vehicles.get(v_idx)?;
    if !is_road_vehicle_kind(v.kind) || v.is_articulated_unit() || !v.running {
        return None;
    }
    if crate::road_movement::rvsb::is_bay_road_state(v.road_state)
        || matches!(
            v.road_depot_phase,
            crate::vehicle::RoadDepotPhase::InDepot
                | crate::vehicle::RoadDepotPhase::Entering { .. }
                | crate::vehicle::RoadDepotPhase::Exiting { .. }
        )
        || v.blocked_ctr > BLOCKED_CTR_LIMIT
    {
        return None;
    }

    let mut best: Option<(u32, usize)> = None;
    for y in (v.pos.y - 1)..=(v.pos.y + 1) {
        for x in (v.pos.x - 1)..=(v.pos.x + 1) {
            for &other_idx in index.at(TileCoord::new(x, y)) {
                let Some(other) = vehicles.get(other_idx) else {
                    continue;
                };
                if other_idx == v_idx
                    || !other.running
                    || other.is_articulated_unit()
                    || crate::road_movement::rvsb::is_bay_road_state(other.road_state)
                    || other.overtaking != v.overtaking
                    || other.direction != v.direction
                {
                    continue;
                }
                let overlaps = v.pos == other.pos && v.frame == other.frame;
                if overlaps && other.id > v.id
                    || !overlaps && !is_ahead(v.pos, v.frame, other.pos, other.frame, v.direction)
                {
                    continue;
                }
                let dist = axial_distance(v.pos, v.frame, other.pos, other.frame, v.direction);
                if best.is_none_or(|(best_dist, best_idx)| {
                    dist < best_dist || dist == best_dist && other_idx < best_idx
                }) {
                    best = Some((dist, other_idx));
                }
            }
        }
    }
    best.map(|(_, idx)| idx)
}

/// Aplica el resultado de `FindCloseTo`: overtake o sync velocidad + `blocked_ctr`.
///
/// Si ya se adelanta (`overtaking != 0`) no bloquea. Si no, intenta
/// `RoadVehCheckOvertake`; solo sincroniza velocidad si sigue sin adelantar.
pub fn apply_road_veh_close_to(vehicles: &mut [Vehicle], v_idx: usize, map: Option<&Map>) -> bool {
    apply_road_veh_close_to_with_blocker(
        vehicles,
        v_idx,
        map,
        road_veh_find_close_to(vehicles, v_idx),
        &[],
    )
}

/// Variante indexada de [`apply_road_veh_close_to`] para el hot path del tick.
pub fn apply_road_veh_close_to_indexed(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    index: &RoadTrafficIndex,
) -> bool {
    apply_road_veh_close_to_indexed_with_catalog(vehicles, v_idx, map, index, &[])
}

/// Variante indexada que usa el catálogo activo para comparar velocidades en
/// `RoadVehCheckOvertake` (incluidos CB36 de motores `NewGRF`).
pub fn apply_road_veh_close_to_indexed_with_catalog(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    index: &RoadTrafficIndex,
    engine_catalog: &[crate::engine::EngineDef],
) -> bool {
    let blocker = road_veh_find_close_to_indexed(vehicles, v_idx, index);
    apply_road_veh_close_to_with_blocker(vehicles, v_idx, map, blocker, engine_catalog)
}

fn apply_road_veh_close_to_with_blocker(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    blocker: Option<usize>,
    engine_catalog: &[crate::engine::EngineDef],
) -> bool {
    if vehicles
        .get(v_idx)
        .is_some_and(|v| v.overtaking != 0 || v.crashed)
    {
        vehicles[v_idx].blocked_ctr = 0;
        return false;
    }
    let Some(blocker) = blocker else {
        vehicles[v_idx].blocked_ctr = 0;
        return false;
    };
    crate::road_movement::overtake::road_veh_check_overtake_with_catalog(
        vehicles,
        v_idx,
        blocker,
        map,
        engine_catalog,
    );
    if vehicles[v_idx].overtaking != 0 {
        vehicles[v_idx].blocked_ctr = 0;
        return false;
    }
    let blocker_speed = vehicles[blocker].cur_speed;
    if vehicles[v_idx].cur_speed > blocker_speed {
        vehicles[v_idx].cur_speed = blocker_speed;
        vehicles[v_idx].subspeed = vehicles[v_idx].subspeed.min(vehicles[blocker].subspeed);
    }
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
        assert!(apply_road_veh_close_to(&mut vehicles, 0, None));
        // Sin mapa (recta libre) puede iniciar overtake en vez de sync.
        if vehicles[0].overtaking == 0 {
            assert_eq!(vehicles[0].cur_speed, 10);
            assert!(vehicles[0].blocked_ctr > 0);
        } else {
            assert_eq!(
                vehicles[0].overtaking,
                crate::road_movement::rvsb::RVSB_DRIVE_SIDE
            );
        }
    }

    #[test]
    fn blocked_ctr_above_limit_allows_pass() {
        let mut vehicles = vec![bus_at(1, 0, 2, 40), bus_at(2, 1, 4, 10)];
        vehicles[0].blocked_ctr = BLOCKED_CTR_LIMIT;
        assert!(!apply_road_veh_close_to(&mut vehicles, 0, None));
        assert_eq!(vehicles[0].blocked_ctr, 0);
    }

    #[test]
    fn faster_leader_never_accelerates_follower() {
        let mut vehicles = vec![bus_at(1, 0, 2, 10), bus_at(2, 1, 4, 40)];
        assert!(apply_road_veh_close_to(&mut vehicles, 0, None));
        assert_eq!(vehicles[0].cur_speed, 10);
    }

    #[test]
    fn vehicle_overtaking_in_other_lane_does_not_block() {
        let mut vehicles = vec![bus_at(1, 0, 2, 40), bus_at(2, 1, 4, 10)];
        vehicles[1].overtaking = crate::road_movement::rvsb::RVSB_DRIVE_SIDE;
        assert_eq!(road_veh_find_close_to(&vehicles, 0), None);
    }

    #[test]
    fn exact_overlap_yields_to_lower_vehicle_id() {
        let mut follower = bus_at(2, 0, 4, 40);
        let leader = bus_at(1, 0, 4, 0);
        follower.road_state = crate::road_movement::rvsb::RVSB_IN_DT_ROAD_STOP;
        let mut vehicles = vec![follower, leader];

        assert!(apply_road_veh_close_to(&mut vehicles, 0, None));
        assert_eq!(vehicles[0].cur_speed, 0);
        assert_eq!(vehicles[0].blocked_ctr, 1);

        vehicles[1].pos = TileCoord::new(4, 0);
        assert!(!apply_road_veh_close_to(&mut vehicles, 0, None));
        assert_eq!(vehicles[0].blocked_ctr, 0);
    }

    #[test]
    fn articulated_parts_are_not_counted_as_independent_traffic() {
        let mut head = bus_at(1, 0, 2, 40);
        let mut part = bus_at(2, 0, 2, 40);
        part.newgrf_articulated = true;
        part.prev_unit = Some(head.id);
        head.next_unit = Some(part.id);
        let vehicles = vec![head, part];

        let mut index = RoadTrafficIndex::default();
        index.rebuild(&vehicles);

        assert_eq!(road_veh_find_close_to_indexed(&vehicles, 0, &index), None);
        assert_eq!(road_veh_find_close_to(&vehicles, 1), None);
    }
}
