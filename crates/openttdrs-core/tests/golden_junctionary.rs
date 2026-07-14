//! Golden Junctionary: hashes de tiles/señales por escenario de paridad.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::parity::{
    JunctionBounds, build_scenario, count_signal_tiles, hash_junction_tiles,
};

#[derive(serde::Deserialize)]
struct Fixture {
    version: u32,
    scenarios: Vec<ScenarioRow>,
}

#[derive(serde::Deserialize)]
struct ScenarioRow {
    name: String,
    tile_hash: String,
    signal_tiles: usize,
    bounds: BoundsRow,
}

#[derive(serde::Deserialize)]
struct BoundsRow {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

fn fixture() -> Fixture {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity/junctionary_golden.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("parsear junctionary_golden.json")
}

#[test]
fn golden_junctionary_hashes_match_scenarios() {
    let f = fixture();
    assert_eq!(f.version, 1);
    assert!(!f.scenarios.is_empty());
    for row in &f.scenarios {
        let state = build_scenario(&row.name).unwrap_or_else(|| panic!("escenario {}", row.name));
        let bounds = JunctionBounds {
            x0: row.bounds.x0,
            y0: row.bounds.y0,
            x1: row.bounds.x1,
            y1: row.bounds.y1,
        };
        let hash = hash_junction_tiles(&state.map, bounds);
        let got = format!("{hash:016x}");
        assert_eq!(got, row.tile_hash, "hash {}", row.name);
        assert_eq!(
            count_signal_tiles(&state.map, bounds),
            row.signal_tiles,
            "señales {}",
            row.name
        );
    }
}
