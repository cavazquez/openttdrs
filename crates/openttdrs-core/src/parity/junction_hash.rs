//! Fingerprint estable de tiles rail/señal para goldens Junctionary.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::rail_tile_is_signals;

/// Región rectangular inclusiva `[x0..=x1] × [y0..=y1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JunctionBounds {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl JunctionBounds {
    #[must_use]
    pub const fn full_map(map: &Map) -> Self {
        let (w, h) = map.dimensions();
        Self {
            x0: 0,
            y0: 0,
            x1: w.cast_signed().saturating_sub(1),
            y1: h.cast_signed().saturating_sub(1),
        }
    }
}

/// Hash determinista de tiles de transporte/señal en la región.
///
/// Incluye `kind`, `m2`, `m3`, `m3hi`, `m5`, `m7`, `m8` de `Rail` / `RailDepot` /
/// `RailBridge` / `RailTunnel` / `Station` (rail). Orden: fila-major y → x.
#[must_use]
pub fn hash_junction_tiles(map: &Map, bounds: JunctionBounds) -> u64 {
    let mut hasher = DefaultHasher::new();
    b"junctionary-v1".hash(&mut hasher);
    bounds.x0.hash(&mut hasher);
    bounds.y0.hash(&mut hasher);
    bounds.x1.hash(&mut hasher);
    bounds.y1.hash(&mut hasher);
    for y in bounds.y0..=bounds.y1 {
        for x in bounds.x0..=bounds.x1 {
            let pos = TileCoord::new(x, y);
            let Some(tile) = map.get(pos) else {
                continue;
            };
            if !tile_in_junction_fingerprint(tile.kind, tile.m5) {
                continue;
            }
            (x, y).hash(&mut hasher);
            tile_kind_tag(tile.kind).hash(&mut hasher);
            tile.m2.hash(&mut hasher);
            tile.m3.hash(&mut hasher);
            tile.m3hi.hash(&mut hasher);
            tile.m5.hash(&mut hasher);
            tile.m7.hash(&mut hasher);
            tile.m8.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn tile_in_junction_fingerprint(kind: TileKind, _m5: u8) -> bool {
    matches!(
        kind,
        TileKind::Rail
            | TileKind::RailDepot
            | TileKind::RailBridge
            | TileKind::RailTunnel
            | TileKind::Station
    )
}

const fn tile_kind_tag(kind: TileKind) -> u8 {
    match kind {
        TileKind::Grass => 0,
        TileKind::Water => 1,
        TileKind::Forest => 2,
        TileKind::CoalField => 3,
        TileKind::Road => 4,
        TileKind::Rail => 5,
        TileKind::RoadDepot => 6,
        TileKind::RailDepot => 7,
        TileKind::ShipDepot => 8,
        TileKind::Airport => 9,
        TileKind::RoadTunnel => 10,
        TileKind::RailTunnel => 11,
        TileKind::RoadBridge => 12,
        TileKind::RailBridge => 13,
        TileKind::House => 14,
        TileKind::Station => 15,
        TileKind::Industry => 16,
        TileKind::Void => 17,
        TileKind::Unknown(v) => 0x80 | (v & 0x7F),
    }
}

/// Cuenta teselas con señales en la región.
#[must_use]
pub fn count_signal_tiles(map: &Map, bounds: JunctionBounds) -> usize {
    let mut n = 0usize;
    for y in bounds.y0..=bounds.y1 {
        for x in bounds.x0..=bounds.x1 {
            let pos = TileCoord::new(x, y);
            let Some(tile) = map.get(pos) else {
                continue;
            };
            if tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
                n += 1;
            }
        }
    }
    n
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::parity::build_rail_signals_mixed;

    #[test]
    fn hash_stable_for_rail_signals_mixed() {
        let a = build_rail_signals_mixed();
        let b = build_rail_signals_mixed();
        let bounds = JunctionBounds::full_map(&a.map);
        assert_eq!(
            hash_junction_tiles(&a.map, bounds),
            hash_junction_tiles(&b.map, bounds)
        );
        assert!(count_signal_tiles(&a.map, bounds) >= 6);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod emit {
    use super::*;
    use crate::parity::build_scenario;

    #[test]
    fn emit_junctionary_golden_json() {
        // Escenarios rail/junction estables (town_growth/breakdown pueden exigir datos extra).
        const NAMES: &[&str] = &[
            "truck_bay",
            "train_line",
            "train_supply",
            "train_supply_dual",
            "train_supply_signal",
            "train_signal",
            "train_pbs",
            "ai_rival_line",
            "rail_signals_mixed",
        ];
        if std::env::var_os("EMIT_JUNCTIONARY_GOLDEN").is_none() {
            return;
        }
        let mut scenarios = Vec::new();
        for name in NAMES {
            let state = build_scenario(name).unwrap();
            let bounds = JunctionBounds::full_map(&state.map);
            let hash = hash_junction_tiles(&state.map, bounds);
            let signals = count_signal_tiles(&state.map, bounds);
            scenarios.push(serde_json::json!({
                "name": name,
                "tile_hash": format!("{hash:016x}"),
                "signal_tiles": signals,
                "bounds": {
                    "x0": bounds.x0,
                    "y0": bounds.y0,
                    "x1": bounds.x1,
                    "y1": bounds.y1
                }
            }));
        }
        let doc = serde_json::json!({"version": 1, "scenarios": scenarios});
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/parity/junctionary_golden.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        eprintln!("wrote {}", path.display());
    }
}
