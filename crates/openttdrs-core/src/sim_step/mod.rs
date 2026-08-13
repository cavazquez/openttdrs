//! Simulación del tick principal de `GameState`.
//!
//! ## Fases autoritativas del tick (orden `OpenTTD`; P2.2 / P2.4)
//!
//! 1. **timers**: `tick.advance` + calendario/economía.
//! 2. **`timer_economy`**: economía mensual/anual y rollover de beneficios.
//! 3. **`tile_animation`**: `AnimateAnimatedTiles` (industrias / aeropuertos).
//! 4. **`tile_loop`**: `RunTileLoop` (LFSR).
//! 5. **`path_recompute`**: liberación de depot + rutas (sin PBS completo).
//! 6. **`cargo_transfer`**: descarga y carga (`LoadUnloadStation`) + barridos diarios.
//! 7. **`vehicle_ops_pre_move`**: horarios, autoreemplazo, wander, aeronaves.
//! 8. **`movement`**: movimiento + PBS post-move.
//! 9. **`landscape`**: `CallLandscapeTick` town → trees → station → industry → companies → linkgraph.
//! 10. **`post_tick`**: refits, señales, costos, noticias, paridad.

mod cargo_transfer;
mod economy;
mod landscape;
mod movement;
mod routing;
mod vehicle_ops;

use std::time::Instant;

use crate::GameState;

/// Tiempos por fase de un tick (`GameState::step_profiled` / bin `sim_profile`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickPhaseTimings {
    pub timer_economy_ns: u64,
    pub tile_animation_ns: u64,
    pub tile_loop_ns: u64,
    pub path_recompute_ns: u64,
    /// Subfase: actualizar los destinos a partir de órdenes.
    pub path_order_sync_ns: u64,
    /// Subfase: adjudicar andenes y rutear trenes de estación.
    pub path_station_route_ns: u64,
    /// Trenes de estación que requirieron adjudicación de andén.
    pub path_station_route_trains: u32,
    /// Andenes candidatos considerados para esos trenes.
    pub path_station_route_candidates: u32,
    /// Búsquedas YAPF efectuadas durante la adjudicación de andén.
    pub path_station_route_queries: u32,
    /// Búsquedas YAPF que encontraron una ruta durante la adjudicación.
    pub path_station_route_found: u32,
    /// Tiempo acumulado de las búsquedas YAPF de adjudicación.
    pub path_station_route_search_ns: u64,
    /// Duración de la búsqueda YAPF más lenta de adjudicación.
    pub path_station_route_search_max_ns: u64,
    /// Subfase: rutas de vehículos que no pasaron por la adjudicación de andén.
    pub path_generic_route_ns: u64,
    pub vehicle_ops_pre_move_ns: u64,
    /// Horarios, servicio, autoreemplazo y aeronaves antes de mover.
    pub vehicle_ops_only_ns: u64,
    /// Intentos de autoreemplazo en depósitos dentro de `vehicle_ops`.
    pub vehicle_ops_autoreplace_ns: u64,
    /// Primera pasada de actualización de reservas PBS del tick.
    pub pbs_pre_move_ns: u64,
    /// Barrido diario de calendario repartido entre `DAY_TICKS` slots.
    pub cargo_calendar_day_ns: u64,
    /// Barrido diario de economía/servicio repartido entre `DAY_TICKS` slots.
    pub cargo_economy_day_ns: u64,
    /// Envejecimiento de paquetes de carga antes de descarga/carga.
    pub cargo_aging_ns: u64,
    /// Descarga de vehículos en estación.
    pub cargo_unload_ns: u64,
    /// Carga de vehículos desde industrias o estaciones.
    pub cargo_load_ns: u64,
    /// Total de las cinco subfases anteriores.
    pub cargo_transfer_ns: u64,
    pub movement_ns: u64,
    /// Subfase de avance de vehículos dentro de `movement`.
    pub vehicle_move_ns: u64,
    /// Subfase de detección/resolución de choques de tren dentro de `movement`.
    pub train_collision_ns: u64,
    /// Subfase de timers/eliminación de vehículos estrellados dentro de `movement`.
    pub crashed_vehicle_ns: u64,
    /// Subfase PBS posterior al movimiento dentro de `movement`.
    pub pbs_post_move_ns: u64,
    pub landscape_ns: u64,
    pub post_tick_ns: u64,
    pub total_ns: u64,
}

impl TickPhaseTimings {
    /// Suma las fases en nanosegundos (sin overhead de instrumentación entre fases).
    #[must_use]
    pub const fn phases_sum_ns(self) -> u64 {
        self.timer_economy_ns
            + self.tile_animation_ns
            + self.tile_loop_ns
            + self.path_recompute_ns
            + self.vehicle_ops_pre_move_ns
            + self.cargo_transfer_ns
            + self.movement_ns
            + self.landscape_ns
            + self.post_tick_ns
    }

    /// Acumula otro tick (para promedios).
    pub fn accumulate(&mut self, other: Self) {
        self.timer_economy_ns += other.timer_economy_ns;
        self.tile_animation_ns += other.tile_animation_ns;
        self.tile_loop_ns += other.tile_loop_ns;
        self.path_recompute_ns += other.path_recompute_ns;
        self.path_order_sync_ns += other.path_order_sync_ns;
        self.path_station_route_ns += other.path_station_route_ns;
        self.path_station_route_trains += other.path_station_route_trains;
        self.path_station_route_candidates += other.path_station_route_candidates;
        self.path_station_route_queries += other.path_station_route_queries;
        self.path_station_route_found += other.path_station_route_found;
        self.path_station_route_search_ns += other.path_station_route_search_ns;
        self.path_station_route_search_max_ns = self
            .path_station_route_search_max_ns
            .max(other.path_station_route_search_max_ns);
        self.path_generic_route_ns += other.path_generic_route_ns;
        self.vehicle_ops_pre_move_ns += other.vehicle_ops_pre_move_ns;
        self.vehicle_ops_only_ns += other.vehicle_ops_only_ns;
        self.vehicle_ops_autoreplace_ns += other.vehicle_ops_autoreplace_ns;
        self.pbs_pre_move_ns += other.pbs_pre_move_ns;
        self.cargo_calendar_day_ns += other.cargo_calendar_day_ns;
        self.cargo_economy_day_ns += other.cargo_economy_day_ns;
        self.cargo_aging_ns += other.cargo_aging_ns;
        self.cargo_unload_ns += other.cargo_unload_ns;
        self.cargo_load_ns += other.cargo_load_ns;
        self.cargo_transfer_ns += other.cargo_transfer_ns;
        self.movement_ns += other.movement_ns;
        self.vehicle_move_ns += other.vehicle_move_ns;
        self.train_collision_ns += other.train_collision_ns;
        self.crashed_vehicle_ns += other.crashed_vehicle_ns;
        self.pbs_post_move_ns += other.pbs_post_move_ns;
        self.landscape_ns += other.landscape_ns;
        self.post_tick_ns += other.post_tick_ns;
        self.total_ns += other.total_ns;
    }

    /// Divide totales por `n` ticks (media aritmética).
    #[must_use]
    pub fn mean(self, n: u64) -> Self {
        if n == 0 {
            return Self::default();
        }
        Self {
            timer_economy_ns: self.timer_economy_ns / n,
            tile_animation_ns: self.tile_animation_ns / n,
            tile_loop_ns: self.tile_loop_ns / n,
            path_recompute_ns: self.path_recompute_ns / n,
            path_order_sync_ns: self.path_order_sync_ns / n,
            path_station_route_ns: self.path_station_route_ns / n,
            path_station_route_trains: self.path_station_route_trains
                / u32::try_from(n).unwrap_or(u32::MAX),
            path_station_route_candidates: self.path_station_route_candidates
                / u32::try_from(n).unwrap_or(u32::MAX),
            path_station_route_queries: self.path_station_route_queries
                / u32::try_from(n).unwrap_or(u32::MAX),
            path_station_route_found: self.path_station_route_found
                / u32::try_from(n).unwrap_or(u32::MAX),
            path_station_route_search_ns: self.path_station_route_search_ns / n,
            path_station_route_search_max_ns: self.path_station_route_search_max_ns,
            path_generic_route_ns: self.path_generic_route_ns / n,
            vehicle_ops_pre_move_ns: self.vehicle_ops_pre_move_ns / n,
            vehicle_ops_only_ns: self.vehicle_ops_only_ns / n,
            vehicle_ops_autoreplace_ns: self.vehicle_ops_autoreplace_ns / n,
            pbs_pre_move_ns: self.pbs_pre_move_ns / n,
            cargo_calendar_day_ns: self.cargo_calendar_day_ns / n,
            cargo_economy_day_ns: self.cargo_economy_day_ns / n,
            cargo_aging_ns: self.cargo_aging_ns / n,
            cargo_unload_ns: self.cargo_unload_ns / n,
            cargo_load_ns: self.cargo_load_ns / n,
            cargo_transfer_ns: self.cargo_transfer_ns / n,
            movement_ns: self.movement_ns / n,
            vehicle_move_ns: self.vehicle_move_ns / n,
            train_collision_ns: self.train_collision_ns / n,
            crashed_vehicle_ns: self.crashed_vehicle_ns / n,
            pbs_post_move_ns: self.pbs_post_move_ns / n,
            landscape_ns: self.landscape_ns / n,
            post_tick_ns: self.post_tick_ns / n,
            total_ns: self.total_ns / n,
        }
    }
}

/// Tick principal de la simulación (sin instrumentación).
pub(crate) fn step(state: &mut GameState) {
    // El cliente consume estas listas después de este `step`. Abrir aquí el
    // delta siguiente impide que señales/reservas de ticks viejos fuercen
    // remaps de chunks en cada frame.
    state.runtime.begin_tick_visual_delta();
    state.ensure_companies();
    state.runtime.fleet_index.rebuild(&state.vehicles);
    state
        .runtime
        .terminal_spatial_index
        .rebuild(&state.map, &state.stations);
    state.tick.advance();
    state.advance_game_timers();
    let t = state.tick.get();

    phase_timer_economy(state);
    phase_tile_animation(state, t);
    phase_tile_loop(state, t);
    phase_path_recompute(state);

    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    // Barridos escalonados dentro de CallVehicleTicks (P2.5).
    crate::vehicle::process_vehicle_calendar_day(state);
    crate::vehicle::process_vehicle_economy_day(state);
    economy::age_vehicle_cargo(state);
    // OpenTTD: LoadUnloadStation antes de Vehicle::Tick (movimiento).
    cargo_transfer::unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    cargo_transfer::load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);
    // Ops que pueden cambiar destino van tras la carga (p. ej. wander orderless).
    let _ = phase_vehicle_ops_pre_move(state);
    // PBS grueso tras la carga y antes del move (P2.2: ya no precede a LoadUnload).
    // ChooseTrainTrack (P2.7) elige vía + reserva atómica al entrar en tesela.
    phase_pbs_reservations(state);

    phase_movement(state);
    landscape::call_landscape_tick(state, t);
    phase_post_tick(state);
}

/// Igual que el tick principal interno, midiendo cada fase (solo para profiling / bin `sim_profile`).
#[must_use]
pub fn step_profiled(state: &mut GameState) -> TickPhaseTimings {
    let wall0 = Instant::now();
    let mut timings = TickPhaseTimings::default();

    state.runtime.begin_tick_visual_delta();
    state.ensure_companies();
    state.runtime.fleet_index.rebuild(&state.vehicles);
    state
        .runtime
        .terminal_spatial_index
        .rebuild(&state.map, &state.stations);
    state.tick.advance();
    state.advance_game_timers();
    let t = state.tick.get();

    let p0 = Instant::now();
    phase_timer_economy(state);
    timings.timer_economy_ns = nanos(p0);

    let p0 = Instant::now();
    phase_tile_animation(state, t);
    timings.tile_animation_ns = nanos(p0);

    let p0 = Instant::now();
    phase_tile_loop(state, t);
    timings.tile_loop_ns = nanos(p0);

    let p0 = Instant::now();
    let routing = routing::recompute_vehicle_paths_profiled(state);
    timings.path_recompute_ns = nanos(p0);
    timings.path_order_sync_ns = routing.order_sync_ns;
    timings.path_station_route_ns = routing.station_route_ns;
    timings.path_station_route_trains = routing.station_route_trains;
    timings.path_station_route_candidates = routing.station_route_candidates;
    timings.path_station_route_queries = routing.station_route_path_queries;
    timings.path_station_route_found = routing.station_route_path_found;
    timings.path_station_route_search_ns = routing.station_route_path_ns;
    timings.path_station_route_search_max_ns = routing.station_route_path_max_ns;
    timings.path_generic_route_ns = routing.generic_route_ns;

    let p0 = Instant::now();
    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    let cargo_phase = Instant::now();
    crate::vehicle::process_vehicle_calendar_day(state);
    timings.cargo_calendar_day_ns = nanos(cargo_phase);
    let cargo_phase = Instant::now();
    crate::vehicle::process_vehicle_economy_day(state);
    timings.cargo_economy_day_ns = nanos(cargo_phase);
    let cargo_phase = Instant::now();
    economy::age_vehicle_cargo(state);
    timings.cargo_aging_ns = nanos(cargo_phase);
    let cargo_phase = Instant::now();
    cargo_transfer::unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    timings.cargo_unload_ns = nanos(cargo_phase);
    let cargo_phase = Instant::now();
    cargo_transfer::load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);
    timings.cargo_load_ns = nanos(cargo_phase);
    timings.cargo_transfer_ns = nanos(p0);

    let p0 = Instant::now();
    let vehicle_ops = Instant::now();
    let vehicle_ops_timings = phase_vehicle_ops_pre_move(state);
    timings.vehicle_ops_only_ns = nanos(vehicle_ops);
    timings.vehicle_ops_autoreplace_ns = vehicle_ops_timings.autoreplace_ns;
    let pbs = Instant::now();
    phase_pbs_reservations(state);
    timings.pbs_pre_move_ns = nanos(pbs);
    timings.vehicle_ops_pre_move_ns = nanos(p0);

    let p0 = Instant::now();
    let vehicle_move = Instant::now();
    movement::move_vehicles(state);
    timings.vehicle_move_ns = nanos(vehicle_move);
    let collisions = Instant::now();
    crate::train_collision::resolve_train_collisions(state);
    timings.train_collision_ns = nanos(collisions);
    let crashed = Instant::now();
    crate::ground_crash::tick_crashed_vehicles(state);
    timings.crashed_vehicle_ns = nanos(crashed);
    let pbs = Instant::now();
    phase_pbs_reservations(state);
    timings.pbs_post_move_ns = nanos(pbs);
    timings.movement_ns = nanos(p0);

    let p0 = Instant::now();
    landscape::call_landscape_tick(state, t);
    timings.landscape_ns = nanos(p0);

    let p0 = Instant::now();
    phase_post_tick(state);
    timings.post_tick_ns = nanos(p0);
    timings.total_ns = nanos(wall0);

    timings
}

fn nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Economía disparada por timers (mes/año), no landscape.
fn phase_timer_economy(state: &mut GameState) {
    if state.runtime.economy_triggers.new_month {
        economy::process_monthly_economy(state);
    }
    if state.runtime.calendar_triggers.new_year {
        economy::rollover_vehicle_profit_year(state);
        crate::town::increment_all_house_ages(&mut state.map);
    }
}

/// `AnimateAnimatedTiles`: animación de industrias y aeropuertos.
///
/// Usa las visitas del tile loop del tick anterior (si las hay) más la lista de industrias.
fn phase_tile_animation(state: &mut GameState, t: u64) {
    let visits = std::mem::take(&mut state.runtime.tile_loop_visited);
    let bubble_spawns = crate::map::bubble_generator_spawns_from_visits(&visits);
    let _lift_dirty = crate::map::step_house_lifts(
        &mut state.map,
        t,
        &visits,
        &mut state.random,
        &mut state.runtime.active_house_lifts,
    );
    state.runtime.industry_tile_dirty = crate::map::step_industry_tiles_with_seed(
        &mut state.map,
        t,
        &visits,
        state.world_seed,
        &state.industries,
    );
    let animation_coords: Vec<_> = state
        .industries
        .iter()
        .flat_map(|industry| {
            if industry.tiles.is_empty() {
                vec![industry.pos]
            } else {
                industry.tiles.clone()
            }
        })
        .collect();
    state
        .runtime
        .industry_tile_dirty
        .extend(crate::map::advance_newgrf_industry_animated_tiles(
            &mut state.map,
            t,
            &animation_coords,
            &state.industry_tile_spec_catalog,
            state.world_seed,
            &mut state.newgrf_animated_industry_tiles,
        ));
    state
        .runtime
        .industry_tile_dirty
        .sort_by_key(|coord| (coord.x, coord.y));
    state.runtime.industry_tile_dirty.dedup();
    for at in bubble_spawns {
        state
            .runtime
            .pending_sim_events
            .push(crate::sim_events::SimEvent::Bubble {
                at,
                direction: (state.random.next() & 3) as u8,
            });
    }
    let airport_dirty = crate::map::step_airport_tiles(&mut state.map, t, &state.stations);
    state.runtime.industry_tile_dirty.extend(airport_dirty);
    state
        .runtime
        .industry_tile_dirty
        .sort_by_key(|coord| (coord.x, coord.y));
    state.runtime.industry_tile_dirty.dedup();
}

/// `RunTileLoop`: LFSR de Galois.
fn phase_tile_loop(state: &mut GameState, t: u64) {
    state.runtime.landscape_tile_dirty.clear();
    state.runtime.tile_loop_visited =
        crate::map::collect_tile_loop_visits(&state.map, t, &mut state.cur_tileloop_tile);
}

/// Recálculo de rutas sin reservas PBS (el PBS se resuelve tras el movimiento / en B4).
fn phase_path_recompute(state: &mut GameState) {
    crate::parity::release_staged_depot_trains(state);
    routing::recompute_vehicle_paths(state);
}

/// Horarios, autoreemplazo, extensión de rutas, fases de aeronaves (antes del movimiento).
#[derive(Debug, Clone, Copy, Default)]
struct VehicleOpsTimings {
    autoreplace_ns: u64,
}

fn phase_vehicle_ops_pre_move(state: &mut GameState) -> VehicleOpsTimings {
    let mut timings = VehicleOpsTimings::default();
    vehicle_ops::tick_vehicle_timetables(state);
    vehicle_ops::sync_autoreplace_depot_flags(state);
    let p0 = Instant::now();
    vehicle_ops::run_autoreplace_in_depots(state);
    timings.autoreplace_ns = nanos(p0);
    vehicle_ops::update_servicing_and_road_depot_orders(state);
    routing::extend_orderless_vehicle_paths(state);
    routing::assign_orderless_wander_destinations(state);
    movement::tick_aircraft_phases(state);
    timings
}

/// Reservas PBS + sync a `m2_hi` (fase gruesa hasta B4).
fn phase_pbs_reservations(state: &mut GameState) {
    let wormholes_pbs = state.jgr_tunnel_wormholes();
    let wh_pbs = if wormholes_pbs.is_empty() {
        None
    } else {
        Some(&wormholes_pbs)
    };
    let dirty_before = state.runtime.reservation_tile_dirty.len();
    crate::rail_pbs::update_train_reservations_incremental_with_wormholes(
        &state.map,
        &mut state.vehicles,
        state.pathfinding,
        wh_pbs,
    );
    crate::rail_pbs::sync_reservations_to_map(
        &mut state.map,
        &state.vehicles,
        &mut state.runtime.reservation_tiles_active,
        &mut state.runtime.reservation_tile_dirty,
    );
    if state.runtime.reservation_tile_dirty.len() > dirty_before {
        crate::rail_signals::enqueue_pbs_reservations_for_signal_update(
            &mut state.runtime.signal_globset,
            &state.vehicles,
        );
        routing::drain_signal_globset_now(state);
    }
}

/// Movimiento de vehículos, colisiones y PBS post-move.
fn phase_movement(state: &mut GameState) {
    movement::move_vehicles(state);
    crate::train_collision::resolve_train_collisions(state);
    crate::ground_crash::tick_crashed_vehicles(state);
    // OpenTTD sigue/libera la reserva al cruzar tesela; recalcular evita un tick de retraso.
    phase_pbs_reservations(state);
}

/// Refits, señales post-movimiento, sincronización de destinos, costos, noticias, paridad.
fn phase_post_tick(state: &mut GameState) {
    vehicle_ops::apply_pending_depot_order_refits(state);

    // También cubre cambios externos de la flota (tests, red, carga) que no
    // pasan por el movimiento y por lo tanto no encolan su tesela anterior.
    // Las reservas PBS sí se actualizan sólo al cambiar para evitar barrerlas
    // completas cada tick.
    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.runtime.signal_globset,
        &state.vehicles,
    );
    routing::drain_signal_globset_now(state);

    vehicle_ops::sync_vehicle_order_destinations(state);
    economy::apply_vehicle_running_costs(state);
    crate::news::poll_vehicle_advice_news(state);
    crate::news::maybe_purge_old_news(state);
    crate::parity::record_tick(state);
}
