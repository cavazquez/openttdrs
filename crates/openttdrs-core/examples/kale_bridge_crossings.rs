//! Observe real Kale trains crossing existing bridges without construction commands.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::{BufWriter, Write};
use std::path::Path;

use openttdrs_core::{GameState, TileCoord, TileKind, VehicleKind, rail_bridge_other_end};

const TICKS: u32 = 5_000;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: kale_bridge_crossings SOURCE.sav TRACE.csv".into());
    }
    let source = Path::new(&args[0]);
    let trace_path = Path::new(&args[1]);
    let summary = trace_path.with_extension("summary.json");
    std::fs::write(&summary, b"{\"passed\":false,\"stage\":\"preparing\"}\n")?;
    let mut state = GameState::from_sav_game(openttdrs_core::sav::load(&std::fs::read(source)?)?);
    state.ai.enabled = false;
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;
    let mut pairs = BTreeSet::new();
    let mut unpaired = Vec::new();
    let (width, height) = state.map.dimensions();
    for y in 0..height {
        for x in 0..width {
            let pos = TileCoord::new(i32::try_from(x)?, i32::try_from(y)?);
            if state.map.get_kind(pos) != Some(TileKind::RailBridge) {
                continue;
            }
            if let Some(other) = rail_bridge_other_end(&state.map, pos)
                && rail_bridge_other_end(&state.map, other) == Some(pos)
            {
                pairs.insert((pos.min(other), pos.max(other)));
            } else {
                unpaired.push(pos);
            }
        }
    }
    let in_rindington = |p: TileCoord| (185..=217).contains(&p.x) && (95..=146).contains(&p.y);
    let region_pairs: Vec<_> = pairs
        .iter()
        .filter(|(a, b)| in_rindington(*a) && in_rindington(*b))
        .collect();
    let mut trace = BufWriter::new(std::fs::File::create(trace_path)?);
    writeln!(
        trace,
        "relative_tick,train,entry_x,entry_y,exit_x,exit_y,direction,speed,bridge_type,rindington"
    )?;
    let mut crossings = Vec::new();
    for tick in 1..=TICKS {
        let previous: BTreeMap<_, _> = state
            .vehicles
            .iter()
            .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
            .map(|v| (v.id, v.pos))
            .collect();
        state.step();
        for v in state
            .vehicles
            .iter()
            .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        {
            let Some(&old) = previous.get(&v.id).filter(|&&old| old != v.pos) else {
                continue;
            };
            // A fast tick can move beyond the far head. Follow the new tile
            // history back to the previous position instead of losing that crossing.
            let mut movement = vec![v.pos];
            for &pos in &v.rail_tile_history {
                movement.push(pos);
                if pos == old {
                    break;
                }
            }
            if movement.last() != Some(&old) {
                continue;
            }
            for step in movement.windows(2).rev() {
                let (entry, exit) = (step[1], step[0]);
                if rail_bridge_other_end(&state.map, entry) != Some(exit) {
                    continue;
                }
                let bridge_type = openttdrs_core::bridge_type_from_m6(
                    state.map.get(entry).ok_or("missing bridge head")?.m6,
                );
                let regional = in_rindington(entry) && in_rindington(exit);
                writeln!(
                    trace,
                    "{tick},{},{},{},{},{},{},{},{bridge_type:?},{regional}",
                    v.id, entry.x, entry.y, exit.x, exit.y, v.direction, v.cur_speed,
                )?;
                crossings.push(serde_json::json!({
                    "tick": tick, "train": v.id, "entry": entry, "exit": exit,
                    "bridge_type": bridge_type, "rindington": regional,
                }));
            }
        }
    }
    trace.flush()?;
    let regional: Vec<_> = crossings
        .iter()
        .filter(|c| c["rindington"] == true)
        .collect();
    let passed = unpaired.is_empty() && !regional.is_empty();
    let report = serde_json::json!({
        "passed": passed, "source": source, "ticks": TICKS,
        "paired_rail_bridges": pairs.len(), "unpaired_heads": unpaired,
        "rindington_pairs": region_pairs, "crossings": crossings.len(),
        "rindington_crossings": regional,
        "scope": "original Kale fleet and orders; no construction commands; AI, disasters and breakdowns disabled; topological crossings, not native bridge travel-time certification",
    });
    std::fs::write(&summary, serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    if passed {
        Ok(())
    } else {
        Err(
            "No complete regional bridge crossing, or an unpaired bridge head; inspect summary"
                .into(),
        )
    }
}
