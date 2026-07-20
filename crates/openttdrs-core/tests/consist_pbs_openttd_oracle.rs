//! Fixture PBS multi-vagón OpenTTD 15.3: consist de 3 unidades + oráculo v2.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use openttdrs_core::{
    GameState, TileCoord, VehicleKind, consist_unit_ids, consist_unit_poses, sav,
};
use serde::Deserialize;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[derive(Debug, Clone, Deserialize)]
struct OracleUnit {
    index: usize,
    x: i32,
    y: i32,
    rail_pixel: u8,
    direction: u8,
}

#[derive(Debug, Clone, Deserialize)]
struct OracleTrain {
    progress: u8,
    speed: u16,
    subspeed: u8,
    direction: u8,
    x: i32,
    y: i32,
    units: Option<Vec<OracleUnit>>,
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
    schema_version: Option<u32>,
    producer: Option<String>,
    trains: Option<Vec<OracleTrain>>,
    rail_reservations: Option<Vec<OracleReservation>>,
}

fn load_oracle() -> Vec<OracleRow> {
    let raw = std::fs::read_to_string(fixture_path(
        "parity/train_consist_2wagon_pbs_15_3_openttd.jsonl",
    ))
    .expect("traza oráculo PBS multi-vagón");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("JSONL oráculo"))
        .collect()
}

fn rail_reservations(state: &GameState) -> Vec<(i32, i32, u8)> {
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
                out.push((x, y, bits));
            }
        }
    }
    out
}

#[test]
fn imports_consist_2wagon_with_three_chained_units() {
    let raw = std::fs::read(fixture_path("train_consist_2wagon_pbs_15_3.sav"))
        .expect("fixture multi-vagón");
    let sav = sav::load(&raw).expect("cargar fixture multi-vagón");
    assert_eq!(sav.version, 362);
    assert_eq!(sav.map.dimensions(), (64, 64));
    assert_eq!(sav.vehicles.len(), 3, "locomotora + 2 vagones");
    assert!(!sav.vehicles[0].is_wagon);
    assert!(sav.vehicles[1].is_wagon);
    assert!(sav.vehicles[2].is_wagon);

    let state = GameState::from_sav_game(sav);
    let head = state
        .vehicles
        .iter()
        .find(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        .expect("cabeza");
    let ids = consist_unit_ids(&state.vehicles, head.id);
    assert_eq!(ids.len(), 3, "consist cabeza→cola");
    assert_eq!(ids[0], head.id);
    let poses = consist_unit_poses(&state.vehicles, head.id);
    assert_eq!(poses.len(), 3);
    assert!(
        poses[0].tile != poses[2].tile || poses[0].rail_pixel != poses[2].rail_pixel,
        "la cola no debe coincidir con la cabeza"
    );
}

#[test]
fn oracle_trace_declares_schema_v2_with_units() {
    let rows = load_oracle();
    assert_eq!(rows.len(), 42, "metadata + initial + 40 ticks");
    assert_eq!(rows[0].kind, "metadata");
    assert_eq!(rows[0].schema_version, Some(2));
    assert_eq!(rows[0].producer.as_deref(), Some("openttd"));
    assert_eq!(rows[1].kind, "initial");
    let units = rows[1].trains.as_ref().unwrap()[0]
        .units
        .as_ref()
        .expect("units v2");
    assert_eq!(units.len(), 3);
    assert_eq!(units[0].index, 0);
    assert_eq!(units[2].index, 2);
}

#[test]
fn rust_matches_openttd_consist_oracle_for_forty_ticks() {
    let oracle = load_oracle();
    let raw = std::fs::read(fixture_path("train_consist_2wagon_pbs_15_3.sav")).expect("fixture");
    let sav = sav::load(&raw).expect("load");
    let mut state = GameState::from_sav_game(sav);

    let assert_frame = |state: &GameState, row: &OracleRow, label: &str| {
        let expected = &row.trains.as_ref().expect("trains")[0];
        let head = state
            .vehicles
            .iter()
            .find(|v| v.kind == VehicleKind::Train && v.is_consist_head())
            .expect("tren");
        assert_eq!(
            (
                head.progress,
                head.cur_speed,
                head.subspeed,
                head.pos.x,
                head.pos.y,
                head.direction
            ),
            (
                expected.progress,
                expected.speed,
                expected.subspeed,
                expected.x,
                expected.y,
                expected.direction
            ),
            "cinemática cabeza en {label}"
        );

        let expected_units = expected.units.as_ref().expect("units");
        let poses = consist_unit_poses(&state.vehicles, head.id);
        assert_eq!(
            poses.len(),
            expected_units.len(),
            "cantidad de units en {label}"
        );
        for (pose, want) in poses.iter().zip(expected_units) {
            assert_eq!(
                (pose.tile.x, pose.tile.y, pose.rail_pixel, pose.direction),
                (want.x, want.y, want.rail_pixel, want.direction),
                "pose unidad index={} en {label}",
                want.index
            );
        }

        let expected_res: Vec<_> = row
            .rail_reservations
            .as_ref()
            .expect("reservas")
            .iter()
            .map(|r| (r.x, r.y, r.track_bits))
            .collect();
        assert_eq!(
            rail_reservations(state),
            expected_res,
            "reservas PBS en {label}"
        );
    };

    assert_frame(&state, &oracle[1], "initial");
    for (i, row) in oracle.iter().enumerate().skip(2) {
        state.step();
        assert_frame(&state, row, &format!("oracle índice {i}"));
    }
}
