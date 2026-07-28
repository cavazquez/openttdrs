//! Tests del módulo PBS.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{HashSet, VecDeque};

use super::model::RAIL_TB_HORZ;
use super::train_reservation::reservation_ends_at_safe_wait_steps;
use super::*;
use crate::GameState;
use crate::command::{Command, apply_command};
use crate::map::{OTTD_MP_ROAD, TileCoord, TileKind, is_road_level_crossing};
use crate::parity::{
    TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y, TRAIN_DUAL_VEHICLE_2_ID, TRAIN_DUAL_VEHICLE_ID,
    build_train_supply_dual,
};
use crate::rail_signals::{
    RAIL_TILE_NORMAL, SIGTYPE_BLOCK, SIGTYPE_PATH, update_rail_signal_states,
};
use crate::vehicle::{Vehicle, VehicleKind};

#[test]
fn encode_decode_roundtrip_horz_and_single() {
    assert_eq!(decode_rail_reservation_m2_hi(0), 0);
    assert_eq!(
        decode_rail_reservation_m2_hi(encode_rail_reservation_to_m2_hi(0x04)),
        0x04
    );
    assert_eq!(
        decode_rail_reservation_m2_hi(encode_rail_reservation_to_m2_hi(RAIL_TB_HORZ)),
        RAIL_TB_HORZ
    );
}

#[test]
fn parallel_tracks_get_disjoint_reservations() {
    let mut state = build_train_supply_dual();
    {
        let t2 = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        t2.pos = TileCoord::new(7, TRAIN_DUAL_TRACK_RET_Y);
        t2.path = VecDeque::from([
            TileCoord::new(6, TRAIN_DUAL_TRACK_RET_Y),
            TileCoord::new(5, TRAIN_DUAL_TRACK_RET_Y),
        ]);
        t2.running = true;
    }
    {
        let t1 = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren 1");
        t1.pos = TileCoord::new(5, TRAIN_DUAL_TRACK_OUT_Y);
        t1.path = VecDeque::from([
            TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y),
            TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y),
        ]);
        t1.running = true;
    }

    let mut dirty = Vec::new();
    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state.vehicles,
        &mut dirty,
        true,
    );
    update_train_reservations(&state.map, &mut state.vehicles);
    let t1 = state.vehicles.iter().find(|v| v.id == 1).expect("tren 1");
    let t2 = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
        .expect("tren 2");
    assert!(
        t1.reserved_steps
            .iter()
            .all(|s| s.tile.y == TRAIN_DUAL_TRACK_OUT_Y)
    );
    assert!(
        t2.reserved_steps
            .iter()
            .all(|s| s.tile.y == TRAIN_DUAL_TRACK_RET_Y)
    );
    assert!(
        t1.reserved_steps.len() >= 3,
        "tren 1 reserva ida: {:?}",
        t1.reserved_steps
    );
    assert!(
        t2.reserved_steps.len() >= 3,
        "tren 2 reserva vuelta: {:?}",
        t2.reserved_steps
    );
}

#[test]
fn disjoint_tracks_on_same_tile_do_not_conflict() {
    let tile = TileCoord::new(5, 4);
    let upper = 0x04;
    let lower = 0x08;
    let mut reserved = HashSet::from([ReservedRailStep::new(tile, upper)]);
    let lower_step = ReservedRailStep::new(tile, lower);
    assert!(!reserved.contains(&lower_step));
    reserved.insert(lower_step);
    assert_eq!(reserved.len(), 2);
}

#[test]
fn follower_reservation_stops_before_leader_on_same_track() {
    let mut state = build_train_supply_dual();
    let leader_pos = TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y);
    let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);
    {
        let leader = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren 1");
        leader.pos = leader_pos;
        leader.path.clear();
        leader.running = true;
    }
    let mut follower = crate::vehicle::Vehicle::new(
        2,
        VehicleKind::Train,
        follower_pos,
        TileCoord::new(13, TRAIN_DUAL_TRACK_OUT_Y),
    );
    follower.path = VecDeque::from([
        TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
        leader_pos,
        TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y),
    ]);
    follower.running = true;
    state.vehicles.push(follower);

    update_train_reservations(&state.map, &mut state.vehicles);
    let follower = state.vehicles.iter().find(|v| v.id == 2).expect("tren 2");
    assert!(
        follower
            .reserved_steps
            .iter()
            .all(|s| s.tile.x <= follower_pos.x),
        "no debe reservar más allá del líder: {:?}",
        follower.reserved_steps
    );
    assert!(
        !follower
            .reserved_steps
            .iter()
            .any(|s| s.tile.x > follower_pos.x),
        "reserva cortada antes del líder: {:?}",
        follower.reserved_steps
    );
}

#[test]
fn connector_tile_stays_reserved_while_train_turns() {
    let mut state = build_train_supply_dual();
    let connector = TileCoord::new(10, 5);
    {
        let leader = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren 1");
        leader.pos = connector;
        leader.path = VecDeque::from([TileCoord::new(10, TRAIN_DUAL_TRACK_RET_Y)]);
        leader.running = true;
    }

    update_train_reservations(&state.map, &mut state.vehicles);
    let leader = state.vehicles.iter().find(|v| v.id == 1).expect("tren 1");
    assert!(
        leader.reserved_steps.iter().any(|s| s.tile == connector),
        "conector ocupado: {:?}",
        leader.reserved_steps
    );
}

#[test]
fn sync_sets_m2_reservation_bits_on_rail() {
    let mut state = build_train_supply_dual();
    let tile = TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y);
    let rails_before = state.map.get(tile).expect("vía").m5 & 0x3F;
    let track =
        track_on_departure_tile(&state.map, tile, TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y))
            .expect("pista");
    state.vehicles[0].reserved_steps = vec![ReservedRailStep::new(tile, track)];
    let mut prev = HashSet::new();
    let mut dirty = Vec::new();
    sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
    let t = state.map.get(tile).expect("vía");
    assert_eq!(
        t.m5 & 0x3F,
        rails_before,
        "reserva no debe alterar TrackBits"
    );
    assert_ne!(decode_rail_reservation_m2_hi(t.m2_hi), 0);
    assert!(!dirty.is_empty());

    state.vehicles[0].reserved_steps.clear();
    sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
    let t = state.map.get(tile).expect("vía");
    assert_eq!(decode_rail_reservation_m2_hi(t.m2_hi), 0);
}

#[test]
fn sync_sets_crossing_m5_reservation_bit() {
    let mut state = GameState::new(8, 4);
    let c = TileCoord::new(2, 1);
    state.map.set_kind(c, TileKind::Road).expect("road");
    let mut t = state.map.get(c).expect("tile");
    t.mapt = OTTD_MP_ROAD << 4;
    t.m5 = 1 << 6; // RoadTileType::Crossing
    state.map.set_tile(c, t).expect("crossing");
    assert!(is_road_level_crossing(
        state.map.get(c).unwrap().mapt,
        state.map.get(c).unwrap().m5,
        TileKind::Road
    ));

    let mut train = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, 1),
        TileCoord::new(3, 1),
    );
    train.running = true;
    train.reserved_steps = vec![ReservedRailStep::new(c, 0x01)];
    state.vehicles = vec![train];

    let mut prev = HashSet::new();
    let mut dirty = Vec::new();
    sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
    let t = state.map.get(c).expect("crossing");
    assert_ne!(t.m5 & CROSSING_RESERVATION_M5_BIT, 0);
    assert!(dirty.contains(&c));

    state.vehicles[0].reserved_steps.clear();
    sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
    let t = state.map.get(c).expect("crossing");
    assert_eq!(t.m5 & CROSSING_RESERVATION_M5_BIT, 0);
}

/// Path rojo no debe impedir extender la reserva (rompe deadlock reserva↔verde).
#[test]
fn path_signal_allows_reservation_while_red() {
    const RAIL_TB_X: u8 = 0x01;
    let mut state = GameState::new(12, 4);
    let y = 1;
    for x in 0..=8 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
        t.m5 = RAIL_TB_X | (RAIL_TILE_NORMAL << 6);
        state.map.set_tile(TileCoord::new(x, y), t).expect("x");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(6, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path 2");

    let mut train = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(8, y),
    );
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
        TileCoord::new(8, y),
    ]);
    train.running = true;
    state.vehicles = vec![train];

    let mut dirty = Vec::new();
    // Primera pasada: path queda rojo (sin reserva aún).
    update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
    let sig = state.map.get(TileCoord::new(3, y)).expect("sig");
    assert!(
        crate::rail_signals::rail_signal_state_mask(sig.m3hi) == 0
            || !crate::rail_signals::signal_is_green(sig.m3hi, 0)
                && !crate::rail_signals::signal_is_green(sig.m3hi, 2),
        "path debería estar rojo antes de reservar: m3hi={:#x}",
        sig.m3hi
    );

    update_train_reservations(&state.map, &mut state.vehicles);
    let reserved = &state.vehicles[0].reserved_steps;
    assert!(
        reserved.iter().any(|s| s.tile.x >= 4),
        "reserva debe cruzar path roja: {reserved:?}"
    );

    update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, false);
    let sig = state.map.get(TileCoord::new(3, y)).expect("sig");
    let present = crate::rail_signals::rail_signal_present_mask(sig.m3);
    let any_green = (0..4u8).any(|bit| {
        present & (1 << bit) != 0 && crate::rail_signals::signal_is_green(sig.m3hi, bit)
    });
    assert!(
        any_green,
        "path debe ponerse verde con reserva: m3hi={:#x}",
        sig.m3hi
    );
}

/// Dos corredores paralelos con path signals: ambos reservan sin deadlock (rojo↔reserva).
#[test]
fn path_signals_parallel_corridors_reserve() {
    let mut state = GameState::new(16, 8);
    for &y in &[2, 4] {
        for x in 1..=10 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
            let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
            t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6); // TRACK_X
            state.map.set_tile(TileCoord::new(x, y), t).expect("x");
        }
        for &x in &[3, 7] {
            apply_command(
                &mut state,
                &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
            )
            .expect("path");
        }
    }

    let mut t1 = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(2, 2),
        TileCoord::new(9, 2),
    );
    t1.path = VecDeque::from([
        TileCoord::new(3, 2),
        TileCoord::new(4, 2),
        TileCoord::new(5, 2),
        TileCoord::new(6, 2),
        TileCoord::new(7, 2),
        TileCoord::new(8, 2),
        TileCoord::new(9, 2),
    ]);
    t1.running = true;

    let mut t2 = crate::vehicle::Vehicle::new(
        2,
        VehicleKind::Train,
        TileCoord::new(2, 4),
        TileCoord::new(9, 4),
    );
    t2.path = VecDeque::from([
        TileCoord::new(3, 4),
        TileCoord::new(4, 4),
        TileCoord::new(5, 4),
        TileCoord::new(6, 4),
        TileCoord::new(7, 4),
        TileCoord::new(8, 4),
        TileCoord::new(9, 4),
    ]);
    t2.running = true;
    state.vehicles = vec![t1, t2];

    let mut dirty = Vec::new();
    update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
    update_train_reservations(&state.map, &mut state.vehicles);

    let r1 = &state.vehicles[0].reserved_steps;
    let r2 = &state.vehicles[1].reserved_steps;
    assert!(r1.len() >= 4, "tren norte reserva: {r1:?}");
    assert!(r2.len() >= 4, "tren sur reserva: {r2:?}");
    assert!(
        r1.iter().all(|s| s.tile.y == 2) && r2.iter().all(|s| s.tile.y == 4),
        "reservas disjuntas por corredor: {r1:?} / {r2:?}"
    );
    assert!(
        r1.iter().any(|s| s.tile.x >= 4) && r2.iter().any(|s| s.tile.x >= 4),
        "ambos cruzan path: {r1:?} / {r2:?}"
    );
    // Safe wait: cortar delante de la 2.ª path (x=7) → último paso x=6.
    assert!(
        r1.iter().map(|s| s.tile.x).max() == Some(6),
        "debe cortar en safe wait delante de path x=7: {r1:?}"
    );
    assert!(
        reservation_ends_at_safe_wait(&state.map, &state.vehicles[0]),
        "reserva completa hasta safe wait"
    );
}

#[test]
fn depot_is_safe_waiting_position() {
    let mut state = GameState::new(12, 6);
    let y = 2;
    for x in 1..=6 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailDepotDir(TileCoord::new(1, 3), 3),
    )
    .expect("depósito");
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(4, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");

    let depot = TileCoord::new(1, 3);
    assert!(
        is_safe_waiting_position(&state.map, depot, Some(TileCoord::new(1, y)), false),
        "depósito es safe wait"
    );

    let mut train =
        crate::vehicle::Vehicle::new(1, VehicleKind::Train, TileCoord::new(2, y), depot);
    // Path hacia el depósito vía (1,y) → (1,3).
    train.path = VecDeque::from([
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
    ]);
    train.running = true;
    state.vehicles = vec![train];
    update_train_reservations(&state.map, &mut state.vehicles);
    let reserved = &state.vehicles[0].reserved_steps;
    // Sin depósito en el path: corta en fin de path (x=6) o delante de nada.
    assert!(
        reservation_ends_at_safe_wait(&state.map, &state.vehicles[0]),
        "fin de path es safe wait: {reserved:?}"
    );
}

#[test]
fn reservation_stops_before_next_path_signal() {
    let mut state = GameState::new(14, 4);
    let y = 1;
    for x in 0..=10 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
        t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
        state.map.set_tile(TileCoord::new(x, y), t).expect("x");
    }
    for &x in &[3, 7] {
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");
    }

    let mut train = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(10, y),
    );
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
        TileCoord::new(8, y),
        TileCoord::new(9, y),
        TileCoord::new(10, y),
    ]);
    train.running = true;
    state.vehicles = vec![train];
    update_train_reservations(&state.map, &mut state.vehicles);
    let reserved = &state.vehicles[0].reserved_steps;
    let max_x = reserved.iter().map(|s| s.tile.x).max();
    assert_eq!(
        max_x,
        Some(6),
        "reserva hasta delante de path x=7, no más allá: {reserved:?}"
    );
    assert!(
        !reserved.iter().any(|s| s.tile.x >= 7),
        "no debe incluir la 2.ª path ni más allá: {reserved:?}"
    );
    assert!(reservation_ends_at_safe_wait(
        &state.map,
        &state.vehicles[0]
    ));
}

#[test]
fn wait_for_pbs_path_marks_stuck_and_reverses_on_timeout() {
    use crate::pathfinding_settings::PathfindingSettings;
    use crate::vehicle::DIR_SW;

    let mut state = GameState::new(12, 4);
    let y = 1;
    for x in 0..=8 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
        t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
        state.map.set_tile(TileCoord::new(x, y), t).expect("x");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");

    // Bloqueador en el bloque: impide reserva completa.
    let mut blocker = crate::vehicle::Vehicle::new(
        2,
        VehicleKind::Train,
        TileCoord::new(5, y),
        TileCoord::new(5, y),
    );
    blocker.running = true;
    blocker.path.clear();

    let mut train = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(2, y),
        TileCoord::new(8, y),
    );
    train.path = VecDeque::from([
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
        TileCoord::new(8, y),
    ]);
    train.running = true;
    train.direction = DIR_SW; // hacia +x
    state.vehicles = vec![train, blocker];
    update_train_reservations(&state.map, &mut state.vehicles);

    assert!(
        train_waiting_for_pbs_path(&state.map, &state.vehicles[0]),
        "debe esperar path sin reserva completa"
    );

    let settings = PathfindingSettings {
        wait_for_pbs_path: 2, // 2 días → 148 ticks
        ..Default::default()
    };
    let timeout = settings.pbs_reverse_timeout_ticks().expect("timeout");
    let dir_before = state.vehicles[0].direction;

    for _ in 0..timeout.saturating_sub(1) {
        let reversed =
            tick_pbs_wait_and_maybe_reverse(&state.map, &mut state.vehicles[0], settings, false);
        assert!(!reversed);
        assert!(state.vehicles[0].pbs_stuck);
    }
    let reversed =
        tick_pbs_wait_and_maybe_reverse(&state.map, &mut state.vehicles[0], settings, false);
    assert!(reversed, "debe girar al timeout");
    assert_ne!(state.vehicles[0].direction, dir_before);
    assert!(!state.vehicles[0].pbs_stuck);
    assert!(state.vehicles[0].path.is_empty());
}

#[test]
fn wait_for_pbs_path_255_never_reverses() {
    use crate::pathfinding_settings::{PBS_WAIT_FOREVER, PathfindingSettings};

    let mut state = GameState::new(10, 4);
    let y = 1;
    for x in 0..=6 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(2, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");
    let mut blocker = crate::vehicle::Vehicle::new(
        2,
        VehicleKind::Train,
        TileCoord::new(4, y),
        TileCoord::new(4, y),
    );
    blocker.running = true;
    let mut train = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(6, y),
    );
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
    ]);
    train.running = true;
    state.vehicles = vec![train, blocker];
    update_train_reservations(&state.map, &mut state.vehicles);

    let settings = PathfindingSettings {
        wait_for_pbs_path: PBS_WAIT_FOREVER,
        ..Default::default()
    };
    for _ in 0..500 {
        assert!(!tick_pbs_wait_and_maybe_reverse(
            &state.map,
            &mut state.vehicles[0],
            settings,
            false
        ));
    }
    assert!(state.vehicles[0].pbs_stuck);
}

#[test]
fn find_path_to_safe_wait_reaches_next_path() {
    let mut state = GameState::new(14, 4);
    let y = 1;
    for x in 0..=10 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
        t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
        state.map.set_tile(TileCoord::new(x, y), t).expect("x");
    }
    for &x in &[3, 7] {
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
        )
        .expect("path");
    }
    let from = TileCoord::new(1, y);
    let preferred = [
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
    ];
    let path = find_path_to_safe_wait(&state.map, &[], 1, from, &preferred, &HashSet::new())
        .expect("safe wait path");
    assert!(
        path.iter().any(|c| c.x == 6),
        "debe llegar delante de path x=7: {path:?}"
    );
    assert!(
        !path.iter().any(|c| c.x >= 7),
        "no debe incluir la 2.ª path: {path:?}"
    );
}

/// Línea principal bloqueada + desvío libre: `TryReserve` solo corre en ticks de backoff.
#[test]
#[allow(clippy::too_many_lines)]
fn path_backoff_interval_throttles_try_reserve() {
    use crate::pathfinding_settings::PathfindingSettings;

    let mut state = GameState::new(12, 6);
    let y = 2;
    // Vía principal X.
    for x in 0..=8 {
        apply_command(
            &mut state,
            &Command::SetRailBits(TileCoord::new(x, y), 0x01),
        )
        .expect("vía X");
    }
    // Cruce + desvío hacia y=0 (fin de vía = safe wait).
    apply_command(
        &mut state,
        &Command::SetRailBits(TileCoord::new(4, y), 0x03),
    )
    .expect("cruce");
    apply_command(
        &mut state,
        &Command::SetRailBits(TileCoord::new(4, 1), 0x02),
    )
    .expect("desvío");
    apply_command(
        &mut state,
        &Command::SetRailBits(TileCoord::new(4, 0), 0x02),
    )
    .expect("fin desvío");
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");

    let mut blocker = crate::vehicle::Vehicle::new(
        2,
        VehicleKind::Train,
        TileCoord::new(6, y),
        TileCoord::new(6, y),
    );
    blocker.running = true;
    blocker.path.clear();

    let mut train = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(2, y),
        TileCoord::new(8, y),
    );
    train.path = VecDeque::from([
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
        TileCoord::new(8, y),
    ]);
    train.running = true;
    train.pbs_stuck = true;
    state.vehicles = vec![train, blocker];

    let settings = PathfindingSettings {
        path_backoff_interval: 20,
        ..Default::default()
    };

    // Tick intermedio: no TryReserve → no usa el desvío.
    state.vehicles[0].wait_counter = 7;
    let mid = compute_train_reservation_with_settings(
        &state.map,
        &state.vehicles,
        0,
        &HashSet::new(),
        settings,
    );
    assert!(
        !mid.iter().any(|s| s.tile.y != y),
        "sin backoff no debe desviarse: {mid:?}"
    );
    assert!(
        !reservation_ends_at_safe_wait_steps(
            &state.map,
            state.vehicles[0].pos,
            &state.vehicles[0].path.iter().copied().collect::<Vec<_>>(),
            &mid
        ),
        "reserva intermedia incompleta: {mid:?}"
    );

    // Múltiplo del intervalo: TryReserve encuentra el desvío hasta safe wait.
    state.vehicles[0].wait_counter = 40;
    let on_backoff = compute_train_reservation_with_settings(
        &state.map,
        &state.vehicles,
        0,
        &HashSet::new(),
        settings,
    );
    assert!(
        on_backoff.iter().any(|s| s.tile == TileCoord::new(4, 0)),
        "con backoff debe reservar el desvío: {on_backoff:?}"
    );
    assert!(
        reservation_ends_at_safe_wait_steps(
            &state.map,
            state.vehicles[0].pos,
            &state.vehicles[0].path.iter().copied().collect::<Vec<_>>(),
            &on_backoff
        ),
        "desvío debe terminar en safe wait: {on_backoff:?}"
    );

    // 255: look-ahead off aunque wait_counter sea múltiplo.
    let off = PathfindingSettings {
        path_backoff_interval: crate::pathfinding_settings::PBS_WAIT_FOREVER,
        ..Default::default()
    };
    state.vehicles[0].wait_counter = 40;
    let forever = compute_train_reservation_with_settings(
        &state.map,
        &state.vehicles,
        0,
        &HashSet::new(),
        off,
    );
    assert!(
        !forever.iter().any(|s| s.tile.y != y),
        "255 no debe hacer TryReserve: {forever:?}"
    );
}

#[test]
fn consist_tail_blocks_other_train_reservation() {
    // Tren largo (historial) ocupa (3,1); otro no puede reservar esa tesela.
    let mut state = GameState::new(12, 4);
    let y = 1;
    for x in 0..=8 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
    }
    let mut leader = crate::vehicle::Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(4, y),
        TileCoord::new(8, y),
    );
    leader.running = true;
    leader.unit_length = 100;
    leader.cached_total_length = 300; // span ≥ 2
    leader.rail_tile_history = VecDeque::from([TileCoord::new(3, y), TileCoord::new(2, y)]);
    leader.path = VecDeque::from([
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
        TileCoord::new(8, y),
    ]);
    let mut follower = crate::vehicle::Vehicle::new(
        2,
        VehicleKind::Train,
        TileCoord::new(0, y),
        TileCoord::new(8, y),
    );
    follower.running = true;
    follower.path = VecDeque::from([
        TileCoord::new(1, y),
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
    ]);
    state.vehicles = vec![leader, follower];
    update_train_reservations(&state.map, &mut state.vehicles);
    let follower_res = &state.vehicles[1].reserved_steps;
    assert!(
        !follower_res.iter().any(|s| s.tile == TileCoord::new(3, y)),
        "no debe reservar la cola del líder: {follower_res:?}"
    );
}

#[test]
fn platform_reservation_appended_for_station_order() {
    let mut state = GameState::new(16, 12);
    for x in 2..=8 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 6))).expect("vía");
    }
    let station = TileCoord::new(1, 6);
    apply_command(&mut state, &Command::PlaceRailStation(station, 2)).expect("estación");
    let platforms = crate::station::rail_station_platform_tiles(&state.map, station);
    assert!(!platforms.is_empty());
    let mut train =
        crate::vehicle::Vehicle::new(1, VehicleKind::Train, TileCoord::new(4, 6), station);
    train.running = true;
    train.set_vehicle_orders(vec![crate::vehicle::VehicleOrder::station(station)]);
    train.sync_order_destination(&state.map);
    train.path = VecDeque::from([TileCoord::new(3, 6), TileCoord::new(2, 6), station]);
    state.vehicles = vec![train];
    update_train_reservations(&state.map, &mut state.vehicles);
    let reserved = &state.vehicles[0].reserved_steps;
    assert!(
        platforms
            .iter()
            .any(|p| reserved.iter().any(|s| s.tile == *p)),
        "debe reservar plataforma: platforms={platforms:?} reserved={reserved:?}"
    );
}

#[test]
fn follow_train_reservation_keeps_previous_when_try_reserve_empty() {
    let pos = TileCoord::new(3, 0);
    let mut vehicle = Vehicle::new(1, VehicleKind::Train, pos, TileCoord::new(8, 0));
    vehicle.path = VecDeque::from([
        TileCoord::new(4, 0),
        TileCoord::new(5, 0),
        TileCoord::new(6, 0),
    ]);
    let previous = vec![
        ReservedRailStep::new(pos, 0x01),
        ReservedRailStep::new(TileCoord::new(4, 0), 0x01),
        ReservedRailStep::new(TileCoord::new(5, 0), 0x01),
    ];
    let kept = follow_train_reservation(&previous, Vec::new(), &vehicle);
    assert_eq!(kept.len(), 3);
    assert!(kept.iter().any(|s| s.tile == pos));
    assert!(kept.iter().any(|s| s.tile == TileCoord::new(5, 0)));
    // Pasos detrás del path (fuera de path y pos) no se conservan.
    let previous_stale = vec![
        ReservedRailStep::new(TileCoord::new(1, 0), 0x01),
        ReservedRailStep::new(pos, 0x01),
    ];
    let kept_stale = follow_train_reservation(&previous_stale, Vec::new(), &vehicle);
    assert!(!kept_stale.iter().any(|s| s.tile == TileCoord::new(1, 0)));
    assert!(kept_stale.iter().any(|s| s.tile == pos));
}

#[test]
fn free_train_track_reservation_clears_steps_and_m2() {
    let mut state = GameState::new(10, 4);
    let y = 1;
    for x in 0..=6 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(2, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");
    let mut train = Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(6, y),
    );
    train.running = true;
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
    ]);
    train.reserved_steps = vec![
        ReservedRailStep::new(TileCoord::new(1, y), 0x01),
        ReservedRailStep::new(TileCoord::new(2, y), 0x01),
        ReservedRailStep::new(TileCoord::new(3, y), 0x01),
    ];
    state.vehicles = vec![train];
    sync_reservations_to_map(
        &mut state.map,
        &state.vehicles,
        &mut state.runtime.reservation_tiles_active,
        &mut Vec::new(),
    );
    assert_ne!(
        decode_rail_reservation_m2_hi(state.map.get(TileCoord::new(2, y)).unwrap().m2_hi),
        0
    );
    let mut dirty = Vec::new();
    free_train_track_reservation(&mut state.map, &mut state.vehicles[0], &mut dirty);
    assert!(state.vehicles[0].reserved_steps.is_empty());
    assert_eq!(
        decode_rail_reservation_m2_hi(state.map.get(TileCoord::new(2, y)).unwrap().m2_hi),
        0
    );
    assert!(!dirty.is_empty());
}

#[test]
fn choose_train_track_reserves_on_enter() {
    let mut state = GameState::new(10, 4);
    let y = 1;
    for x in 0..=6 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
    }
    let mut train = Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(6, y),
    );
    train.running = true;
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
    ]);
    let chosen = choose_train_track_on_enter(&state.map, &mut train, None);
    assert!(chosen.is_some());
    assert!(
        train
            .reserved_steps
            .iter()
            .any(|s| s.tile == TileCoord::new(2, y)),
        "debe reservar la tesela de entrada"
    );
}

/// #222: red solo block no crea reservas PBS con `reserve_paths=false` (default).
#[test]
fn block_only_network_skips_pbs_reservation_unless_reserve_paths() {
    use crate::pathfinding_settings::PathfindingSettings;

    let mut state = GameState::new(12, 4);
    let y = 1;
    for x in 0..=8 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
        t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
        state.map.set_tile(TileCoord::new(x, y), t).expect("x");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_BLOCK),
    )
    .expect("block");
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(6, y), 0, 128, 128, SIGTYPE_BLOCK),
    )
    .expect("block 2");

    let mut train = Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(8, y),
    );
    train.running = true;
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
        TileCoord::new(6, y),
        TileCoord::new(7, y),
        TileCoord::new(8, y),
    ]);
    state.vehicles = vec![train];

    let default_pf = PathfindingSettings::default();
    assert!(!default_pf.reserve_paths);
    let empty = HashSet::new();
    let none =
        compute_train_reservation_with_settings(&state.map, &state.vehicles, 0, &empty, default_pf);
    assert!(
        none.is_empty(),
        "block-only + reserve_paths=false → sin reserva PBS: {none:?}"
    );

    assert!(
        !crate::rail_pbs::train_reservation::vehicle_segment_requires_path_reserve(
            &state.map,
            &state.vehicles[0],
        ),
        "red solo block no clasifica segmento PBS"
    );

    let mut force = default_pf;
    force.reserve_paths = true;
    let forced =
        compute_train_reservation_with_settings(&state.map, &state.vehicles, 0, &empty, force);
    assert!(
        forced.iter().any(|s| s.tile.x > 1),
        "reserve_paths=true fuerza reserva por delante: {forced:?}"
    );
}

/// #222: path signal delante activa reserva con `reserve_paths=false`.
#[test]
fn path_signal_segment_reserves_with_default_reserve_paths() {
    use crate::pathfinding_settings::PathfindingSettings;

    let mut state = GameState::new(12, 4);
    let y = 1;
    for x in 0..=8 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía");
        let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
        t.m5 = 0x01 | (RAIL_TILE_NORMAL << 6);
        state.map.set_tile(TileCoord::new(x, y), t).expect("x");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TileCoord::new(3, y), 0, 128, 128, SIGTYPE_PATH),
    )
    .expect("path");

    let mut train = Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(1, y),
        TileCoord::new(8, y),
    );
    train.running = true;
    train.path = VecDeque::from([
        TileCoord::new(2, y),
        TileCoord::new(3, y),
        TileCoord::new(4, y),
        TileCoord::new(5, y),
    ]);
    state.vehicles = vec![train];

    assert!(
        crate::rail_pbs::train_reservation::vehicle_segment_requires_path_reserve(
            &state.map,
            &state.vehicles[0],
        ),
        "path en el path de órdenes clasifica PBS"
    );
    let reserved = compute_train_reservation_with_settings(
        &state.map,
        &state.vehicles,
        0,
        &HashSet::new(),
        PathfindingSettings::default(),
    );
    assert!(
        reserved.iter().any(|s| s.tile.x >= 2),
        "default reserve_paths=false aún reserva ante path signal: {reserved:?}"
    );
}
