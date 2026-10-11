//! Shared complete-cycle proof for the existing Kale bus and truck.

use std::collections::BTreeSet;
use std::error::Error;
use std::io::{BufWriter, Write};
use std::path::Path;

use openttdrs_core::{GameState, TileCoord, Vehicle, VehicleKind, VehicleOrder};

#[derive(Clone, Copy)]
pub struct Case {
    pub id: u32,
    pub kind: VehicleKind,
    pub home_order: usize,
    pub example: &'static str,
    pub label: &'static str,
}

impl Case {
    fn destination_order(self) -> usize {
        (self.home_order + 1) % 3
    }
    fn depot_order(self) -> usize {
        (self.home_order + 2) % 3
    }
}
const MAX_TICKS: u64 = 20_000;
const HASH_INTERVAL: u64 = 64;

struct PreparedRoadVehicle {
    state: GameState,
    warmup: u64,
    home: TileCoord,
    destination: TileCoord,
    depot: TileCoord,
}

fn selected(state: &GameState, case: Case) -> Result<&Vehicle, Box<dyn Error>> {
    state
        .vehicles
        .iter()
        .find(|v| v.id == case.id)
        .ok_or_else(|| format!("Kale {} {} is missing", case.label, case.id).into())
}

fn continuation_snapshot(state: &GameState, case: Case) -> Result<String, Box<dyn Error>> {
    Ok(serde_json::to_string(&(
        selected(state, case)?,
        state.random,
        state.interactive_random,
        &state.companies,
        &state.stats,
        &state.calendar,
        &state.economy_timer,
        state.cur_tileloop_tile,
    ))?)
}

fn prepare(source: &Path, case: Case) -> Result<PreparedRoadVehicle, Box<dyn Error>> {
    let mut state = GameState::from_sav_game(openttdrs_core::sav::load(&std::fs::read(source)?)?);
    let vehicle = selected(&state, case)?;
    if vehicle.kind != case.kind || vehicle.orders.len() != 3 {
        return Err(
            "Expected the existing road vehicle with two stops and its unbunch depot".into(),
        );
    }
    let home = vehicle.orders[case.home_order].destination();
    let destination = vehicle.orders[case.destination_order()].destination();
    let depot = vehicle.orders[case.depot_order()].destination();
    if !matches!(
        vehicle.orders[case.depot_order()],
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
    state.vehicles.retain(|vehicle| vehicle.id == case.id);
    state
        .shared_order_lists
        .retain(|list| Some(list.id) == shared);
    state
        .cargo_payments
        .retain(|payment| payment.front_vehicle_id == Some(case.id));
    state.ai.enabled = false;
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;
    state.hydrate_runtime();
    for warmup in 0..MAX_TICKS {
        let vehicle = selected(&state, case)?;
        if vehicle.last_station_visited == Some(home)
            && vehicle.current_order == case.home_order
            && vehicle.awaiting_load_window
        {
            return Ok(PreparedRoadVehicle {
                state,
                warmup,
                home,
                destination,
                depot,
            });
        }
        state.step();
    }
    let vehicle = selected(&state, case)?;
    Err(format!(
        "Vehicle never reached its starting stop: tile={:?}, frame={}, order={}, transfer={}",
        vehicle.pos,
        vehicle.frame,
        vehicle.current_order,
        vehicle.cargo_transfer_active()
    )
    .into())
}

pub fn run(case: Case) -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(format!("Usage: {} SOURCE.sav COPY.json TRACE.csv", case.example).into());
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
    let PreparedRoadVehicle {
        mut state,
        warmup,
        home,
        destination,
        depot,
    } = prepare(source, case)?;
    openttdrs_core::save::save(&state, output)?;
    let home_stop = selected(&state, case)?.pos;
    let initial_services = selected(&state, case)?.depot_service_generation();
    let start_hash = state.canonical_hash();
    let original_orders = selected(&state, case)?.orders.clone();
    let initial_income = state.companies[selected(&state, case)?.owner.index()].cargo_income_earned;
    let initial_delivered = state.stats.cargo_units_final_delivered;
    let initial_loaded = state.stats.cargo_units_loaded;
    let mut trace = BufWriter::new(std::fs::File::create(trace_path)?);
    writeln!(
        trace,
        "relative_tick,x,y,road_x,road_y,direction,frame,road_state,order,loading,cargo,cargo_loading,cargo_unloading,running,speed,cargo_income,delivered_units,depot_phase,service_generation,last_station_x,last_station_y"
    )?;
    let mut visited = BTreeSet::new();
    let mut directions = BTreeSet::new();
    let mut seen_destination = false;
    let mut seen_depot = false;
    let mut exited_depot = false;
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
        let vehicle = selected(&state, case)?;
        visited.insert(vehicle.pos);
        directions.insert(vehicle.direction);
        let income = state.companies[vehicle.owner.index()].cargo_income_earned;
        writeln!(
            trace,
            "{tick},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
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
            state.stats.cargo_units_final_delivered,
            match vehicle.road_depot_phase {
                openttdrs_core::vehicle::RoadDepotPhase::None => "none",
                openttdrs_core::vehicle::RoadDepotPhase::InDepot => "inside",
                openttdrs_core::vehicle::RoadDepotPhase::Entering { .. } => "entering",
                openttdrs_core::vehicle::RoadDepotPhase::Exiting { .. } => "leaving",
            },
            vehicle.depot_service_generation(),
            vehicle.last_station_visited.map_or(-1, |s| s.x),
            vehicle.last_station_visited.map_or(-1, |s| s.y)
        )?;
        if vehicle.orders != original_orders {
            failure = Some("The route orders changed during the isolated run");
            break;
        }
        if departure_cargo.is_none()
            && vehicle.current_order == case.destination_order()
            && vehicle.pos != home_stop
        {
            departure_cargo = Some(vehicle.cargo);
            departure_delivered = Some(state.stats.cargo_units_final_delivered);
        }
        if !seen_destination
            && vehicle.last_station_visited == Some(destination)
            && vehicle.awaiting_load_window
        {
            seen_destination = true;
            destination_tick = Some(tick);
        }
        if seen_destination
            && destination_delivered.is_none()
            && vehicle.current_order == case.depot_order()
        {
            destination_delivered = Some(
                state
                    .stats
                    .cargo_units_final_delivered
                    .saturating_sub(departure_delivered.unwrap_or(initial_delivered)),
            );
        }
        if seen_destination
            && !seen_depot
            && vehicle.pos == depot
            && vehicle.road_state == openttdrs_core::road_movement::RVSB_IN_DEPOT
            && vehicle.road_depot_phase == openttdrs_core::vehicle::RoadDepotPhase::InDepot
            && vehicle.depot_service_generation() > initial_services
        {
            seen_depot = true;
            depot_tick = Some(tick);
        }
        if seen_depot && vehicle.pos != depot && vehicle.current_order == case.home_order {
            exited_depot = true;
        }
        if seen_destination
            && seen_depot
            && exited_depot
            && vehicle.last_station_visited == Some(home)
            && vehicle.current_order == case.home_order
            && vehicle.awaiting_load_window
        {
            completed_at = Some(tick);
            break;
        }
        if restored.is_none()
            && vehicle.current_order == case.destination_order()
            && vehicle.pos != home_stop
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
            if continuation_snapshot(&state, case)? != continuation_snapshot(reloaded, case)? {
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
    let vehicle = selected(&state, case)?;
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
    let mut report = serde_json::json!({
        "passed": passed, "source": source, "copy": output, "vehicle_id": case.id, "kind": case.kind,
        "warmup_ticks": warmup, "relative_ticks": last_tick,
        "home": [home.x,home.y], "home_stop": [home_stop.x,home_stop.y],
        "home_order": case.home_order, "destination_order": case.destination_order(), "depot_order": case.depot_order(), "destination": [destination.x,destination.y], "depot": [depot.x,depot.y],
        "destination_tick": destination_tick, "depot_tick": depot_tick,
        "depot_service_generations": vehicle.depot_service_generation().saturating_sub(initial_services), "exited_depot": exited_depot,
        "returned_home": completed_at.is_some(), "departure_cargo": departure_cargo,
        "delivered_units": delivered, "cargo_income": income,
        "loaded_units": loaded, "destination_delivered_units": destination_delivered,
        "reload_tick": reload_tick, "reload_matching_ticks": matching_ticks,
        "reload_matching_full_hashes": matching_hashes, "full_hash_interval": HASH_INTERVAL,
        "visited_tiles": visited.len(), "directions": directions,
        "initial_hash": start_hash, "final_hash": state.canonical_hash(),
        "final_tile": [vehicle.pos.x,vehicle.pos.y], "final_frame": vehicle.frame,
        "final_cargo": vehicle.cargo, "failure": failure,
        "scope": "one isolated existing road vehicle with unchanged orders and no construction commands; world advances normally; disasters, AI and breakdowns disabled; no full-fleet/native-tick parity claim"
    });
    report[case.label] = serde_json::json!(case.id);
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    if passed {
        Ok(())
    } else {
        Err(
            "The road vehicle did not complete its reproducible service cycle; inspect the summary"
                .into(),
        )
    }
}
