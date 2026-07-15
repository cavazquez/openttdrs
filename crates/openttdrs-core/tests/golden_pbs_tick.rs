//! Golden tick-a-tick interno PBS (#97).
//!
//! Traza `pos` + `reserved_len` cada tick (no muestreado). Comparable en formato
//! a una captura OpenTTD; el fixture actual es regresión openttdrs.
//!
//! Regenerar: `OPENTTDRS_UPDATE_GOLDEN=1 cargo test -p openttdrs-core --test golden_pbs_tick`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::parity::{TRAIN_PBS_NORTH_ID, TRAIN_PBS_SOUTH_ID, build_train_pbs};
use serde::{Deserialize, Serialize};

const TICKS: u64 = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TickRow {
    tick: u64,
    vehicle: u32,
    x: i32,
    y: i32,
    progress: u8,
    reserved_len: u16,
    blocked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Fixture {
    note: String,
    ticks: u64,
    rows: Vec<TickRow>,
}

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity/train_pbs_tick_golden.json")
}

fn collect_rows() -> Vec<TickRow> {
    let mut state = build_train_pbs();
    state.enable_parity_trace();
    for _ in 0..TICKS {
        state.step();
    }
    let records = state.take_parity_records();
    let mut out = Vec::new();
    for r in &records {
        for v in &r.vehicles {
            if v.id != TRAIN_PBS_NORTH_ID && v.id != TRAIN_PBS_SOUTH_ID {
                continue;
            }
            let Some(rail) = v.rail.as_ref() else {
                continue;
            };
            out.push(TickRow {
                tick: r.tick,
                vehicle: v.id,
                x: v.tile.x,
                y: v.tile.y,
                progress: v.progress,
                reserved_len: rail.reserved_len,
                blocked: rail.blocked_by_reservation,
            });
        }
    }
    out.sort_by(|a, b| a.tick.cmp(&b.tick).then(a.vehicle.cmp(&b.vehicle)));
    out
}

#[test]
fn train_pbs_tick_trace_matches_golden() {
    let rows = collect_rows();
    assert!(
        rows.len() >= TICKS as usize,
        "trazas insuficientes: {}",
        rows.len()
    );

    let path = fixture_path();
    if std::env::var_os("OPENTTDRS_UPDATE_GOLDEN").is_some() {
        let fixture = Fixture {
            note: "Tick-a-tick interno train_pbs (#97). Formato listo para diff vs OpenTTD.".into(),
            ticks: TICKS,
            rows: rows.clone(),
        };
        let json = serde_json::to_string_pretty(&fixture).expect("serialize");
        std::fs::write(&path, format!("{json}\n")).expect("write");
        eprintln!("actualizado {}", path.display());
        return;
    }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "fixture {}: {e} (regenerá con OPENTTDRS_UPDATE_GOLDEN=1)",
            path.display()
        )
    });
    let fixture: Fixture = serde_json::from_str(&raw).expect("parse");
    assert_eq!(fixture.ticks, TICKS);
    assert_eq!(
        fixture.rows, rows,
        "divergencia tick PBS — regenerá con OPENTTDRS_UPDATE_GOLDEN=1 si es intencional"
    );
}
