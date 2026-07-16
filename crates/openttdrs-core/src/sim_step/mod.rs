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

use crate::{GameState, station};

/// Tick principal de la simulación.
///
/// Avanza el estado del juego un tick, ejecutando todas las fases de economía, mundo, vehículos
/// y lógica del juego en el orden correcto para `OpenTTD`.
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

/// Fase 1: economía mensual, producción de industrias/ciudades, envejecimiento de carga, subsidios.
fn phase_economy_and_world(state: &mut GameState, t: u64) {
    economy::process_monthly_economy(state, t);
    economy::rollover_vehicle_profit_year(state, t);
    crate::ai::tick_ai_companies(state, t);
    crate::gs::tick_gs(state);
    economy::produce_industries(state, t);
    economy::produce_town_demand(state, t);
    economy::grow_towns(state, t);
    economy::age_vehicle_cargo(state);

    if t > 0 && t.is_multiple_of(u64::from(crate::economy::TICKS_PER_TRANSIT_DAY)) {
        station::tick_station_cargo_age(&mut state.stations, state.order.selectgoods);
    }

    crate::subsidy::tick_subsidies(state);
    state.landscape_tile_dirty.clear();
    crate::map::tree_tile_loop::tick_tree_tile_loop(state);
    crate::disaster::tick_disasters(state);
}

/// Fase 2: rutas de vehículos, señales y reservas PBS.
fn phase_routing_and_signals(state: &mut GameState) {
    crate::parity::release_staged_depot_trains(state);
    routing::recompute_vehicle_paths(state);

    // Señales: solo `_globset` (sin barrido global).
    state.signal_tile_dirty.clear();
    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.signal_globset,
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
        &mut state.reservation_tiles_active,
        &mut state.reservation_tile_dirty,
    );
    crate::rail_signals::enqueue_pbs_reservations_for_signal_update(
        &mut state.signal_globset,
        &state.vehicles,
    );
    routing::drain_signal_globset_now(state);
}

/// Fase 3: animación de teselas de industrias y aeropuertos.
fn phase_tile_animation(state: &mut GameState, t: u64) {
    state.industry_tile_dirty = crate::map::step_industry_tiles_with_seed(
        &mut state.map,
        t,
        state.world_seed,
        &state.industries,
    );
    let airport_dirty = crate::map::step_airport_tiles(&mut state.map, t, &state.stations);
    state.industry_tile_dirty.extend(airport_dirty);
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
}

/// Fase 7: refits, señales post-movimiento, sincronización de destinos, costos, noticias, paridad.
fn phase_post_tick(state: &mut GameState) {
    vehicle_ops::apply_pending_depot_order_refits(state);

    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.signal_globset,
        &state.vehicles,
    );
    crate::rail_signals::enqueue_pbs_reservations_for_signal_update(
        &mut state.signal_globset,
        &state.vehicles,
    );
    routing::drain_signal_globset_now(state);

    vehicle_ops::sync_vehicle_order_destinations(state);
    economy::apply_vehicle_running_costs(state);
    crate::news::poll_vehicle_advice_news(state);
    crate::news::maybe_purge_old_news(state);
    crate::parity::record_tick(state);
}
