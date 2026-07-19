//! Fixture PBS real de OpenTTD 15.3: import + paridad cinemática con oráculo.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use openttdrs_core::{GameState, TileCoord, VehicleKind, sav};
use serde::Deserialize;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct OracleTrain {
    progress: u8,
    speed: u16,
    subspeed: u8,
    direction: u8,
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize)]
struct OracleReservation {
    x: i32,
    y: i32,
    track_bits: u8,
}

#[derive(Debug, Deserialize)]
struct OracleRow {
    kind: String,
    trains: Option<Vec<OracleTrain>>,
    rail_reservations: Option<Vec<OracleReservation>>,
}

fn load_oracle() -> Vec<OracleRow> {
    let raw = std::fs::read_to_string(fixture_path("parity/train_pbs_15_3_openttd.jsonl"))
        .expect("traza oráculo PBS");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("JSONL oráculo"))
        .collect()
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
        .find(|vehicle| vehicle.kind == VehicleKind::Train)
        .expect("tren importado");
    assert_eq!(imported_train.progress, 51, "progreso físico de VEHS");
    assert_eq!(imported_train.cur_speed, 73);
    assert_eq!(imported_train.subspeed, 52);
    assert_eq!(imported_train.direction, 1);
    assert_eq!(imported_train.rail_pixel, 5, "píxel derivado de x_pos");
    assert!(imported_train.cur_speed > 0, "velocidad de VEHS");

    state.step();
    let train = state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.kind == VehicleKind::Train)
        .expect("tren tras tick");
    assert_eq!(train.pos, TileCoord::new(47, 37));
    assert_eq!(train.progress, 159, "primer tick = oráculo OpenTTD");
    assert_eq!(train.cur_speed, 73);
    assert_eq!(train.subspeed, 170);
    assert_eq!(train.dest, TileCoord::new(42, 37));

    let reservations: Vec<_> = {
        let (w, h) = state.map.dimensions();
        let mut out = Vec::new();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let tile = TileCoord::new(x, y);
                let Some(data) = state.map.get(tile) else {
                    continue;
                };
                if data.kind != openttdrs_core::TileKind::Rail {
                    continue;
                }
                let bits = openttdrs_core::decode_rail_reservation_m2_hi(data.m2_hi);
                if bits != 0 {
                    out.push((tile, bits));
                }
            }
        }
        out
    };
    assert_eq!(
        reservations,
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
    let rows = load_oracle();
    assert_eq!(rows.len(), 42, "metadata + initial + 40 ticks");
    assert_eq!(rows[0].kind, "metadata");
    assert_eq!(rows[1].kind, "initial");
    assert_eq!(rows[1].trains.as_ref().unwrap()[0].progress, 51);
    assert_eq!(rows[2].trains.as_ref().unwrap()[0].progress, 159);
    assert_eq!(rows[1].rail_reservations.as_ref().map(Vec::len), Some(5));
}

#[test]
fn rust_matches_openttd_oracle_for_forty_ticks() {
    let oracle = load_oracle();
    let raw = std::fs::read(fixture_path("train_pbs_15_3.sav")).expect("fixture PBS");
    let sav = sav::load(&raw).expect("load");
    let mut state = GameState::from_sav_game(sav);

    let initial = oracle[1].trains.as_ref().unwrap()[0];
    let train = state
        .vehicles
        .iter()
        .find(|v| v.kind == VehicleKind::Train)
        .expect("tren");
    assert_eq!(train.progress, initial.progress);
    assert_eq!(train.cur_speed, initial.speed);
    assert_eq!(train.subspeed, initial.subspeed);
    assert_eq!(train.direction, initial.direction);
    assert_eq!(train.pos, TileCoord::new(initial.x, initial.y));

    for (i, row) in oracle.iter().enumerate().skip(2) {
        state.step();
        let expected = row.trains.as_ref().expect("trains")[0];
        let train = state
            .vehicles
            .iter()
            .find(|v| v.kind == VehicleKind::Train)
            .expect("tren");
        assert_eq!(
            (
                train.progress,
                train.cur_speed,
                train.subspeed,
                train.pos.x,
                train.pos.y,
                train.direction
            ),
            (
                expected.progress,
                expected.speed,
                expected.subspeed,
                expected.x,
                expected.y,
                expected.direction
            ),
            "divergencia en muestra oracle índice {i}"
        );

        let expected_res = row.rail_reservations.as_ref().expect("reservas");
        let mut got = Vec::new();
        let (w, h) = state.map.dimensions();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let tile = TileCoord::new(x, y);
                let Some(data) = state.map.get(tile) else {
                    continue;
                };
                if data.kind != openttdrs_core::TileKind::Rail {
                    continue;
                }
                let bits = openttdrs_core::decode_rail_reservation_m2_hi(data.m2_hi);
                if bits != 0 {
                    got.push((x, y, bits));
                }
            }
        }
        let want: Vec<_> = expected_res
            .iter()
            .map(|r| (r.x, r.y, r.track_bits))
            .collect();
        assert_eq!(got, want, "reservas PBS en muestra oracle índice {i}");
    }
}
