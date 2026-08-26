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
    let mut road_traffic = crate::road_movement::RoadTrafficIndex::default();
    road_traffic.rebuild(&state.vehicles);
    let mut train_crashes = crate::ground_crash::TrainCrashIndex::default();
    train_crashes.rebuild(&state.vehicles);
    for i in 0..vehicle_count {
        state.vehicles[i].sim_tick = tick;
        // Vagones y partes articuladas: no se mueven solos; se sincronizan
        // tras la cabeza para conservar una única cinemática por vehículo.
        if state.vehicles[i].is_wagon_unit() || state.vehicles[i].is_articulated_unit() {
            continue;
        }
        // Espera ~37 ticks + reserva/PBS de boca (`CheckTrainStayInDepot`).
        if state.vehicles[i].kind == VehicleKind::Train
            && crate::depot_leave::tick_train_stay_in_depot_indexed(
                &mut state.map,
                &mut state.vehicles,
                i,
                pf,
                &state.runtime.fleet_index,
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
            crate::depot_leave::activate_depot_leave_units_indexed(
                &state.map,
                &mut state.vehicles,
                head_id,
                &state.runtime.fleet_index,
            );
        }
        let previous_road_pos = state.vehicles[i].pos;
        if tick_road_depot_movement(state, i) {
            let articulated = sync_road_articulated_parts(state, i);
            road_traffic.update_vehicle(&state.vehicles, i, previous_road_pos);
            for (slot, previous) in articulated {
                road_traffic.update_vehicle(&state.vehicles, slot, previous);
            }
            continue;
        }
        if state.vehicles[i].crashed {
            continue;
        }
        if matches!(
            state.vehicles[i].kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
        ) {
            let was_waiting_for_station_load = state.vehicles[i].awaiting_load_window;
            let map = Some(&state.map);
            let drive_on_right = state.construction.road_drive_on_right();
            let road_acceleration_model = state.road_vehicle_acceleration_model;
            // Split borrow: tick road con flota completa para FindCloseTo.
            let mut vehicles = std::mem::take(&mut state.vehicles);
            crate::road_movement::road_vehicle_tick_side_indexed_with_acceleration(
                &mut vehicles,
                i,
                map,
                drive_on_right,
                road_acceleration_model,
                &mut road_traffic,
            );
            state.vehicles = vehicles;
            let articulated = sync_road_articulated_parts(state, i);
            // `RoadVehArrivesAt` abre `BeginLoading` dentro del controller.
            // Disparar tras recuperar el préstamo completo conserva el tile
            // exacto de llegada y habilita CB140 sin acoplar el runtime NewGRF
            // al controlador vial.
            super::trigger_pending_road_stop_departure(state, i, previous_road_pos);
            if !was_waiting_for_station_load
                && state.vehicles[i].awaiting_load_window
                && !state.vehicles[i].crashed
            {
                super::trigger_road_stop_animation_at(
                    state,
                    state.vehicles[i].pos,
                    crate::StationAnimationTrigger::VehicleArrives,
                    None,
                );
            }
            let _ = crate::ground_crash::maybe_road_train_crash_indexed(state, i, &train_crashes);
            for (slot, previous) in articulated {
                road_traffic.update_vehicle(&state.vehicles, slot, previous);
            }
            continue;
        }
        if state.vehicles[i].kind == VehicleKind::Train && state.vehicles[i].is_consist_head() {
            // Cerrar primero la parada: la nueva orden puede exigir salir en el
            // sentido opuesto. La inversión del consist ocurre en un tick
            // detenido y antes de evaluar tráfico o mover un solo píxel.
            let was_at_station = state.vehicles[i].awaiting_load_window;
            state.vehicles[i].complete_station_load_window();
            // El cierre sin transferencia decide la salida dentro de la fase
            // de movimiento; consumir el evento antes de re-rutear o avanzar.
            super::trigger_pending_train_station_departure(state, i);
            if was_at_station && !state.vehicles[i].awaiting_load_window {
                state.vehicles[i].sync_order_destination(&state.map);
            }
            let head_id = state.vehicles[i].id;
            if crate::train_consist::reverse_consist_at_stop_indexed(
                &mut state.vehicles,
                &state.runtime.fleet_index,
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
                        || crate::rail_signals::train_blocked_by_traffic_indexed(
                            &state.map,
                            vehicles,
                            vehicle,
                            &state.runtime.fleet_index,
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
        let was_waiting_for_station_load = state.vehicles[i].awaiting_load_window;
        let train_previous_positions: Vec<(usize, crate::TileCoord)> =
            if vehicle_kind == VehicleKind::Train {
                state
                    .runtime
                    .fleet_index
                    .consist(vehicle_id)
                    .iter()
                    .filter_map(|&unit_id| {
                        state
                            .runtime
                            .fleet_index
                            .slot(unit_id)
                            .map(|slot| (slot, state.vehicles[slot].pos))
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let train_accel = state.train_acceleration_model;
        refresh_vehicle_track_speed_cap(state, i, vehicle_kind);
        state.vehicles[i].step_with_map_and_accel(Some(&state.map), train_accel);
        refresh_vehicle_track_speed_cap(state, i, vehicle_kind);
        // `Vehicle::MoveTo` calls `PlayVehicleSound(VSE_TUNNEL)` only at the
        // entrance frame. Detect the same outside→inside edge here instead of
        // emitting once per interior tile (or once per consist wagon).
        if vehicle_entered_train_tunnel(state, i, prev_pos) {
            state
                .runtime
                .pending_sim_events
                .push(crate::sim_events::SimEvent::VehicleTunnel {
                    vehicle_id,
                    at: state.vehicles[i].pos,
                    kind: vehicle_kind,
                });
        }
        if vehicle_kind == VehicleKind::Train
            && state.vehicles[i].is_consist_head()
            && !was_waiting_for_station_load
            && state.vehicles[i].awaiting_load_window
        {
            // `Train::BeginLoading`: OpenTTD dispara CB140 justo después de
            // abrir la carga de la llegada, con alcance TA_PLATFORM.
            super::trigger_station_platform_animation(
                state,
                state.vehicles[i].pos,
                crate::StationAnimationTrigger::VehicleArrives,
            );
        }
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
            // Escenarios/tests que crean `Vehicle::new(Train)` directamente
            // todavía no tienen sus cachés de potencia/peso. Una vez armadas,
            // el hot path sólo propaga poses de los seguidores.
            if state.vehicles[i].cached_weight_t == 0 {
                crate::train_consist::consist_changed_with_map(
                    &mut state.vehicles,
                    vehicle_id,
                    Some(&state.map),
                );
            } else {
                crate::train_consist::propagate_consist_unit_poses_with_map_indexed(
                    &mut state.vehicles,
                    &state.runtime.fleet_index,
                    vehicle_id,
                    Some(&state.map),
                );
            }
            // La propagación puede desplazar vagones a otra tesela; la
            // siguiente comprobación vial debe ver su posición actual.
            for (slot, previous) in train_previous_positions {
                train_crashes.update_vehicle(&state.vehicles, slot, previous);
            }
        }
        if state.vehicles[i].pos != prev_pos && vehicle_kind == VehicleKind::Train {
            super::routing::enqueue_signal_glob_flush(state, prev_pos);
            let pos = state.vehicles[i].pos;
            super::routing::enqueue_signal_glob_flush(state, pos);
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

/// Detecta el borde exterior→interior de una boca de túnel para la cabeza de
/// un consist. Mantenerlo separado del hot path permite cubrir la regla sin
/// depender de la geometría sub-tesela del renderer.
#[must_use]
fn vehicle_entered_train_tunnel(
    state: &GameState,
    vehicle_idx: usize,
    prev_pos: crate::TileCoord,
) -> bool {
    let vehicle = &state.vehicles[vehicle_idx];
    vehicle.kind == VehicleKind::Train
        && vehicle.is_consist_head()
        && vehicle.pos != prev_pos
        && state.map.get_kind(vehicle.pos) == Some(crate::TileKind::RailTunnel)
        && state.map.get_kind(prev_pos) != Some(crate::TileKind::RailTunnel)
}

/// Propaga la pose de la cabeza a las unidades road creadas por CB16.
///
/// `OpenTTD` sólo ejecuta el controlador vial para la primera unidad de una
/// cadena articulada. Las demás conservan estado de depósito/carril y se
/// colocan detrás según la longitud de cada eslabón; si se procesaran como
/// vehículos independientes avanzarían por la misma ruta y bloquearían al
/// frente.
fn sync_road_articulated_parts(
    state: &mut GameState,
    head_idx: usize,
) -> Vec<(usize, crate::TileCoord)> {
    let Some(head) = state
        .vehicles
        .get(head_idx)
        .filter(|v| {
            matches!(
                v.kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            ) && v.is_consist_head()
        })
        .cloned()
    else {
        return Vec::new();
    };
    let ids = crate::train_consist::consist_unit_ids(&state.vehicles, head.id);
    if ids.len() < 2 {
        return Vec::new();
    }

    let depot_phase = head.road_depot_phase;
    let in_depot = !matches!(depot_phase, crate::vehicle::RoadDepotPhase::None)
        || head.road_state == crate::road_movement::RVSB_IN_DEPOT;
    let head_pose = crate::road_movement::VehiclePose::from_vehicle(&head);
    let mut previous_length = u16::from(head.unit_length.max(1));
    let mut back_fractions = 0_u16;
    let mut changed = Vec::with_capacity(ids.len().saturating_sub(1));

    for unit_id in ids.into_iter().skip(1) {
        let Some(slot) = state.vehicles.iter().position(|v| v.id == unit_id) else {
            continue;
        };
        let unit_length = u16::from(state.vehicles[slot].unit_length.max(1));
        // `unit_length` is measured in sixteenths of a tile. The centers of
        // adjacent units are separated by half the sum of their lengths.
        back_fractions =
            back_fractions.saturating_add(previous_length.saturating_add(unit_length).div_ceil(2));
        let previous_pos = state.vehicles[slot].pos;
        if in_depot {
            let unit = &mut state.vehicles[slot];
            unit.pos = head.pos;
            unit.origin = head.origin;
            unit.dest = head.dest;
            unit.progress = head.progress;
            unit.frame = head.frame;
            unit.direction = head.direction;
            unit.road_state = crate::road_movement::RVSB_IN_DEPOT;
            unit.road_depot_phase = depot_phase;
            unit.running = head.running;
            unit.cur_speed = head.cur_speed;
            unit.subspeed = head.subspeed;
            unit.path.clone_from(&head.path);
            unit.depart_turn = head.depart_turn;
            unit.depot_leave_cleared = head.depot_leave_cleared;
            unit.z_pos = head.z_pos;
        } else {
            // Convert sixteenths of a tile to the 0..=255 road progress scale.
            let back_progress = back_fractions.saturating_mul(255).div_ceil(16);
            let back_progress =
                u8::try_from(back_progress.min(u16::from(u8::MAX))).unwrap_or(u8::MAX);
            let pose = crate::road_movement::retreat_vehicle_pose(&head, head_pose, back_progress);
            let unit = &mut state.vehicles[slot];
            unit.pos = pose.pos;
            unit.origin = head.origin;
            unit.dest = head.dest;
            unit.progress = pose.progress;
            unit.frame = head.frame;
            unit.direction = head.direction;
            unit.road_state = head.road_state;
            unit.road_depot_phase = crate::vehicle::RoadDepotPhase::None;
            unit.running = head.running;
            unit.cur_speed = head.cur_speed;
            unit.subspeed = head.subspeed;
            unit.path.clone_from(&head.path);
            unit.depart_turn = head.depart_turn;
            unit.depot_leave_cleared = head.depot_leave_cleared;
            unit.z_pos = head.z_pos;
        }
        changed.push((slot, previous_pos));
        previous_length = unit_length;
    }
    changed
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{sync_road_articulated_parts, vehicle_entered_train_tunnel};
    use crate::{GameState, TileCoord, TileKind, Vehicle, VehicleKind};
    use std::collections::VecDeque;

    #[test]
    fn tunnel_sound_edge_only_fires_on_outside_to_inside_transition() {
        let mut state = GameState::new(4, 4);
        let outside = TileCoord::new(0, 1);
        let entrance = TileCoord::new(1, 1);
        assert!(state.map.set_kind(entrance, TileKind::RailTunnel).is_ok());
        state
            .vehicles
            .push(Vehicle::new(1, VehicleKind::Train, entrance, entrance));

        assert!(vehicle_entered_train_tunnel(&state, 0, outside));
        assert!(!vehicle_entered_train_tunnel(&state, 0, entrance));
    }

    #[test]
    fn road_articulated_parts_follow_head_without_becoming_heads() {
        let mut state = GameState::new(8, 8);
        let pos = TileCoord::new(2, 2);
        let dest = TileCoord::new(5, 2);
        let mut head = Vehicle::new(1, VehicleKind::Bus, pos, dest);
        head.running = true;
        head.progress = 128;
        head.frame = 4;
        head.path = VecDeque::from([TileCoord::new(3, 2), TileCoord::new(4, 2), dest]);
        let mut part = Vehicle::new(2, VehicleKind::Bus, pos, dest);
        part.running = false;
        part.newgrf_articulated = true;
        part.prev_unit = Some(head.id);
        head.next_unit = Some(part.id);
        state.vehicles.extend([head, part]);

        let changed = sync_road_articulated_parts(&mut state, 0);

        assert_eq!(changed, vec![(1, pos)]);
        assert!(state.vehicles[1].is_articulated_unit());
        assert!(!state.vehicles[1].is_consist_head());
        assert_eq!(state.vehicles[1].running, state.vehicles[0].running);
        assert_eq!(state.vehicles[1].frame, state.vehicles[0].frame);
        assert!(state.vehicles[1].progress < state.vehicles[0].progress);
    }

    #[test]
    fn road_articulated_parts_stay_hidden_inside_depot() {
        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        let mut head = Vehicle::new(1, VehicleKind::Bus, depot, depot);
        head.running = true;
        head.road_depot_phase = crate::vehicle::RoadDepotPhase::InDepot;
        head.road_state = crate::road_movement::RVSB_IN_DEPOT;
        let mut part = Vehicle::new(2, VehicleKind::Bus, depot, depot);
        part.newgrf_articulated = true;
        part.prev_unit = Some(head.id);
        head.next_unit = Some(part.id);
        state.vehicles.extend([head, part]);

        sync_road_articulated_parts(&mut state, 0);

        assert_eq!(state.vehicles[1].pos, depot);
        assert_eq!(
            state.vehicles[1].road_depot_phase,
            crate::vehicle::RoadDepotPhase::InDepot
        );
        assert_eq!(
            state.vehicles[1].road_state,
            crate::road_movement::RVSB_IN_DEPOT
        );
    }
}
