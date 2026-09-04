use crate::{ALL_CARGO_TYPES, CUSTOM_CARGO_COUNT, GameState, TileCoord, economy, town};

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
            ALL_CARGO_TYPES
                .iter()
                .copied()
                .filter_map(move |cargo| {
                    (station.cargo_stock.get(cargo) > before.get(cargo))
                        .then_some((station.pos, cargo))
                })
                .chain((0..CUSTOM_CARGO_COUNT).filter_map(move |slot| {
                    let cargo = crate::cargo::custom_cargo(slot);
                    (station.cargo_stock.get(cargo) > before.get(cargo))
                        .then_some((station.pos, cargo))
                }))
        })
        .collect();
    for (station_pos, cargo) in arrivals {
        let dirty =
            crate::map::trigger_newgrf_station_animation_for_station_with_world_and_cargo_catalog(
                &mut state.map,
                state.tick.get(),
                &mut state.stations,
                &state.companies,
                &state.industries,
                &state.cargo_spec_catalog,
                state.climate,
                &state.station_spec_catalog,
                &mut state.newgrf_animated_station_tiles,
                station_pos,
                crate::StationAnimationTrigger::NewCargo,
                Some(cargo),
            );
        state.runtime.industry_tile_dirty.extend(dirty);
        super::trigger_airport_animation_at(
            state,
            station_pos,
            crate::AirportAnimationTrigger::NewCargo,
            Some(cargo),
        );
        super::trigger_road_stop_animation_at(
            state,
            station_pos,
            crate::StationAnimationTrigger::NewCargo,
            Some(cargo),
        );
    }
}

#[allow(clippy::too_many_lines)]
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
        industry.rollover_accepted_history();
    }
    // OpenTTD evalúa CB35 después de actualizar las estadísticas mensuales.
    maybe_change_industry_production_monthly(state);
}

fn roll_station_newgrf_month(stations: &mut [crate::Station]) {
    for station in stations {
        for cargo in crate::ALL_CARGO_TYPES {
            station.goods.get_mut(cargo).roll_newgrf_month();
        }
        for slot in 0..CUSTOM_CARGO_COUNT {
            station
                .goods
                .get_mut(crate::cargo::custom_cargo(slot))
                .roll_newgrf_month();
        }
    }
}

const INDUSTRY_CUT_TREE_TICKS: u64 = crate::industry::INDUSTRY_PRODUCE_TICKS * 2;

fn industry_behaviour(industry: &crate::Industry, def: Option<&crate::IndustrySpecDef>) -> u32 {
    if let Some(def) = def {
        return def.behaviour;
    }
    match industry.spec {
        Some(crate::IndustrySpec::Farm | crate::IndustrySpec::FarmTropic) => {
            crate::INDUSTRY_BEHAVIOUR_PLANT_FIELDS_MASK
        }
        Some(crate::IndustrySpec::LumberMill) => crate::INDUSTRY_BEHAVIOUR_CUT_TREES_MASK,
        _ => 0,
    }
}

fn industry_footprint_dimensions(footprint: &[TileCoord], origin: TileCoord) -> (i32, i32) {
    let max_x = footprint
        .iter()
        .map(|coord| coord.x.saturating_sub(origin.x))
        .max()
        .unwrap_or(0);
    let max_y = footprint
        .iter()
        .map(|coord| coord.y.saturating_sub(origin.y))
        .max()
        .unwrap_or(0);
    (
        max_x.saturating_add(1).max(1),
        max_y.saturating_add(1).max(1),
    )
}

/// Ejecuta la elección de `ProduceIndustryGoods` conservando el consumo RNG
/// de `OpenTTD`: si hay callback se consume primero su `Random()` y sólo un
/// `CALLBACK_FAILED` cae al algoritmo vanilla.
fn industry_special_effect(
    rng: &mut crate::linkgraph_parity::Randomizer,
    industry: &mut crate::Industry,
    def: Option<&crate::IndustrySpecDef>,
    effect: u8,
    fallback_chance: Option<u32>,
) -> bool {
    let callback = def
        .filter(|def| def.has_special_effect_callback())
        .and_then(|def| {
            let random = rng.next();
            crate::newgrf_callback::resolve_industry_special_effect_callback(
                def, industry, random, effect,
            )
        });
    match callback {
        Some(value) => value,
        None => fallback_chance.is_some_and(|denominator| rng.random_range(denominator) == 0),
    }
}

/// Ejecuta `TriggerIndustryProduction` para las industrias que recibieron
/// carga durante la pasada de una estación.
///
/// `OpenTTD` no produce en mitad de `LoadUnloadStation`: primero termina de
/// descargar/cargar todos los vehículos y recién entonces procesa el conjunto
/// `_cargo_delivery_destinations`. El llamador mantiene ese orden mediante la
/// cola efímera de [`SimulationRuntime`].
#[allow(clippy::too_many_lines)]
pub(super) fn trigger_delivered_industries(state: &mut GameState, destinations: &[usize]) {
    for &index in destinations {
        if index >= state.industries.len() {
            continue;
        }
        let newgrf_def = state.industries[index].newgrf_type_id.and_then(|type_id| {
            state
                .industry_spec_catalog
                .iter()
                .find(|def| def.id == type_id)
                .cloned()
        });
        let callback_on_arrival = newgrf_def.as_ref().is_some_and(
            crate::industry_spec::IndustrySpecDef::has_production_cargo_arrival_callback,
        );
        let callback_on_tick = newgrf_def
            .as_ref()
            .is_some_and(crate::industry_spec::IndustrySpecDef::has_production_256_ticks_callback);

        state.industries[index].was_cargo_delivered = true;
        let output_cargos = state.industries[index].produced_cargos();
        let output_before: Vec<u32> = output_cargos
            .iter()
            .map(|&cargo| {
                if Some(cargo) == state.industries[index].newgrf_output_cargo
                    || cargo == state.industries[index].output_cargo()
                {
                    state.industries[index].stock
                } else if Some(cargo) == state.industries[index].newgrf_secondary_output_cargo
                    || Some(cargo) == state.industries[index].secondary_output_cargo()
                {
                    state.industries[index].secondary_stock
                } else {
                    state.industries[index].extra_produced_cargo(cargo)
                }
            })
            .collect();

        if callback_on_arrival {
            if let Some(def) = newgrf_def.as_ref() {
                crate::newgrf_callback::apply_industry_production_callback(
                    def,
                    &mut state.industries[index],
                    0,
                    &mut state.random,
                );
            }
        } else if !callback_on_tick {
            state.industries[index].process_accepted_cargo_without_callback();
        }

        let produced = output_cargos
            .iter()
            .enumerate()
            .map(|(output_idx, &cargo)| {
                let current = if Some(cargo) == state.industries[index].newgrf_output_cargo
                    || cargo == state.industries[index].output_cargo()
                {
                    state.industries[index].stock
                } else if Some(cargo) == state.industries[index].newgrf_secondary_output_cargo
                    || Some(cargo) == state.industries[index].secondary_output_cargo()
                {
                    state.industries[index].secondary_stock
                } else {
                    state.industries[index].extra_produced_cargo(cargo)
                };
                current.saturating_sub(output_before[output_idx])
            })
            .sum::<u32>();
        if produced > 0 {
            state.stats.industry_cargo_units_produced = state
                .stats
                .industry_cargo_units_produced
                .saturating_add(u64::from(produced));
            state.industries[index].produced_total = state.industries[index]
                .produced_total
                .saturating_add(u64::from(produced));
            state.industries[index].last_prod_year = state.economy_timer.year;
        }

        let tiles = state.industries[index].tiles.clone();
        let pos = state.industries[index].pos;
        let footprint = if tiles.is_empty() { vec![pos] } else { tiles };
        let dirty = crate::map::trigger_industry_randomisation_at_with_catalog_and_world(
            &mut state.map,
            &footprint,
            crate::map::IndustryRandomTrigger::CargoReceived,
            state.world_seed,
            state.tick.get(),
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
        );
        state.runtime.industry_tile_dirty.extend(dirty);
        let dirty = crate::map::trigger_newgrf_industry_animation_with_world(
            &mut state.map,
            state.tick.get(),
            &footprint,
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
            state.world_seed,
            &mut state.newgrf_animated_industry_tiles,
            crate::map::IndustryAnimationTrigger::CargoReceived,
        );
        state.runtime.industry_tile_dirty.extend(dirty);
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
    // Las industrias NewGRF con CB29 no deben caer al algoritmo vanilla cuando
    // el callback devuelve `CALLBACK_FAILED`: OpenTTD interpreta ese resultado
    // como “sin cambio”. Clonamos el spec para no mantener un borrow cruzado
    // mientras el resolver consume el RNG del estado.
    let def = state.industries[idx].newgrf_type_id.and_then(|type_id| {
        state
            .industry_spec_catalog
            .iter()
            .find(|def| def.id == type_id)
            .cloned()
    });
    let callback_action = def.as_ref().and_then(|def| {
        crate::newgrf_callback::resolve_industry_production_change_callback(
            def,
            &mut state.industries[idx],
            false,
            &mut state.random,
        )
    });
    let change = match callback_action {
        Some(crate::IndustryProductionAction::Standard) => {
            crate::industry::change_industry_production(
                &mut state.industries[idx],
                false,
                climate,
                &mut state.random,
            )
        }
        Some(action) => {
            crate::industry::apply_industry_production_action(&mut state.industries[idx], action)
        }
        None => crate::industry::change_industry_production(
            &mut state.industries[idx],
            false,
            climate,
            &mut state.random,
        ),
    };
    if change == crate::industry::IndustryProductionChange::Closing {
        let at = state.industries[idx].pos;
        crate::news::report_industry_closing(state, at);
    }
}

/// Ejecuta CB35 para todas las industrias que lo declararon durante el cierre
/// mensual. Las industrias sin callback mantienen el comportamiento existente
/// (el algoritmo vanilla mensual todavía no modifica `prod_level` en este
/// recorte); `CALLBACK_FAILED` es un no-op observable y no un fallback.
pub(super) fn maybe_change_industry_production_monthly(state: &mut GameState) {
    for idx in 0..state.industries.len() {
        let Some(type_id) = state.industries[idx].newgrf_type_id else {
            continue;
        };
        let Some(def) = state
            .industry_spec_catalog
            .iter()
            .find(|def| def.id == type_id)
            .cloned()
        else {
            continue;
        };
        let Some(action) = crate::newgrf_callback::resolve_industry_production_change_callback(
            &def,
            &mut state.industries[idx],
            true,
            &mut state.random,
        ) else {
            continue;
        };
        let change = match action {
            crate::IndustryProductionAction::Standard => {
                crate::industry::change_industry_production(
                    &mut state.industries[idx],
                    true,
                    state.climate,
                    &mut state.random,
                )
            }
            action => crate::industry::apply_industry_production_action(
                &mut state.industries[idx],
                action,
            ),
        };
        if change == crate::industry::IndustryProductionChange::Closing {
            let at = state.industries[idx].pos;
            crate::news::report_industry_closing(state, at);
        }
    }
}

#[allow(clippy::too_many_lines)] // Mantiene juntos los caminos vanilla y CB1/CB2.
pub(super) fn produce_industries(state: &mut GameState, tick: u64) {
    for i in 0..state.industries.len() {
        let newgrf_def = state.industries[i].newgrf_type_id.and_then(|type_id| {
            state
                .industry_spec_catalog
                .iter()
                .find(|def| def.id == type_id)
                .cloned()
        });
        let callback_on_arrival = newgrf_def.as_ref().is_some_and(
            crate::industry_spec::IndustrySpecDef::has_production_cargo_arrival_callback,
        );
        let callback_on_tick = newgrf_def
            .as_ref()
            .is_some_and(crate::industry_spec::IndustrySpecDef::has_production_256_ticks_callback);
        let before = state.industries[i].stock;
        let secondary_before = state.industries[i].secondary_stock;
        let extra_before = state.industries[i].newgrf_extra_produced_cargo;
        let tiles = state.industries[i].tiles.clone();
        let pos = state.industries[i].pos;
        let footprint: Vec<TileCoord> = if tiles.is_empty() { vec![pos] } else { tiles };
        let production_tick = state.industries[i].produces_on_tick(tick);
        if state.industries[i].requires_station_inputs() {
            let processed = if callback_on_arrival || callback_on_tick {
                state.industries[i].produce_from_nearby_stations_with_callback_and_newgrf(
                    &mut state.stations,
                    tick,
                    true,
                    newgrf_def.as_ref(),
                )
            } else {
                state.industries[i].produce_from_nearby_stations_with_callback_and_newgrf(
                    &mut state.stations,
                    tick,
                    false,
                    newgrf_def.as_ref(),
                )
            };
            if processed {
                state.industries[i].was_cargo_delivered = true;
                if callback_on_arrival && let Some(def) = newgrf_def.as_ref() {
                    crate::newgrf_callback::apply_industry_production_callback(
                        def,
                        &mut state.industries[i],
                        0,
                        &mut state.random,
                    );
                }
                let dirty = crate::map::trigger_industry_randomisation_at_with_catalog_and_world(
                    &mut state.map,
                    &footprint,
                    crate::map::IndustryRandomTrigger::CargoReceived,
                    state.world_seed,
                    tick,
                    &mut state.industries,
                    &state.towns,
                    &state.industry_tile_spec_catalog,
                    &state.industry_spec_catalog,
                    state.climate,
                );
                state.runtime.industry_tile_dirty.extend(dirty);
            }
            if callback_on_tick
                && production_tick
                && let Some(def) = newgrf_def.as_ref()
            {
                crate::newgrf_callback::apply_industry_production_callback(
                    def,
                    &mut state.industries[i],
                    1,
                    &mut state.random,
                );
            }
        } else if callback_on_tick
            && production_tick
            && let Some(def) = newgrf_def.as_ref()
        {
            crate::newgrf_callback::apply_industry_production_callback(
                def,
                &mut state.industries[i],
                1,
                &mut state.random,
            );
        } else {
            state.industries[i].produce(tick);
        }
        if production_tick {
            let behaviour = industry_behaviour(&state.industries[i], newgrf_def.as_ref());
            if behaviour & crate::INDUSTRY_BEHAVIOUR_PLANT_FIELDS_MASK != 0
                && industry_special_effect(
                    &mut state.random,
                    &mut state.industries[i],
                    newgrf_def.as_ref(),
                    0,
                    Some(8),
                )
            {
                let (width, height) = industry_footprint_dimensions(&footprint, pos);
                let industry_id = state.industries[i].instance_id;
                // `PopCtx` recibe el RNG como referencia separada del
                // `GameState`; clonar su estado de dos palabras permite
                // mutar el mapa y devolver exactamente el stream consumido
                // sin crear dos fuentes aleatorias.
                let mut effect_rng = state.random;
                crate::world_gen::plant_random_farm_field_runtime(
                    state,
                    pos,
                    width,
                    height,
                    industry_id,
                    &mut effect_rng,
                );
                state.random = effect_rng;
            }
            if behaviour & crate::INDUSTRY_BEHAVIOUR_CUT_TREES_MASK != 0 {
                let cut = if let Some(def) = newgrf_def
                    .as_ref()
                    .filter(|def| def.has_special_effect_callback())
                {
                    let random = state.random.next();
                    crate::newgrf_callback::resolve_industry_special_effect_callback(
                        def,
                        &mut state.industries[i],
                        random,
                        1,
                    )
                    .unwrap_or_else(|| {
                        (tick + u64::from(state.industries[i].counter))
                            .is_multiple_of(INDUSTRY_CUT_TREE_TICKS)
                    })
                } else {
                    // `i->counter` is represented by the fixed phase in the
                    // local model; adding it to the global tick reproduces the
                    // decremented counter used by OpenTTD's modulo check.
                    (tick + u64::from(state.industries[i].counter))
                        .is_multiple_of(INDUSTRY_CUT_TREE_TICKS)
                };
                if cut
                    && !state.industries[i].produced_cargos().is_empty()
                    && let Some(cut_tile) = crate::map::tree_tile_loop::chop_lumber_mill_tree(
                        &mut state.map,
                        pos,
                        &footprint,
                    )
                {
                    let cargo = state.industries[i].produced_cargos()[0];
                    state.industries[i].add_newgrf_produced_cargo(cargo, 45);
                    state.runtime.landscape_tile_dirty.push(cut_tile);
                }
            }
        }
        // `TriggerIndustryRandomisation(i, IndustryTick)` ocurre en cada
        // ciclo de 256 ticks, incluso cuando la industria no logró producir
        // por falta de insumos o su callback devolvió cero.
        if production_tick {
            let dirty = crate::map::trigger_industry_randomisation_at_with_catalog_and_world(
                &mut state.map,
                &footprint,
                crate::map::IndustryRandomTrigger::IndustryTick,
                state.world_seed,
                tick,
                &mut state.industries,
                &state.towns,
                &state.industry_tile_spec_catalog,
                &state.industry_spec_catalog,
                state.climate,
            );
            state.runtime.industry_tile_dirty.extend(dirty);
            let dirty = crate::map::trigger_newgrf_industry_animation_with_world(
                &mut state.map,
                tick,
                &footprint,
                &mut state.industries,
                &state.towns,
                &state.industry_tile_spec_catalog,
                &state.industry_spec_catalog,
                state.climate,
                state.world_seed,
                &mut state.newgrf_animated_industry_tiles,
                crate::map::IndustryAnimationTrigger::IndustryTick,
            );
            state.runtime.industry_tile_dirty.extend(dirty);
        }
        let extra_produced = state.industries[i]
            .produced_cargos()
            .iter()
            .skip(2)
            .map(|&cargo| {
                u64::from(
                    state.industries[i]
                        .extra_produced_cargo(cargo)
                        .saturating_sub(extra_before.get(cargo)),
                )
            })
            .sum::<u64>();
        let produced = u64::from(state.industries[i].stock.saturating_sub(before))
            .saturating_add(u64::from(
                state.industries[i]
                    .secondary_stock
                    .saturating_sub(secondary_before),
            ))
            .saturating_add(extra_produced);
        state.stats.industry_cargo_units_produced += produced;
        state.industries[i].produced_total =
            state.industries[i].produced_total.saturating_add(produced);
        if produced > 0 {
            // `UpdateIndustryStatistics` actualiza este año cuando la
            // producción mensual tuvo actividad. Mantenerlo aquí también
            // cubre callbacks CB1/CB2 que producen entre cierres de mes.
            state.industries[i].last_prod_year = state.economy_timer.year;
        }

        // La producción no se queda en la mina: se reparte a las estaciones de la cobertura
        // según su rating (`TransportIndustryGoods` / `MoveGoodsToStation`).
        let station_stock_before: Vec<_> = state
            .stations
            .iter()
            .map(|station| station.cargo_stock)
            .collect();
        let moved = crate::industry::transport_industry_goods_with_settings(
            &mut state.industries[i],
            &mut state.stations,
            state.order.selectgoods,
            state.serve_neutral_industries,
        );
        if moved > 0 {
            let dirty = crate::map::trigger_newgrf_industry_animation_with_world(
                &mut state.map,
                tick,
                &footprint,
                &mut state.industries,
                &state.towns,
                &state.industry_tile_spec_catalog,
                &state.industry_spec_catalog,
                state.climate,
                state.world_seed,
                &mut state.newgrf_animated_industry_tiles,
                crate::map::IndustryAnimationTrigger::CargoDistributed,
            );
            state.runtime.industry_tile_dirty.extend(dirty);
        }
        trigger_station_new_cargo_since(state, &station_stock_before);
    }
}

pub(super) fn produce_town_demand(state: &mut GameState, tick: u64) {
    let station_stock_before: Vec<_> = state
        .stations
        .iter()
        .map(|station| station.cargo_stock)
        .collect();
    let (passengers, mail) = town::produce_town_cargo_with_towns(
        &state.map,
        &state.industries,
        &mut state.stations,
        &mut state.towns,
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
                let Some(engine) = state.vehicles[slot]
                    .engine_id
                    .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
                    .cloned()
                else {
                    let unit = &state.vehicles[slot];
                    let mut cost = economy::engine_running_cost_year(unit.effective_engine());
                    if unit.other_multiheaded_part.is_some() {
                        cost /= 2;
                    }
                    return cost;
                };
                let mut cost = economy::engine_running_cost_year_with_callbacks(
                    &engine,
                    &mut state.vehicles[slot],
                );
                if state.vehicles[slot].other_multiheaded_part.is_some() {
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
