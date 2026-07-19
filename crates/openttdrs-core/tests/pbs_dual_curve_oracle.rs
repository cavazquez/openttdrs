//! Oráculo externo: `train_dual_pbs_curve_15_3.sav` (2 trenes, PBS, curva, plataformas).
//!
//! Paridad cerrada en la muestra `initial`. El primer tick de juego aún diverge
//! (cinemática / techos Realistic en estación); ver
//! `docs/PBS_EXTERNAL_ORACLE.md`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use openttdrs_core::{GameState, TileCoord, VehicleKind, sav};
use serde::Deserialize;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
struct OracleTrain {
    progress: u8,
    speed: u16,
    subspeed: u8,
    direction: u8,
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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
    let raw = std::fs::read_to_string(fixture_path(
        "parity/train_dual_pbs_curve_15_3_openttd.jsonl",
    ))
    .expect("traza oráculo dual PBS");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("JSONL oráculo"))
        .collect()
}

fn sorted_trains(trains: &[OracleTrain]) -> Vec<OracleTrain> {
    let mut v = trains.to_vec();
    v.sort_by_key(|t| (t.x, t.y, t.direction));
    v
}

fn runtime_trains(state: &GameState) -> Vec<OracleTrain> {
    let mut trains: Vec<_> = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        .map(|v| OracleTrain {
            progress: v.progress,
            speed: v.cur_speed,
            subspeed: v.subspeed,
            direction: v.direction,
            x: v.pos.x,
            y: v.pos.y,
        })
        .collect();
    trains.sort_by_key(|t| (t.x, t.y, t.direction));
    trains
}

#[test]
fn imports_dual_pbs_curve_fixture_shape() {
    let raw = std::fs::read(fixture_path("train_dual_pbs_curve_15_3.sav")).expect("fixture");
    let sav = sav::load(&raw).expect("cargar");
    assert_eq!(sav.version, 362);
    assert_eq!(sav.map.dimensions(), (64, 64));
    assert_eq!(
        sav.vehicles
            .iter()
            .filter(|v| v.kind == openttdrs_core::SavVehicleKind::Train)
            .count(),
        2
    );
    let state = GameState::from_sav_game(sav);
    assert_eq!(
        state.train_acceleration_model,
        openttdrs_core::TrainAccelerationModel::Realistic
    );
    assert_eq!(
        state
            .vehicles
            .iter()
            .filter(|v| v.kind == VehicleKind::Train)
            .count(),
        2
    );
}

#[test]
fn oracle_trace_has_two_trains_and_forty_ticks() {
    let rows = load_oracle();
    assert_eq!(rows.len(), 42, "metadata + initial + 40 ticks");
    assert_eq!(rows[0].kind, "metadata");
    assert_eq!(rows[1].kind, "initial");
    let initial = rows[1].trains.as_ref().expect("trains");
    assert_eq!(initial.len(), 2);
    assert_eq!(
        sorted_trains(initial),
        vec![
            OracleTrain {
                x: 25,
                y: 14,
                progress: 117,
                speed: 25,
                subspeed: 67,
                direction: 3,
            },
            OracleTrain {
                x: 26,
                y: 7,
                progress: 187,
                speed: 95,
                subspeed: 215,
                direction: 7,
            },
        ]
    );
    assert_eq!(
        rows[1].rail_reservations.as_ref().expect("reservas"),
        &vec![OracleReservation {
            x: 26,
            y: 7,
            track_bits: 2,
        }]
    );
    assert!(rows.iter().skip(2).all(|r| r.kind == "tick"));
}

#[test]
fn rust_matches_openttd_initial_sample() {
    let oracle = load_oracle();
    let raw = std::fs::read(fixture_path("train_dual_pbs_curve_15_3.sav")).expect("fixture");
    let state = GameState::from_sav_game(sav::load(&raw).expect("load"));
    let expected = sorted_trains(oracle[1].trains.as_ref().expect("trains"));
    assert_eq!(runtime_trains(&state), expected, "cinemática initial");

    let expected_res = oracle[1].rail_reservations.as_ref().expect("reservas");
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
                got.push(OracleReservation {
                    x,
                    y,
                    track_bits: bits,
                });
            }
        }
    }
    got.sort_by_key(|r| (r.x, r.y, r.track_bits));
    let mut want = expected_res.clone();
    want.sort_by_key(|r| (r.x, r.y, r.track_bits));
    assert_eq!(got, want, "reservas PBS initial");
}

/// Documenta la primera divergencia conocida (tick 1) hasta cerrar cinemática
/// Realistic en plataforma / path signal.
#[test]
fn first_tick_still_diverges_from_openttd() {
    let oracle = load_oracle();
    let raw = std::fs::read(fixture_path("train_dual_pbs_curve_15_3.sav")).expect("fixture");
    let mut state = GameState::from_sav_game(sav::load(&raw).expect("load"));
    assert_eq!(runtime_trains(&state), sorted_trains(oracle[1].trains.as_ref().unwrap()));

    state.step();
    let expected = sorted_trains(oracle[2].trains.as_ref().expect("tick1 trains"));
    let got = runtime_trains(&state);
    assert_ne!(
        got, expected,
        "si este assert falla, el tick 1 ya está alineado: actualizá a paridad completa"
    );
    // Tren en plataforma sur: OpenTTD avanza progress; Rust aún no en este fixture.
    let south = got.iter().find(|t| t.x == 25 && t.y == 14).expect("sur");
    let south_ottd = expected
        .iter()
        .find(|t| t.x == 25 && t.y == 14)
        .expect("sur ottd");
    assert_eq!(south.progress, 117);
    assert_eq!(south_ottd.progress, 153);
}
