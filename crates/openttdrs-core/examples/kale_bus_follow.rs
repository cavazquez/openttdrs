//! Prepare Kale bus 1888 at its first stop, then prove a complete service cycle.
//! The original SAV is untouched; see docs/parity/kale-bus-follow.md.

use std::collections::BTreeSet;
use std::error::Error;
use std::io::{BufWriter, Write};
use std::path::Path;

use openttdrs_core::{GameState, TileCoord, Vehicle, VehicleKind, VehicleOrder};

const BUS: u32 = 1888;
const MAX_TICKS: u64 = 20_000;
const HASH_INTERVAL: u64 = 64;

struct PreparedBus {
    state: GameState,
    warmup: u64,
    home: TileCoord,
    destination: TileCoord,
    depot: TileCoord,
}

fn bus(state: &GameState) -> Result<&Vehicle, Box<dyn Error>> {
    state
        .vehicles
        .iter()
        .find(|v| v.id == BUS)
        .ok_or_else(|| "Kale bus 1888 is missing".into())
}

fn continuation_snapshot(state: &GameState) -> Result<String, Box<dyn Error>> {
    Ok(serde_json::to_string(&(
        bus(state)?,
        state.random,
        state.interactive_random,
        &state.companies,
        &state.stats,
        &state.calendar,
        &state.economy_timer,
        state.cur_tileloop_tile,
    ))?)
}

fn prepare(source: &Path) -> Result<PreparedBus, Box<dyn Error>> {
    let mut state = GameState::from_sav_game(openttdrs_core::sav::load(&std::fs::read(source)?)?);
    let vehicle = bus(&state)?;
    if vehicle.kind != VehicleKind::Bus || vehicle.orders.len() != 3 {
        return Err("Expected Kale bus 1888 with two stops and its unbunch depot".into());
    }
    let home = vehicle.orders[0].destination();
    let destination = vehicle.orders[1].destination();
    let depot = vehicle.orders[2].destination();
    if !matches!(
        vehicle.orders[2],
        VehicleOrder::Depot {
            unbunch: true,
            stop: false,
            ..
        }
    ) {
        return Err("The native unbunch order must resume rather than halt".into());
    }
    if state.map.get_kind(depot) != Some(openttdrs_core::TileKind::RoadDepot) {
        return Err("The imported DepotID must resolve to a real road depot".into());
    }
    let shared = vehicle.shared_order_id;
    state.vehicles.retain(|vehicle| vehicle.id == BUS);
    state
        .shared_order_lists
        .retain(|list| Some(list.id) == shared);
    state
        .cargo_payments
        .retain(|payment| payment.front_vehicle_id == Some(BUS));
    state.ai.enabled = false;
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;
    state.hydrate_runtime();
    for warmup in 0..MAX_TICKS {
        let vehicle = bus(&state)?;
        if vehicle.pos == home && vehicle.current_order == 0 && vehicle.awaiting_load_window {
            return Ok(PreparedBus {
                state,
                warmup,
                home,
                destination,
                depot,
            });
        }
        state.step();
    }
    let vehicle = bus(&state)?;
    Err(format!(
        "Bus never reached its starting stop: tile={:?}, frame={}, order={}, transfer={}",
        vehicle.pos,
        vehicle.frame,
        vehicle.current_order,
        vehicle.cargo_transfer_active()
    )
    .into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: kale_bus_follow SOURCE.sav COPY.json TRACE.csv".into());
    }
    let source = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    let trace_path = Path::new(&args[2]);
    let summary_path = trace_path.with_extension("summary.json");
    if source == output
        || output
            .extension()
            .is_none_or(|extension| extension != "json")
    {
        return Err("The copy must be a separate JSON file".into());
    }
    // Overwrite the verdict before preparing: even a warmup failure cannot
    // leave a successful report from an earlier execution behind.
    std::fs::write(
        &summary_path,
        b"{\"passed\":false,\"stage\":\"preparing\"}\n",
    )?;
    let PreparedBus {
        mut state,
        warmup,
        home,
        destination,
        depot,
    } = prepare(source)?;
    openttdrs_core::save::save(&state, output)?;
    let start_hash = state.canonical_hash();
    let original_orders = bus(&state)?.orders.clone();
    let initial_income = state.companies[bus(&state)?.owner.index()].cargo_income_earned;
    let initial_delivered = state.stats.cargo_units_final_delivered;
    let initial_loaded = state.stats.cargo_units_loaded;
    let mut trace = BufWriter::new(std::fs::File::create(trace_path)?);
    writeln!(
        trace,
        "relative_tick,x,y,road_x,road_y,direction,frame,road_state,order,loading,cargo,cargo_loading,cargo_unloading,running,speed,cargo_income,delivered_units"
    )?;
    let mut visited = BTreeSet::new();
    let mut directions = BTreeSet::new();
    let mut seen_destination = false;
    let mut seen_depot = false;
    let mut departure_cargo = None;
    let mut departure_delivered = None;
    let mut destination_delivered = None;
    let mut destination_tick = None;
    let mut depot_tick = None;
    let mut completed_at = None;
    let mut restored = None;
    let mut reload_tick = None;
    let mut matching_ticks = 0_u64;
    let mut matching_hashes = 0_u64;
    let mut failure = None;
    let mut last_tick = 0;
    for tick in 0..MAX_TICKS {
        last_tick = tick;
        let vehicle = bus(&state)?;
        visited.insert(vehicle.pos);
        directions.insert(vehicle.direction);
        let income = state.companies[vehicle.owner.index()].cargo_income_earned;
        writeln!(
            trace,
            "{tick},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            vehicle.pos.x,
            vehicle.pos.y,
            vehicle.road_x,
            vehicle.road_y,
            vehicle.direction,
            vehicle.frame,
            vehicle.road_state,
            vehicle.current_order,
            vehicle.awaiting_load_window,
            vehicle.cargo,
            vehicle.cargo_loading,
            vehicle.cargo_unloading,
            vehicle.running,
            vehicle.cur_speed,
            income,
            state.stats.cargo_units_final_delivered
        )?;
        if vehicle.orders != original_orders {
            failure = Some("The route orders changed during the isolated run");
            break;
        }
        if departure_cargo.is_none() && vehicle.current_order == 1 && vehicle.pos != home {
            departure_cargo = Some(vehicle.cargo);
            departure_delivered = Some(state.stats.cargo_units_final_delivered);
        }
        if !seen_destination && vehicle.pos == destination && vehicle.awaiting_load_window {
            seen_destination = true;
            destination_tick = Some(tick);
        }
        if seen_destination && destination_delivered.is_none() && vehicle.current_order == 2 {
            destination_delivered = Some(
                state
                    .stats
                    .cargo_units_final_delivered
                    .saturating_sub(departure_delivered.unwrap_or(initial_delivered)),
            );
        }
        if seen_destination && !seen_depot && vehicle.pos == depot {
            seen_depot = true;
            depot_tick = Some(tick);
        }
        if seen_destination
            && seen_depot
            && vehicle.pos == home
            && vehicle.current_order == 0
            && vehicle.awaiting_load_window
        {
            completed_at = Some(tick);
            break;
        }
        if restored.is_none()
            && vehicle.current_order == 1
            && vehicle.pos != home
            && vehicle.cargo > 0
            && visited.len() >= 4
        {
            let checkpoint = trace_path.with_extension("mid.json");
            openttdrs_core::save::save(&state, &checkpoint)?;
            let reloaded = openttdrs_core::save::load(&checkpoint)?;
            if state.canonical_hash() != reloaded.canonical_hash() {
                failure = Some("Save/load changed the checkpoint hash");
                break;
            }
            restored = Some(reloaded);
            reload_tick = Some(tick);
        }
        state.step();
        if let Some(reloaded) = &mut restored {
            reloaded.step();
            if continuation_snapshot(&state)? != continuation_snapshot(reloaded)? {
                failure = Some("The reloaded branch diverged during the service cycle");
                break;
            }
            // The complete 256x256 state is expensive to hash. Check the
            // operational state every tick and the entire world every 64.
            if tick % HASH_INTERVAL == 0 {
                if state.canonical_hash() != reloaded.canonical_hash() {
                    failure = Some("The full reloaded world diverged at a checkpoint");
                    break;
                }
                matching_hashes += 1;
            }
            matching_ticks += 1;
        }
    }
    if let Some(reloaded) = &restored {
        if state.canonical_hash() != reloaded.canonical_hash() {
            failure = Some("The final full-world hashes differ");
        } else {
            matching_hashes += 1;
        }
    }
    trace.flush()?;
    let vehicle = bus(&state)?;
    let delivered = state
        .stats
        .cargo_units_final_delivered
        .saturating_sub(initial_delivered);
    let income = state.companies[vehicle.owner.index()]
        .cargo_income_earned
        .saturating_sub(initial_income);
    let loaded = state
        .stats
        .cargo_units_loaded
        .saturating_sub(initial_loaded);
    let passed = failure.is_none()
        && completed_at.is_some()
        && departure_cargo.is_some_and(|cargo| cargo > 0)
        && delivered > 0
        && income > 0
        && loaded > 0
        && destination_delivered.is_some_and(|units| units > 0)
        && reload_tick.is_some()
        && matching_ticks > 0;
    let report = serde_json::json!({
        "passed": passed, "source": source, "copy": output, "bus": BUS,
        "warmup_ticks": warmup, "relative_ticks": last_tick,
        "home": [home.x,home.y], "destination": [destination.x,destination.y], "depot": [depot.x,depot.y],
        "destination_tick": destination_tick, "depot_tick": depot_tick,
        "returned_home": completed_at.is_some(), "departure_cargo": departure_cargo,
        "delivered_units": delivered, "cargo_income": income,
        "loaded_units": loaded, "destination_delivered_units": destination_delivered,
        "reload_tick": reload_tick, "reload_matching_ticks": matching_ticks,
        "reload_matching_full_hashes": matching_hashes, "full_hash_interval": HASH_INTERVAL,
        "visited_tiles": visited.len(), "directions": directions,
        "initial_hash": start_hash, "final_hash": state.canonical_hash(),
        "final_tile": [vehicle.pos.x,vehicle.pos.y], "final_frame": vehicle.frame,
        "final_cargo": vehicle.cargo, "failure": failure,
        "scope": "one isolated existing bus with unchanged orders and no construction commands; world advances normally; disasters, AI and breakdowns disabled; no full-fleet/native-tick parity claim"
    });
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    if passed {
        Ok(())
    } else {
        Err("The bus did not complete its reproducible service cycle; inspect the summary".into())
    }
}
