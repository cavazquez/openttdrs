//! Golden interno PBS (`train_pbs`) — observabilidad #54.
//!
//! Compara snapshots por tick de `reserved_len` / `reservation_end` /
//! `blocked_by_reservation` contra `train_pbs_golden.json`.
//!
//! No es tick-a-tick vs OpenTTD (follow-up #54 / captura externa).
//! Regenerar: `OPENTTDRS_UPDATE_GOLDEN=1 cargo test -p openttdrs-core --test golden_pbs`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::parity::{
    TRAIN_PBS_NORTH_ID, TRAIN_PBS_SOUTH_ID, build_train_pbs, detect_known_divergences,
};
use serde::{Deserialize, Serialize};

const TICKS: u64 = 80;
const SAMPLE_EVERY: u64 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Snapshot {
    tick: u64,
    vehicle: u32,
    reserved_len: u16,
    reservation_end: Option<(i32, i32)>,
    blocked_by_reservation: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Fixture {
    /// Golden interno openttdrs (no OpenTTD).
    note: String,
    ticks: u64,
    sample_every: u64,
    snapshots: Vec<Snapshot>,
}

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity/train_pbs_golden.json")
}

fn collect_snapshots() -> Vec<Snapshot> {
    let mut state = build_train_pbs();
    state.enable_parity_trace();
    for _ in 0..TICKS {
        state.step();
    }
    let records = state.take_parity_records();
    let mut out = Vec::new();
    for r in &records {
        if r.tick % SAMPLE_EVERY != 0 && r.tick != TICKS {
            continue;
        }
        for v in &r.vehicles {
            let Some(rail) = v.rail.as_ref() else {
                continue;
            };
            if v.id != TRAIN_PBS_NORTH_ID && v.id != TRAIN_PBS_SOUTH_ID {
                continue;
            }
            out.push(Snapshot {
                tick: r.tick,
                vehicle: v.id,
                reserved_len: rail.reserved_len,
                reservation_end: rail.reservation_end.map(|c| (c.x, c.y)),
                blocked_by_reservation: rail.blocked_by_reservation,
            });
        }
    }
    out.sort_by(|a, b| a.tick.cmp(&b.tick).then(a.vehicle.cmp(&b.vehicle)));
    out
}

#[test]
fn train_pbs_reservation_trace_matches_golden() {
    let snapshots = collect_snapshots();
    assert!(
        snapshots
            .iter()
            .any(|s| s.vehicle == TRAIN_PBS_NORTH_ID && s.reserved_len >= 3),
        "norte sin reserva: {snapshots:?}"
    );
    assert!(
        snapshots
            .iter()
            .any(|s| s.vehicle == TRAIN_PBS_SOUTH_ID && s.reserved_len >= 3),
        "sur sin reserva: {snapshots:?}"
    );

    let path = fixture_path();
    if std::env::var_os("OPENTTDRS_UPDATE_GOLDEN").is_some() {
        let fixture = Fixture {
            note: "Golden interno openttdrs train_pbs (#54). No comparar contra OpenTTD.".into(),
            ticks: TICKS,
            sample_every: SAMPLE_EVERY,
            snapshots: snapshots.clone(),
        };
        let json = serde_json::to_string_pretty(&fixture).expect("serialize");
        std::fs::write(&path, format!("{json}\n")).expect("write golden");
        eprintln!("actualizado {}", path.display());
        return;
    }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "fixture {}: {e} (regenerá con OPENTTDRS_UPDATE_GOLDEN=1)",
            path.display()
        )
    });
    let fixture: Fixture = serde_json::from_str(&raw).expect("parse golden");
    assert_eq!(fixture.ticks, TICKS);
    assert_eq!(fixture.sample_every, SAMPLE_EVERY);
    assert_eq!(
        fixture.snapshots, snapshots,
        "divergencia PBS — regenerá con OPENTTDRS_UPDATE_GOLDEN=1 si el cambio es intencional"
    );
}

#[test]
fn train_pbs_divergence_check_passes() {
    let mut state = build_train_pbs();
    state.enable_parity_trace();
    for _ in 0..TICKS {
        state.step();
    }
    let records = state.take_parity_records();
    let divs = detect_known_divergences(&records);
    let pbs = divs
        .iter()
        .find(|d| d.id == "train_pbs_reservation_active")
        .expect("chequeo PBS");
    assert!(!pbs.detected, "{}", pbs.evidence);
}
