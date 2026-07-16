use crate::GameState;

pub(super) fn tick_vehicle_timetables(state: &mut GameState) {
    let tick = state.tick.get();
    for vehicle in &mut state.vehicles {
        vehicle.sim_tick = tick;
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
    if state.autoreplace_rules.is_empty() {
        return;
    }
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
        if let Some(idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) {
            state.vehicles[idx].autoreplace_attempted_this_stop = true;
        }
        let _ = crate::autoreplace::try_autoreplace_vehicle(state, vehicle_id);
    }
}

pub(super) fn apply_pending_depot_order_refits(state: &mut GameState) {
    let pending: Vec<(u32, crate::cargo::CargoType)> = state
        .vehicles
        .iter()
        .filter_map(|v| v.pending_depot_order_refit.map(|cargo| (v.id, cargo)))
        .collect();
    for (head_id, cargo) in pending {
        if let Some(v) = state.vehicles.iter_mut().find(|v| v.id == head_id) {
            v.pending_depot_order_refit = None;
        }
        let unit_ids = crate::consist_unit_ids(&state.vehicles, head_id);
        for unit_id in unit_ids {
            let Some(idx) = state.vehicles.iter().position(|v| v.id == unit_id) else {
                continue;
            };
            if state.vehicles[idx].cargo > 0 {
                continue;
            }
            if !crate::refit::refittable_cargo_types(&state.vehicles[idx]).contains(&cargo) {
                continue;
            }
            state.vehicles[idx].cargo_type = Some(cargo);
        }
    }
}

pub(super) fn sync_vehicle_order_destinations(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        vehicle.sync_order_destination(&state.map);
    }
}
