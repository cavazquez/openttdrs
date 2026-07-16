use crate::GameState;
use crate::vehicle::VehicleKind;

pub(super) fn tick_aircraft_phases(state: &mut GameState) {
    use crate::aircraft_movement::{AircraftPhaseEvent, tick_aircraft_phase};
    use crate::sim_events::SimEvent;

    for i in 0..state.vehicles.len() {
        let ev = tick_aircraft_phase(&mut state.vehicles[i], &state.map, &state.stations);
        let id = state.vehicles[i].id;
        let at = state.vehicles[i].pos;
        match ev {
            AircraftPhaseEvent::Takeoff => {
                state
                    .pending_sim_events
                    .push(SimEvent::AircraftTakeoff { vehicle_id: id, at });
            }
            AircraftPhaseEvent::Landing => {
                state
                    .pending_sim_events
                    .push(SimEvent::AircraftLanding { vehicle_id: id, at });
            }
            AircraftPhaseEvent::None => {}
        }
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
        // Espera ~37 ticks + chequeo de boca (`CheckTrainStayInDepot`).
        if state.vehicles[i].kind == VehicleKind::Train
            && crate::depot_leave::tick_train_stay_in_depot(&state.map, &mut state.vehicles, i)
        {
            continue;
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
            state.vehicles[i].cur_speed = 0;
            let reversed = crate::rail_pbs::tick_pbs_wait_and_maybe_reverse(
                &state.map,
                &mut state.vehicles[i],
                pf,
            );
            if reversed {
                let vehicle_id = state.vehicles[i].id;
                let order = state.vehicles[i].current_order;
                let pos = state.vehicles[i].pos;
                state.vehicles[i].sync_order_destination(&state.map);
                state.vehicles[i].pbs_stuck = true;
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
        // Liberó el path PBS: limpiar stuck (no tocar wait_counter de esclusas).
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
        let broke_down = state.vehicles[i].check_breakdown(tick);
        if broke_down {
            state
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Breakdown {
                    vehicle_id: state.vehicles[i].id,
                    at: state.vehicles[i].pos,
                    kind: state.vehicles[i].kind,
                });
        }
        if state.vehicles[i].breakdown_ticks_remaining > 0 {
            continue;
        }
        let prev_speed = state.vehicles[i].cur_speed;
        let prev_pos = state.vehicles[i].pos;
        let vehicle_id = state.vehicles[i].id;
        let vehicle_kind = state.vehicles[i].kind;
        let vehicle_running = state.vehicles[i].running;
        state.vehicles[i].step_with_map(Some(&state.map));
        if vehicle_kind == VehicleKind::Train {
            crate::train_consist::consist_changed(&mut state.vehicles, vehicle_id);
        }
        if state.vehicles[i].pos != prev_pos {
            crate::ship_movement::maybe_start_lock_transit(&mut state.vehicles[i], &state.map);
            if vehicle_kind == VehicleKind::Train {
                crate::rail_signals::enqueue_signal_glob(&mut state.signal_globset, prev_pos);
                crate::rail_signals::enqueue_signal_glob(
                    &mut state.signal_globset,
                    state.vehicles[i].pos,
                );
            }
        }
        if vehicle_running {
            if prev_speed == 0 && state.vehicles[i].cur_speed > 0 {
                state
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
