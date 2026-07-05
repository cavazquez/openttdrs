//! Golden ferroviario contra tablas de `OpenTTD` (fixture
//! `tests/fixtures/parity/train_movement_golden.json`, generado por
//! `scripts/extract_train_movement.py`).
//!
//! Valida:
//! - tablas portadas en `train_movement.rs`;
//! - conectividad `rail_bit_for_sides` × lados;
//! - encoding básico de señales;
//! - tren cruza túnel/puente ferroviario sin quedar atrapado.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use openttdrs_core::{
    ACCEL_SLOWDOWN, AccelSlowdownParams, BridgeType, Command, DELTACOORD_LEAVE_OFFSET, DIR_E,
    DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, FRACTCOORDS_BEHIND, FRACTCOORDS_ENTER,
    GameState, PathNetwork, RAIL_TOUCHING_SIDE_NE, RAIL_TOUCHING_SIDE_NW, RAIL_TOUCHING_SIDE_SE,
    RAIL_TOUCHING_SIDE_SW, SIGTYPE_BLOCK, SignalTrack, TRAIN_UPDATE_SPEED_ACCEL_MUL,
    TRAIN_UPDATE_SPEED_BRAKE_MUL, TUNNEL_VISIBILITY_FRAME, TileCoord, TileKind,
    VEHICLE_INITIAL_X_FRACT, VEHICLE_INITIAL_Y_FRACT, VEHICLE_SUBCOORD, Vehicle, VehicleKind,
    accelerate_train_speed, apply_command, dir_difference, find_path, is_45_degree_turn,
    rail_bit_for_sides, resolve_signal_track, signal_on_track_mask, signal_type_for_track,
    tracks_overlap, train_acceleration,
};

const TRACKS: [&str; 6] = [
    "TRACK_X",
    "TRACK_Y",
    "TRACK_UPPER",
    "TRACK_LOWER",
    "TRACK_LEFT",
    "TRACK_RIGHT",
];
const ENTERS: [&str; 4] = ["NE", "SE", "SW", "NW"];

#[derive(serde::Deserialize)]
struct Fixture {
    accel_slowdown: Vec<AccelSlowdownRow>,
    vehicle_initial_x_fract: Vec<u8>,
    vehicle_initial_y_fract: Vec<u8>,
    fractcoords_enter: Vec<Coord>,
    fractcoords_behind: Vec<Coord>,
    deltacoord_leaveoffset: Vec<Coord>,
    vehicle_subcoord: HashMap<String, HashMap<String, Option<SubcoordFixture>>>,
    tunnel_visibility_frame: Vec<u8>,
    update_speed_am_original: UpdateSpeedFixture,
    connectivity: ConnectivityFixture,
}

#[derive(serde::Deserialize)]
struct AccelSlowdownRow {
    small_turn: u8,
    large_turn: u8,
    z_up: u8,
    z_down: u8,
}

#[derive(serde::Deserialize)]
struct Coord {
    x: i32,
    y: i32,
}

#[derive(Debug, serde::Deserialize)]
struct SubcoordFixture {
    x: u8,
    y: u8,
    dir: String,
}

#[derive(serde::Deserialize)]
struct UpdateSpeedFixture {
    accel_multiplier: i32,
    brake_multiplier: i32,
}

#[derive(serde::Deserialize)]
struct ConnectivityFixture {
    side_pairs: Vec<SidePair>,
    touching_side_masks: HashMap<String, u8>,
}

#[derive(serde::Deserialize)]
struct SidePair {
    sides: [u8; 2],
    bit: u8,
    track: String,
}

fn load_fixture() -> Fixture {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/parity/train_movement_golden.json"
    );
    let text = std::fs::read_to_string(path)
        .expect("fixture golden (correr scripts/extract_train_movement.py)");
    serde_json::from_str(&text).expect("fixture golden JSON válido")
}

fn dir_from_openttd(name: &str) -> u8 {
    match name {
        "N" => DIR_N,
        "NE" => DIR_NE,
        "E" => DIR_E,
        "SE" => DIR_SE,
        "S" => DIR_S,
        "SW" => DIR_SW,
        "W" => DIR_W,
        "NW" => DIR_NW,
        other => panic!("dirección OpenTTD desconocida: {other}"),
    }
}

#[test]
fn fixture_structure_is_complete() {
    let f = load_fixture();
    assert_eq!(f.accel_slowdown.len(), 3);
    assert_eq!(f.vehicle_initial_x_fract.len(), 4);
    assert_eq!(f.fractcoords_enter.len(), 4);
    assert_eq!(f.vehicle_subcoord.len(), 4);
    assert_eq!(f.tunnel_visibility_frame, [12, 8, 8, 12]);
    for enter in ENTERS {
        let block = &f.vehicle_subcoord[enter];
        for track in TRACKS {
            assert!(block.contains_key(track), "{enter}/{track}");
        }
    }
}

#[test]
fn accel_slowdown_matches_rust_copy() {
    let f = load_fixture();
    for (i, row) in f.accel_slowdown.iter().enumerate() {
        let rust = ACCEL_SLOWDOWN[i];
        assert_eq!(
            rust,
            AccelSlowdownParams {
                small_turn: row.small_turn,
                large_turn: row.large_turn,
                z_up: row.z_up,
                z_down: row.z_down,
            },
            "fila {i}"
        );
    }
    assert_eq!(ACCEL_SLOWDOWN[0].small_turn, 64);
    assert_eq!(ACCEL_SLOWDOWN[0].large_turn, 128);
}

#[test]
fn vehicle_initial_fractcoords_match_rust_copy() {
    let f = load_fixture();
    assert_eq!(f.vehicle_initial_x_fract, VEHICLE_INITIAL_X_FRACT.to_vec());
    assert_eq!(f.vehicle_initial_y_fract, VEHICLE_INITIAL_Y_FRACT.to_vec());
}

#[test]
fn depot_fractcoords_match_rust_copy() {
    let f = load_fixture();
    for (i, c) in f.fractcoords_enter.iter().enumerate() {
        assert_eq!(FRACTCOORDS_ENTER[i], (c.x as u8, c.y as u8));
    }
    for (i, c) in f.fractcoords_behind.iter().enumerate() {
        assert_eq!(FRACTCOORDS_BEHIND[i], (c.x as u8, c.y as u8));
    }
    for (i, c) in f.deltacoord_leaveoffset.iter().enumerate() {
        assert_eq!(
            DELTACOORD_LEAVE_OFFSET[i],
            (c.x as i8, c.y as i8),
            "leave offset {i}"
        );
    }
}

#[test]
fn tunnel_visibility_frame_matches_rust_copy() {
    let f = load_fixture();
    assert_eq!(f.tunnel_visibility_frame, TUNNEL_VISIBILITY_FRAME.to_vec());
}

#[test]
fn kirby_acceleration_formula_matches_golden() {
    assert_eq!(train_acceleration(300, 47), 24);
    let mut cur = 0_u16;
    let mut sub = 0_u8;
    let mut ticks = 0_u32;
    while cur < 1 && ticks < 20 {
        (cur, sub) = accelerate_train_speed(cur, sub, 300, 47, 64);
        ticks += 1;
    }
    assert_eq!(cur, 1);
    assert_eq!(ticks, 6, "48·6 = 288 → subspeed overflow + cur_speed +1");
}

#[test]
fn update_speed_constants_match_upstream() {
    let f = load_fixture();
    assert_eq!(
        f.update_speed_am_original.accel_multiplier,
        TRAIN_UPDATE_SPEED_ACCEL_MUL
    );
    assert_eq!(
        f.update_speed_am_original.brake_multiplier,
        TRAIN_UPDATE_SPEED_BRAKE_MUL
    );
}

#[test]
fn vehicle_subcoord_matches_rust_copy() {
    let f = load_fixture();
    for (ei, enter) in ENTERS.iter().enumerate() {
        let block = &f.vehicle_subcoord[*enter];
        for (ti, track) in TRACKS.iter().enumerate() {
            let expected = block.get(*track).unwrap();
            let rust = VEHICLE_SUBCOORD[ei][ti];
            match (expected, rust) {
                (None, None) => {}
                (Some(exp), Some(got)) => {
                    assert_eq!(got.x, exp.x, "{enter}/{track} x");
                    assert_eq!(got.y, exp.y, "{enter}/{track} y");
                    assert_eq!(got.dir, dir_from_openttd(&exp.dir), "{enter}/{track} dir");
                }
                _ => panic!("{enter}/{track}: fixture={expected:?} rust={rust:?}"),
            }
        }
    }
}

#[test]
fn rail_bit_for_sides_matches_track_type_semantics() {
    let f = load_fixture();
    for pair in &f.connectivity.side_pairs {
        let (a, b) = (pair.sides[0], pair.sides[1]);
        assert_eq!(
            rail_bit_for_sides(a, b),
            pair.bit,
            "par {:?} → {}",
            pair.sides,
            pair.track
        );
        assert_eq!(
            rail_bit_for_sides(b, a),
            pair.bit,
            "simetría {:?}",
            pair.sides
        );
    }
    assert_eq!(
        f.connectivity.touching_side_masks["NE"],
        RAIL_TOUCHING_SIDE_NE
    );
    assert_eq!(
        f.connectivity.touching_side_masks["SE"],
        RAIL_TOUCHING_SIDE_SE
    );
    assert_eq!(
        f.connectivity.touching_side_masks["SW"],
        RAIL_TOUCHING_SIDE_SW
    );
    assert_eq!(
        f.connectivity.touching_side_masks["NW"],
        RAIL_TOUCHING_SIDE_NW
    );
}

#[test]
fn signal_encoding_matches_openttd_layout() {
    assert_eq!(SIGTYPE_BLOCK, 0);
    // Máscara de presencia en `m3` (pares de sig_bits por carril; `rail_map.h`).
    assert_eq!(signal_on_track_mask(SignalTrack::X), 0x0C);
    assert_eq!(signal_on_track_mask(SignalTrack::Y), 0x0C);
    assert_eq!(signal_on_track_mask(SignalTrack::Upper), 0x0C);
    assert_eq!(signal_on_track_mask(SignalTrack::Lower), 0x03);
    assert_eq!(signal_on_track_mask(SignalTrack::Left), 0x0C);
    assert_eq!(signal_on_track_mask(SignalTrack::Right), 0x03);
    assert!(tracks_overlap(0x01 | 0x02));
    assert!(!tracks_overlap(0x01));
    assert_eq!(signal_type_for_track(0, SignalTrack::X), SIGTYPE_BLOCK);
    assert_eq!(
        resolve_signal_track(0x08, 200, 100),
        Some(SignalTrack::Lower)
    );
    assert!(resolve_signal_track(0x03, 128, 128).is_none());
}

#[test]
fn train_crosses_rail_tunnel_without_stopping_short_of_exit() {
    let mut s = GameState::new(16, 16);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    // Misma huella que el test de túnel viario en `command/tests/station.rs`.
    s.map.set_height(c(5, 5), 2).unwrap();
    s.map.set_height(c(5, 6), 2).unwrap();
    s.map.set_height(c(6, 5), 1).unwrap();
    s.map.set_height(c(6, 6), 1).unwrap();
    s.map.set_height(c(3, 5), 1).unwrap();
    s.map.set_height(c(3, 6), 1).unwrap();
    s.map.set_height(c(4, 5), 2).unwrap();
    s.map.set_height(c(4, 6), 2).unwrap();

    let tunnel_start = c(5, 5);
    let tunnel_end = c(3, 5);
    apply_command(&mut s, &Command::PlaceRailTunnel(tunnel_start, tunnel_end))
        .expect("túnel ferroviario");

    for x in [2, 6] {
        apply_command(&mut s, &Command::PlaceRail(c(x, 5))).expect("acceso al túnel");
    }

    let west = c(2, 5);
    let east = c(6, 5);
    assert!(
        find_path(&s.map, west, east, PathNetwork::Rail).is_some(),
        "ruta a través del túnel"
    );

    let mut train = Vehicle::new(1, VehicleKind::Train, west, east);
    train.path = find_path(&s.map, west, east, PathNetwork::Rail)
        .unwrap()
        .into();
    train.set_cruise_speed();
    s.vehicles.push(train);

    for _ in 0..800 {
        s.step();
        if s.vehicles[0].pos == east {
            break;
        }
    }
    assert_eq!(
        s.vehicles[0].pos, east,
        "el tren debe atravesar el túnel y llegar al este"
    );
    assert!(
        s.vehicles[0].cur_speed > 0 || s.vehicles[0].progress > 0,
        "no debe quedar atascado en el túnel"
    );
}

#[test]
fn rail_bridge_placement_and_train_enters_ramp_from_land() {
    let mut s = GameState::new(16, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for x in 2..=5 {
        s.map.set_kind(c(x, 4), TileKind::Water).unwrap();
    }
    let west_ramp = c(1, 4);
    let east_ramp = c(6, 4);
    apply_command(
        &mut s,
        &Command::PlaceRailBridge(west_ramp, east_ramp, BridgeType::Wooden),
    )
    .expect("colocación de puente ferroviario sobre agua");
    assert_eq!(s.map.get(west_ramp).unwrap().kind, TileKind::RailBridge);
    assert_eq!(s.map.get(east_ramp).unwrap().kind, TileKind::RailBridge);

    let land = c(0, 4);
    apply_command(&mut s, &Command::PlaceRail(land)).expect("vía de acceso");

    assert!(
        find_path(&s.map, land, west_ramp, PathNetwork::Rail).is_some(),
        "la vía debe conectar con la rampa oeste del puente"
    );

    let mut train = Vehicle::new(1, VehicleKind::Train, land, west_ramp);
    train.path = find_path(&s.map, land, west_ramp, PathNetwork::Rail)
        .unwrap()
        .into();
    train.set_cruise_speed();
    s.vehicles.push(train);

    for _ in 0..400 {
        s.step();
        if s.vehicles[0].pos == west_ramp {
            break;
        }
    }
    assert_eq!(
        s.vehicles[0].pos, west_ramp,
        "el tren debe entrar a la rampa del puente sin descarrilar"
    );
    // El vano central sigue siendo `Water` con `mapt` de puente: el pathfinder aún
    // no atraviesa el tramo (divergencia documentada; Fase Rail 3E o pathfinder).
    assert!(
        find_path(&s.map, west_ramp, east_ramp, PathNetwork::Rail).is_none(),
        "cruzar el vano completo aún no está soportado por el pathfinder"
    );
}

/// Divergencias rail conocidas en `train_line`: regresiones 3B–3E + divergencias documentadas.
#[test]
fn train_line_divergences_are_absent_after_rail_3b() {
    use std::collections::HashMap;

    use openttdrs_core::parity::{self, report::detect_known_divergences};

    let mut state = parity::build_train_line();
    state.enable_parity_trace();
    for _ in 0..600 {
        state.step();
    }
    let records = state.take_parity_records();
    let divergences = detect_known_divergences(&records);
    let by_id: HashMap<&str, bool> = divergences.iter().map(|d| (d.id, d.detected)).collect();
    assert_eq!(
        by_id.get("train_road_acceleration"),
        Some(&false),
        "regresión: el tren debe usar aceleración AM_ORIGINAL (Rail 3B)"
    );
    assert_eq!(
        by_id.get("train_no_curve_braking"),
        Some(&false),
        "regresión: el tren debe frenar en curva con _accel_slowdown (Rail 3B)"
    );
    assert_eq!(
        by_id.get("train_platform_stop"),
        Some(&false),
        "regresión: el tren debe cargar desde la plataforma (Rail 3C)"
    );
    assert_eq!(
        by_id.get("train_render_subtile_consistency"),
        Some(&false),
        "regresión: traza rail y render lógico deben coincidir (Rail 3E)"
    );
    assert_eq!(
        by_id.get("train_diagonal_subcoord_approximation"),
        Some(&true),
        "divergencia cosmética documentada en piezas diagonales (Rail 3E/4)"
    );
}

/// Divergencias rail 3D: escenario `train_signal` con espera medida en la traza.
#[test]
fn train_signal_divergences_are_absent_after_rail_3d() {
    use std::collections::HashMap;

    use openttdrs_core::parity::{self, report::detect_known_divergences};

    let mut state = parity::build_train_signal();
    state
        .vehicles
        .retain(|v| v.id != parity::TRAIN_SIGNAL_BLOCKER_ID);
    state.enable_parity_trace();
    let mut blocker = openttdrs_core::Vehicle::new(
        parity::TRAIN_SIGNAL_BLOCKER_ID,
        openttdrs_core::VehicleKind::Train,
        parity::TRAIN_SIGNAL_BLOCK_TILE,
        parity::TRAIN_SIGNAL_BLOCK_TILE,
    );
    blocker.running = false;
    state.vehicles.push(blocker);
    for _ in 0..30 {
        state.step();
    }
    state
        .vehicles
        .retain(|v| v.id != parity::TRAIN_SIGNAL_BLOCKER_ID);
    for _ in 0..120 {
        state.step();
    }
    let records = state.take_parity_records();
    let divergences = detect_known_divergences(&records);
    let by_id: HashMap<&str, bool> = divergences.iter().map(|d| (d.id, d.detected)).collect();
    assert_eq!(
        by_id.get("train_signal_wait"),
        Some(&false),
        "regresión: el líder debe emitir SignalWait* al liberarse el bloque (Rail 3D)"
    );
}

#[test]
fn dir_difference_matches_openttd_encoding() {
    assert_eq!(dir_difference(DIR_SE, DIR_SW), 6);
    assert!(is_45_degree_turn(DIR_NE, DIR_E));
    assert!(!is_45_degree_turn(DIR_SE, DIR_SW));
}
