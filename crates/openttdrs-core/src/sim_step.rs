use crate::vehicle::VehicleKind;
use crate::{
    CargoType, GameState, STATION_COVERAGE_RADIUS, TileCoord, economy, pathfinder, station, town,
    vehicle_ai,
};

pub(crate) fn step(state: &mut GameState) {
    state.tick.advance();
    let t = state.tick.get();

    process_monthly_economy(state, t);
    produce_industries(state, t);
    produce_town_demand(state, t);
    grow_towns(state, t);
    age_vehicle_cargo(state);

    if t > 0 && t.is_multiple_of(u64::from(economy::TICKS_PER_TRANSIT_DAY)) {
        station::tick_station_cargo_age(&mut state.stations);
    }

    crate::subsidy::tick_subsidies(state);
    crate::map::tree_tile_loop::tick_tree_tile_loop(state);
    crate::disaster::tick_disasters(state);

    recompute_vehicle_paths(state);

    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state.vehicles,
        &mut state.signal_tile_dirty,
        true,
    );

    state.industry_tile_dirty = crate::map::step_industry_tiles(&mut state.map, t);

    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);
    tick_vehicle_timetables(state);
    sync_autoreplace_depot_flags(state);
    run_autoreplace_in_depots(state);
    assign_orderless_wander_destinations(state);
    move_vehicles(state);

    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state.vehicles,
        &mut state.signal_tile_dirty,
        false,
    );

    sync_vehicle_order_destinations(state);
    apply_vehicle_running_costs(state);
    crate::news::poll_vehicle_advice_news(state);
    crate::news::maybe_purge_old_news(state);
    crate::parity::record_tick(state);
}

fn tick_vehicle_timetables(state: &mut GameState) {
    let tick = state.tick.get();
    for vehicle in &mut state.vehicles {
        vehicle.sim_tick = tick;
        vehicle.tick_timetable_wait();
    }
}

fn sync_autoreplace_depot_flags(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        if vehicle.running || !crate::refit::vehicle_in_depot(&state.map, vehicle.pos) {
            vehicle.autoreplace_attempted_this_stop = false;
        }
    }
}

fn run_autoreplace_in_depots(state: &mut GameState) {
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

fn grow_towns(state: &mut GameState, tick: u64) {
    town::grow_town_if_served(
        &state.map,
        &state.industries,
        &state.stations,
        &mut state.towns,
        tick,
    );
}

fn process_monthly_economy(state: &mut GameState, tick: u64) {
    if tick == 0 || !tick.is_multiple_of(economy::TICKS_PER_MONTH) {
        return;
    }
    let interest = economy::monthly_loan_interest(state.economy.loan);
    if interest > 0 {
        state.economy.money -= interest;
        state
            .pending_sim_events
            .push(crate::sim_events::SimEvent::LoanInterestPaid { amount: interest });
    }
    if economy::check_bankruptcy(state.economy.money, state.economy.max_loan) {
        state
            .pending_sim_events
            .push(crate::sim_events::SimEvent::BankruptcyWarning);
    }
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
        let allow_top_up = state.vehicles[i]
            .orders
            .get(state.vehicles[i].current_order)
            .is_some_and(|o| o.full_load());
        if state.vehicles[i].cargo != 0 && !allow_top_up {
            continue;
        }
        if allow_top_up && state.vehicles[i].cargo >= state.vehicles[i].capacity {
            continue;
        }
        let vehicle_kind = state.vehicles[i].kind;
        let Some(station_idx) = station_index_for_industry_load(state, &state.vehicles[i]) else {
            if let Some(station_idx) = station_index_at_vehicle(state, &state.vehicles[i]) {
                try_load_at_station(state, i, station_idx, loaded_flag, unloaded_this_tick[i]);
            }
            continue;
        };
        let Some(station) = state.stations.get(station_idx) else {
            continue;
        };
        if !station.can_service_vehicle(vehicle_kind) {
            continue;
        }
        if !station_matches_current_order(&state.vehicles[i], station.pos) {
            continue;
        }
        let physically_at =
            station_index_at_vehicle(state, &state.vehicles[i]) == Some(station_idx);

        if try_load_from_industry(state, i, station_idx, loaded_flag) {
            continue;
        }
        if unloaded_this_tick[i] {
            continue;
        }
        if physically_at {
            try_load_from_station_waiting_cargo(state, i, station_idx, loaded_flag);
        }
    }
}

fn try_load_at_station(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
    unloaded_this_tick: bool,
) {
    let vehicle = &state.vehicles[vehicle_idx];
    let Some(station) = state.stations.get(station_idx) else {
        return;
    };
    if !station.can_service_vehicle(vehicle.kind) {
        return;
    }
    if !station_matches_current_order(vehicle, station.pos) {
        return;
    }
    if try_load_from_industry(state, vehicle_idx, station_idx, loaded_flag) {
        return;
    }
    if unloaded_this_tick {
        return;
    }
    try_load_from_station_waiting_cargo(state, vehicle_idx, station_idx, loaded_flag);
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
    state.vehicles[vehicle_idx].advance_after_loading();
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
    let room = vcap.saturating_sub(state.vehicles[vehicle_idx].cargo);
    if room == 0 {
        return false;
    }
    let preferred = state.vehicles[vehicle_idx].cargo_type;
    let stock = state.stations[station_idx].cargo_stock;

    let station_pos = state.stations[station_idx].pos;
    let cargo = match kind {
        VehicleKind::Bus => preferred.unwrap_or(CargoType::Passengers),
        VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => {
            let Some(cargo) = stock.pick_freight_to_load(preferred) else {
                return false;
            };
            if matches!(cargo, CargoType::Coal | CargoType::Wood | CargoType::Oil)
                && !station::station_is_freight_pickup_stop(
                    &state.map,
                    &state.industries,
                    station_pos,
                    cargo,
                )
                && !state.vehicles[vehicle_idx].orders.is_empty()
            {
                // Con órdenes activas, carbón/madera/petróleo solo en paradas de carga (mina, bosque…).
                return false;
            }
            cargo
        }
        VehicleKind::Aircraft => return false,
    };

    if !state.stations[station_idx].accepts_cargo(cargo) {
        return false;
    }

    let available = stock.get(cargo);
    let rating = station::station_rating_for_cargo(&state.stations[station_idx], cargo);
    let mut load = station::load_amount_for_rating(available.min(room), rating);
    if load == 0 && available > 0 && rating > 0 {
        load = 1;
    }
    if load == 0 {
        return false;
    }

    let _ = state.stations[station_idx].cargo_stock.take(cargo, load);
    let source = state.stations[station_idx].pos;
    state.vehicles[vehicle_idx].cargo_type = Some(cargo);
    state.vehicles[vehicle_idx].cargo += load;
    state.vehicles[vehicle_idx].mark_cargo_loaded(source);
    station::on_station_cargo_pickup(&mut state.stations[station_idx], cargo);
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
        let Some(station_idx) = station_index_at_vehicle(state, &state.vehicles[i]) else {
            continue;
        };
        if vcargo == 0 {
            continue;
        }
        if !vehicle_should_unload_at_station(&state.vehicles[i], state) {
            continue;
        }
        let cargo_type = vcargo_type.unwrap_or(CargoType::Goods);
        let Some(st) = state.stations.get(station_idx) else {
            continue;
        };
        if !station_matches_current_order(&state.vehicles[i], st.pos) {
            continue;
        }
        let station_pos = st.pos;
        if !st.accepts_cargo(cargo_type) || !st.can_service_vehicle(state.vehicles[i].kind) {
            continue;
        }
        let source = state.vehicles[i]
            .cargo_source
            .unwrap_or(state.vehicles[i].pos);
        let distance = economy::manhattan_distance(source, station_pos);
        let transit_days = economy::ticks_to_transit_days(state.vehicles[i].cargo_transit_ticks);
        let mut payment =
            economy::transported_goods_income(vcargo, distance, transit_days, cargo_type, tick);
        let _ = crate::subsidy::try_award_subsidy(state, station_pos, cargo_type, source);
        payment = payment.saturating_mul(crate::subsidy::delivery_income_multiplier(
            state,
            station_pos,
            cargo_type,
            source,
        ));
        let st = &mut state.stations[station_idx];
        let town_cargo = cargo_type.is_town_cargo();
        if town_cargo {
            town::record_delivery_near_town(&mut state.towns, station_pos, cargo_type, vcargo);
        }
        if !town_cargo {
            st.stock += vcargo;
            st.cargo_stock.add(cargo_type, vcargo);
        }
        st.income += payment.cast_unsigned();
        state.economy.money += payment;
        state.stats.cargo_income_earned += payment.cast_unsigned();
        state.pending_income_popups.push(crate::IncomePopup {
            amount: payment,
            at: vpos,
        });
        state
            .pending_sim_events
            .push(crate::sim_events::SimEvent::Income {
                amount: payment,
                at: vpos,
            });
        let first_delivery = state.stats.cargo_deliveries == 0;
        crate::news::push_cargo_delivery_news(
            state,
            vcargo,
            cargo_type,
            payment,
            station_pos,
            first_delivery,
        );
        state.stats.cargo_deliveries += 1;
        state.stats.cargo_units_delivered += u64::from(vcargo);
        state.vehicles[i].clear_cargo();
        unloaded_this_tick[i] = true;
        if station::vehicle_at_road_stop(&state.map, &state.vehicles[i]) {
            state.vehicles[i].advance_after_unloading();
            state.vehicles[i].sync_order_destination(&state.map);
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
    let tick = state.tick.get();
    let vehicles_snapshot = state.vehicles.clone();
    for vehicle in &mut state.vehicles {
        vehicle.sim_tick = tick;
        if vehicle.kind == VehicleKind::Train
            && vehicle.running
            && vehicle.movement_target().is_some()
        {
            let blocked = if vehicle.force_proceed {
                crate::rail_signals::train_blocked_by_traffic(
                    &state.map,
                    &vehicles_snapshot,
                    vehicle,
                )
            } else {
                crate::rail_signals::train_blocked_by_signal(
                    &state.map,
                    &vehicles_snapshot,
                    vehicle,
                ) || crate::rail_signals::train_blocked_by_traffic(
                    &state.map,
                    &vehicles_snapshot,
                    vehicle,
                )
            };
            if blocked {
                vehicle.cur_speed = 0;
                continue;
            }
        }
        let had_force = vehicle.force_proceed;
        let broke_down = vehicle.check_breakdown(tick);
        if broke_down {
            state
                .pending_sim_events
                .push(crate::sim_events::SimEvent::Breakdown {
                    vehicle_id: vehicle.id,
                    at: vehicle.pos,
                });
        }
        if vehicle.breakdown_ticks_remaining > 0 {
            continue;
        }
        let prev_speed = vehicle.cur_speed;
        let prev_pos = vehicle.pos;
        vehicle.step();
        if vehicle.running {
            if prev_speed == 0 && vehicle.cur_speed > 0 {
                state
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::VehicleDepart {
                        vehicle_id: vehicle.id,
                        at: vehicle.pos,
                    });
            }
            if vehicle.kind == VehicleKind::Train
                && vehicle.pos != prev_pos
                && let Some(tile) = state.map.get(vehicle.pos)
                && crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind)
            {
                state
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::LevelCrossing { at: vehicle.pos });
            }
        }
        if had_force && vehicle.kind == VehicleKind::Train {
            vehicle.force_proceed = false;
        }
    }
}

fn vehicle_should_unload_at_station(vehicle: &crate::Vehicle, state: &GameState) -> bool {
    if vehicle.cargo == 0 {
        return false;
    }
    if !station::vehicle_at_road_stop(&state.map, vehicle) {
        return false;
    }
    if let Some(crate::VehicleOrder::Station { station, .. }) =
        vehicle.orders.get(vehicle.current_order)
        && let Some(cargo) = vehicle.cargo_type
        && station::station_is_freight_pickup_stop(&state.map, &state.industries, *station, cargo)
    {
        // Parada de carga: no descargar el lote recién recogido aquí.
        return false;
    }
    !vehicle
        .orders
        .get(vehicle.current_order)
        .is_some_and(|o| o.no_unload())
}

fn station_matches_current_order(vehicle: &crate::Vehicle, station_pos: TileCoord) -> bool {
    let Some(order) = vehicle.orders.get(vehicle.current_order) else {
        return true;
    };
    if matches!(
        order,
        crate::VehicleOrder::Tile(_)
            | crate::VehicleOrder::Waypoint { .. }
            | crate::VehicleOrder::Depot { .. }
    ) {
        return true;
    }
    matches!(
        order,
        crate::VehicleOrder::Station { station, .. } if *station == station_pos
    )
}

fn station_index_at_vehicle(state: &GameState, vehicle: &crate::Vehicle) -> Option<usize> {
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| station::vehicle_physically_at_station(&state.map, vehicle, station))
        .min_by_key(|(_, station)| {
            (station.pos.x - vehicle.pos.x).abs() + (station.pos.y - vehicle.pos.y).abs()
        })
        .map(|(idx, _)| idx)
}

/// Estación válida para cargar desde industria: en la parada o sobre la tesela de la industria.
fn station_index_for_industry_load(state: &GameState, vehicle: &crate::Vehicle) -> Option<usize> {
    if let Some(idx) = station_index_at_vehicle(state, vehicle) {
        return Some(idx);
    }
    let vpos = vehicle.pos;
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| station.can_service_vehicle(vehicle.kind))
        .find(|(_, station)| {
            state.industries.iter().any(|ind| {
                ind.pos == vpos
                    && ind.stock > 0
                    && station::industry_in_station_coverage(
                        ind,
                        station.pos,
                        STATION_COVERAGE_RADIUS,
                    )
            })
        })
        .map(|(idx, _)| idx)
}
