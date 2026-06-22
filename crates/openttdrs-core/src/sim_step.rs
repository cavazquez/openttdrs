use crate::vehicle::VehicleKind;
use crate::{
    CargoType, GameState, STATION_COVERAGE_RADIUS, TileCoord, economy, pathfinder, station, town,
    vehicle_ai,
};

pub(crate) fn step(state: &mut GameState) {
    state.tick.advance();
    let t = state.tick.get();

    produce_industries(state, t);
    produce_town_demand(state, t);
    age_vehicle_cargo(state);

    recompute_vehicle_paths(state);

    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state
            .vehicles
            .iter()
            .filter(|v| v.kind == VehicleKind::Train)
            .map(|v| v.pos)
            .collect::<Vec<_>>(),
    );

    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);
    assign_orderless_wander_destinations(state);
    move_vehicles(state);
    sync_vehicle_order_destinations(state);
    apply_vehicle_running_costs(state);
}

fn sync_vehicle_order_destinations(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        vehicle.sync_order_destination(&state.map);
    }
}

fn age_vehicle_cargo(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        if vehicle.cargo > 0 {
            vehicle.cargo_transit_ticks = vehicle.cargo_transit_ticks.saturating_add(1);
        }
    }
}

fn apply_vehicle_running_costs(state: &mut GameState) {
    for vehicle in &state.vehicles {
        let moving = vehicle.running && vehicle.cur_speed > 0;
        let cost = economy::vehicle_running_cost_per_tick(vehicle.kind, vehicle.running, moving);
        if cost > 0 {
            state.economy.money -= cost;
            state.stats.vehicle_running_costs += cost.cast_unsigned();
        }
    }
}

fn produce_industries(state: &mut GameState, tick: u64) {
    for i in 0..state.industries.len() {
        let before = state.industries[i].stock;
        if state.industries[i].requires_station_inputs() {
            let _ = state.industries[i].produce_from_nearby_stations(&mut state.stations, tick);
        } else {
            state.industries[i].produce(tick);
        }
        state.stats.industry_cargo_units_produced +=
            u64::from(state.industries[i].stock.saturating_sub(before));
    }
}

fn produce_town_demand(state: &mut GameState, tick: u64) {
    let (passengers, mail) =
        town::produce_town_cargo(&state.map, &state.industries, &mut state.stations, tick);
    state.stats.town_passengers_generated += passengers;
    state.stats.town_mail_generated += mail;
}

fn load_vehicles(
    state: &mut GameState,
    loaded_this_tick: &mut [bool],
    unloaded_this_tick: &[bool],
) {
    for (i, loaded_flag) in loaded_this_tick
        .iter_mut()
        .enumerate()
        .take(state.vehicles.len())
    {
        if state.vehicles[i].cargo != 0 {
            continue;
        }
        let vpos = state.vehicles[i].pos;
        let Some(station_idx) = station_index_covering_tile(state, vpos) else {
            continue;
        };
        let Some(station) = state.stations.get(station_idx) else {
            continue;
        };
        if !station.can_service_vehicle(state.vehicles[i].kind) {
            continue;
        }

        if try_load_from_industry(state, i, station_idx, loaded_flag) {
            continue;
        }
        if unloaded_this_tick[i] {
            continue;
        }
        try_load_from_station_waiting_cargo(state, i, station_idx, loaded_flag);
    }
}

fn try_load_from_industry(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
) -> bool {
    let vcap = state.vehicles[vehicle_idx].capacity;
    let vcargo_type = state.vehicles[vehicle_idx].cargo_type;
    let station_pos = state.stations[station_idx].pos;

    let Some(ind_idx) = state.industries.iter().position(|ind| {
        let output = ind.output_cargo();
        ind.stock > 0
            && vcargo_type.is_none_or(|c| c == output)
            && state.stations[station_idx].accepts_cargo(output)
            && station::industry_in_station_coverage(ind, station_pos, STATION_COVERAGE_RADIUS)
    }) else {
        return false;
    };

    let load = state.industries[ind_idx].stock.min(vcap);
    if load == 0 {
        return false;
    }

    let output = state.industries[ind_idx].output_cargo();
    let source = state.industries[ind_idx].pos;
    state.vehicles[vehicle_idx].cargo_type = Some(output);
    state.vehicles[vehicle_idx].cargo = load;
    state.vehicles[vehicle_idx].mark_cargo_loaded(source);
    state.industries[ind_idx].stock -= load;
    *loaded_flag = true;
    state.stats.cargo_pickups += 1;
    state.stats.cargo_units_loaded += u64::from(load);
    true
}

fn try_load_from_station_waiting_cargo(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
) -> bool {
    let kind = state.vehicles[vehicle_idx].kind;
    let vcap = state.vehicles[vehicle_idx].capacity;
    let preferred = state.vehicles[vehicle_idx].cargo_type;
    let stock = state.stations[station_idx].cargo_stock;

    let cargo = match kind {
        VehicleKind::Bus => preferred.unwrap_or(CargoType::Passengers),
        VehicleKind::Truck | VehicleKind::Train => {
            let Some(cargo) = stock.pick_freight_to_load(preferred) else {
                return false;
            };
            cargo
        }
    };

    if !state.stations[station_idx].accepts_cargo(cargo) {
        return false;
    }

    let available = stock.get(cargo);
    let load = available.min(vcap);
    if load == 0 {
        return false;
    }

    let _ = state.stations[station_idx].cargo_stock.take(cargo, load);
    let source = state.stations[station_idx].pos;
    state.vehicles[vehicle_idx].cargo_type = Some(cargo);
    state.vehicles[vehicle_idx].cargo = load;
    state.vehicles[vehicle_idx].mark_cargo_loaded(source);
    *loaded_flag = true;
    state.stats.cargo_pickups += 1;
    state.stats.cargo_units_loaded += u64::from(load);
    true
}

fn unload_vehicles(
    state: &mut GameState,
    tick: u64,
    loaded_this_tick: &[bool],
    unloaded_this_tick: &mut [bool],
) {
    for (i, loaded_flag) in loaded_this_tick
        .iter()
        .enumerate()
        .take(state.vehicles.len())
    {
        if *loaded_flag {
            continue;
        }
        let vpos = state.vehicles[i].pos;
        let vcargo = state.vehicles[i].cargo;
        let vcargo_type = state.vehicles[i].cargo_type;
        let Some(station_idx) = station_index_covering_tile(state, vpos) else {
            continue;
        };
        if vcargo == 0 {
            continue;
        }
        if !vehicle_should_unload_at_station(&state.vehicles[i]) {
            continue;
        }
        let cargo_type = vcargo_type.unwrap_or(CargoType::Goods);
        if let Some(st) = state.stations.get_mut(station_idx) {
            if !st.accepts_cargo(cargo_type) || !st.can_service_vehicle(state.vehicles[i].kind) {
                continue;
            }
            let town_cargo = cargo_type.is_town_cargo();
            if !town_cargo {
                st.stock += vcargo;
                st.cargo_stock.add(cargo_type, vcargo);
            }
            let source = state.vehicles[i]
                .cargo_source
                .unwrap_or(state.vehicles[i].pos);
            let distance = economy::manhattan_distance(source, st.pos);
            let transit_days =
                economy::ticks_to_transit_days(state.vehicles[i].cargo_transit_ticks);
            let payment =
                economy::transported_goods_income(vcargo, distance, transit_days, cargo_type, tick);
            st.income += payment.cast_unsigned();
            state.economy.money += payment;
            state.stats.cargo_income_earned += payment.cast_unsigned();
            state.pending_income_popups.push(crate::IncomePopup {
                amount: payment,
                at: vpos,
            });
            state.stats.cargo_deliveries += 1;
            state.stats.cargo_units_delivered += u64::from(vcargo);
            state.vehicles[i].clear_cargo();
            unloaded_this_tick[i] = true;
        }
    }
}

fn assign_orderless_wander_destinations(state: &mut GameState) {
    for i in 0..state.vehicles.len() {
        if state.vehicles[i].running
            && state.vehicles[i].orders.is_empty()
            && state.vehicles[i].path.is_empty()
            && state.vehicles[i].pos == state.vehicles[i].dest
            && let Some(dest) = vehicle_ai::orderless_wander_destination(
                &state.map,
                state.vehicles[i].id,
                state.vehicles[i].pos,
                state.vehicles[i].origin,
                state.tick,
            )
        {
            state.vehicles[i].dest = dest;
        }
    }
}

fn recompute_vehicle_paths(state: &mut GameState) {
    state.path_cache.begin_tick(state.tick.get());
    let wormholes =
        pathfinder::TunnelWormholes::from_jgr_records(&state.map, &state.jgr_tunnels_from_footer);
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    for i in 0..state.vehicles.len() {
        state.vehicles[i].sync_order_destination(&state.map);
        if !state.vehicles[i].path.is_empty() {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
        if state.vehicles[i].pos == state.vehicles[i].dest {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
        let from = state.vehicles[i].pos;
        let to = state.vehicles[i].dest;
        let has_orders = !state.vehicles[i].orders.is_empty();
        let net = pathfinder::path_network_for_vehicle(state.vehicles[i].kind);
        match pathfinder::find_path_cached(&state.map, &mut state.path_cache, from, to, net, wh) {
            Some(path) => {
                state.vehicles[i].path = path.into_iter().collect();
                state.vehicles[i].no_network_route_to_order = false;
            }
            None => {
                state.vehicles[i].no_network_route_to_order = has_orders;
            }
        }
    }
}

fn move_vehicles(state: &mut GameState) {
    let train_positions: Vec<_> = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train)
        .map(|v| v.pos)
        .collect();
    for vehicle in &mut state.vehicles {
        if vehicle.kind == VehicleKind::Train
            && vehicle.running
            && let Some(next) = vehicle.movement_target()
            && crate::rail_signals::train_blocked_by_signal(
                &state.map,
                &train_positions,
                vehicle.pos,
                next,
            )
        {
            vehicle.cur_speed = 0;
            continue;
        }
        vehicle.step();
    }
}

fn vehicle_should_unload_at_station(vehicle: &crate::Vehicle) -> bool {
    vehicle.cargo > 0 && vehicle.manhattan_to_dest() == 0
}

fn station_index_covering_tile(state: &GameState, tile: TileCoord) -> Option<usize> {
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| {
            station::station_covers_tile(station.pos, tile, STATION_COVERAGE_RADIUS)
        })
        .min_by_key(|(_, station)| (station.pos.x - tile.x).abs() + (station.pos.y - tile.y).abs())
        .map(|(idx, _)| idx)
}
