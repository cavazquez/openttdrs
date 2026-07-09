use crate::vehicle::VehicleKind;
use crate::{
    CargoType, GameState, STATION_COVERAGE_RADIUS, TileCoord, economy, pathfinder, station, town,
    vehicle_ai,
};

pub(crate) fn step(state: &mut GameState) {
    state.ensure_companies();
    state.tick.advance();
    let t = state.tick.get();

    process_monthly_economy(state, t);
    crate::ai::tick_ai_companies(state, t);
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

    crate::parity::release_staged_depot_trains(state);
    recompute_vehicle_paths(state);

    // Señales: solo `_globset` (sin barrido global).
    state.signal_tile_dirty.clear();
    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.signal_globset,
        &state.vehicles,
    );
    drain_signal_globset_now(state);

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
    drain_signal_globset_now(state);

    state.industry_tile_dirty = crate::map::step_industry_tiles(&mut state.map, t);

    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    let mut unloaded_this_tick = vec![false; state.vehicles.len()];
    unload_vehicles(state, t, &loaded_this_tick, &mut unloaded_this_tick);
    load_vehicles(state, &mut loaded_this_tick, &unloaded_this_tick);
    tick_vehicle_timetables(state);
    sync_autoreplace_depot_flags(state);
    run_autoreplace_in_depots(state);
    extend_orderless_vehicle_paths(state);
    assign_orderless_wander_destinations(state);
    tick_aircraft_phases(state);
    move_vehicles(state);

    crate::rail_signals::enqueue_trains_for_signal_update(
        &mut state.signal_globset,
        &state.vehicles,
    );
    crate::rail_signals::enqueue_pbs_reservations_for_signal_update(
        &mut state.signal_globset,
        &state.vehicles,
    );
    drain_signal_globset_now(state);

    sync_vehicle_order_destinations(state);
    apply_vehicle_running_costs(state);
    crate::news::poll_vehicle_advice_news(state);
    crate::news::maybe_purge_old_news(state);
    crate::parity::record_tick(state);
}

fn drain_signal_globset_now(state: &mut GameState) {
    let wormholes = state.jgr_tunnel_wormholes();
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    crate::rail_signals::drain_signal_globset_with_wormholes(
        &mut state.map,
        &state.vehicles,
        &mut state.signal_tile_dirty,
        &mut state.signal_globset,
        wh,
    );
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

fn tick_aircraft_phases(state: &mut GameState) {
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

fn age_vehicle_cargo(state: &mut GameState) {
    let day = state.tick.get() > 0
        && state
            .tick
            .get()
            .is_multiple_of(u64::from(economy::TICKS_PER_TRANSIT_DAY));
    for vehicle in &mut state.vehicles {
        vehicle.ensure_packets_from_legacy();
        if vehicle.cargo == 0 {
            continue;
        }
        vehicle.cargo_transit_ticks = vehicle.cargo_transit_ticks.saturating_add(1);
        if day {
            vehicle.cargo_packets.age_one_day();
            vehicle.sync_cargo_from_packets();
        }
    }
}

fn apply_vehicle_running_costs(state: &mut GameState) {
    for i in 0..state.vehicles.len() {
        let kind = state.vehicles[i].kind;
        let running = state.vehicles[i].running;
        let moving = running && state.vehicles[i].cur_speed > 0;
        let owner = state.vehicles[i].owner;
        let cost = economy::vehicle_running_cost_per_tick(kind, running, moving);
        if cost > 0 {
            state.debit_company(owner, cost);
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
    // Intereses por compañía; eventos de UI solo para la activa (jugador).
    for i in 0..state.companies.len() {
        let loan = state.companies[i].economy.loan;
        let max_loan = state.companies[i].economy.max_loan;
        let interest = economy::monthly_loan_interest(loan);
        if interest > 0 {
            state.companies[i].economy.money -= interest;
        }
        let money = state.companies[i].economy.money;
        let is_active = state.companies[i].id == state.active_company;
        if is_active {
            state.economy = state.companies[i].economy;
            if interest > 0 {
                state
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::LoanInterestPaid { amount: interest });
            }
            if economy::check_bankruptcy(money, max_loan) {
                state
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::BankruptcyWarning);
            }
        }
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
        let loading = state.vehicles[i].cargo_loading;
        // Con carga a bordo: solo seguir cargando si full_load o carga gradual.
        if state.vehicles[i].cargo != 0 && !allow_top_up && !loading {
            continue;
        }
        if state.vehicles[i].cargo_unloading {
            // Descarga gradual: no cargar en el mismo tick.
            continue;
        }
        if (allow_top_up || loading) && state.vehicles[i].cargo >= state.vehicles[i].capacity {
            state.vehicles[i].cargo_loading = false;
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
    state.vehicles[vehicle_idx].ensure_packets_from_legacy();
    let vcap = state.vehicles[vehicle_idx].capacity;
    let room = vcap.saturating_sub(state.vehicles[vehicle_idx].cargo);
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

    if room == 0 {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
        return true;
    }

    let output = state.industries[ind_idx].output_cargo();
    let speed = crate::cargo_packet::load_unload_speed(output);
    let load = state.industries[ind_idx].stock.min(room).min(speed);
    if load == 0 {
        return false;
    }

    let source = state.industries[ind_idx].pos;
    #[allow(clippy::cast_possible_truncation)]
    let count = load.min(u32::from(u16::MAX)) as u16;
    let mut packet = crate::cargo_packet::CargoPacket::new(output, count, source);
    packet.first_station = Some(station_pos);
    let first_pickup = state.vehicles[vehicle_idx].cargo == 0;
    state.vehicles[vehicle_idx].cargo_packets.push(packet);
    state.vehicles[vehicle_idx].mark_cargo_loaded(source);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    state.industries[ind_idx].stock -= u32::from(count);
    *loaded_flag = true;
    if first_pickup {
        state.stats.cargo_pickups += 1;
    }
    state.stats.cargo_units_loaded += u64::from(count);

    let full = state.vehicles[vehicle_idx].cargo >= vcap;
    let industry_empty = state.industries[ind_idx].stock == 0;
    let full_load = state.vehicles[vehicle_idx]
        .orders
        .get(state.vehicles[vehicle_idx].current_order)
        .is_some_and(|o| o.full_load());
    if full || (!full_load && industry_empty) {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
    } else {
        state.vehicles[vehicle_idx].cargo_loading = true;
    }
    true
}

fn try_load_from_station_waiting_cargo(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
) -> bool {
    state.vehicles[vehicle_idx].ensure_packets_from_legacy();
    state.stations[station_idx].ensure_packets_from_stock();
    let kind = state.vehicles[vehicle_idx].kind;
    let vcap = state.vehicles[vehicle_idx].capacity;
    let room = vcap.saturating_sub(state.vehicles[vehicle_idx].cargo);
    if room == 0 {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
        return true;
    }
    let preferred = state.vehicles[vehicle_idx].cargo_type;
    let stock = state.stations[station_idx].cargo_stock;

    let station_pos = state.stations[station_idx].pos;
    let cargo = match kind {
        VehicleKind::Bus | VehicleKind::Aircraft => preferred.unwrap_or(CargoType::Passengers),
        VehicleKind::Truck | VehicleKind::Train => {
            let Some(cargo) = stock.pick_freight_to_load(preferred) else {
                if state.vehicles[vehicle_idx].cargo_loading {
                    state.vehicles[vehicle_idx].cargo_loading = false;
                    state.vehicles[vehicle_idx].advance_after_loading();
                }
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
                return false;
            }
            cargo
        }
        VehicleKind::Ship => {
            if preferred.is_some_and(|c| !c.is_freight()) {
                preferred.unwrap_or(CargoType::Passengers)
            } else if let Some(cargo) = stock.pick_freight_to_load(preferred) {
                cargo
            } else if stock.get(CargoType::Passengers) > 0 {
                CargoType::Passengers
            } else {
                return false;
            }
        }
    };

    if !state.stations[station_idx].accepts_cargo(cargo) {
        return false;
    }

    let available = stock.get(cargo);
    let rating = station::station_rating_for_cargo(&state.stations[station_idx], cargo);
    let speed = crate::cargo_packet::load_unload_speed(cargo);
    let mut load = station::load_amount_for_rating(available.min(room).min(speed), rating);
    if load == 0 && available > 0 && rating > 0 {
        load = 1.min(speed).min(room);
    }
    if load == 0 {
        return false;
    }

    let taken = state.stations[station_idx].take_waiting_cargo(cargo, load);
    if taken.is_empty() {
        return false;
    }
    let loaded_units: u32 = taken.iter().map(|p| u32::from(p.count)).sum();
    let first_pickup = state.vehicles[vehicle_idx].cargo == 0;
    state.vehicles[vehicle_idx]
        .cargo_packets
        .append_packets(taken);
    state.vehicles[vehicle_idx].mark_cargo_loaded(station_pos);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    station::on_station_cargo_pickup(&mut state.stations[station_idx], cargo);
    *loaded_flag = true;
    if first_pickup {
        state.stats.cargo_pickups += 1;
    }
    state.stats.cargo_units_loaded += u64::from(loaded_units);

    let full = state.vehicles[vehicle_idx].cargo >= vcap;
    let station_empty = state.stations[station_idx].cargo_stock.get(cargo) == 0;
    let full_load = state.vehicles[vehicle_idx]
        .orders
        .get(state.vehicles[vehicle_idx].current_order)
        .is_some_and(|o| o.full_load());
    if full || (!full_load && station_empty) {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
    } else {
        state.vehicles[vehicle_idx].cargo_loading = true;
    }
    true
}

#[allow(clippy::too_many_lines)]
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
        state.vehicles[i].ensure_packets_from_legacy();
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

        let speed = crate::cargo_packet::load_unload_speed(cargo_type);
        let taken = state.vehicles[i].cargo_packets.take_amount(speed);
        if taken.is_empty() {
            continue;
        }
        let unload_units: u32 = taken.iter().map(|p| u32::from(p.count)).sum();
        let vehicle_owner = state.vehicles[i].owner;
        let mut payment = 0_i64;
        let mut feeder_total = 0_i64;
        for packet in &taken {
            let distance = economy::manhattan_distance(packet.source, station_pos);
            let mut part = economy::transported_goods_income(
                u32::from(packet.count),
                distance,
                packet.periods_in_transit,
                packet.cargo,
                tick,
            );
            let _ =
                crate::subsidy::try_award_subsidy(state, station_pos, packet.cargo, packet.source);
            part = part.saturating_mul(crate::subsidy::delivery_income_multiplier(
                state,
                station_pos,
                packet.cargo,
                packet.source,
            ));
            // Feeder: 25 % al owner de first_station si es distinta del destino.
            if !packet.feeder_paid
                && let Some(first) = packet.first_station
                && first != station_pos
            {
                let share = crate::company::feeder_share_of(part);
                if share > 0
                    && let Some(feeder_st) = state.stations.iter().find(|s| s.pos == first)
                {
                    let feeder_owner = feeder_st.owner;
                    state.credit_company(feeder_owner, share);
                    feeder_total = feeder_total.saturating_add(share);
                    part = part.saturating_sub(share);
                }
            }
            payment = payment.saturating_add(part);
        }

        let town_cargo = cargo_type.is_town_cargo();
        if town_cargo {
            town::record_delivery_near_town(
                &mut state.towns,
                station_pos,
                cargo_type,
                unload_units,
            );
        }
        if !town_cargo {
            state.stations[station_idx].add_waiting_cargo(cargo_type, unload_units);
        }
        state.stations[station_idx].income += payment.cast_unsigned();
        state.credit_company(vehicle_owner, payment);
        let shown = payment.saturating_add(feeder_total);
        state.stats.cargo_income_earned += shown.cast_unsigned();
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
        let first_chunk = !state.vehicles[i].cargo_unloading;
        let first_delivery = state.stats.cargo_deliveries == 0 && first_chunk;
        if first_chunk {
            crate::news::push_cargo_delivery_news(
                state,
                unload_units,
                cargo_type,
                payment,
                station_pos,
                first_delivery,
            );
            state.stats.cargo_deliveries += 1;
        }
        state.stats.cargo_units_delivered += u64::from(unload_units);
        state.vehicles[i].sync_cargo_from_packets();
        unloaded_this_tick[i] = true;

        if state.vehicles[i].cargo == 0 {
            state.vehicles[i].cargo_unloading = false;
            state.vehicles[i].clear_cargo();
            // Avanzar orden al terminar descarga (road stop o plataforma rail).
            state.vehicles[i].advance_after_unloading();
            state.vehicles[i].sync_order_destination(&state.map);
        } else {
            state.vehicles[i].cargo_unloading = true;
        }
    }
}

fn assign_orderless_wander_destinations(state: &mut GameState) {
    // Compat: camiones sin red de carretera siguen usando Manhattan hacia `dest`.
    for i in 0..state.vehicles.len() {
        if !matches!(
            state.vehicles[i].kind,
            VehicleKind::Bus | VehicleKind::Truck
        ) {
            continue;
        }
        if state.vehicles[i].running
            && state.vehicles[i].orders.is_empty()
            && state.vehicles[i].path.is_empty()
            && state.vehicles[i].pos == state.vehicles[i].dest
            && vehicle_ai::orderless_road_next(
                &state.map,
                state.vehicles[i].pos,
                if state.vehicles[i].origin == state.vehicles[i].pos {
                    None
                } else {
                    Some(state.vehicles[i].origin)
                },
                state.vehicles[i].id,
                state.tick,
            )
            .is_none()
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

/// Extiende el camino de vehículos sin órdenes (paridad `OpenTTD`: trenes/barcos
/// siguen la red; carretera elige ramas al azar; aviones van al hangar).
fn extend_orderless_vehicle_paths(state: &mut GameState) {
    let wormholes =
        pathfinder::TunnelWormholes::from_jgr_records(&state.map, &state.jgr_tunnels_from_footer);
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    for i in 0..state.vehicles.len() {
        if !state.vehicles[i].running || !state.vehicles[i].orders.is_empty() {
            continue;
        }
        if !state.vehicles[i].path.is_empty() {
            continue;
        }
        let pos = state.vehicles[i].pos;
        let prev = if state.vehicles[i].origin == pos {
            None
        } else {
            Some(state.vehicles[i].origin)
        };
        let preferred = dir_from_vehicle(&state.vehicles[i], prev);
        let id = state.vehicles[i].id;
        let tick = state.tick;

        match state.vehicles[i].kind {
            VehicleKind::Train => {
                if let Some(next) =
                    vehicle_ai::orderless_rail_next(&state.map, pos, prev, preferred, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Ship => {
                if let Some(next) =
                    vehicle_ai::orderless_water_next(&state.map, pos, prev, preferred, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Bus | VehicleKind::Truck => {
                if let Some(next) = vehicle_ai::orderless_road_next(&state.map, pos, prev, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Aircraft => {
                if pos != state.vehicles[i].dest {
                    continue;
                }
                let Some(hangar) = vehicle_ai::orderless_aircraft_hangar(&state.map, pos) else {
                    continue;
                };
                if hangar == pos {
                    continue;
                }
                state.vehicles[i].dest = hangar;
                if let Some(path) = pathfinder::find_path_cached(
                    &state.map,
                    &mut state.path_cache,
                    pos,
                    hangar,
                    pathfinder::PathNetwork::Air,
                    wh,
                ) {
                    state.vehicles[i].path = path.into_iter().collect();
                }
            }
        }
    }
}

fn dir_from_vehicle(vehicle: &crate::Vehicle, prev: Option<TileCoord>) -> u8 {
    if let Some(previous) = prev
        && let Some(dir) = crate::rail_signals::dir_from_to(previous, vehicle.pos)
    {
        return dir;
    }
    vehicle_ai::vehicle_direction_to_diag(vehicle.direction)
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
        if state.vehicles[i].orders.is_empty() {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
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
        let path = if net == pathfinder::PathNetwork::Rail {
            pathfinder::find_rail_path_for_engine(
                &state.map,
                from,
                to,
                wh,
                state.vehicles[i].engine_id,
            )
        } else {
            pathfinder::find_path_cached(&state.map, &mut state.path_cache, from, to, net, wh)
        };
        match path {
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

#[allow(clippy::too_many_lines)]
fn move_vehicles(state: &mut GameState) {
    let tick = state.tick.get();
    let vehicle_count = state.vehicles.len();
    let pf = state.pathfinding;
    for i in 0..vehicle_count {
        state.vehicles[i].sim_tick = tick;
        // Vagones: no se mueven solos; se sincronizan tras la cabeza.
        if state.vehicles[i].is_wagon_unit() {
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
                    crate::rail_signals::train_blocked_by_traffic(&state.map, vehicles, vehicle)
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
                state.vehicles[i].sync_order_destination(&state.map);
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
        state.vehicles[i].step();
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
        if had_force && vehicle_kind == VehicleKind::Train {
            state.vehicles[i].force_proceed = false;
        }
    }
}

fn vehicle_should_unload_at_station(vehicle: &crate::Vehicle, state: &GameState) -> bool {
    if vehicle.cargo == 0 {
        return false;
    }
    // En parada física (bahía road o plataforma rail) o descarga gradual en curso.
    let Some(station_idx) = station_index_at_vehicle(state, vehicle) else {
        return false;
    };
    let at_stop = station::vehicle_at_road_stop(&state.map, vehicle)
        || vehicle.cargo_unloading
        || station::vehicle_physically_at_station(
            &state.map,
            vehicle,
            &state.stations[station_idx],
        );
    if !at_stop {
        return false;
    }
    let station_pos = state.stations[station_idx].pos;
    if let Some(cargo) = vehicle.cargo_type {
        // Pax/mail: nunca descargar en la estación de origen.
        if cargo.is_town_cargo() && vehicle.cargo_source == Some(station_pos) {
            return false;
        }
        // Freight: no descargar en la parada de la orden actual si es de recogida
        // (mina en cobertura). No usar la estación física sola: una entrega
        // cercana a la mina también tendría cobertura.
        if let Some(crate::VehicleOrder::Station { station, .. }) =
            vehicle.orders.get(vehicle.current_order)
            && station::station_is_freight_pickup_stop(
                &state.map,
                &state.industries,
                *station,
                cargo,
            )
        {
            return false;
        }
        // Sin órdenes: no descargar en el origen del lote (carga en hub/industria).
        if vehicle.orders.is_empty() && vehicle.cargo_source == Some(station_pos) {
            return false;
        }
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
