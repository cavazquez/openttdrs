//! Prepare and exercise a controlled copy of Kale's existing train 2902.
//! See docs/parity/kale-train-follow.md for changes to the copy and limits.

use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::io::{BufWriter, Write};
use std::path::Path;

use openttdrs_core::{Command, GameState, TileCoord, TileKind, apply_command};

const TRAIN: u32 = 2902;
const MAX_TICKS: u64 = 20_000;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let without_signals = args.iter().any(|arg| arg == "--without-signals");
    args.retain(|arg| arg != "--without-signals");
    if args.len() != 3 {
        return Err(
            "Usage: kale_train_follow SOURCE.sav COPY.json TRACE.csv [--without-signals]".into(),
        );
    }
    let source = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    if source == output || output.extension().is_none_or(|ext| ext != "json") {
        return Err("The copy must be a separate .json file".into());
    }
    let mut state = GameState::from_sav_game(openttdrs_core::sav::load(&std::fs::read(source)?)?);
    let head = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN)
        .ok_or("Kale train 2902 is missing")?;
    if head.orders.len() != 2 {
        return Err("The selected train must have its two original station orders".into());
    }
    let home = head.orders[0].destination();
    let destination = head.orders[1].destination();
    let mut ids = BTreeSet::new();
    let mut cursor = Some(TRAIN);
    while let Some(id) = cursor {
        if !ids.insert(id) {
            return Err("Cyclic consist".into());
        }
        cursor = state
            .vehicles
            .iter()
            .find(|v| v.id == id)
            .ok_or("Missing consist unit")?
            .next_unit;
    }
    state.vehicles.retain(|v| ids.contains(&v.id));
    // The native save starts during full loading. Use the normal skip-order
    // command once to depart with the existing cargo; preserve both orders.
    apply_command(&mut state, &Command::SkipVehicleOrder(TRAIN)).map_err(|e| format!("{e:?}"))?;
    let mut removed_signals = 0;
    if without_signals {
        let (width, height) = state.map.dimensions();
        for y in 0..i32::try_from(height)? {
            for x in 0..i32::try_from(width)? {
                let tile = TileCoord::new(x, y);
                if !state.map.get(tile).is_some_and(|raw| {
                    raw.kind == TileKind::Rail
                        && openttdrs_core::rail_signals::rail_tile_is_signals(raw.m5)
                }) {
                    continue;
                }
                for (fx, fy) in [(64, 64), (200, 200), (128, 128), (200, 64), (64, 200)] {
                    if apply_command(&mut state, &Command::RemoveRailSignal(tile, fx, fy)).is_ok() {
                        removed_signals += 1;
                    }
                }
            }
        }
    }
    state.hydrate_runtime();
    // Clear reservations belonging to removed trains. An empty previous set
    // deliberately preserves imported reservations, so include the whole map.
    let (width, height) = state.map.dimensions();
    let mut active = HashSet::new();
    for y in 0..i32::try_from(height)? {
        for x in 0..i32::try_from(width)? {
            active.insert(TileCoord::new(x, y));
        }
    }
    let mut dirty = Vec::new();
    openttdrs_core::rail_pbs::sync_reservations_to_map(
        &mut state.map,
        &state.vehicles,
        &mut active,
        &mut dirty,
    );
    state.runtime.reservation_tiles_active = active;
    state.runtime.reservation_tile_dirty = dirty;
    openttdrs_core::save::save(&state, output)?;
    let trace_path = Path::new(&args[2]);
    let mut trace = BufWriter::new(std::fs::File::create(trace_path)?);
    writeln!(
        trace,
        "relative_tick,id,x,y,direction,rail_pixel,progress,speed,head_order,loading,cargo"
    )?;
    let mut visited = BTreeSet::new();
    let mut visited_destination = false;
    let mut emptied = false;
    let initial_cargo: u32 = state.vehicles.iter().map(|v| v.cargo).sum();
    if initial_cargo == 0 {
        return Err("The departure has no native cargo to test unloading".into());
    }
    let mut completed_at = None;
    for tick in 0..MAX_TICKS {
        let head = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN)
            .ok_or("Train disappeared")?;
        visited.insert(head.pos);
        let cargo: u32 = state.vehicles.iter().map(|v| v.cargo).sum();
        emptied |= cargo == 0;
        visited_destination |=
            head.awaiting_load_window && head.last_station_visited == Some(destination);
        for unit in &state.vehicles {
            writeln!(
                trace,
                "{tick},{},{},{},{},{},{},{},{},{},{}",
                unit.id,
                unit.pos.x,
                unit.pos.y,
                unit.direction,
                unit.rail_pixel,
                unit.progress,
                unit.cur_speed,
                head.current_order,
                head.awaiting_load_window,
                unit.cargo
            )?;
        }
        if visited_destination
            && emptied
            && head.current_order == 0
            && head.awaiting_load_window
            && head.last_station_visited == Some(home)
        {
            completed_at = Some(tick);
            break;
        }
        state.step();
    }
    trace.flush()?;
    let head = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN)
        .ok_or("Train disappeared")?;
    let report = serde_json::json!({
        "passed": completed_at.is_some(), "source": source, "copy": output, "train": TRAIN,
        "units": ids, "removed_signals": removed_signals,
        "relative_ticks": completed_at.unwrap_or(MAX_TICKS),
        "visited_tiles": visited.len(), "initial_cargo": initial_cargo,
        "destination_visited": visited_destination, "emptied": emptied,
        "returned_home": completed_at.is_some(),
        "final_tile": [head.pos.x, head.pos.y], "pbs_stuck": head.pbs_stuck,
        "scope": "controlled isolated consist; not a full-fleet or signalling parity certificate"
    });
    // Also overwrite the summary on failure so a previous successful run
    // cannot masquerade as the current result.
    std::fs::write(
        trace_path.with_extension("summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    if completed_at.is_some() {
        Ok(())
    } else {
        Err(format!(
            "Train did not complete the cycle within {MAX_TICKS} ticks; inspect {}",
            trace_path.display()
        )
        .into())
    }
}
