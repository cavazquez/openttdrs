use crate::GameState;

pub(super) fn tick_vehicle_timetables(state: &mut GameState) {
    let tick = state.tick.get();
    for vehicle in &mut state.vehicles {
        vehicle.sim_tick = tick;
        vehicle.tick_timetable_clock();
        vehicle.tick_timetable_wait();
    }
}

pub(super) fn sync_autoreplace_depot_flags(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        if vehicle.running || !crate::refit::vehicle_in_depot(&state.map, vehicle.pos) {
            vehicle.autoreplace_attempted_this_stop = false;
        }
    }
}

pub(super) fn run_autoreplace_in_depots(state: &mut GameState) {
    let candidates: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| {
            !v.running
                && v.cargo == 0
                && !v.autoreplace_attempted_this_stop
                && crate::refit::vehicle_in_depot(&state.map, v.pos)
        })
        .map(|v| v.id)
        .collect();
    for vehicle_id in candidates {
        if let Some(idx) = state.runtime.fleet_index.slot(vehicle_id) {
            state.vehicles[idx].autoreplace_attempted_this_stop = true;
        }
        let _ = crate::autoreplace::try_autoreplace_vehicle(state, vehicle_id);
    }
}

pub(super) fn update_servicing_and_road_depot_orders(state: &mut GameState) {
    crate::vehicle::update_vehicle_servicing_flags(state);
}

pub(super) fn apply_pending_depot_order_refits(state: &mut GameState) {
    let pending: Vec<(u32, crate::cargo::CargoType)> = state
        .vehicles
        .iter()
        .filter_map(|v| v.pending_depot_order_refit.map(|cargo| (v.id, cargo)))
        .collect();
    for (head_id, cargo) in pending {
        if let Some(slot) = state.runtime.fleet_index.slot(head_id)
            && let Some(v) = state.vehicles.get_mut(slot)
        {
            v.pending_depot_order_refit = None;
        }
        let unit_ids = state.runtime.fleet_index.consist(head_id).to_vec();
        let mut refits = Vec::new();
        let mut total_cost = 0_i64;
        for unit_id in unit_ids {
            let Some(idx) = state.runtime.fleet_index.slot(unit_id) else {
                continue;
            };
            if state.vehicles[idx].cargo > 0 {
                continue;
            }
            let allowed = state.vehicles[idx]
                .engine_id
                .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
                .map_or_else(
                    || {
                        crate::refit::refittable_cargo_types_with_catalog_and_climate(
                            &state.vehicles[idx],
                            &state.engine_catalog,
                            &state.cargo_spec_catalog,
                            state.climate,
                        )
                    },
                    |engine| {
                        crate::refit::refittable_cargo_types_for_engine_with_catalog_and_climate(
                            engine,
                            &state.cargo_spec_catalog,
                            state.climate,
                        )
                    },
                );
            if !allowed.contains(&cargo) {
                continue;
            }
            let engine = state.vehicles[idx]
                .engine_id
                .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
                .cloned()
                .or_else(|| {
                    state.vehicles[idx]
                        .engine_id
                        .and_then(crate::engine::engine_by_id)
                        .cloned()
                });
            let cost = engine.as_ref().map_or(0, |engine| {
                let subtype = state.vehicles[idx].cargo_subtype;
                crate::economy::vehicle_refit_cost_with_callbacks(
                    &state.global_economy,
                    engine,
                    &mut state.vehicles[idx],
                    cargo,
                    subtype,
                    state.climate,
                    &state.cargo_spec_catalog,
                )
                .0
            });
            total_cost = total_cost.saturating_add(cost);
            refits.push((idx, cost));
        }
        if total_cost > state.economy.money {
            continue;
        }
        for (idx, _cost) in refits {
            state.vehicles[idx].cargo_type = Some(cargo);
            state.vehicles[idx].refit_capacity =
                u16::try_from(state.vehicles[idx].capacity).unwrap_or(u16::MAX);
        }
        state.economy.money -= total_cost;
    }
}

pub(super) fn sync_vehicle_order_destinations(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        vehicle.sync_order_destination(&state.map);
    }
}
