//! Simulación del tick principal de `GameState`.
//!
//! ## Fases autoritativas del tick (orden fijo; no reordenar sin evidencia)
//!
//! 1. **`economy_and_world`**: economía mensual, producción de industrias, crecimiento de ciudades,
//!    envejecimiento de carga en vehículos, subsidios, animación de teselas (árboles, desastres).
//! 2. **`routing_and_signals`**: liberación de depots, recomputación de rutas, actualización de
//!    señales y reservas PBS.
//! 3. **`tile_animation`**: animación de teselas de industrias y aeropuertos.
//! 4. **`cargo_transfer`**: descarga y carga de vehículos en estaciones/industrias.
//! 5. **`vehicle_ops_pre_move`**: horarios, autoreemplazo, extensión de rutas para vehículos sin
//!    órdenes, fases de aeronaves.
//! 6. **movement**: movimiento de todos los vehículos, colisiones de trenes.
//! 7. **`post_tick`**: refits pendientes, actualización de señales post-movimiento, sincronización
//!    de destinos, costos de operación, noticias, registro de paridad.
//!
//! Mantiene el orden exacto de llamadas del antiguo `sim_step.rs` para preservar el comportamiento
//! observable de la simulación.

mod cargo_transfer;
mod economy;
mod movement;
mod routing;
mod vehicle_ops;

use std::time::Instant;

use crate::{GameState, station};

/// Tiempos por fase de un tick (`GameState::step_profiled` / bin `sim_profile`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickPhaseTimings {
    pub economy_and_world_ns: u64,
    pub routing_and_signals_ns: u64,
    pub tile_animation_ns: u64,
    pub cargo_transfer_ns: u64,
    pub vehicle_ops_pre_move_ns: u64,
    pub movement_ns: u64,
    pub post_tick_ns: u64,
    pub total_ns: u64,
}

impl TickPhaseTimings {
    /// Suma las fases en nanosegundos (sin overhead de instrumentación entre fases).
    #[must_use]
    pub const fn phases_sum_ns(self) -> u64 {
        self.economy_and_world_ns
            + self.routing_and_signals_ns
            + self.tile_animation_ns
            + self.cargo_transfer_ns
            + self.vehicle_ops_pre_move_ns
            + self.movement_ns
            + self.post_tick_ns
    }

    /// Acumula otro tick (para promedios).
    pub fn accumulate(&mut self, other: Self) {
        self.economy_and_world_ns += other.economy_and_world_ns;
        self.routing_and_signals_ns += other.routing_and_signals_ns;
        self.tile_animation_ns += other.tile_animation_ns;
        self.cargo_transfer_ns += other.cargo_transfer_ns;
        self.vehicle_ops_pre_move_ns += other.vehicle_ops_pre_move_ns;
        self.movement_ns += other.movement_ns;
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
            economy_and_world_ns: self.economy_and_world_ns / n,
            routing_and_signals_ns: self.routing_and_signals_ns / n,
            tile_animation_ns: self.tile_animation_ns / n,
            cargo_transfer_ns: self.cargo_transfer_ns / n,
            vehicle_ops_pre_move_ns: self.vehicle_ops_pre_move_ns / n,
            movement_ns: self.movement_ns / n,
            post_tick_ns: self.post_tick_ns / n,
            total_ns: self.total_ns / n,
        }
    }
}

/// Tick principal de la simulación (sin instrumentación).
pub(crate) fn step(state: &mut GameState) {
    state.ensure_companies();
    state.tick.advance();
    let t = state.tick.get();

    phase_economy_and_world(state, t);
    phase_routing_and_signals(state);
    phase_tile_animation(state, t);

    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    cargo_transfer::unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    cargo_transfer::load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);

    phase_vehicle_ops_pre_move(state);
    phase_movement(state);
    phase_post_tick(state);
}

/// Igual que [`step`], midiendo cada fase (solo para profiling / bin `sim_profile`).
#[must_use]
pub fn step_profiled(state: &mut GameState) -> TickPhaseTimings {
    let wall0 = Instant::now();
    let mut timings = TickPhaseTimings::default();

    state.ensure_companies();
    state.tick.advance();
    let t = state.tick.get();

    let p0 = Instant::now();
    phase_economy_and_world(state, t);
    timings.economy_and_world_ns = nanos(p0);

    let p0 = Instant::now();
    phase_routing_and_signals(state);
    timings.routing_and_signals_ns = nanos(p0);

    let p0 = Instant::now();
    phase_tile_animation(state, t);
    timings.tile_animation_ns = nanos(p0);

    let p0 = Instant::now();
    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    cargo_transfer::unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    cargo_transfer::load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);
    timings.cargo_transfer_ns = nanos(p0);

    let p0 = Instant::now();
    phase_vehicle_ops_pre_move(state);
    timings.vehicle_ops_pre_move_ns = nanos(p0);

    let p0 = Instant::now();
    phase_movement(state);
    timings.movement_ns = nanos(p0);

    let p0 = Instant::now();
    phase_post_tick(state);
    timings.post_tick_ns = nanos(p0);
    timings.total_ns = nanos(wall0);

    timings
}

fn nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Fase 1: economía mensual, producción de industrias/ciudades, envejecimiento de carga, subsidios.
fn phase_economy_and_world(state: &mut GameState, t: u64) {
    economy::process_monthly_economy(state, t);
    economy::rollover_vehicle_profit_year(state, t);
    crate::ai::tick_ai_companies(state, t);
    crate::gs::tick_gs(state);
    economy::produce_industries(state, t);
    economy::maybe_change_industry_production(state, t);
    economy::produce_town_demand(state, t);
    economy::grow_towns(state, t);
    economy::age_vehicle_cargo(state);

    // `UpdateStationRating` corre en su propio ciclo de 185 ticks, no una vez por día.
    if t > 0 && t.is_multiple_of(u64::from(crate::economy::STATION_RATING_TICKS)) {
        station::update_station_ratings(
            &mut state.stations,
            state.order.selectgoods,
            &mut state.cargo_rng,
        );
    }

    crate::subsidy::tick_subsidies(state);
    state.runtime.landscape_tile_dirty.clear();
    crate::map::tree_tile_loop::tick_tree_tile_loop(state);
    crate::disaster::tick_disasters(state);
}

/// Fase 2: rutas de vehículos, señales y reservas PBS.
fn phase_routing_and_signals(state: &mut GameState) {
    crate::parity::release_staged_depot_trains(state);
    routing::recompute_vehicle_paths(state);

    // Señales: solo `_globset` (sin barrido global).
    state.runtime.signal_tile_dirty.clear();
    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.runtime.signal_globset,
        &state.vehicles,
    );
    routing::drain_signal_globset_now(state);

    // PBS Fase 3: reservas con huella de consist; TryReserve usa wormholes de túnel.
    let wormholes_pbs = state.jgr_tunnel_wormholes();
    let wh_pbs = if wormholes_pbs.is_empty() {
        None
    } else {
        Some(&wormholes_pbs)
    };
    crate::rail_pbs::update_train_reservations_with_wormholes(
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
    crate::rail_signals::enqueue_pbs_reservations_for_signal_update(
        &mut state.runtime.signal_globset,
        &state.vehicles,
    );
    routing::drain_signal_globset_now(state);
}

/// Fase 3: animación de teselas de industrias y aeropuertos.
fn phase_tile_animation(state: &mut GameState, t: u64) {
    state.runtime.industry_tile_dirty = crate::map::step_industry_tiles_with_seed(
        &mut state.map,
        t,
        state.world_seed,
        &state.industries,
    );
    let airport_dirty = crate::map::step_airport_tiles(&mut state.map, t, &state.stations);
    state.runtime.industry_tile_dirty.extend(airport_dirty);
}

/// Fase 5: horarios, autoreemplazo, extensión de rutas, fases de aeronaves (antes del movimiento).
fn phase_vehicle_ops_pre_move(state: &mut GameState) {
    vehicle_ops::tick_vehicle_timetables(state);
    vehicle_ops::sync_autoreplace_depot_flags(state);
    vehicle_ops::run_autoreplace_in_depots(state);
    routing::extend_orderless_vehicle_paths(state);
    routing::assign_orderless_wander_destinations(state);
    movement::tick_aircraft_phases(state);
}

/// Fase 6: movimiento de vehículos y colisiones de trenes.
fn phase_movement(state: &mut GameState) {
    movement::move_vehicles(state);
    crate::train_collision::resolve_train_collisions(state);
    // OpenTTD sigue/libera la reserva al cruzar tesela dentro del tick del tren.
    // Recalcular tras el movimiento evita un tick de retraso en `m2_hi`.
    let wormholes_pbs = state.jgr_tunnel_wormholes();
    let wh_pbs = if wormholes_pbs.is_empty() {
        None
    } else {
        Some(&wormholes_pbs)
    };
    crate::rail_pbs::update_train_reservations_with_wormholes(
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
}

/// Fase 7: refits, señales post-movimiento, sincronización de destinos, costos, noticias, paridad.
fn phase_post_tick(state: &mut GameState) {
    vehicle_ops::apply_pending_depot_order_refits(state);

    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.runtime.signal_globset,
        &state.vehicles,
    );
    crate::rail_signals::enqueue_pbs_reservations_for_signal_update(
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
