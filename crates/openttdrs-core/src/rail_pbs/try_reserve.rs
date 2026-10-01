//! `TryPathReserve` con semántica de depósito (`train_cmd.cpp`).

use std::collections::HashSet;

use crate::depot::{has_depot_reservation, rail_depot_entrance_tile, set_depot_reservation};
use crate::map::{Map, TileKind};
use crate::pathfinding_settings::PathfindingSettings;
use crate::vehicle::{Vehicle, VehicleKind};

use super::conflicts::TrainOccupancyIndex;
use super::search::is_safe_waiting_position;
use super::train_reservation::{
    compute_train_reservation_with_settings, compute_train_reservation_with_wormholes_indexed,
    follow_train_reservation, reservation_ends_at_safe_wait_steps,
    vehicle_segment_requires_path_reserve,
};

/// Intenta reservar camino PBS; con cabeza en depósito aplica bit tentativo + rollback.
///
/// Paridad de `TryPathReserve` cuando `moving_front->track == Track::Depot`
/// (`!depot_leave_cleared` es el proxy de `Track::Depot`).
pub fn try_path_reserve(
    map: &mut Map,
    vehicles: &mut [Vehicle],
    vehicle_idx: usize,
    mark_as_stuck: bool,
    settings: PathfindingSettings,
) -> bool {
    try_path_reserve_impl::<true>(map, vehicles, vehicle_idx, mark_as_stuck, settings)
}

fn try_path_reserve_impl<const INDEX_OCCUPANCY: bool>(
    map: &mut Map,
    vehicles: &mut [Vehicle],
    vehicle_idx: usize,
    mark_as_stuck: bool,
    settings: PathfindingSettings,
) -> bool {
    let Some(vehicle) = vehicles.get(vehicle_idx) else {
        return false;
    };
    if vehicle.kind != VehicleKind::Train || !vehicle.is_consist_head() {
        return false;
    }

    let depot_pos = vehicle.pos;
    let in_depot_track =
        map.get_kind(depot_pos) == Some(TileKind::RailDepot) && !vehicle.depot_leave_cleared;

    if in_depot_track {
        if has_depot_reservation(map, depot_pos) {
            if mark_as_stuck {
                vehicles[vehicle_idx].pbs_stuck = true;
            }
            return false;
        }
        if let Some(entrance) = rail_depot_entrance_tile(map, depot_pos)
            && entrance_reserved_by_other(vehicles, vehicle.id, entrance)
        {
            if mark_as_stuck {
                vehicles[vehicle_idx].pbs_stuck = true;
            }
            return false;
        }
    }

    let previous = vehicles[vehicle_idx].reserved_steps.clone();
    let path_hint: Vec<_> = vehicles[vehicle_idx].path.iter().copied().collect();
    let current_is_safe = previous.is_empty()
        && path_hint
            .first()
            .is_some_and(|next| is_safe_waiting_position(map, depot_pos, Some(*next), true));
    let reservation_is_safe =
        reservation_ends_at_safe_wait_steps(map, depot_pos, &path_hint, &previous)
            && previous
                .iter()
                .any(|s| s.tile == depot_pos || path_hint.contains(&s.tile));
    if current_is_safe || reservation_is_safe {
        vehicles[vehicle_idx].pbs_stuck = false;
        return true;
    }

    if in_depot_track {
        let _ = set_depot_reservation(map, depot_pos, true);
    }

    let mut global = HashSet::new();
    for (i, v) in vehicles.iter().enumerate() {
        if i == vehicle_idx || v.kind != VehicleKind::Train || !v.is_consist_head() {
            continue;
        }
        for step in &v.reserved_steps {
            global.insert(*step);
        }
    }

    // This attempt sees the live poses, including a just-reversed consist.
    // Build occupancy once, rather than rebuilding the whole fleet for every
    // train checked on every path edge and platform tile. Duplicate IDs retain
    // the legacy per-vehicle semantics instead of being merged by the index.
    let occupancy = if INDEX_OCCUPANCY
        && (settings.reserve_paths
            || vehicle_segment_requires_path_reserve(map, &vehicles[vehicle_idx]))
    {
        let mut fleet = crate::fleet_index::FleetIndex::default();
        fleet.rebuild(vehicles);
        (!fleet.has_duplicate_ids())
            .then(|| TrainOccupancyIndex::from_vehicles(map, vehicles, &fleet))
    } else {
        None
    };
    let reserved = if let Some(occupancy) = &occupancy {
        compute_train_reservation_with_wormholes_indexed(
            map,
            vehicles,
            vehicle_idx,
            &global,
            settings,
            None,
            occupancy,
        )
    } else {
        compute_train_reservation_with_settings(map, vehicles, vehicle_idx, &global, settings)
    };
    let reserved = follow_train_reservation(&previous, reserved, &vehicles[vehicle_idx]);

    // Vanilla (`pf.reserve_paths=false`) sin path signal delante: no hay reserva PBS
    // que crear. Bloquear aquí dejaba trenes eternos en depósito (#200 + d2a0fdf).
    if reserved.is_empty()
        && !settings.reserve_paths
        && !vehicle_segment_requires_path_reserve(map, &vehicles[vehicle_idx])
    {
        vehicles[vehicle_idx].pbs_stuck = false;
        return true;
    }

    let reached_beyond = reserved.iter().any(|s| s.tile != depot_pos);
    let ends_safe = reservation_ends_at_safe_wait_steps(map, depot_pos, &path_hint, &reserved);
    // Fuera de un depósito, conservar sólo la tesela actual es un resultado
    // válido cuando el tren ya está detenido delante de un conflicto. Es la
    // misma salida que `FollowTrainReservation` usa para un safe wait local;
    // en depósitos se mantiene el requisito estricto de alcanzar la boca.
    let current_only = !in_depot_track && reserved.len() == 1 && reserved[0].tile == depot_pos;
    let ok = !reserved.is_empty() && (reached_beyond || ends_safe || current_only);

    if !ok {
        if in_depot_track {
            let _ = set_depot_reservation(map, depot_pos, false);
        }
        if mark_as_stuck {
            vehicles[vehicle_idx].pbs_stuck = true;
        }
        return false;
    }

    vehicles[vehicle_idx].reserved_steps = reserved;
    vehicles[vehicle_idx].pbs_stuck = false;
    true
}

fn entrance_reserved_by_other(
    vehicles: &[Vehicle],
    self_id: u32,
    tile: crate::map::TileCoord,
) -> bool {
    vehicles.iter().any(|v| {
        v.id != self_id
            && v.kind == VehicleKind::Train
            && v.is_consist_head()
            && v.reserved_steps.iter().any(|s| s.tile == tile)
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command};
    use crate::{GameState, TileCoord, VehicleOrder};
    use std::collections::VecDeque;

    fn reservation_fixture(mode: u8) -> GameState {
        let mut state = GameState::new(16, 12);
        let c = |x| TileCoord::new(x, 6);
        for x in 2..=8 {
            apply_command(&mut state, &Command::PlaceRail(c(x))).expect("rail");
        }
        let station = c(1);
        apply_command(&mut state, &Command::PlaceRailStation(station, 2)).expect("platform");
        let platform = crate::station::rail_station_platform_tiles(&state.map, station);
        let mut train = Vehicle::new(1, VehicleKind::Train, c(4), station);
        train.running = true;
        train.set_vehicle_orders(vec![VehicleOrder::station(station)]);
        train.sync_order_destination(&state.map);
        train.path = VecDeque::from([c(3), c(2), station]);
        state.vehicles.push(train);
        let mut other = Vehicle::new(2, VehicleKind::Train, c(7), c(8));
        other.running = true;
        other.path = VecDeque::from([c(8)]);
        match mode {
            1 => {
                other.pos = c(3);
                other.path = VecDeque::from([c(2), station]);
            }
            2 => {
                other.cached_total_length = 48;
                other.rail_tile_history = VecDeque::from([c(3), c(2)]);
            }
            3 => {
                other.next_unit = Some(3);
                let mut wagon = Vehicle::new(3, VehicleKind::Train, c(3), c(8));
                wagon.prev_unit = Some(other.id);
                state.vehicles.push(wagon);
            }
            4 => {
                other.reserved_steps = platform
                    .iter()
                    .map(|&tile| super::super::ReservedRailStep::new(tile, 1))
                    .collect();
            }
            5 => {
                other.pos = station;
                other.path.clear();
            }
            6 => {
                other.id = 1;
                other.pos = c(3);
            }
            7 => {
                other.kind = VehicleKind::Bus;
                other.pos = c(3);
                state.vehicles[0].next_unit = Some(3);
                let mut own_tail = Vehicle::new(3, VehicleKind::Train, c(3), station);
                own_tail.prev_unit = Some(1);
                state.vehicles.push(own_tail);
            }
            _ => {}
        }
        state.vehicles.push(other);
        state
    }

    #[test]
    fn indexed_attempt_matches_live_occupancy_and_legacy_mutations() {
        let mut successful = 0;
        let mut reserved_platforms = 0;
        for mode in 0..8 {
            for backoff in [0, 255] {
                for mark_as_stuck in [false, true] {
                    let mut state = reservation_fixture(mode);
                    let c = |x| TileCoord::new(x, 6);
                    let platform = crate::station::rail_station_platform_tiles(&state.map, c(1));
                    let mut expected = state.clone();
                    let settings = PathfindingSettings {
                        reserve_paths: true,
                        path_backoff_interval: backoff,
                        ..PathfindingSettings::default()
                    };
                    let before = try_path_reserve_impl::<false>(
                        &mut expected.map,
                        &mut expected.vehicles,
                        0,
                        mark_as_stuck,
                        settings,
                    );
                    let after = try_path_reserve(
                        &mut state.map,
                        &mut state.vehicles,
                        0,
                        mark_as_stuck,
                        settings,
                    );
                    assert_eq!(after, before, "mode {mode}, backoff {backoff}");
                    assert_eq!(state.map.tiles(), expected.map.tiles());
                    assert_eq!(
                        serde_json::to_value(&state.vehicles).expect("vehicles"),
                        serde_json::to_value(&expected.vehicles).expect("legacy vehicles")
                    );
                    assert_eq!(state.random, expected.random);
                    successful += usize::from(after);
                    reserved_platforms += usize::from(
                        state.vehicles[0]
                            .reserved_steps
                            .iter()
                            .any(|step| platform.contains(&step.tile)),
                    );

                    // Moving a previously blocking head must be visible to the
                    // next attempt: occupancy is not retained across calls.
                    let last = state.vehicles.len() - 1;
                    state.vehicles[last].pos = c(8);
                    expected.vehicles[last].pos = c(8);
                    state.vehicles[0].reserved_steps.clear();
                    expected.vehicles[0].reserved_steps.clear();
                    assert_eq!(
                        try_path_reserve(
                            &mut state.map,
                            &mut state.vehicles,
                            0,
                            mark_as_stuck,
                            settings,
                        ),
                        try_path_reserve_impl::<false>(
                            &mut expected.map,
                            &mut expected.vehicles,
                            0,
                            mark_as_stuck,
                            settings,
                        ),
                    );
                    assert_eq!(state.map.tiles(), expected.map.tiles());
                    assert_eq!(
                        serde_json::to_value(&state.vehicles).expect("moved vehicles"),
                        serde_json::to_value(&expected.vehicles).expect("moved legacy vehicles")
                    );
                }
            }
        }
        assert!(successful > 0);
        assert!(reserved_platforms > 0);
    }

    #[test]
    fn indexed_depot_attempt_preserves_tentative_bit_and_rollback() {
        let mut successful = 0;
        let mut blocked = 0;
        for mode in 0..7 {
            for reserve_paths in [false, true] {
                for mark_as_stuck in [false, true] {
                    let mut state = GameState::new(16, 12);
                    let c = |x| TileCoord::new(x, 4);
                    for x in 2..=10 {
                        apply_command(&mut state, &Command::PlaceRail(c(x))).expect("rail");
                    }
                    let depot = TileCoord::new(5, 5);
                    apply_command(&mut state, &Command::PlaceRailDepotDir(depot, 3))
                        .expect("depot");
                    let mut train = Vehicle::new(1, VehicleKind::Train, depot, c(10));
                    train.running = true;
                    train.depot_leave_cleared = false;
                    train.path = (5..=10).map(c).collect();
                    let mut other =
                        Vehicle::new(2, VehicleKind::Train, TileCoord::new(2, 2), c(10));
                    match mode {
                        1 => other.pos = c(5),
                        2 => {
                            train.reserved_steps =
                                vec![super::super::ReservedRailStep::new(c(5), 1)];
                        }
                        3 => {
                            set_depot_reservation(&mut state.map, depot, true);
                        }
                        4 => {
                            other.reserved_steps =
                                vec![super::super::ReservedRailStep::new(c(5), 1)];
                        }
                        5 => train.running = false,
                        6 => train.path.clear(),
                        _ => {}
                    }
                    state.vehicles = vec![train, other];
                    let mut expected = state.clone();
                    let settings = PathfindingSettings {
                        reserve_paths,
                        ..PathfindingSettings::default()
                    };
                    let before = try_path_reserve_impl::<false>(
                        &mut expected.map,
                        &mut expected.vehicles,
                        0,
                        mark_as_stuck,
                        settings,
                    );
                    let after = try_path_reserve(
                        &mut state.map,
                        &mut state.vehicles,
                        0,
                        mark_as_stuck,
                        settings,
                    );
                    assert_eq!(after, before, "mode {mode}, reserve_paths {reserve_paths}");
                    assert_eq!(state.map.tiles(), expected.map.tiles());
                    assert_eq!(
                        serde_json::to_value(&state.vehicles).expect("depot vehicles"),
                        serde_json::to_value(&expected.vehicles).expect("legacy depot vehicles")
                    );
                    assert_eq!(state.random, expected.random);
                    successful += usize::from(after);
                    blocked += usize::from(!after);
                }
            }
        }
        assert!(successful > 0);
        assert!(blocked > 0);
    }
}
