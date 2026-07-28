use crate::GameState;
use crate::vehicle::VehicleKind;

pub(super) fn tick_aircraft_phases(state: &mut GameState) {
    use crate::aircraft_movement::{AircraftPhaseEvent, tick_aircraft_phase};
    use crate::sim_events::SimEvent;

    let mut brake_checks = Vec::new();
    for i in 0..state.vehicles.len() {
        let prev_pos = state.vehicles[i].airport_pos;
        let prev_fta = state.vehicles[i].airport_fta_active;
        let ev = tick_aircraft_phase(&mut state.vehicles[i], &state.map, &mut state.stations);
        let id = state.vehicles[i].id;
        let at = state.vehicles[i].pos;
        let engine_id = state.vehicles[i]
            .engine_id
            .unwrap_or_else(|| crate::engine::default_engine_id(VehicleKind::Aircraft));
        match ev {
            AircraftPhaseEvent::Takeoff => {
                state
                    .runtime
                    .pending_sim_events
                    .push(SimEvent::AircraftTakeoff {
                        vehicle_id: id,
                        engine_id,
                        at,
                    });
            }
            AircraftPhaseEvent::Landing => {
                state
                    .runtime
                    .pending_sim_events
                    .push(SimEvent::AircraftLanding {
                        vehicle_id: id,
                        engine_id,
                        at,
                    });
            }
            AircraftPhaseEvent::None => {}
        }
        brake_checks.push((id, prev_pos, prev_fta));
    }
    for (id, prev_pos, prev_fta) in brake_checks {
        let _ = crate::aircraft_crash::maybe_crash_after_brake_tick(state, id, prev_pos, prev_fta);
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn move_vehicles(state: &mut GameState) {
    let tick = state.tick.get();
    let vehicle_count = state.vehicles.len();
    let pf = state.pathfinding;
    for i in 0..vehicle_count {
        state.vehicles[i].sim_tick = tick;
        // Vagones: no se mueven solos; se sincronizan tras la cabeza.
        if state.vehicles[i].is_wagon_unit() {
            continue;
        }
        // Espera ~37 ticks + reserva/PBS de boca (`CheckTrainStayInDepot`).
        if state.vehicles[i].kind == VehicleKind::Train
            && crate::depot_leave::tick_train_stay_in_depot(
                &mut state.map,
                &mut state.vehicles,
                i,
                pf,
            )
        {
            continue;
        }
        // Activación escalonada de vagones aún en Track::Depot.
        if state.vehicles[i].kind == VehicleKind::Train
            && state.vehicles[i].is_consist_head()
            && state.vehicles[i].depot_leave_cleared
        {
            let head_id = state.vehicles[i].id;
            crate::depot_leave::activate_depot_leave_units(
                &state.map,
                &mut state.vehicles,
                head_id,
            );
        }
        if tick_road_depot_movement(state, i) {
            continue;
        }
        if state.vehicles[i].crashed {
            continue;
        }
        if matches!(
            state.vehicles[i].kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
        ) {
            let map = Some(&state.map);
            // Split borrow: tick road con flota completa para FindCloseTo.
            let mut vehicles = std::mem::take(&mut state.vehicles);
            crate::road_movement::road_vehicle_tick(&mut vehicles, i, map);
            state.vehicles = vehicles;
            let _ = crate::ground_crash::maybe_road_train_crash(state, i);
            continue;
        }
        if state.vehicles[i].kind == VehicleKind::Train && state.vehicles[i].is_consist_head() {
            // Cerrar primero la parada: la nueva orden puede exigir salir en el
            // sentido opuesto. La inversión del consist ocurre en un tick
            // detenido y antes de evaluar tráfico o mover un solo píxel.
            let was_at_station = state.vehicles[i].awaiting_load_window;
            state.vehicles[i].complete_station_load_window();
            if was_at_station && !state.vehicles[i].awaiting_load_window {
                state.vehicles[i].sync_order_destination(&state.map);
            }
            let head_id = state.vehicles[i].id;
            if crate::train_consist::reverse_consist_at_stop(
                &mut state.vehicles,
                head_id,
                &state.map,
            ) {
                continue;
            }
        }
        let blocked = {
            let vehicles = &state.vehicles;
            let vehicle = &vehicles[i];
            if vehicle.kind == VehicleKind::Train
                && vehicle.running
                && vehicle.movement_target().is_some()
            {
                if vehicle.force_proceed {
                    // Ignorar señales/PBS/tráfico: puede provocar choque (OpenTTD).
                    false
                } else {
                    // PBS fase 2: reserva por pista; bloqueo solo si el paso no está reservado.
                    crate::rail_pbs::train_blocked_by_reservation(&state.map, vehicle)
                        || crate::rail_signals::train_blocked_by_signal(
                            &state.map, vehicles, vehicle,
                        )
                        || crate::rail_signals::train_blocked_by_traffic(
                            &state.map, vehicles, vehicle,
                        )
                }
            } else {
                false
            }
        };
        if blocked {
            let head_on = crate::rail_signals::train_facing_head_on_traffic(
                &state.map,
                &state.vehicles,
                &state.vehicles[i],
            );
            let waiting_pbs =
                crate::rail_pbs::train_waiting_for_pbs_path(&state.map, &state.vehicles[i]);
            let waiting_signal = !waiting_pbs
                && crate::rail_signals::train_blocked_by_signal(
                    &state.map,
                    &state.vehicles,
                    &state.vehicles[i],
                );
            state.vehicles[i].cur_speed = 0;
            // PBS / head-on: timeout `wait_for_pbs_path`. Señal de bloque: oneway/twoway.
            let steps_before = state.vehicles[i].reserved_steps.clone();
            let reversed = if waiting_pbs || head_on {
                crate::rail_pbs::tick_pbs_wait_and_maybe_reverse(
                    &state.map,
                    &mut state.vehicles[i],
                    pf,
                    head_on,
                )
            } else if waiting_signal {
                crate::rail_pbs::tick_signal_wait_and_maybe_reverse(
                    &state.map,
                    &mut state.vehicles[i],
                    pf,
                )
            } else {
                false
            };
            if reversed {
                let vehicle_id = state.vehicles[i].id;
                let order = state.vehicles[i].current_order;
                let pos = state.vehicles[i].pos;
                // Liberar reserva al girar (FreeTrainTrackReservation walk + PBS rojo).
                state.vehicles[i].reserved_steps = steps_before;
                crate::rail_pbs::free_train_track_reservation(
                    &mut state.map,
                    &mut state.vehicles[i],
                    &mut state.runtime.reservation_tile_dirty,
                );
                state.vehicles[i].sync_order_destination(&state.map);
                if head_on {
                    reroute_head_on_to_alt_platform(state, i);
                }
                crate::news::push_vehicle_advice_news(
                    state,
                    vehicle_id,
                    order,
                    pos,
                    crate::news::VehicleAdviceKind::PbsStuck,
                );
            }
            continue;
        }
        // Liberó el path PBS / señal: limpiar stuck (no tocar wait_counter de esclusas).
        if state.vehicles[i].kind == VehicleKind::Train
            && (state.vehicles[i].pbs_stuck || state.vehicles[i].wait_counter > 0)
        {
            state.vehicles[i].pbs_stuck = false;
            state.vehicles[i].wait_counter = 0;
        }
        if crate::ship_movement::tick_ship_lock_wait(&mut state.vehicles[i]) {
            continue;
        }
        let had_force = state.vehicles[i].force_proceed;
        let just_broke = state.vehicles[i].handle_breakdown(tick);
        if just_broke {
            state
                .runtime
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Breakdown {
                    vehicle_id: state.vehicles[i].id,
                    at: state.vehicles[i].pos,
                    kind: state.vehicles[i].kind,
                });
        }
        if state.vehicles[i].is_broken_down() {
            continue;
        }
        let prev_speed = state.vehicles[i].cur_speed;
        let prev_pos = state.vehicles[i].pos;
        let vehicle_id = state.vehicles[i].id;
        let vehicle_kind = state.vehicles[i].kind;
        let vehicle_running = state.vehicles[i].running;
        let train_accel = state.train_acceleration_model;
        refresh_vehicle_track_speed_cap(state, i, vehicle_kind);
        state.vehicles[i].step_with_map_and_accel(Some(&state.map), train_accel);
        refresh_vehicle_track_speed_cap(state, i, vehicle_kind);
        if matches!(
            vehicle_kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
        ) && state.vehicles[i].pos != prev_pos
            && state.map.get_kind(state.vehicles[i].pos) == Some(crate::TileKind::RoadDepot)
            && matches!(
                state.vehicles[i].road_depot_phase,
                crate::vehicle::RoadDepotPhase::None
            )
        {
            let mouth =
                crate::depot::road_depot_mouth_dir(&state.map, state.vehicles[i].pos).unwrap_or(0);
            let direction = crate::road_movement::road_depot_entry_direction(mouth);
            state.vehicles[i].road_depot_phase = crate::vehicle::RoadDepotPhase::Entering {
                direction,
                progress: 0,
            };
            state.vehicles[i].cur_speed = 0;
        }
        if vehicle_kind == VehicleKind::Train {
            crate::train_consist::consist_changed_with_map(
                &mut state.vehicles,
                vehicle_id,
                Some(&state.map),
            );
        }
        if state.vehicles[i].pos != prev_pos {
            crate::ship_movement::maybe_start_lock_transit(&mut state.vehicles[i], &state.map);
            if vehicle_kind == VehicleKind::Train {
                super::routing::enqueue_signal_glob_flush(state, prev_pos);
                let pos = state.vehicles[i].pos;
                super::routing::enqueue_signal_glob_flush(state, pos);
            }
        }
        if vehicle_running {
            if prev_speed == 0 && state.vehicles[i].cur_speed > 0 {
                state
                    .runtime
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::VehicleDepart {
                        vehicle_id,
                        at: state.vehicles[i].pos,
                        kind: vehicle_kind,
                    });
            }
            if vehicle_kind == VehicleKind::Train
                && state.vehicles[i].pos != prev_pos
                && let Some(tile) = state.map.get(state.vehicles[i].pos)
                && crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind)
            {
                state
                    .runtime
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::LevelCrossing {
                        at: state.vehicles[i].pos,
                    });
            }
        }
        update_vehicle_running_sounds(state, i, tick);
        if had_force && vehicle_kind == VehicleKind::Train {
            state.vehicles[i].force_proceed = false;
        }
    }
}

/// Cruce de la boca de un depósito road. El vehículo queda oculto mientras
/// está dentro y aparece recién al iniciar el frame de salida.
fn tick_road_depot_movement(state: &mut GameState, i: usize) -> bool {
    use crate::VehicleKind;
    use crate::road_movement::{
        ROAD_DEPOT_EXIT_START, ROAD_DEPOT_PROGRESS_STEP, road_depot_exit_direction,
    };
    use crate::vehicle::RoadDepotPhase;

    if !matches!(
        state.vehicles[i].kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        return false;
    }
    let phase = state.vehicles[i].road_depot_phase;
    let depot = state.vehicles[i].pos;
    match phase {
        RoadDepotPhase::None => false,
        RoadDepotPhase::Entering {
            direction,
            progress,
        } => {
            let next = progress.saturating_add(ROAD_DEPOT_PROGRESS_STEP);
            if next >= crate::road_movement::ROAD_DEPOT_ENTRY_STOP {
                state.vehicles[i].road_depot_phase = RoadDepotPhase::InDepot;
                state.vehicles[i].progress = 0;
                state.vehicles[i].cur_speed = 0;
                state.vehicles[i].running = false;
            } else {
                state.vehicles[i].road_depot_phase = RoadDepotPhase::Entering {
                    direction,
                    progress: next,
                };
                state.vehicles[i].progress = next;
            }
            true
        }
        RoadDepotPhase::InDepot => {
            if !state.vehicles[i].running {
                return true;
            }
            let Some(exit) = crate::depot::road_depot_entrance_tile(&state.map, depot) else {
                return true;
            };
            let blocked = state.vehicles.iter().enumerate().any(|(other_i, other)| {
                other_i != i
                    && matches!(
                        other.kind,
                        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
                    )
                    && other.pos == exit
                    && !matches!(other.road_depot_phase, RoadDepotPhase::InDepot)
            });
            if blocked {
                state.vehicles[i].cur_speed = 0;
                return true;
            }
            let Some(mouth) = crate::depot::road_depot_mouth_dir(&state.map, depot) else {
                return true;
            };
            let direction = road_depot_exit_direction(mouth);
            state.vehicles[i].direction = direction;
            state.vehicles[i].progress = ROAD_DEPOT_EXIT_START;
            state.vehicles[i].road_depot_phase = RoadDepotPhase::Exiting {
                direction,
                progress: ROAD_DEPOT_EXIT_START,
            };
            true
        }
        RoadDepotPhase::Exiting {
            direction,
            progress,
        } => {
            let next = progress.saturating_add(ROAD_DEPOT_PROGRESS_STEP);
            state.vehicles[i].progress = next;
            if next == u8::MAX {
                if let Some(exit) = crate::depot::road_depot_entrance_tile(&state.map, depot) {
                    state.vehicles[i].pos = exit;
                    state.vehicles[i].origin = exit;
                }
                state.vehicles[i].road_depot_phase = RoadDepotPhase::None;
                state.vehicles[i].progress = 0;
            } else {
                state.vehicles[i].road_depot_phase = RoadDepotPhase::Exiting {
                    direction,
                    progress: next,
                };
            }
            true
        }
    }
}

/// SFX de motor en marcha (`vehicle.cpp` `motion_counter` / `VSE_RUNNING*`).
fn update_vehicle_running_sounds(state: &mut GameState, i: usize, tick: u64) {
    use crate::map::TileKind;
    use crate::sim_events::VehicleRunningPhase;
    use crate::vehicle::AircraftPhase;

    let vehicle = &state.vehicles[i];
    if vehicle.is_wagon_unit() {
        return;
    }
    if !vehicle.running && vehicle.cur_speed == 0 {
        return;
    }
    let in_depot = match state.map.get(vehicle.pos).map(|t| t.kind) {
        Some(TileKind::RailDepot | TileKind::RoadDepot | TileKind::ShipDepot) => true,
        _ => {
            vehicle.kind == VehicleKind::Aircraft
                && matches!(vehicle.aircraft_phase, AircraftPhase::InHangar)
        }
    };
    if in_depot {
        return;
    }

    let speed = vehicle.cur_speed;
    let kind = vehicle.kind;
    let vehicle_id = vehicle.id;
    let at = vehicle.pos;
    let running_flag = vehicle.running;

    if speed > 0 {
        let mc = state.vehicles[i].motion_counter.wrapping_add(speed);
        state.vehicles[i].motion_counter = mc;
        if (mc & 0xFF) < speed {
            state
                .runtime
                .pending_sim_events
                .push(crate::sim_events::SimEvent::VehicleRunning {
                    vehicle_id,
                    at,
                    kind,
                    phase: VehicleRunningPhase::Running,
                });
        }
    }

    if tick.is_multiple_of(16) {
        let moving = speed > 0 && running_flag;
        state
            .runtime
            .pending_sim_events
            .push(crate::sim_events::SimEvent::VehicleRunning {
                vehicle_id,
                at,
                kind,
                phase: if moving {
                    VehicleRunningPhase::Running16
                } else {
                    VehicleRunningPhase::Stopped16
                },
            });
    }
}

/// Techos Action0 `0x14` (railtypes / roadtypes) → `cached_max_track_speed`.
fn refresh_vehicle_track_speed_cap(state: &mut GameState, vehicle_idx: usize, kind: VehicleKind) {
    if kind == VehicleKind::Train {
        let caps = state.runtime.rail_type_max_speed;
        state.vehicles[vehicle_idx].refresh_cached_max_track_speed(&state.map, caps);
        return;
    }
    if matches!(
        kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        let pos = state.vehicles[vehicle_idx].pos;
        let cap = state
            .map
            .get(pos)
            .map(|tile| crate::road_type::road_type_from_tile(&tile))
            .and_then(|id| {
                state
                    .road_type_catalog
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.max_speed)
            })
            .unwrap_or(0);
        state.vehicles[vehicle_idx].cached_max_track_speed = cap;
    }
}

/// Tras un reverse por head-on, cambia el destino a otro andén de la misma estación
/// si hay ruta, para no volver a encararse en la misma vía.
fn reroute_head_on_to_alt_platform(state: &mut GameState, vehicle_idx: usize) {
    let Some(crate::vehicle::VehicleOrder::Station { station, .. }) =
        state.vehicles[vehicle_idx].current_order_ref().copied()
    else {
        return;
    };
    let from = state.vehicles[vehicle_idx].pos;
    let current_dest = state.vehicles[vehicle_idx].dest;
    let engine_id = state.vehicles[vehicle_idx].engine_id;
    let wormholes = crate::pathfinder::TunnelWormholes::from_jgr_records(
        &state.map,
        &state.jgr_tunnels_from_footer,
    );
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    let candidates = crate::station::rail_station_stop_candidates(&state.map, station, from);
    for alt in candidates {
        if alt == current_dest {
            continue;
        }
        if let Some(path) =
            crate::pathfinder::find_rail_path_for_engine(&state.map, from, alt, wh, engine_id)
        {
            state.vehicles[vehicle_idx].dest = alt;
            state.vehicles[vehicle_idx].path = path.into_iter().collect();
            state.vehicles[vehicle_idx].no_network_route_to_order = false;
            return;
        }
    }
}
