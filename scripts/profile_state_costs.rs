//! Costes de estado de sólo lectura; compilar contra el core release actual.
//! Método y reproducción: docs/parity/openttd-source-performance-review.md.

use std::hint::black_box;
use std::time::Instant;

use openttdrs_core::{GameState, apply_newgrf_stack_catalogs_default_dirs, sav};

fn measure<T>(name: &str, mut operation: impl FnMut() -> T) {
    drop(black_box(operation()));
    for sample in 1..=5 {
        let start = Instant::now();
        let result = black_box(operation());
        let ms = start.elapsed().as_secs_f64() * 1_000.0;
        println!("{name},{sample},{ms:.6}");
        drop(result);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: state-costs SAVE.sav")?;
    let raw = std::fs::read(path)?;
    let loaded = sav::load(&raw).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    let json = state.save_json()?;
    eprintln!(
        "tick={} vehicles={} stations={} json_bytes={}",
        state.tick.get(),
        state.vehicles.len(),
        state.stations.len(),
        json.len()
    );
    println!("operation,sample,ms");
    measure("save_json_pretty", || state.save_json().expect("serialize"));
    measure("canonical_hash_v4", || state.canonical_hash());
    measure("clone_with_runtime", || state.clone());
    measure("load_json_with_hydration", || {
        GameState::load_json(&json).expect("deserialize")
    });
    Ok(())
}
