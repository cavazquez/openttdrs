//! Fixture FTA de aeropuerto real de `OpenTTD` 15.3 (Helidepot): import +
//! comparación con oráculo (issue #198).
//!
//! El estado *inicial* y la secuencia dinámica de `pos`/`state`/`z_pos` se
//! comparan contra el oráculo. El comparador Python también exige `x_pos`/`y_pos`
//! físicos cuando ambas trazas los publican. El vuelo libre posterior al
//! despegue conserva todavía un contrato separado y no se usa para declarar
//! cerrada la paridad completa de aeronaves. Ver
//! `scripts/compare_airport_fta_traces.py` para el contrato Python.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use openttdrs_core::{AirportHeading, GameState, StopKind, VehicleKind, sav};
use serde::Deserialize;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // `engine` documenta el esquema aunque no se compare aún.
struct OracleAircraft {
    pos: u8,
    previous_pos: u8,
    state: u8,
    targetairport: u32,
    speed: u16,
    direction: u8,
    running: bool,
    engine: u16,
    z_pos: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // `layout` documenta el esquema aunque no se compare aún.
struct OracleAirport {
    station: u32,
    x: i32,
    y: i32,
    w: u16,
    h: u16,
    #[serde(rename = "type")]
    airport_type: u8,
    layout: u8,
    blocks: u64,
}

#[derive(Debug, Deserialize)]
struct OracleRow {
    kind: String,
    #[serde(default)]
    tick: u64,
    #[serde(default)]
    aircraft: Vec<OracleAircraft>,
    #[serde(default)]
    airports: Vec<OracleAirport>,
}

fn load_oracle() -> Vec<OracleRow> {
    let raw = std::fs::read_to_string(fixture_path(
        "parity/helidepot_fta_cycle_15_3_openttd.jsonl",
    ))
    .expect("traza oráculo FTA aeropuerto");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("JSONL oráculo"))
        .collect()
}

/// Carga el `.sav` y devuelve el estado ya importado (aeropuertos + avión FTA).
fn load_state() -> GameState {
    let raw = std::fs::read(fixture_path("helidepot_fta_cycle_15_3.sav")).expect("fixture .sav");
    let sav = sav::load(&raw).expect("cargar fixture Helidepot");
    GameState::from_sav_game(sav)
}

#[test]
fn imports_two_helidepots_and_one_helicopter_with_active_fta() {
    let state = load_state();

    let airports: Vec<_> = state
        .stations
        .iter()
        .filter(|s| s.stop_kind == StopKind::Airport)
        .collect();
    assert_eq!(airports.len(), 2, "dos helidepots");
    for st in &airports {
        assert_eq!(st.airport_spec, openttdrs_core::AirportSpecId::Helidepot);
        assert_eq!(st.airport_tiles.len(), 4, "helidepot 2×2");
    }

    let aircraft: Vec<_> = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Aircraft)
        .collect();
    assert_eq!(aircraft.len(), 1, "un helicóptero Tricario");
    let heli = aircraft[0];
    assert!(heli.airport_fta_active, "FTA debe importarse activo");
    assert_eq!(heli.airport_pos, 11);
    assert_eq!(heli.airport_prev_pos, 17);
    assert_eq!(heli.airport_heading, AirportHeading::HeliTakeoff);
    assert!(heli.running);
    assert_eq!(heli.cur_speed, 320);
    assert_eq!(heli.direction, 2);
    assert_eq!(heli.aircraft_rotor_speed, 32);
    assert_eq!(heli.z_pos, Some(117));
    assert_eq!(heli.airport_sub_x, 696);
    assert_eq!(heli.airport_sub_y, 744);
}

#[test]
fn oracle_trace_declares_openttd_and_helidepot_cycle() {
    let rows = load_oracle();
    assert_eq!(rows[0].kind, "metadata");
    assert_eq!(rows[1].kind, "initial");
    assert_eq!(rows[1].aircraft.len(), 1);
    assert_eq!(rows[1].airports.len(), 2);
}

/// Compara el estado inicial importado contra el oráculo, campo a campo.
#[test]
fn initial_state_matches_openttd_oracle_strongly() {
    let oracle = load_oracle();
    let initial = &oracle[1];
    assert_eq!(initial.kind, "initial");

    let state = load_state();

    let mut oracle_airports = initial.airports.clone();
    oracle_airports.sort_by_key(|a| a.station);
    let mut state_airports: Vec<_> = state
        .stations
        .iter()
        .filter(|s| s.stop_kind == StopKind::Airport)
        .collect();
    state_airports.sort_by_key(|s| (s.pos.x, s.pos.y));
    assert_eq!(state_airports.len(), oracle_airports.len());
    for (expected, actual) in oracle_airports.iter().zip(state_airports.iter()) {
        assert_eq!(actual.pos.x, expected.x, "airport {} x", expected.station);
        assert_eq!(actual.pos.y, expected.y, "airport {} y", expected.station);
        assert_eq!(
            actual.airport_tiles.len(),
            usize::from(expected.w) * usize::from(expected.h),
            "airport {} footprint w*h",
            expected.station
        );
        assert_eq!(
            actual.airport_blocks, expected.blocks,
            "airport {} blocks",
            expected.station
        );
        assert_eq!(
            actual.airport_spec,
            openttdrs_core::AirportSpecId::from_ottd_airport_type(expected.airport_type),
            "airport {} type",
            expected.station
        );
    }

    assert_eq!(initial.aircraft.len(), 1);
    let expected = &initial.aircraft[0];
    let heli = state
        .vehicles
        .iter()
        .find(|v| v.kind == VehicleKind::Aircraft)
        .expect("avión importado");
    assert_eq!(heli.airport_pos, expected.pos, "pos (nodo FTA)");
    assert_eq!(heli.airport_prev_pos, expected.previous_pos, "previous_pos");
    assert_eq!(
        heli.airport_heading.as_u8(),
        expected.state,
        "state (heading FTA)"
    );
    assert_eq!(heli.cur_speed, expected.speed, "speed");
    assert_eq!(heli.direction, expected.direction, "direction");
    assert_eq!(heli.running, expected.running, "running");
    assert!(heli.airport_fta_active, "FTA activo en el import");

    // `targetairport` es un `StationID` de OpenTTD; lo resolvemos por
    // posición contra el aeropuerto correspondiente (nuestro `Vehicle` no
    // guarda el ID crudo, solo el tile destino).
    let target_station = oracle_airports
        .iter()
        .find(|a| a.station == expected.targetairport)
        .expect("targetairport debe resolver a un aeropuerto del oráculo");
    assert_eq!(heli.dest.x, target_station.x, "dest.x = targetairport");
    assert_eq!(heli.dest.y, target_station.y, "dest.y = targetairport");
}

/// Recorre los ticks del oráculo y exige igualdad en la secuencia dinámica
/// `pos`/`state`/`z_pos` del FTA.
#[test]
fn tick_sequence_matches_oracle_for_fta_and_flight_level() {
    let oracle = load_oracle();
    let mut state = load_state();

    for (i, row) in oracle.iter().enumerate().skip(2) {
        state.step();
        let expected = row.aircraft.first().expect("aircraft esperado en oráculo");
        let tick = row.tick;
        let heli = state
            .vehicles
            .iter()
            .find(|v| v.kind == VehicleKind::Aircraft)
            .expect("avión vivo durante toda la traza");
        assert_eq!(
            heli.airport_pos, expected.pos,
            "pos en tick {tick} (índice {i})"
        );
        assert_eq!(
            heli.airport_heading.as_u8(),
            expected.state,
            "state en tick {tick} (índice {i})"
        );
        assert_eq!(
            heli.z_pos.map_or(0, i32::from),
            expected.z_pos,
            "z_pos en tick {tick} (índice {i})"
        );
    }
}
