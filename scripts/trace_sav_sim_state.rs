//! Differential trace for a loaded SAV with NewGRF catalogues.
//!
//! Compile against each core build and run with SAVE.sav TICKS OUTPUT_DIR.
//! The SAV importer currently collects shared order lists from a HashMap.
//! Sort those lists by their stable ID once, before advancing either state,
//! so both executions start with the same serialized vector order.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;

use openttdrs_core::{GameState, apply_newgrf_stack_catalogs_default_dirs, sav};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: state-trace SAVE.sav TICKS OUTPUT_DIR")?;
    let ticks: u32 = args.get(2).ok_or("ticks")?.parse()?;
    let out = std::path::PathBuf::from(args.get(3).ok_or("output dir")?);
    std::fs::create_dir_all(&out)?;
    let loaded = sav::load(&std::fs::read(path)?).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    state.shared_order_lists.sort_by_key(|list| list.id);
    println!("phase,tick,canonical_hash_v4,event_hash_rust_1_98");
    for phase in 0..=ticks {
        if phase != 0 {
            state.step();
        }
        let mut event_hasher = DefaultHasher::new();
        format!(
            "{:?}",
            state.runtime.pending_sim_events.iter().collect::<Vec<_>>()
        )
        .hash(&mut event_hasher);
        format!("{:?}", state.runtime.pending_income_popups).hash(&mut event_hasher);
        format!("{:?}", state.runtime.pending_newgrf_sounds).hash(&mut event_hasher);
        let mut file = std::io::BufWriter::new(std::fs::File::create(
            out.join(format!("tiles-{phase}.txt")),
        )?);
        let (width, height) = state.map.dimensions();
        writeln!(file, "{width},{height}")?;
        for (index, tile) in state.map.tiles().iter().enumerate() {
            writeln!(file, "{index},{tile:?}")?;
        }
        println!(
            "{phase},{},{:016x},{:016x}",
            state.tick.get(),
            state.canonical_hash(),
            event_hasher.finish()
        );
    }
    Ok(())
}
