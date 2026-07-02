//! Golden contra las tablas de movimiento de `OpenTTD`
//! (`src/table/roadveh_movement.h`, extraídas por
//! `scripts/extract_roadveh_movement.py` al fixture JSON).
//!
//! Dos familias de asserts:
//!
//! 1. **Paridad real**: las tablas copiadas en `road_movement.rs` y las
//!    constantes de avance (`192/256`, `GetAdvanceSpeed = speed*3/4`)
//!    coinciden con el upstream. Si dejan de coincidir, el test falla.
//! 2. **Divergencias conocidas**: cada divergencia identificada en la Fase 1
//!    se verifica con su estado actual y se documenta en
//!    `docs/parity/divergences_found.md` — no rompen CI. Las ya corregidas en
//!    la Fase 2 (penalización de curva −25 %) se asertan como AUSENTES
//!    (regresión); las pendientes (parada dentro de la bahía, carga gradual,
//!    tick rate) se asertan como presentes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use openttdrs_core::parity::{self, report::detect_known_divergences};
use openttdrs_core::{
    DIR_NE, DIR_NW, DIR_SE, DIR_SW, straight_subtile, tile_progress_length, turn_curve_points,
};

#[derive(serde::Deserialize)]
struct Fixture {
    drive_data: HashMap<String, DriveTable>,
    road_stop_stop_frame: Vec<u8>,
    constants: Constants,
}

#[derive(serde::Deserialize)]
struct DriveTable {
    frames: Vec<(f32, f32)>,
    end: EndMarker,
}

#[derive(serde::Deserialize)]
struct EndMarker {
    flag: String,
    diagdir: String,
}

#[derive(serde::Deserialize)]
struct Constants {
    tile_axial_distance: u32,
    tile_corner_distance: u32,
    advance_speed_numerator: u32,
    advance_speed_denominator: u32,
    curve_speed_penalty_shift: u32,
}

fn load_fixture() -> Fixture {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/parity/roadveh_movement_golden.json"
    );
    let text = std::fs::read_to_string(path)
        .expect("fixture golden (correr scripts/extract_roadveh_movement.py)");
    serde_json::from_str(&text).expect("fixture golden JSON válido")
}

#[test]
fn drive_data_0_matches_rust_straight_ne_lane() {
    let fixture = load_fixture();
    let table = &fixture.drive_data["_roadveh_drive_data_0"];
    assert_eq!(table.frames.len(), 16, "recta NE: 16 frames upstream");
    assert_eq!(table.end.flag, "RDE_NEXT_TILE");
    assert_eq!(table.end.diagdir, "NE");
    for (i, &(x, y)) in table.frames.iter().enumerate() {
        assert!((x - (15.0 - i as f32)).abs() < f32::EPSILON, "frame {i}");
        assert!((y - 5.0).abs() < f32::EPSILON, "frame {i}");
    }
    // La recta NE del render Rust interpola exactamente esos extremos.
    assert_eq!(straight_subtile(DIR_NE, 0), (15.0, 5.0));
    assert_eq!(straight_subtile(DIR_NE, 255), (0.0, 5.0));
}

#[test]
fn curve_tables_match_rust_copies() {
    let fixture = load_fixture();
    let cases = [
        ("_roadveh_drive_data_2", DIR_NW, DIR_NE),
        ("_roadveh_drive_data_3", DIR_NE, DIR_SE),
    ];
    for (name, entry, exit) in cases {
        let upstream = &fixture.drive_data[name];
        let rust = turn_curve_points(entry, exit)
            .unwrap_or_else(|| panic!("curva {entry}->{exit} no definida en road_movement.rs"));
        assert_eq!(
            rust.len(),
            upstream.frames.len(),
            "{name}: longitud de la curva"
        );
        for (i, (&(rx, ry), &(ux, uy))) in rust.iter().zip(&upstream.frames).enumerate() {
            assert!(
                (rx - ux).abs() < f32::EPSILON && (ry - uy).abs() < f32::EPSILON,
                "{name} frame {i}: rust=({rx},{ry}) upstream=({ux},{uy})"
            );
        }
    }
}

#[test]
fn advance_constants_match_upstream() {
    let fixture = load_fixture();
    let c = &fixture.constants;
    // GetAdvanceDistance: 192 en diagonal, 256 en cardinal (`vehicle_base.h:451`).
    assert_eq!(tile_progress_length(DIR_NE), c.tile_axial_distance);
    assert_eq!(tile_progress_length(DIR_SW), c.tile_axial_distance);
    assert_eq!(
        tile_progress_length(openttdrs_core::DIR_N),
        c.tile_corner_distance
    );

    // GetAdvanceSpeed = speed * 3 / 4 (`vehicle_base.h:439`): el paso de progreso
    // del port es proporcional al avance upstream con la misma fórmula.
    assert_eq!(c.advance_speed_numerator, 3);
    assert_eq!(c.advance_speed_denominator, 4);
    let step_for = |speed: u32, tile_len: u32| -> u32 {
        let advance = speed * c.advance_speed_numerator / c.advance_speed_denominator;
        let reference_advance = 112 * c.advance_speed_numerator / c.advance_speed_denominator;
        advance * 51 * c.tile_axial_distance / (reference_advance * tile_len)
    };
    for speed in [32_u16, 64, 96, 112] {
        let expected_diag = step_for(u32::from(speed), c.tile_axial_distance).clamp(1, 255);
        assert_eq!(
            u32::from(openttdrs_core::progress_step_for_speed(speed, DIR_SW)),
            expected_diag,
            "paso diagonal a velocidad {speed}"
        );
        let expected_card = step_for(u32::from(speed), c.tile_corner_distance).clamp(1, 255);
        assert_eq!(
            u32::from(openttdrs_core::progress_step_for_speed(
                speed,
                openttdrs_core::DIR_N
            )),
            expected_card,
            "paso cardinal a velocidad {speed}"
        );
    }
}

#[test]
fn stop_frames_are_within_bay_range() {
    let fixture = load_fixture();
    assert_eq!(fixture.road_stop_stop_frame.len(), 32);
    for (i, &frame) in fixture.road_stop_stop_frame.iter().enumerate() {
        assert!(
            (11..=20).contains(&frame),
            "stop frame {i} fuera de rango: {frame}"
        );
    }
    // Penalización de curva upstream: cur_speed -= cur_speed >> 2 (−25 %).
    assert_eq!(fixture.constants.curve_speed_penalty_shift, 2);
}

/// Divergencias conocidas: las pendientes se confirman presentes (no rompen
/// CI) y las corregidas en Fase 2 se confirman ausentes (test de regresión).
/// Todo queda documentado en `docs/parity/divergences_found.md`.
#[test]
fn known_divergences_are_confirmed_by_trace() {
    let mut state = parity::build_truck_bay();
    state.enable_parity_trace();
    for _ in 0..500 {
        state.step();
    }
    let records = state.take_parity_records();
    let divergences = detect_known_divergences(&records);

    let by_id: HashMap<&str, bool> = divergences.iter().map(|d| (d.id, d.detected)).collect();
    assert_eq!(
        by_id.get("curve_speed_penalty"),
        Some(&false),
        "regresión: la penalización de curva −25 % (Fase 2) dejó de aplicarse"
    );
    assert_eq!(
        by_id.get("bay_stop_position"),
        Some(&false),
        "regresión: el camión debe entrar a la tesela de la bahía (Fase 2)"
    );
    assert_eq!(by_id.get("instant_loading"), Some(&true));
    assert_eq!(by_id.get("tick_rate"), Some(&true));

    let markdown = parity::report::divergences_markdown(&divergences);
    for id in [
        "curve_speed_penalty",
        "bay_stop_position",
        "instant_loading",
        "tick_rate",
    ] {
        assert!(markdown.contains(id), "el markdown documenta {id}");
    }
    assert!(markdown.contains("roadveh_cmd.cpp:1481"));
    assert!(markdown.contains("roadveh_movement.h:1087"));
}
