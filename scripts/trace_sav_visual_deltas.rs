//! State/map trace plus ordered landscape redraw notifications for a loaded SAV.
//!
//! Writes dirty-PHASE.csv with the notified coordinates and identifies raw
//! CLEAR_FIELDS changes limited to the internal MAP5 counter. Chunk counts
//! use the client's 16×16 geometry; they are not viewport remap counts.
//! Hashing and full tile dumps make this a parity tool, not a timing benchmark.
//!
//! Compile against each core build and run with SAVE.sav TICKS OUTPUT_DIR.
//! Older SAV importers collected shared order lists from a HashMap. Sort
//! those lists by their stable ID once, before advancing either state, so
//! comparisons with those builds start with the same serialized vector order.
//! Use --keep-order-list-order to check the importer without that preparation.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;

use openttdrs_core::{GameState, apply_newgrf_stack_catalogs_default_dirs, sav};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: visual-delta-trace SAVE.sav TICKS OUTPUT_DIR")?;
    let ticks: u32 = args.get(2).ok_or("ticks")?.parse()?;
    let out = std::path::PathBuf::from(args.get(3).ok_or("output dir")?);
    let sort_shared_lists = match args.get(4).map(String::as_str) {
        None => true,
        Some("--keep-order-list-order") => false,
        Some(_) => return Err("unknown option: expected --keep-order-list-order".into()),
    };
    std::fs::create_dir_all(&out)?;
    let loaded = sav::load(&std::fs::read(path)?).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    if sort_shared_lists {
        state.shared_order_lists.sort_by_key(|list| list.id);
    }
    println!(
        "phase,tick,canonical_hash_v4,event_hash_rust_1_98,landscape_dirty_count,landscape_dirty_chunks,counter_only_field_changes"
    );
    let mut previous_tiles = state.map.tiles().to_vec();
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
        let (width, _) = state.map.dimensions();
        let mut counter_only = std::collections::HashSet::new();
        for (index, (&previous, &current)) in
            previous_tiles.iter().zip(state.map.tiles()).enumerate()
        {
            if previous == current
                || previous.kind != openttdrs_core::TileKind::Grass
                || previous.ottd_type_nibble() != 0
                || ((previous.m5 >> 2) & 7) != 3
            {
                continue;
            }
            let mut normalized = current;
            normalized.m5 = (current.m5 & 0x1F) | (previous.m5 & 0xE0);
            if normalized == previous {
                counter_only.insert(openttdrs_core::TileCoord::new(
                    (index % width as usize) as i32,
                    (index / width as usize) as i32,
                ));
            }
        }
        let mut dirty_file = std::io::BufWriter::new(std::fs::File::create(
            out.join(format!("dirty-{phase}.csv")),
        )?);
        writeln!(dirty_file, "x,y,counter_only_field_change")?;
        let mut chunks = std::collections::HashSet::new();
        for coord in &state.runtime.landscape_tile_dirty {
            writeln!(
                dirty_file,
                "{},{},{}",
                coord.x,
                coord.y,
                counter_only.contains(coord)
            )?;
            chunks.insert((coord.x.div_euclid(16), coord.y.div_euclid(16)));
        }
        println!(
            "{phase},{},{:016x},{:016x},{},{},{}",
            state.tick.get(),
            state.canonical_hash(),
            event_hasher.finish(),
            state.runtime.landscape_tile_dirty.len(),
            chunks.len(),
            counter_only.len(),
        );
        previous_tiles = state.map.tiles().to_vec();
    }
    Ok(())
}
