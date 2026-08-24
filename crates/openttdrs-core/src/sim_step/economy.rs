use crate::{ALL_CARGO_TYPES, GameState, TileCoord, economy, town};

/// Dispara `NewCargo` sólo para las colas que crecieron durante una operación
/// de producción/distribución. La economía puede repartir un lote entre varias
/// estaciones; cada estación/cargo que recibió unidades obtiene su CB140 de
/// área completa sin inventar eventos para las que sólo quedaron en cobertura.
fn trigger_station_new_cargo_since(state: &mut GameState, before: &[crate::CargoStock]) {
    let arrivals: Vec<_> = state
        .stations
        .iter()
        .zip(before)
        .flat_map(|(station, before)| {
            ALL_CARGO_TYPES.iter().copied().filter_map(move |cargo| {
                (station.cargo_stock.get(cargo) > before.get(cargo)).then_some((station.pos, cargo))
            })
        })
        .collect();
    for (station_pos, cargo) in arrivals {
        let dirty = crate::map::trigger_newgrf_station_animation_for_station(
            &mut state.map,
            state.tick.get(),
            &mut state.stations,
            &state.companies,
            state.climate,
            &state.station_spec_catalog,
            &mut state.newgrf_animated_station_tiles,
            station_pos,
            crate::StationAnimationTrigger::NewCargo,
            Some(cargo),
        );
        state.runtime.industry_tile_dirty.extend(dirty);
        super::trigger_road_stop_animation_at(
            state,
            station_pos,
            crate::StationAnimationTrigger::NewCargo,
            Some(cargo),
        );
    }
}

pub(super) fn process_monthly_economy(state: &mut GameState) {
    apply_monthly_inflation_and_fluctuations(state);
    apply_monthly_interest_and_bankruptcy(state);
    roll_station_newgrf_month(&mut state.stations);
    // Industrias ya marcadas con prod_level = 0 el mes pasado: fuera del mapa.
    let closed = crate::industry::remove_closed_industries(&mut state.industries, &mut state.map);
    for at in closed {
        crate::news::report_industry_closed(state, at);
    }
    // Cierre mensual tras intereses: deltas por compañía + espejo global (activa).
    for i in 0..state.companies.len() {
        let company_id = state.companies[i].id;
        let money = state.companies[i].economy.money;
        let loan = state.companies[i].economy.loan;
        let income = state.companies[i].cargo_income_earned;
        let costs = state.companies[i].vehicle_running_costs;
        let deliveries = state.companies[i].cargo_deliveries;
        let liquid_value = crate::game_state::company_net_value(money, loan);
        state.companies[i].economy_history.push_month_from_totals(
            income,
            costs,
            deliveries,
            liquid_value,
        );
        let month = state.companies[i]
            .economy_history
            .samples
            .last()
            .copied()
            .unwrap_or_default();
        let quarter_deliveries = state.companies[i]
            .quarterly_economy
            .cur_deliveries
            .saturating_add(month.deliveries);
        let performance = crate::economy_quarterly::calculate_performance_rating(
            state,
            company_id,
            quarter_deliveries,
        );
        let company_value = crate::economy_quarterly::calculate_company_value(state, company_id);
        state.companies[i].quarterly_economy.push_month(
            month.income,
            month.running_costs,
            month.deliveries,
            performance,
            company_value,
        );
    }
    // Espejo legacy en `stats` = compañía activa (saves / Finances).
    let active_idx = state.active_company.index();
    if let Some(active) = state.companies.get(active_idx) {
        state.stats.economy_history = active.economy_history.clone();
    } else {
        state.stats.economy_history.push_month_from_totals(
            state.stats.cargo_income_earned,
            state.stats.vehicle_running_costs,
            state.stats.cargo_deliveries,
            crate::game_state::company_net_value(state.economy.money, state.economy.loan),
        );
    }
    state.link_graph.rollover_month();
    // Flows desde totales del link graph (mapper ingenuo; sin MCF).
    state.rebuild_station_flows();
    // La financiación vial continúa una vez por mes durante sus seis meses.
    // Se hace antes de decrementar el contador dentro del procesamiento urbano.
    let road_seed = state.calendar.date ^ u32::try_from(state.tick.get()).unwrap_or(0);
    let mut road_dirty = Vec::new();
    for town in &state.towns {
        if town.road_build_months == 0 {
            continue;
        }
        if let Some(pos) = crate::town_expand::fund_town_road_once(
            &mut state.map,
            town,
            road_seed.wrapping_add(town.id.wrapping_mul(0x9E37_79B9)),
        ) {
            road_dirty.push(pos);
        }
    }
    state.runtime.landscape_tile_dirty.extend(road_dirty);
    // Metas de crecimiento urbano + historiales de pueblos e industrias (UI-3).
    let company_count = state.companies.len();
    town::process_town_monthly_growth(
        &mut state.towns,
        &state.stations,
        &state.map,
        &state.industries,
        state.climate,
        state.world_seed,
        &mut state.random,
        company_count,
    );
    let active_rating_company = state.active_company;
    for town in &mut state.towns {
        let population = town.population;
        let passengers = town.passengers_served;
        let mail = town.mail_served;
        let rating = town.authority_rating(active_rating_company);
        town.history
            .push_month(population, passengers, mail, rating);
    }
    for industry in &mut state.industries {
        let stock = industry.stock;
        let produced = industry.produced_total;
        let transported = industry.transported_total;
        industry.history.push_month(stock, produced, transported);
    }
}

fn roll_station_newgrf_month(stations: &mut [crate::Station]) {
    for station in stations {
        for cargo in crate::ALL_CARGO_TYPES {
            station.goods.get_mut(cargo).roll_newgrf_month();
        }
    }
}

fn apply_monthly_inflation_and_fluctuations(state: &mut GameState) {
    let calendar_year = state.calendar.year;
    if !state
        .global_economy
        .add_monthly_inflation(calendar_year, true)
    {
        state.sync_scaled_max_loan();
    }
    if let Some(event) = state
        .global_economy
        .handle_monthly_fluctuations(&mut state.random)
    {
        crate::news::push_economy_fluctuation_news(state, event);
    }
}

fn apply_monthly_interest_and_bankruptcy(state: &mut GameState) {
    let month = state.economy_timer.month;
    let rate = i64::from(state.global_economy.interest_rate);
    let maintenance = economy::monthly_station_maintenance_fee(&state.global_economy);
    for i in 0..state.companies.len() {
        let loan = state.companies[i].economy.loan;
        let max_loan = state.companies[i].economy.max_loan;
        let money = state.companies[i].economy.money;
        let interest = economy::monthly_company_interest(loan, money, rate, month);
        let monthly_fee = interest.saturating_add(maintenance);
        if monthly_fee > 0 {
            state.companies[i].economy.money -= monthly_fee;
        }
        let money = state.companies[i].economy.money;
        let is_active = state.companies[i].id == state.active_company;
        let company_name = state.companies[i].name.clone();
        if is_active {
            state.economy = state.companies[i].economy;
            if monthly_fee > 0 {
                state.runtime.pending_sim_events.push(
                    crate::sim_events::SimEvent::LoanInterestPaid {
                        amount: monthly_fee,
                    },
                );
            }
            if economy::check_bankruptcy(money, loan, max_loan) {
                state.bankruptcy_streak = state.bankruptcy_streak.saturating_add(1);
                state
                    .runtime
                    .pending_sim_events
                    .push(crate::sim_events::SimEvent::BankruptcyWarning);
                crate::news::push_bankruptcy_news(
                    state,
                    &company_name,
                    state.bankruptcy_streak,
                    crate::score::BANKRUPTCY_STREAK_LIMIT,
                );
                if state.bankruptcy_streak >= crate::score::BANKRUPTCY_STREAK_LIMIT {
                    let _ =
                        crate::score::finish_game(state, crate::score::GameOverReason::Bankruptcy);
                }
            } else {
                state.bankruptcy_streak = 0;
            }
        } else if economy::check_bankruptcy(money, loan, max_loan) {
            state.companies[i].bankruptcy_months =
                state.companies[i].bankruptcy_months.saturating_add(1);
            let months = state.companies[i].bankruptcy_months;
            crate::news::push_bankruptcy_news(
                state,
                &company_name,
                months,
                crate::score::BANKRUPTCY_STREAK_LIMIT,
            );
        } else {
            state.companies[i].bankruptcy_months = 0;
        }
    }
}

/// Una industria al azar cambia de producción cada día de calendario (modo original).
///
/// En mapas grandes `OpenTTD` escala el número de cambios; aquí bastará con uno por día,
/// que es lo que tocaba en el mapa 256×256 clásico.
pub(super) fn maybe_change_industry_production(state: &mut GameState) {
    if state.industries.is_empty() {
        return;
    }
    let idx = state
        .random
        .random_range(u32::try_from(state.industries.len()).unwrap_or(1)) as usize;
    let climate = state.climate;
    let change = crate::industry::change_industry_production(
        &mut state.industries[idx],
        false,
        climate,
        &mut state.random,
    );
    if change == crate::industry::IndustryProductionChange::Closing {
        let at = state.industries[idx].pos;
        crate::news::report_industry_closing(state, at);
    }
}

pub(super) fn produce_industries(state: &mut GameState, tick: u64) {
    for i in 0..state.industries.len() {
        let before = state.industries[i].stock;
        let tiles = state.industries[i].tiles.clone();
        let pos = state.industries[i].pos;
        let footprint: Vec<TileCoord> = if tiles.is_empty() { vec![pos] } else { tiles };
        if state.industries[i].requires_station_inputs() {
            let processed =
                state.industries[i].produce_from_nearby_stations(&mut state.stations, tick);
            if processed {
                let dirty = crate::map::trigger_industry_randomisation_at(
                    &mut state.map,
                    &footprint,
                    crate::map::IndustryRandomTrigger::CargoReceived,
                    state.world_seed,
                    tick,
                );
                state.runtime.industry_tile_dirty.extend(dirty);
            }
        } else {
            state.industries[i].produce(tick);
            if state.industries[i].stock > before {
                let dirty = crate::map::trigger_industry_randomisation_at(
                    &mut state.map,
                    &footprint,
                    crate::map::IndustryRandomTrigger::IndustryTick,
                    state.world_seed,
                    tick,
                );
                state.runtime.industry_tile_dirty.extend(dirty);
            }
        }
        let produced = u64::from(state.industries[i].stock.saturating_sub(before));
        state.stats.industry_cargo_units_produced += produced;
        state.industries[i].produced_total =
            state.industries[i].produced_total.saturating_add(produced);

        // La producción no se queda en la mina: se reparte a las estaciones de la cobertura
        // según su rating (`TransportIndustryGoods` / `MoveGoodsToStation`).
        let station_stock_before: Vec<_> = state
            .stations
            .iter()
            .map(|station| station.cargo_stock)
            .collect();
        let _moved = crate::industry::transport_industry_goods(
            &mut state.industries[i],
            &mut state.stations,
            state.order.selectgoods,
        );
        trigger_station_new_cargo_since(state, &station_stock_before);
    }
}

pub(super) fn produce_town_demand(state: &mut GameState, tick: u64) {
    let station_stock_before: Vec<_> = state
        .stations
        .iter()
        .map(|station| station.cargo_stock)
        .collect();
    let (passengers, mail) = town::produce_town_cargo(
        &state.map,
        &state.industries,
        &mut state.stations,
        &state.towns,
        tick,
        state.order.selectgoods,
    );
    trigger_station_new_cargo_since(state, &station_stock_before);
    state.stats.town_passengers_generated += passengers;
    state.stats.town_mail_generated += mail;
}

pub(super) fn grow_towns(state: &mut GameState, tick: u64) {
    let dirty = town::grow_town_if_served_with_ctx(
        &mut state.map,
        &state.industries,
        &state.stations,
        &mut state.towns,
        tick,
        state.climate,
        state.calendar.year,
        &state.house_spec_catalog,
        &state.house_overrides,
    );
    state.runtime.landscape_tile_dirty.extend(dirty);
}

pub(super) fn age_vehicle_cargo(state: &mut GameState) {
    let aging_tick = state.tick.get() > 0
        && state
            .tick
            .get()
            .is_multiple_of(u64::from(economy::CARGO_AGING_TICKS));
    for vehicle in &mut state.vehicles {
        vehicle.ensure_packets_from_legacy();
        if vehicle.cargo == 0 {
            continue;
        }
        vehicle.cargo_transit_ticks = vehicle.cargo_transit_ticks.saturating_add(1);
        if aging_tick {
            vehicle.cargo_packets.age_one_period();
            vehicle.sync_cargo_from_packets();
        }
    }
}

pub(super) fn rollover_vehicle_profit_year(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        if !vehicle.is_consist_head() {
            continue;
        }
        vehicle.profit_last_year = vehicle.profit_this_year;
        vehicle.profit_this_year = 0;
    }
}

pub(super) fn apply_vehicle_running_costs(state: &mut GameState) {
    // La topología puede haber cambiado por un choque o desacople durante el
    // movimiento. Construirla una vez evita que cada unidad reconstruya un
    // `FleetIndex` completo para calcular el coste de su cabeza.
    state.runtime.fleet_index.rebuild(&state.vehicles);
    let len = state.vehicles.len();
    for i in 0..len {
        let head_id = state.vehicles[i].id;
        if !state.vehicles[i].is_consist_head()
            || !economy::vehicle_counts_running_tick(&state.vehicles[i])
        {
            continue;
        }
        let yearly = state
            .runtime
            .fleet_index
            .consist(head_id)
            .iter()
            .filter_map(|&unit_id| state.runtime.fleet_index.slot(unit_id))
            .map(|slot| {
                let unit = &state.vehicles[slot];
                let mut cost = economy::engine_running_cost_year(unit.effective_engine());
                if unit.other_multiheaded_part.is_some() {
                    cost /= 2;
                }
                cost
            })
            .fold(0_i64, i64::saturating_add);
        let cost = economy::accumulate_running_cost_for_head(&mut state.vehicles[i], yearly);
        if cost <= 0 {
            continue;
        }
        let owner = state.vehicles[i].owner;
        state.debit_company(owner, cost);
        let cost_u = cost.cast_unsigned();
        state.stats.vehicle_running_costs += cost_u;
        if let Some(c) = state.companies.get_mut(owner.index()) {
            c.vehicle_running_costs += cost_u;
        }
        state.vehicles[i].profit_this_year =
            state.vehicles[i].profit_this_year.saturating_sub(cost);
    }
}
