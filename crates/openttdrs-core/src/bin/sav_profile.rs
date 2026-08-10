//! Perfila carga y ticks de una partida real de `OpenTTD`.
//!
//! ```bash
//! cargo run -p openttdrs-core --release --bin sav_profile -- \
//!   save/Kale_TitleGame.sav --ticks 180
//! ```

#![allow(clippy::print_stdout)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use openttdrs_core::{
    CargoStock, GameState, TickPhaseTimings, TileKind, VehicleKind, VehicleOrder, sav,
    step_profiled,
};

struct Args {
    save: PathBuf,
    ticks: u32,
}

#[derive(Default)]
struct FleetSummary {
    trains: usize,
    road: usize,
    ships: usize,
    aircraft: usize,
    running: usize,
    with_orders: usize,
    route_pending: usize,
    route_pending_moving: usize,
    route_pending_trains: usize,
    route_pending_road: usize,
    route_pending_ships: usize,
    route_pending_aircraft: usize,
}

#[derive(Default)]
struct TrainRouteSummary {
    heads: usize,
    running: usize,
    moving: usize,
    station_orders: usize,
    pending: usize,
    pending_moving: usize,
    pending_station_order: usize,
    pending_stationary: usize,
}

#[derive(Default)]
struct CargoLoadSiteSummary {
    vehicles_on_indexed_terminal: usize,
    vehicles_on_industry_tile: usize,
    stations_with_waiting_cargo: usize,
    vehicles_loading: usize,
    vehicles_awaiting_load_window: usize,
}

/// Volumen de cambios visuales producidos por el core en un tick.
///
/// Estos vectores son una frontera core → cliente: el renderer sólo debe
/// remapear los sprites correspondientes al tick actual.
#[derive(Default)]
struct VisualDirtySummary {
    max_industry: usize,
    max_landscape: usize,
    max_signal: usize,
    max_reservation: usize,
    last_industry: usize,
    last_landscape: usize,
    last_signal: usize,
    last_reservation: usize,
}

impl VisualDirtySummary {
    fn observe(&mut self, state: &GameState) {
        let industry = state.runtime.industry_tile_dirty.len();
        let landscape = state.runtime.landscape_tile_dirty.len();
        let signal = state.runtime.signal_tile_dirty.len();
        let reservation = state.runtime.reservation_tile_dirty.len();
        self.max_industry = self.max_industry.max(industry);
        self.max_landscape = self.max_landscape.max(landscape);
        self.max_signal = self.max_signal.max(signal);
        self.max_reservation = self.max_reservation.max(reservation);
        self.last_industry = industry;
        self.last_landscape = landscape;
        self.last_signal = signal;
        self.last_reservation = reservation;
    }
}

fn print_usage() {
    eprintln!("uso: sav_profile <partida.sav> [--ticks N]");
}

fn parse_args() -> Result<Args, String> {
    let mut save = None;
    let mut ticks = 180_u32;
    let mut values = std::env::args().skip(1);

    while let Some(value) = values.next() {
        match value.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--ticks" => {
                let raw = values
                    .next()
                    .ok_or_else(|| "falta el valor de --ticks".to_string())?;
                ticks = raw
                    .parse()
                    .map_err(|error| format!("--ticks inválido {raw:?}: {error}"))?;
            }
            value if value.starts_with('-') => {
                return Err(format!("opción desconocida: {value}"));
            }
            value if save.is_none() => save = Some(PathBuf::from(value)),
            value => return Err(format!("argumento inesperado: {value}")),
        }
    }

    let save = save.ok_or_else(|| "falta <partida.sav>".to_string())?;
    Ok(Args { save, ticks })
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn summarize_fleet(state: &GameState) -> FleetSummary {
    let mut summary = FleetSummary::default();
    for vehicle in &state.vehicles {
        match vehicle.kind {
            VehicleKind::Train => summary.trains += usize::from(vehicle.is_consist_head()),
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => summary.road += 1,
            VehicleKind::Ship => summary.ships += 1,
            VehicleKind::Aircraft => summary.aircraft += 1,
        }
        summary.running += usize::from(vehicle.running);
        summary.with_orders += usize::from(!vehicle.orders.is_empty());
        let route_pending =
            vehicle.running && !vehicle.orders.is_empty() && vehicle.path.is_empty();
        summary.route_pending += usize::from(route_pending);
        summary.route_pending_moving += usize::from(route_pending && vehicle.cur_speed > 0);
        if route_pending {
            match vehicle.kind {
                VehicleKind::Train => {
                    summary.route_pending_trains += usize::from(vehicle.is_consist_head());
                }
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
                    summary.route_pending_road += 1;
                }
                VehicleKind::Ship => summary.route_pending_ships += 1,
                VehicleKind::Aircraft => summary.route_pending_aircraft += 1,
            }
        }
    }
    summary
}

fn summarize_train_routes(state: &GameState) -> TrainRouteSummary {
    let mut summary = TrainRouteSummary::default();
    for vehicle in state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
    {
        summary.heads += 1;
        summary.running += usize::from(vehicle.running);
        let moving = vehicle.running && vehicle.cur_speed > 0;
        summary.moving += usize::from(moving);
        let station_order = matches!(
            vehicle.current_order_ref(),
            Some(VehicleOrder::Station { .. })
        );
        summary.station_orders += usize::from(station_order);
        let pending = vehicle.running && vehicle.path.is_empty();
        summary.pending += usize::from(pending);
        summary.pending_moving += usize::from(pending && moving);
        summary.pending_station_order += usize::from(pending && station_order);
        summary.pending_stationary += usize::from(pending && !moving);
    }
    summary
}

fn summarize_cargo_load_sites(state: &GameState) -> CargoLoadSiteSummary {
    let mut summary = CargoLoadSiteSummary::default();
    for vehicle in &state.vehicles {
        summary.vehicles_on_indexed_terminal += usize::from(
            !state
                .runtime
                .terminal_spatial_index
                .at(vehicle.pos)
                .is_empty(),
        );
        summary.vehicles_on_industry_tile +=
            usize::from(state.map.get_kind(vehicle.pos) == Some(TileKind::Industry));
        summary.vehicles_loading += usize::from(vehicle.cargo_loading);
        summary.vehicles_awaiting_load_window += usize::from(vehicle.awaiting_load_window);
    }
    summary.stations_with_waiting_cargo = state
        .stations
        .iter()
        .filter(|station| station.cargo_stock != CargoStock::default())
        .count();
    summary
}

fn print_phase(name: &str, ns: u64, total_ns: u64) {
    let micros = ns as f64 / 1_000.0;
    let percent = ns as f64 * 100.0 / total_ns.max(1) as f64;
    println!("{name:>24}  {micros:>10.1} µs  {percent:>5.1}%");
}

fn print_tick_timings(label: &str, timings: TickPhaseTimings) {
    println!("\n=== {label} ===");
    print_phase("timer_economy", timings.timer_economy_ns, timings.total_ns);
    print_phase(
        "tile_animation",
        timings.tile_animation_ns,
        timings.total_ns,
    );
    print_phase("tile_loop", timings.tile_loop_ns, timings.total_ns);
    print_phase(
        "path_recompute",
        timings.path_recompute_ns,
        timings.total_ns,
    );
    print_phase(
        "  path_order_sync",
        timings.path_order_sync_ns,
        timings.total_ns,
    );
    print_phase(
        "  path_station_route",
        timings.path_station_route_ns,
        timings.total_ns,
    );
    let route_search_ms = timings.path_station_route_search_ns as f64 / 1_000_000.0;
    let route_search_max_ms = timings.path_station_route_search_max_ns as f64 / 1_000_000.0;
    let path_failed = timings
        .path_station_route_queries
        .saturating_sub(timings.path_station_route_found);
    println!(
        "  station YAPF: trenes {}, andenes {}, búsquedas {} ({} ok, {} sin ruta), total {:.1} ms, peor {:.1} ms",
        timings.path_station_route_trains,
        timings.path_station_route_candidates,
        timings.path_station_route_queries,
        timings.path_station_route_found,
        path_failed,
        route_search_ms,
        route_search_max_ms,
    );
    print_phase(
        "  path_generic_route",
        timings.path_generic_route_ns,
        timings.total_ns,
    );
    print_phase(
        "vehicle_ops_pre_move",
        timings.vehicle_ops_pre_move_ns,
        timings.total_ns,
    );
    print_phase(
        "  vehicle_ops_only",
        timings.vehicle_ops_only_ns,
        timings.total_ns,
    );
    print_phase(
        "    autoreplace",
        timings.vehicle_ops_autoreplace_ns,
        timings.total_ns,
    );
    print_phase("  pbs_pre_move", timings.pbs_pre_move_ns, timings.total_ns);
    print_phase(
        "cargo_transfer",
        timings.cargo_transfer_ns,
        timings.total_ns,
    );
    print_phase(
        "  cargo_calendar_day",
        timings.cargo_calendar_day_ns,
        timings.total_ns,
    );
    print_phase(
        "  cargo_economy_day",
        timings.cargo_economy_day_ns,
        timings.total_ns,
    );
    print_phase("  cargo_aging", timings.cargo_aging_ns, timings.total_ns);
    print_phase("  cargo_unload", timings.cargo_unload_ns, timings.total_ns);
    print_phase("  cargo_load", timings.cargo_load_ns, timings.total_ns);
    print_phase("movement", timings.movement_ns, timings.total_ns);
    print_phase("  vehicle_move", timings.vehicle_move_ns, timings.total_ns);
    print_phase(
        "  train_collision",
        timings.train_collision_ns,
        timings.total_ns,
    );
    print_phase(
        "  crashed_vehicle",
        timings.crashed_vehicle_ns,
        timings.total_ns,
    );
    print_phase(
        "  pbs_post_move",
        timings.pbs_post_move_ns,
        timings.total_ns,
    );
    print_phase("landscape", timings.landscape_ns, timings.total_ns);
    print_phase("post_tick", timings.post_tick_ns, timings.total_ns);
    print_phase("TOTAL", timings.total_ns, timings.total_ns);
}

fn run(args: &Args) -> Result<(), String> {
    let read_start = Instant::now();
    let raw = std::fs::read(&args.save)
        .map_err(|error| format!("no se pudo leer {}: {error}", args.save.display()))?;
    let read_time = read_start.elapsed();

    let decode_start = Instant::now();
    let sav = sav::load(&raw)
        .map_err(|error| format!("save inválido {}: {error}", args.save.display()))?;
    let decode_time = decode_start.elapsed();
    let version = sav.version;
    let dimensions = sav.map.dimensions();

    let import_start = Instant::now();
    let mut state = GameState::from_sav_game(sav);
    let import_time = import_start.elapsed();
    state
        .runtime
        .terminal_spatial_index
        .rebuild(&state.map, &state.stations);
    let fleet = summarize_fleet(&state);
    let train_routes = summarize_train_routes(&state);
    let cargo_load_sites = summarize_cargo_load_sites(&state);

    println!("=== sav_profile ===");
    println!("save: {}", args.save.display());
    println!(
        "SLV {version}, mapa {}×{}, {} bytes",
        dimensions.0,
        dimensions.1,
        raw.len()
    );
    println!("read:   {:>8.1} ms", milliseconds(read_time));
    println!("decode: {:>8.1} ms", milliseconds(decode_time));
    println!("import: {:>8.1} ms", milliseconds(import_time));
    println!(
        "vehículos: {} (tren cabeza {}, carretera {}, barco {}, avión {}); activos {}, con órdenes {}, rutas pendientes {}",
        state.vehicles.len(),
        fleet.trains,
        fleet.road,
        fleet.ships,
        fleet.aircraft,
        fleet.running,
        fleet.with_orders,
        fleet.route_pending,
    );
    println!(
        "  pendientes: tren {}, carretera {}, barco {}, avión {}; en marcha {}, estacionados {}",
        fleet.route_pending_trains,
        fleet.route_pending_road,
        fleet.route_pending_ships,
        fleet.route_pending_aircraft,
        fleet.route_pending_moving,
        fleet
            .route_pending
            .saturating_sub(fleet.route_pending_moving),
    );
    println!("estaciones: {}", state.stations.len());
    println!(
        "carga: {} industrias, {} estaciones con espera; vehículos en tesela terminal indexada {}, en tesela de industria {}; loading {}, ventana de carga {}",
        state.industries.len(),
        cargo_load_sites.stations_with_waiting_cargo,
        cargo_load_sites.vehicles_on_indexed_terminal,
        cargo_load_sites.vehicles_on_industry_tile,
        cargo_load_sites.vehicles_loading,
        cargo_load_sites.vehicles_awaiting_load_window,
    );
    println!(
        "rutas de tren: cabezas {}, activas {}, en marcha {}, orden estación {}; pendientes {} (en marcha {}, estacionadas {}, de estación {})",
        train_routes.heads,
        train_routes.running,
        train_routes.moving,
        train_routes.station_orders,
        train_routes.pending,
        train_routes.pending_moving,
        train_routes.pending_stationary,
        train_routes.pending_station_order,
    );

    if args.ticks == 0 {
        return Ok(());
    }

    let mut aggregate = TickPhaseTimings::default();
    let mut first = None;
    let mut max = TickPhaseTimings::default();
    let mut max_tick = 0_u32;
    let mut visual_dirty = VisualDirtySummary::default();
    for tick_index in 1..=args.ticks {
        let timings = step_profiled(&mut state);
        visual_dirty.observe(&state);
        first.get_or_insert(timings);
        aggregate.accumulate(timings);
        if timings.total_ns > max.total_ns {
            max = timings;
            max_tick = tick_index;
        }
    }

    print_tick_timings("primer tick", first.unwrap_or_default());
    print_tick_timings(
        &format!("media de {} ticks", args.ticks),
        aggregate.mean(u64::from(args.ticks)),
    );
    print_tick_timings(&format!("peor tick (muestra {max_tick})"), max);
    println!(
        "\nvisual dirty (máximo / último): industria {} / {}, paisaje {} / {}, señales {} / {}, reservas {} / {}",
        visual_dirty.max_industry,
        visual_dirty.last_industry,
        visual_dirty.max_landscape,
        visual_dirty.last_landscape,
        visual_dirty.max_signal,
        visual_dirty.last_signal,
        visual_dirty.max_reservation,
        visual_dirty.last_reservation,
    );
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
