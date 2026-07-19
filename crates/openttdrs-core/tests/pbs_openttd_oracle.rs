//! Fixture PBS real de OpenTTD 15.3: import + reserva inicial.

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use openttdrs_core::{GameState, TileCoord, sav};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn imports_train_pbs_15_3_with_initial_path_reservation() {
    let raw = std::fs::read(fixture_path("train_pbs_15_3.sav")).expect("fixture PBS OpenTTD");
    let sav = sav::load(&raw).expect("cargar fixture PBS OpenTTD");
    assert_eq!(sav.version, 362);
    assert_eq!(sav.map.dimensions(), (64, 64));
    assert_eq!(
        sav.vehicles
            .iter()
            .filter(|vehicle| vehicle.kind == openttdrs_core::SavVehicleKind::Train)
            .count(),
        1,
        "fixture mínimo: un tren"
    );

    let mut state = GameState::from_sav_game(sav);
    let imported_train = state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.kind == openttdrs_core::VehicleKind::Train)
        .expect("tren importado");
    assert_eq!(imported_train.progress, 51, "progreso sub-tesela de VEHS");
    assert!(imported_train.cur_speed > 0, "velocidad de VEHS");
    state.enable_parity_trace();
    state.step();
    let record = state.take_parity_records().pop().expect("un tick");
    let train = record
        .vehicles
        .iter()
        .find(|vehicle| vehicle.rail.is_some())
        .expect("tren importado");
    assert_eq!(train.tile, TileCoord::new(47, 37));
    assert_eq!(
        train.progress, 115,
        "primer tick Rust con velocidad importada"
    );
    assert_eq!(train.order_kind.as_deref(), Some("station"));
    assert_eq!(train.dest, TileCoord::new(42, 37));
    assert_eq!(
        record
            .rail_reservations
            .iter()
            .map(|reservation| (reservation.tile, reservation.track_bits))
            .collect::<Vec<_>>(),
        vec![
            (TileCoord::new(43, 37), 0x01),
            (TileCoord::new(44, 37), 0x01),
            (TileCoord::new(45, 37), 0x01),
            (TileCoord::new(46, 37), 0x01),
            (TileCoord::new(47, 37), 0x01),
        ],
        "la reserva inicial importada debe llegar a la path signal"
    );
}

#[test]
fn oracle_trace_declares_openttd_and_contains_forty_ticks() {
    let raw = std::fs::read_to_string(fixture_path("parity/train_pbs_15_3_openttd.jsonl"))
        .expect("traza oráculo PBS");
    let rows: Vec<serde_json::Value> = raw
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSONL oráculo"))
        .collect();
    assert_eq!(rows.len(), 42, "metadata + initial + 40 ticks");
    assert_eq!(rows[0]["producer"], "openttd");
    assert_eq!(
        rows[0]["openttd_commit"],
        "14ec60f248547d4d062a1160f0fc26d742319888"
    );
    assert_eq!(rows[1]["kind"], "initial");
    assert_eq!(rows[1]["trains"][0]["progress"], 51);
    assert_eq!(rows[2]["trains"][0]["progress"], 159);
    assert_eq!(
        rows[1]["rail_reservations"].as_array().map(Vec::len),
        Some(5)
    );
}
