//! Differential oracle: the traffic predicates from aee9238a, before filtering
//! the mixed fleet once per query. Keep their traversal and first-match rules.

use super::*;
use crate::map::{RAIL_TB_LOWER, RAIL_TB_UPPER, RAIL_TB_X};
use std::collections::VecDeque;

fn corridor() -> Map {
    let mut map = Map::new_flat(96, 4, 0);
    for x in 0..95 {
        let pos = TileCoord::new(x, 1);
        map.set_kind(pos, TileKind::Rail).expect("fixture");
        let mut tile = map.get(pos).expect("fixture");
        tile.m5 = RAIL_TB_X;
        map.set_tile(pos, tile).expect("fixture");
    }
    map
}

fn train(id: u32, x: i32, target: Option<i32>) -> Vehicle {
    let pos = TileCoord::new(x, 1);
    let mut vehicle = Vehicle::new(id, VehicleKind::Train, pos, pos);
    vehicle.running = true;
    if let Some(target) = target {
        vehicle.path = VecDeque::from([TileCoord::new(target, 1)]);
    }
    vehicle
}

fn assert_queries(map: &Map, vehicles: &[Vehicle], vehicle: &Vehicle) -> (bool, bool) {
    let mut fleet = FleetIndex::default();
    fleet.rebuild(vehicles);
    let actual = (
        train_blocked_by_traffic_indexed(map, vehicles, vehicle, &fleet),
        train_facing_head_on_traffic(map, vehicles, vehicle),
    );
    assert_eq!(
        actual,
        (
            reference_blocked(map, vehicles, vehicle, &fleet),
            reference_head_on(map, vehicles, vehicle),
        ),
        "query={:?}, fleet={:?}",
        (vehicle.id, vehicle.kind, vehicle.pos, vehicle.running),
        vehicles
            .iter()
            .map(|v| (v.id, v.kind, v.pos, v.prev_unit, v.running))
            .collect::<Vec<_>>()
    );
    actual
}

#[test]
fn mixed_fleet_matches_previous_queries_at_lookahead_boundaries() {
    let map = corridor();
    let lead = train(1, 2, Some(3));
    for x in [2, 3, 4, 5, 32, 65, 66, 67, 94] {
        for running in [false, true] {
            for target in [None, Some(x - 1), Some(x + 1)] {
                for role in 0..4 {
                    let mut other = train(2, x, target);
                    other.running = running;
                    match role {
                        1 => other.prev_unit = Some(99),
                        2 => other.kind = VehicleKind::Truck,
                        3 => other.kind = VehicleKind::Aircraft,
                        _ => {}
                    }
                    let mut vehicles = vec![lead.clone()];
                    // Ineligible vehicles still occur before and after a head.
                    for id in 100..132 {
                        let mut decoy = train(id, x, Some(x - 1));
                        if id % 2 == 0 {
                            decoy.kind = VehicleKind::Bus;
                        } else {
                            decoy.prev_unit = Some(99);
                        }
                        vehicles.push(decoy);
                    }
                    vehicles.insert(9, other);
                    // A same-ID head must be excluded even at the next tile.
                    vehicles.push(train(lead.id, 3, Some(2)));
                    assert_queries(&map, &vehicles, &lead);
                }
            }
        }
    }
}

#[test]
fn first_head_on_a_tile_keeps_vector_order_and_duplicate_ids() {
    let map = corridor();
    let lead = train(1, 2, Some(3));
    for second_id in [2, 3] {
        let stopped = train(2, 32, None);
        let approaching = train(second_id, 32, Some(31));
        let forward = train(second_id, 32, Some(33));
        for (other, expected) in [(approaching, (true, true)), (forward, (false, false))] {
            let mut vehicles = vec![lead.clone(), stopped.clone(), other];
            assert_eq!(assert_queries(&map, &vehicles, &lead), (true, false));
            vehicles.swap(1, 2);
            assert_eq!(assert_queries(&map, &vehicles, &lead), expected);
        }
    }
}

#[test]
fn current_positions_and_roles_are_observed_after_fleet_mutations() {
    let map = corridor();
    let lead = train(1, 2, Some(3));
    let mut vehicles = vec![lead.clone(), train(2, 32, Some(31))];
    assert_eq!(assert_queries(&map, &vehicles, &lead), (true, true));
    vehicles[1].pos = TileCoord::new(32, 2);
    assert_eq!(assert_queries(&map, &vehicles, &lead), (false, false));
    vehicles[1].pos = TileCoord::new(32, 1);
    vehicles[1].prev_unit = Some(90);
    assert_eq!(assert_queries(&map, &vehicles, &lead), (false, false));
    vehicles.pop();
    vehicles.insert(0, train(2, 32, Some(31)));
    assert_eq!(assert_queries(&map, &vehicles, &lead), (true, true));
    vehicles.remove(0);
    assert_eq!(assert_queries(&map, &vehicles, &lead), (false, false));
}

#[test]
fn footprints_include_wagons_and_cached_history_beyond_lookahead() {
    let map = corridor();
    let lead = train(1, 2, Some(3));
    let mut other = train(20, 80, Some(81));
    let mut wagon = train(21, 3, None);
    other.next_unit = Some(wagon.id);
    wagon.prev_unit = Some(other.id);
    let vehicles = vec![lead.clone(), other.clone(), wagon];
    assert_eq!(assert_queries(&map, &vehicles, &lead), (true, false));

    other.next_unit = None;
    other.cached_total_length = crate::train_consist::TILE_FRACTIONS * 3;
    other.rail_tile_history = VecDeque::from([TileCoord::new(3, 1), TileCoord::new(4, 1)]);
    assert_eq!(
        assert_queries(&map, &[lead.clone(), other], &lead),
        (true, false)
    );
}

#[test]
fn guards_depots_and_branch_termination_keep_previous_decisions() {
    let mut map = corridor();
    let lead = train(1, 2, Some(3));
    let mut vehicles = vec![lead.clone(), train(2, 32, Some(31))];
    for kind in [VehicleKind::Train, VehicleKind::Truck] {
        for running in [false, true] {
            for prev in [None, Some(9)] {
                for target in [None, Some(3)] {
                    let mut query = train(1, 2, target);
                    query.kind = kind;
                    query.running = running;
                    query.prev_unit = prev;
                    assert_queries(&map, &vehicles, &query);
                }
            }
        }
    }
    let branch = TileCoord::new(15, 1);
    let mut tile = map.get(branch).expect("fixture");
    tile.m5 = RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_LOWER;
    map.set_tile(branch, tile).expect("fixture");
    // Neighbours need connected rails for both diverging tracks.
    for pos in [TileCoord::new(15, 0), TileCoord::new(15, 2)] {
        map.set_kind(pos, TileKind::Rail).expect("fixture");
        let mut tile = map.get(pos).expect("fixture");
        tile.m5 = crate::map::RAIL_TB_Y;
        map.set_tile(pos, tile).expect("fixture");
    }
    assert!(rail_neighbors(&map, branch, Some(TileCoord::new(14, 1))).len() > 1);
    assert_eq!(assert_queries(&map, &vehicles, &lead), (false, false));

    map.set_kind(lead.pos, TileKind::RailDepot)
        .expect("fixture");
    vehicles[1] = train(2, 2, None);
    assert_eq!(assert_queries(&map, &vehicles, &lead), (false, false));
    map.set_kind(lead.pos, TileKind::Rail).expect("fixture");
    assert_eq!(assert_queries(&map, &vehicles, &lead), (true, false));
}

fn reference_blocked(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    fleet: &FleetIndex,
) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    // Solo la cabeza del consist se mueve / bloquea.
    if !vehicle.is_consist_head() {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    let self_id = vehicle.id;
    let foreign = |v: &Vehicle| {
        // Ambos son cabezas: si sus IDs difieren, necesariamente son consists
        // distintos. `same_consist` reconstruía la topología completa para
        // cada comparación de tráfico.
        v.kind == VehicleKind::Train && v.is_consist_head() && v.id != self_id
    };

    if vehicles.iter().any(|v| foreign(v) && v.pos == next) {
        return true;
    }

    // Varios trenes pueden compartir la misma tesela de depósito (OpenTTD).
    if map.get_kind(vehicle.pos) != Some(crate::map::TileKind::RailDepot)
        && vehicles.iter().any(|v| foreign(v) && v.pos == vehicle.pos)
    {
        return true;
    }

    // Colisión con huella multi-tesela de otro consist.
    let self_tiles = consist_occupied_tiles_indexed(vehicles, fleet, self_id);
    for other in vehicles.iter().filter(|v| foreign(v)) {
        let other_tiles = consist_occupied_tiles_indexed(vehicles, fleet, other.id);
        if other_tiles.contains(&next) {
            return true;
        }
        // Varios trenes pueden compartir un depósito (OpenTTD); el solape de
        // huella solo bloquea fuera de teselas `RailDepot`.
        let footprint_conflict = self_tiles.iter().any(|t| {
            other_tiles.contains(t)
                && *t != vehicle.pos
                && map.get_kind(*t) != Some(crate::map::TileKind::RailDepot)
        });
        if footprint_conflict {
            return true;
        }
    }

    let mut prev = vehicle.pos;
    let mut cur = next;
    for dist in 0..64u8 {
        if let Some(other) = vehicles.iter().find(|v| foreign(v) && v.pos == cur) {
            if !other.running {
                return true;
            }
            if let Some(other_next) = other.movement_target() {
                // Frente a frente: bloquear a cualquier distancia.
                if other_next == prev || other_next == vehicle.pos {
                    return true;
                }
                // Misma dirección: solo evitar embestir de cerca (no deadlock a 50 teselas).
                return dist < 2;
            }
            return true;
        }

        let neighbors = rail_neighbors(map, cur, Some(prev));
        let continuations: Vec<_> = neighbors.into_iter().filter(|n| *n != prev).collect();
        if continuations.len() != 1 {
            break;
        }
        prev = cur;
        cur = continuations[0];
    }
    false
}

fn reference_head_on(map: &Map, vehicles: &[Vehicle], vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running || !vehicle.is_consist_head() {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    let self_id = vehicle.id;
    let foreign =
        |v: &Vehicle| v.kind == VehicleKind::Train && v.is_consist_head() && v.id != self_id;

    let mut prev = vehicle.pos;
    let mut cur = next;
    for _ in 0..64 {
        if let Some(other) = vehicles.iter().find(|v| foreign(v) && v.pos == cur) {
            if let Some(other_next) = other.movement_target() {
                return other_next == prev || other_next == vehicle.pos;
            }
            return false;
        }
        let neighbors = rail_neighbors(map, cur, Some(prev));
        let continuations: Vec<_> = neighbors.into_iter().filter(|n| *n != prev).collect();
        if continuations.len() != 1 {
            break;
        }
        prev = cur;
        cur = continuations[0];
    }
    false
}
