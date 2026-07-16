//! Núcleo I8 / #21: `apply_command_log` + detección de desync por hash (#108).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::GameState;
use openttdrs_core::command::Command;
use openttdrs_core::map::TileCoord;
use openttdrs_core::parity::build_truck_bay;

#[test]
fn two_worlds_same_log_same_state() {
    let log = [
        Command::PlaceRail(TileCoord::new(2, 2)),
        Command::PlaceRail(TileCoord::new(3, 2)),
        Command::PlaceRail(TileCoord::new(4, 2)),
    ];
    let mut a = GameState::new(24, 24);
    let mut b = GameState::new(24, 24);
    a.apply_command_log(&log).unwrap();
    b.apply_command_log(&log).unwrap();
    for _ in 0..50 {
        a.step();
        b.step();
    }
    assert_eq!(a.canonical_hash(), b.canonical_hash());
}

#[test]
fn desync_detected_on_hash_mismatch() {
    let mut a = build_truck_bay();
    let mut b = build_truck_bay();
    for _ in 0..80 {
        a.step();
        b.step();
    }
    assert_eq!(
        a.canonical_hash(),
        b.canonical_hash(),
        "baseline: mundos idénticos"
    );
    b.economy.money = b.economy.money.saturating_add(42);
    assert_ne!(
        a.canonical_hash(),
        b.canonical_hash(),
        "desync: hash debe divergir tras mutación"
    );
}

#[test]
fn apply_command_log_stops_on_first_error() {
    let mut state = GameState::new(8, 8);
    let log = [
        Command::PlaceRail(TileCoord::new(1, 1)),
        // Fuera de mapa → error; el rail previo debe quedar aplicado.
        Command::PlaceRail(TileCoord::new(100, 100)),
        Command::PlaceRail(TileCoord::new(2, 1)),
    ];
    assert!(state.apply_command_log(&log).is_err());
    assert!(
        state.map.get(TileCoord::new(1, 1)).is_some(),
        "primer comando aplicado"
    );
}
