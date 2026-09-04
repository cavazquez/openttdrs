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

use std::collections::HashSet;
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
    // El peso de la carga se conoce después de `LoadUnloadStation`; actualizar
    // ahora evita reconstruir la topología antes de cargar y deja la física
    // del mismo tick con el `cached_weight_t` correcto.
    cargo_transfer::refresh_runtime_vehicle_capacities(state);
    cargo_transfer::trigger_pending_industry_deliveries(state);
    // Ops que pueden cambiar destino van tras la carga (p. ej. wander orderless).
    let _ = phase_vehicle_ops_pre_move(state);
    trigger_pending_train_station_departures(state);
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
    cargo_transfer::refresh_runtime_vehicle_capacities(state);
    timings.cargo_load_ns = nanos(cargo_phase);
    cargo_transfer::trigger_pending_industry_deliveries(state);
    timings.cargo_transfer_ns = nanos(p0);

    let p0 = Instant::now();
    let vehicle_ops = Instant::now();
    let vehicle_ops_timings = phase_vehicle_ops_pre_move(state);
    timings.vehicle_ops_only_ns = nanos(vehicle_ops);
    timings.vehicle_ops_autoreplace_ns = vehicle_ops_timings.autoreplace_ns;
    trigger_pending_train_station_departures(state);
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
#[allow(clippy::too_many_lines)]
fn phase_tile_animation(state: &mut GameState, t: u64) {
    let visits = std::mem::take(&mut state.runtime.tile_loop_visited);
    let bubble_spawns = crate::map::bubble_generator_spawns_from_visits(&visits);
    let tile_loop_animation_coords: Vec<_> = visits
        .iter()
        .filter(|(_, tile)| tile.kind == crate::TileKind::Industry)
        .map(|(coord, _)| *coord)
        .collect();
    let _lift_dirty = crate::map::step_house_lifts(
        &mut state.map,
        t,
        &visits,
        &mut state.random,
        &mut state.runtime.active_house_lifts,
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
    let construction_stage_before: Vec<_> = animation_coords
        .iter()
        .filter_map(|&coord| {
            state
                .map
                .get(coord)
                .map(|tile| (coord, crate::map::industry_construction_stage(tile.m1)))
        })
        .collect();
    state.runtime.industry_tile_dirty =
        crate::map::step_industry_tiles_with_seed_and_catalog_and_world(
            &mut state.map,
            t,
            &visits,
            state.world_seed,
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
        );
    let construction_stage_changed: Vec<_> = construction_stage_before
        .into_iter()
        .filter_map(|(coord, before)| {
            state
                .map
                .get(coord)
                .filter(|tile| crate::map::industry_construction_stage(tile.m1) != before)
                .map(|_| coord)
        })
        .collect();
    state.runtime.industry_tile_dirty.extend(
        crate::map::trigger_newgrf_industry_animation_with_world(
            &mut state.map,
            t,
            &construction_stage_changed,
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
            state.world_seed,
            &mut state.newgrf_animated_industry_tiles,
            crate::map::IndustryAnimationTrigger::ConstructionStageChanged,
        ),
    );
    state.runtime.industry_tile_dirty.extend(
        crate::map::trigger_newgrf_industry_animation_with_world(
            &mut state.map,
            t,
            &tile_loop_animation_coords,
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
            state.world_seed,
            &mut state.newgrf_animated_industry_tiles,
            crate::map::IndustryAnimationTrigger::TileLoop,
        ),
    );
    state.runtime.industry_tile_dirty.extend(
        crate::map::advance_newgrf_industry_animation_frames_with_world(
            &mut state.map,
            t,
            &animation_coords,
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
            state.world_seed,
            &mut state.newgrf_animated_industry_tiles,
        ),
    );
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
    let newgrf_airport_dirty = crate::map::step_newgrf_airport_tiles_with_towns(
        &mut state.map,
        t,
        &mut state.stations,
        &state.towns,
        state.climate,
        &state.airport_tile_spec_catalog,
        &mut state.newgrf_animated_airport_tiles,
        &state.newgrf_stack,
        &visits,
    );
    state
        .runtime
        .industry_tile_dirty
        .extend(newgrf_airport_dirty);
    let road_stop_dirty = crate::map::step_newgrf_road_stop_tiles_with_world(
        &state.map,
        t,
        &mut state.stations,
        &state.road_stop_spec_catalog,
        &visits,
        Some(crate::RoadStopCallbackWorld {
            map: &state.map,
            road_stop_catalog: &state.road_stop_spec_catalog,
            cargo_spec_catalog: &state.cargo_spec_catalog,
            towns: &state.towns,
            companies: &state.companies,
            industries: &state.industries,
            road_type_catalog: &state.road_type_catalog,
            climate: state.climate,
        }),
    );
    state.runtime.industry_tile_dirty.extend(road_stop_dirty);
    let station_dirty = crate::map::step_newgrf_station_tiles_with_world(
        &mut state.map,
        t,
        &mut state.stations,
        &state.companies,
        &state.industries,
        state.climate,
        &state.station_spec_catalog,
        &mut state.newgrf_animated_station_tiles,
        &visits,
    );
    state.runtime.industry_tile_dirty.extend(station_dirty);
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
    let station_reservations_before = station_reservation_tiles(state);
    let dirty_before = state.runtime.reservation_tile_dirty.len();
    crate::rail_pbs::update_train_reservations_incremental_with_wormholes(
        &state.map,
        &mut state.vehicles,
        state.pathfinding,
        wh_pbs,
    );
    trigger_station_path_reservation_animations(state, &station_reservations_before);
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

/// Teselas rail de estación actualmente reservadas por una cabeza de tren.
///
/// `HasStationReservation` de `OpenTTD` es un bit por tesela, no por track;
/// guardar sólo la coordenada preserva que CB140 se ejecute una vez cuando la
/// estación pasa de no reservada a reservada.
fn station_reservation_tiles(state: &GameState) -> HashSet<crate::TileCoord> {
    state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.kind == crate::VehicleKind::Train && vehicle.is_consist_head())
        .flat_map(|vehicle| vehicle.reserved_steps.iter().map(|step| step.tile))
        .filter(|&tile| state.map.get_kind(tile) == Some(crate::TileKind::Station))
        .collect()
}

/// Emite CB140 `PathReservation` al reservar por primera vez una tesela rail
/// de estación. El trigger usa `TA_PLATFORM`, igual que `pbs.cpp`.
fn trigger_station_path_reservation_animations(
    state: &mut GameState,
    station_reservations_before: &HashSet<crate::TileCoord>,
) {
    let mut emitted = HashSet::new();
    let mut newly_reserved_tiles = Vec::new();
    for vehicle in state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.kind == crate::VehicleKind::Train && vehicle.is_consist_head())
    {
        for step in &vehicle.reserved_steps {
            let tile = step.tile;
            if state.map.get_kind(tile) != Some(crate::TileKind::Station)
                || station_reservations_before.contains(&tile)
                || !emitted.insert(tile)
            {
                continue;
            }
            newly_reserved_tiles.push(tile);
        }
    }

    for trigger_tile in newly_reserved_tiles {
        trigger_station_platform_animation(
            state,
            trigger_tile,
            crate::StationAnimationTrigger::PathReservation,
        );
    }
}

/// Ejecuta un CB140 ferroviario con área `TA_PLATFORM` en la plataforma que
/// contiene la tesela del tren. Es la semántica compartida por llegada, salida
/// y reserva PBS de `station_cmd.cpp` / `pbs.cpp`.
pub(super) fn trigger_station_platform_animation(
    state: &mut GameState,
    trigger_tile: crate::TileCoord,
    trigger: crate::StationAnimationTrigger,
) {
    let Some(station_anchor) =
        crate::station::station_at_tile(&state.map, &state.stations, trigger_tile)
            .map(|station| station.pos)
    else {
        return;
    };
    let dirty = crate::map::trigger_newgrf_station_animation_for_platform_with_world(
        &mut state.map,
        state.tick.get(),
        &mut state.stations,
        &state.companies,
        &state.industries,
        state.climate,
        &state.station_spec_catalog,
        &mut state.newgrf_animated_station_tiles,
        station_anchor,
        trigger_tile,
        trigger,
    );
    state.runtime.industry_tile_dirty.extend(dirty);
}

/// Ejecuta los triggers `AirportTile` que cubren una estación.
///
/// `TriggerAirportAnimation` de `OpenTTD` recibe el cargo traducido al espacio
/// local del GRF en los bits altos de `var 18`. Los catálogos antiguos no
/// conservan una CTT por tesela, por lo que usamos la traducción moderna
/// global (`bitnum`) como fallback; cuando el catálogo aporte tablas propias
/// esta función seguirá siendo el único punto de integración de los eventos.
pub(super) fn trigger_airport_animation_at(
    state: &mut GameState,
    trigger_tile: crate::TileCoord,
    trigger: crate::AirportAnimationTrigger,
    cargo: Option<crate::CargoType>,
) {
    let Some(station_anchor) =
        crate::station::station_at_tile(&state.map, &state.stations, trigger_tile)
            .filter(|station| station.stop_kind == crate::station::StopKind::Airport)
            .map(|station| station.pos)
    else {
        return;
    };
    let dirty = crate::map::trigger_newgrf_airport_animation_for_station_with_towns(
        &mut state.map,
        state.tick.get(),
        &mut state.stations,
        &state.towns,
        state.climate,
        &state.airport_tile_spec_catalog,
        &mut state.newgrf_animated_airport_tiles,
        &state.newgrf_stack,
        station_anchor,
        trigger,
        cargo,
    );
    state.runtime.industry_tile_dirty.extend(dirty);
}

/// Ejecuta CB140 de un `RoadStop` `NewGRF` en su tesela concreta.
///
/// Los triggers viales de vehículo se resuelven sobre su tesela exacta; los de
/// carga y aceptación recorren todas las teselas custom de la estación, como
/// `TriggerRoadStopAnimation` de `OpenTTD`.
pub(super) fn trigger_road_stop_animation_at(
    state: &mut GameState,
    trigger_tile: crate::TileCoord,
    trigger: crate::StationAnimationTrigger,
    cargo: Option<crate::CargoType>,
) {
    let Some(station_index) = state
        .stations
        .iter()
        .position(|station| station.covers_tile(trigger_tile))
    else {
        return;
    };
    let tick = state.tick.get();
    let climate = state.climate;
    let whole_station = matches!(
        trigger,
        crate::StationAnimationTrigger::NewCargo
            | crate::StationAnimationTrigger::CargoTaken
            | crate::StationAnimationTrigger::AcceptanceTick
    );
    let target_tiles = if whole_station {
        state.stations[station_index].road_stop_custom_tiles()
    } else {
        vec![trigger_tile]
    };

    for tile_pos in target_tiles {
        let Some(spec_id) = state.stations[station_index].road_stop_spec_at(tile_pos) else {
            continue;
        };
        let Some(tile) = state.map.get(tile_pos) else {
            continue;
        };
        let Some(def) =
            crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, spec_id)
        else {
            continue;
        };
        let cargo_local_id = cargo.map(|cargo| {
            def.newgrf_cargo_local_id_with_catalog(cargo, climate, &state.cargo_spec_catalog)
        });
        let randomisation_changed = crate::StationRandomTrigger::from_animation_trigger(trigger)
            .is_some_and(|random_trigger| {
                crate::newgrf_callback::trigger_road_stop_randomisation_at_with_world(
                    def,
                    &mut state.stations[station_index],
                    tile_pos,
                    random_trigger,
                    cargo,
                    crate::newgrf_callback::RoadStopRandomisationContext {
                        climate,
                        world_seed: state.world_seed,
                        tick,
                    },
                    Some(crate::RoadStopCallbackWorld {
                        map: &state.map,
                        road_stop_catalog: &state.road_stop_spec_catalog,
                        cargo_spec_catalog: &state.cargo_spec_catalog,
                        towns: &state.towns,
                        companies: &state.companies,
                        industries: &state.industries,
                        road_type_catalog: &state.road_type_catalog,
                        climate,
                    }),
                )
            });
        let animation_changed = crate::newgrf_callback::trigger_road_stop_animation_at_with_world(
            def,
            &mut state.stations[station_index],
            tile_pos,
            tile.m5,
            trigger,
            cargo_local_id,
            tick,
            Some(crate::RoadStopCallbackWorld {
                map: &state.map,
                road_stop_catalog: &state.road_stop_spec_catalog,
                cargo_spec_catalog: &state.cargo_spec_catalog,
                towns: &state.towns,
                companies: &state.companies,
                industries: &state.industries,
                road_type_catalog: &state.road_type_catalog,
                climate,
            }),
        );
        if randomisation_changed || animation_changed {
            state.runtime.industry_tile_dirty.push(tile_pos);
        }
    }
}

/// Consume las salidas de estación que terminaron antes del movimiento
/// (descarga/carga o espera de horario). Las salidas decididas dentro de
/// `movement` usan la variante por índice inmediatamente antes de avanzar.
fn trigger_pending_train_station_departures(state: &mut GameState) {
    for vehicle_idx in 0..state.vehicles.len() {
        trigger_pending_train_station_departure(state, vehicle_idx);
    }
}

/// Ejecuta una salida ferroviaria pendiente conservando la tesela de la
/// plataforma, antes de que el tren pueda mover un píxel. Equivale al bloque
/// `Vehicle::LeaveStation` de `OpenTTD`.
pub(super) fn trigger_pending_train_station_departure(state: &mut GameState, vehicle_idx: usize) {
    let Some(vehicle) = state.vehicles.get_mut(vehicle_idx) else {
        return;
    };
    if vehicle.kind != crate::VehicleKind::Train || !vehicle.is_consist_head() || vehicle.crashed {
        return;
    }
    let pending = vehicle.take_station_departure();
    let Some(trigger_tile) = pending.then_some(vehicle.pos) else {
        return;
    };
    trigger_station_platform_animation(
        state,
        trigger_tile,
        crate::StationAnimationTrigger::VehicleDeparts,
    );
}

/// Emite la salida de un bus/camión/tranvía antes de que el movimiento deje
/// atrás su `RoadStop`. `road_vehicle_tick` puede cerrar la carga y avanzar en
/// el mismo tick, por lo que recibe la posición tomada antes de conducir.
pub(super) fn trigger_pending_road_stop_departure(
    state: &mut GameState,
    vehicle_idx: usize,
    trigger_tile: crate::TileCoord,
) {
    let Some(vehicle) = state.vehicles.get_mut(vehicle_idx) else {
        return;
    };
    if !matches!(
        vehicle.kind,
        crate::VehicleKind::Bus | crate::VehicleKind::Truck | crate::VehicleKind::Tram
    ) || vehicle.crashed
    {
        return;
    }
    if !vehicle.take_station_departure() {
        return;
    }
    trigger_road_stop_animation_at(
        state,
        trigger_tile,
        crate::StationAnimationTrigger::VehicleDeparts,
        None,
    );
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::command::{Command, apply_command};
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::{
        GameState, PathNetwork, STATION_ANIMATION_TRIGGER_PATH_RESERVATION,
        STATION_ANIMATION_TRIGGER_VEHICLE_ARRIVES, STATION_ANIMATION_TRIGGER_VEHICLE_DEPARTS,
        TileCoord, Vehicle, VehicleKind, VehicleOrder, find_path,
    };

    /// CB140 sintético: conserva en MAP7 el ordinal de `var 18`.
    fn path_reservation_callbacks() -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x18,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn state_with_newgrf_road_stop(trigger_mask: u16) -> (GameState, TileCoord) {
        let pos = TileCoord::new(4, 3);
        let mut state = GameState::new(12, 8);
        let mut tile = state.map.get(pos).unwrap();
        tile.kind = crate::TileKind::Station;
        tile.mapt = 0x50;
        tile.m5 = crate::RSV_DRIVE_THROUGH_X;
        tile.m6 = 2;
        state.map.set_tile(pos, tile).unwrap();
        let mut station = crate::Station::new_with_kind(pos, crate::StopKind::BusStop);
        station.road_stop_spec = Some(7);
        state.stations.push(station);
        state.road_stop_spec_catalog.push(crate::RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "RoadStop animado".into(),
            short_label: "RSAN".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 0x5253_414E,
            newgrf_local_id: 0,
            newgrf_grf_version: 0,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            callback_mask: 0,
            animation_status: 1,
            animation_frames: u8::MAX,
            animation_speed: 0,
            animation_triggers: trigger_mask,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(path_reservation_callbacks())),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        });
        (state, pos)
    }

    #[test]
    fn path_reservation_triggers_station_cb140_once_per_new_station_tile() {
        let station_first = TileCoord::new(4, 3);
        let station_second = TileCoord::new(5, 3);
        let start = TileCoord::new(3, 3);
        let exit = TileCoord::new(6, 3);
        let mut state = GameState::new(12, 8);
        apply_command(
            &mut state,
            &Command::PlaceRailStationArea {
                origin: station_first,
                axis_y: false,
                platforms: 1,
                length: 2,
            },
        )
        .unwrap();
        for tile in [start, exit] {
            apply_command(&mut state, &Command::PlaceRail(tile)).unwrap();
        }
        let station_anchor = state.stations[0].pos;
        let path = find_path(&state.map, start, exit, PathNetwork::Rail).unwrap();
        assert_eq!(path, vec![station_first, station_second, exit]);

        let mut train = Vehicle::new(1, VehicleKind::Train, start, exit);
        train.running = true;
        train.cur_speed = 1;
        train.path = VecDeque::from(path);
        state.vehicles = vec![train];
        state.pathfinding.reserve_paths = true;
        let spec = &mut state.station_spec_catalog[0];
        spec.from_newgrf = true;
        spec.animation_triggers = STATION_ANIMATION_TRIGGER_PATH_RESERVATION;
        spec.newgrf_runtime = Some(Box::new(path_reservation_callbacks()));

        phase_pbs_reservations(&mut state);
        assert!(
            state.vehicles[0]
                .reserved_steps
                .iter()
                .any(|step| step.tile == station_first)
        );
        assert!(
            state.vehicles[0]
                .reserved_steps
                .iter()
                .any(|step| step.tile == station_second)
        );
        assert_eq!(state.map.get(station_first).unwrap().m7, 8);
        assert_eq!(state.map.get(station_second).unwrap().m7, 8);

        // Una reserva ya existente no vuelve a emitir CB140 durante el
        // recálculo incremental posterior.
        for tile in [station_first, station_second] {
            let mut map_tile = state.map.get(tile).unwrap();
            map_tile.m7 = 0;
            state.map.set_tile(tile, map_tile).unwrap();
            state.newgrf_animated_station_tiles.remove(&tile);
        }
        phase_pbs_reservations(&mut state);
        assert_eq!(state.map.get(station_first).unwrap().m7, 0);
        assert_eq!(state.map.get(station_second).unwrap().m7, 0);
        assert_eq!(state.stations[0].pos, station_anchor);
    }

    #[test]
    fn train_arrival_triggers_station_cb140_for_its_platform() {
        let station_tile = TileCoord::new(4, 3);
        let mut state = GameState::new(12, 8);
        apply_command(
            &mut state,
            &Command::PlaceRailStationArea {
                origin: station_tile,
                axis_y: false,
                platforms: 1,
                length: 1,
            },
        )
        .unwrap();

        let mut train = Vehicle::new(1, VehicleKind::Train, station_tile, station_tile);
        train.running = true;
        train.orders.push(VehicleOrder::station(station_tile));
        state.vehicles = vec![train];
        let spec = &mut state.station_spec_catalog[0];
        spec.from_newgrf = true;
        spec.animation_triggers = STATION_ANIMATION_TRIGGER_VEHICLE_ARRIVES;
        spec.newgrf_runtime = Some(Box::new(path_reservation_callbacks()));

        state.step();

        assert!(state.vehicles[0].awaiting_load_window);
        assert_eq!(
            state.map.get(station_tile).unwrap().m7,
            crate::StationAnimationTrigger::VehicleArrives as u8,
            "CB140 debe recibir VehicleArrives=3 al ejecutar BeginLoading"
        );
    }

    #[test]
    fn road_stop_vehicle_arrives_and_departs_trigger_cb140() {
        let (mut state, stop) = state_with_newgrf_road_stop(
            crate::ROADSTOP_ANIMATION_TRIGGER_VEHICLE_ARRIVES
                | crate::ROADSTOP_ANIMATION_TRIGGER_VEHICLE_DEPARTS,
        );

        // La llegada vial alcanza el mismo ordinal CB140 que OpenTTD.
        trigger_road_stop_animation_at(
            &mut state,
            stop,
            crate::StationAnimationTrigger::VehicleArrives,
            None,
        );
        assert_eq!(
            state.stations[0].road_stop_animation_frame,
            crate::StationAnimationTrigger::VehicleArrives as u8,
        );

        // La salida real se decide dentro del controller vial al cerrar
        // BeginLoading; debe consumir el evento antes de abandonar el stop.
        let mut bus = Vehicle::new(41, VehicleKind::Bus, stop, stop);
        bus.running = true;
        bus.progress = u8::MAX;
        bus.awaiting_load_window = true;
        bus.orders = vec![
            VehicleOrder::station(stop),
            VehicleOrder::Tile(TileCoord::new(7, 3)),
        ];
        state.vehicles.push(bus);

        state.step();

        assert_eq!(
            state.stations[0].road_stop_animation_frame,
            crate::StationAnimationTrigger::VehicleDeparts as u8,
            "CB140 debe salir antes de que el bus abandone el RoadStop",
        );
    }

    #[test]
    fn road_stop_compound_state_keeps_tile_events_separate_and_whole_events_broadcast() {
        let (mut state, first) = state_with_newgrf_road_stop(
            crate::ROADSTOP_ANIMATION_TRIGGER_VEHICLE_ARRIVES
                | crate::ROADSTOP_ANIMATION_TRIGGER_NEW_CARGO,
        );
        let second = TileCoord::new(first.x + 1, first.y);
        let mut tile = state.map.get(first).unwrap();
        tile.m5 = crate::RSV_DRIVE_THROUGH_X;
        state.map.set_tile(second, tile).unwrap();
        {
            let station = &mut state.stations[0];
            station.joined_tiles.push(second);
            let first_state = station.ensure_road_stop_tile_state(first);
            first_state.spec = Some(7);
            first_state.animation_frame = 1;
            let second_state = station.ensure_road_stop_tile_state(second);
            second_state.spec = Some(7);
            second_state.animation_frame = 99;
            second_state.random_bits = 0xA4;
            station.sync_legacy_road_stop_anchor();
        }

        // Llegada vial: sólo se anima la tesela que tocó el vehículo.
        trigger_road_stop_animation_at(
            &mut state,
            first,
            crate::StationAnimationTrigger::VehicleArrives,
            None,
        );
        assert_eq!(
            state.stations[0].road_stop_animation_frame_at(first),
            crate::StationAnimationTrigger::VehicleArrives as u8
        );
        assert_eq!(state.stations[0].road_stop_animation_frame_at(second), 99);

        // NewCargo usa área completa: cada RoadStopTileData custom recibe
        // CB140 aunque la carga haya llegado por el ancla de la estación.
        trigger_road_stop_animation_at(
            &mut state,
            first,
            crate::StationAnimationTrigger::NewCargo,
            Some(crate::CargoType::Mail),
        );
        let expected = crate::StationAnimationTrigger::NewCargo as u8;
        assert_eq!(
            state.stations[0].road_stop_animation_frame_at(first),
            expected
        );
        assert_eq!(
            state.stations[0].road_stop_animation_frame_at(second),
            expected
        );
    }

    #[test]
    fn train_departure_after_load_window_triggers_station_cb140() {
        let station_tile = TileCoord::new(4, 3);
        let mut state = GameState::new(12, 8);
        apply_command(
            &mut state,
            &Command::PlaceRailStationArea {
                origin: station_tile,
                axis_y: false,
                platforms: 1,
                length: 1,
            },
        )
        .unwrap();

        let mut train = Vehicle::new(1, VehicleKind::Train, station_tile, station_tile);
        train.running = true;
        train.progress = u8::MAX;
        train.awaiting_load_window = true;
        train.orders.push(VehicleOrder::station(station_tile));
        state.vehicles = vec![train];
        let spec = &mut state.station_spec_catalog[0];
        spec.from_newgrf = true;
        spec.animation_triggers = STATION_ANIMATION_TRIGGER_VEHICLE_DEPARTS;
        spec.newgrf_runtime = Some(Box::new(path_reservation_callbacks()));

        state.step();

        assert_eq!(
            state.map.get(station_tile).unwrap().m7,
            crate::StationAnimationTrigger::VehicleDeparts as u8,
            "CB140 debe recibir VehicleDeparts=4 antes de mover fuera de la plataforma"
        );
    }

    #[test]
    fn train_departure_after_cargo_unload_triggers_station_cb140() {
        let station_tile = TileCoord::new(4, 3);
        let source = TileCoord::new(2, 3);
        let mut state = GameState::new(12, 8);
        apply_command(
            &mut state,
            &Command::PlaceRailStationArea {
                origin: station_tile,
                axis_y: false,
                platforms: 1,
                length: 1,
            },
        )
        .unwrap();

        let mut train = Vehicle::new(1, VehicleKind::Train, station_tile, station_tile);
        train.running = true;
        train.orders.push(VehicleOrder::station(station_tile));
        train
            .cargo_packets
            .push(crate::CargoPacket::new(crate::CargoType::Coal, 1, source));
        train.sync_cargo_from_packets();
        train.last_pickup_station = Some(source);
        state.vehicles = vec![train];
        let spec = &mut state.station_spec_catalog[0];
        spec.from_newgrf = true;
        spec.animation_triggers = STATION_ANIMATION_TRIGGER_VEHICLE_DEPARTS;
        spec.newgrf_runtime = Some(Box::new(path_reservation_callbacks()));

        state.step();

        assert_eq!(
            state.map.get(station_tile).unwrap().m7,
            crate::StationAnimationTrigger::VehicleDeparts as u8,
            "CB140 debe salir también cuando LoadUnloadStation avanzó la orden"
        );
    }

    #[test]
    fn train_departure_waits_for_timetable_before_triggering_station_cb140() {
        let station_tile = TileCoord::new(4, 3);
        let mut state = GameState::new(12, 8);
        apply_command(
            &mut state,
            &Command::PlaceRailStationArea {
                origin: station_tile,
                axis_y: false,
                platforms: 1,
                length: 1,
            },
        )
        .unwrap();

        let mut train = Vehicle::new(1, VehicleKind::Train, station_tile, station_tile);
        train.running = true;
        train.progress = u8::MAX;
        train.awaiting_load_window = true;
        train.timetable_active = true;
        train.orders = vec![
            VehicleOrder::station(station_tile)
                .with_cycled_wait()
                .unwrap(),
            VehicleOrder::Tile(TileCoord::new(6, 3)),
        ];
        state.vehicles = vec![train];
        let spec = &mut state.station_spec_catalog[0];
        spec.from_newgrf = true;
        spec.animation_triggers = STATION_ANIMATION_TRIGGER_VEHICLE_DEPARTS;
        spec.newgrf_runtime = Some(Box::new(path_reservation_callbacks()));

        state.step();
        assert_eq!(state.vehicles[0].timetable_wait_remaining, 30);
        assert_eq!(
            state.map.get(station_tile).unwrap().m7,
            0,
            "CB140 no puede salir mientras el tren todavía espera en la estación"
        );

        state.vehicles[0].timetable_wait_remaining = 1;
        state.step();
        assert_eq!(
            state.map.get(station_tile).unwrap().m7,
            crate::StationAnimationTrigger::VehicleDeparts as u8,
            "CB140 sale al completar la espera y avanzar la orden"
        );
    }
}
