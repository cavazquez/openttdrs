//! Golden de encoding y sprites de señales (`rail_signals_golden.json`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::{
    GameState, SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH,
    SIGTYPE_PATH_ONEWAY, SignalTrack, TileCoord, decode_rail_reservation_m2_hi,
    encode_rail_reservation_to_m2_hi,
    parity::{
        RAIL_SIGNALS_MIXED_TYPES, RAIL_SIGNALS_MIXED_Y, build_rail_signals_mixed,
        rail_signals_mixed_coord,
    },
    rail_tile_is_signals, sav, signal_type_for_track,
};

#[derive(serde::Deserialize)]
struct Fixture {
    encodings: Vec<EncodingRow>,
    pbs_reservation: PbsRow,
    sav_tiles: Vec<SavTileRow>,
}

#[derive(serde::Deserialize)]
struct EncodingRow {
    label: String,
    sig_type: u8,
    m2: u8,
    m3: u8,
    m3hi: u8,
    m5: u8,
}

#[derive(serde::Deserialize)]
struct PbsRow {
    track_bits: u8,
    m2_hi: u8,
}

#[derive(serde::Deserialize)]
struct SavTileRow {
    x: i32,
    y: i32,
    sig_type: u8,
}

fn fixture() -> Fixture {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity/rail_signals_golden.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("parsear rail_signals_golden.json")
}

#[test]
fn golden_encodings_match_signal_type_constants() {
    let f = fixture();
    let names = [
        ("block", SIGTYPE_BLOCK),
        ("entry", SIGTYPE_ENTRY),
        ("exit", SIGTYPE_EXIT),
        ("combo", SIGTYPE_COMBO),
        ("path", SIGTYPE_PATH),
        ("path_oneway", SIGTYPE_PATH_ONEWAY),
    ];
    assert_eq!(f.encodings.len(), names.len());
    for (row, (label, ty)) in f.encodings.iter().zip(names) {
        assert_eq!(row.label, label);
        assert_eq!(row.sig_type, ty);
        assert_eq!(signal_type_for_track(row.m2, SignalTrack::X), ty);
        assert!(rail_tile_is_signals(row.m5));
    }
}

#[test]
fn golden_scenario_matches_encoding_table() {
    let f = fixture();
    let state = build_rail_signals_mixed();
    for (row, &(x, expected)) in f.encodings.iter().zip(RAIL_SIGNALS_MIXED_TYPES) {
        let tile = state.map.get(rail_signals_mixed_coord(x)).unwrap();
        assert_eq!(expected, row.sig_type);
        assert_eq!(tile.m2, row.m2, "m2 {}", row.label);
        assert_eq!(tile.m3, row.m3, "m3 {}", row.label);
        assert_eq!(tile.m3hi, row.m3hi, "m3hi {}", row.label);
        assert_eq!(tile.m5, row.m5, "m5 {}", row.label);
        assert_eq!(
            signal_type_for_track(tile.m2, SignalTrack::X),
            expected,
            "{}",
            row.label
        );
    }
    assert_eq!(RAIL_SIGNALS_MIXED_Y, 18);
}

#[test]
fn golden_pbs_reservation_roundtrip() {
    let row = &fixture().pbs_reservation;
    let encoded = encode_rail_reservation_to_m2_hi(row.track_bits);
    assert_eq!(encoded, row.m2_hi);
    assert_eq!(decode_rail_reservation_m2_hi(row.m2_hi), row.track_bits);
}

#[test]
fn sav_fixture_preserves_mixed_signal_types() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rail_signals_mixed.sav");
    let raw = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("generá el .sav con scripts/gen_rail_signals_sav.py: {e}"));
    let sav = sav::load(&raw).expect("cargar rail_signals_mixed.sav");
    let state = GameState::from_sav_game(sav);

    for row in &fixture().sav_tiles {
        let c = TileCoord::new(row.x, row.y);
        let tile = state.map.get(c).expect("tesela sav");
        assert!(
            rail_tile_is_signals(tile.m5),
            "({},{}) debe ser señal",
            row.x,
            row.y
        );
        assert_eq!(
            signal_type_for_track(tile.m2, SignalTrack::X),
            row.sig_type,
            "tipo en ({},{})",
            row.x,
            row.y
        );
    }
}
