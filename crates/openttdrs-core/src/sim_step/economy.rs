use crate::{
    ALL_CARGO_TYPES, CUSTOM_CARGO_COUNT, GameState, TileCoord, economy, industry_builder, town,
};

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
        let mut station_sounds = Vec::new();
        let dirty =
            crate::map::trigger_newgrf_station_animation_for_station_with_towns_and_world_and_cargo_catalog_and_sounds(
                &mut state.map,
                state.tick.get(),
                &mut state.stations,
                &state.companies,
                &state.towns,
                &state.industries,
                &state.cargo_spec_catalog,
                state.climate,
                &state.station_spec_catalog,
                &mut state.newgrf_animated_station_tiles,
                station_pos,
                crate::StationAnimationTrigger::NewCargo,
                Some(cargo),
                &mut station_sounds,
            );
        state.runtime.industry_tile_dirty.extend(dirty);
        crate::map::play_station_animation_sounds(state, station_sounds);
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
    // `IndustryBuildData::EconomyMonthlyLoop` corre antes de borrar las
    // industrias que cerraron: el contador objetivo nativo todavía ve el pool
    // completo durante este borde mensual. La configuración persistente aún
    // sólo expone la dificultad vanilla por defecto, que permite fundación
    // automática (`ID_FUND_ONLY` se conecta con settings en su propio corte).
    let (map_w, map_h) = state.map.dimensions();
    state.industry_builder.economy_monthly_loop(
        u32::try_from(state.industries.len()).unwrap_or(u32::MAX),
        map_w,
        map_h,
        false,
    );
    // Industrias ya marcadas con prod_level = 0 el mes pasado: fuera del mapa.
    let closed = crate::industry::remove_closed_industries_with_neutral_stations(
        &mut state.industries,
        &mut state.map,
        &mut state.stations,
    );
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
        // `ProduceIndustryGoods` usa `Chance16`, no `RandomRange`: la
        // probabilidad se calcula sobre los 16 bits bajos de la misma palabra
        // que se consume para el callback. Usar los bits altos cambia tanto la
        // decisión como los `Random()` posteriores (por ejemplo, puede plantar
        // un campo que el original rechaza).
        None => fallback_chance.is_some_and(|denominator| rng.chance16(1, denominator)),
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
                crate::newgrf_callback::apply_industry_production_callback_with_catalog(
                    def,
                    &mut state.industries[index],
                    0,
                    &mut state.random,
                    &state.cargo_spec_catalog,
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
        let dirty =
            crate::map::trigger_industry_randomisation_at_with_catalog_and_world_and_cargo_catalog(
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
                &state.cargo_spec_catalog,
            );
        state.runtime.industry_tile_dirty.extend(dirty);
        let dirty =
            crate::map::trigger_newgrf_industry_animation_group_with_world_and_cargo_catalog(
                &mut state.map,
                &footprint,
                &mut state.industries,
                &state.towns,
                &state.industry_tile_spec_catalog,
                &state.industry_spec_catalog,
                state.climate,
                &mut state.newgrf_animated_industry_tiles,
                crate::map::IndustryAnimationTrigger::CargoReceived,
                &state.cargo_spec_catalog,
                &mut state.random,
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

/// Ejecuta una rama de cambio de producción sobre una entidad ya elegida del
/// pool. Es el cuerpo de `ChangeIndustryProduction(i, false)` y queda
/// separado de la lotería diaria para que ésta pueda respetar `IndustryID`.
fn change_industry_production_at(state: &mut GameState, idx: usize) {
    if idx >= state.industries.len() {
        return;
    }
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

/// Devuelve el índice del `IndustryPool` elegido por `Industry::GetRandom`.
///
/// El vector del modelo no es necesariamente el pool: importar una partida o
/// borrar una entidad puede dejarlo en otro orden. `OpenTTD` sortea de facto por
/// el ID sparse del pool al recorrer `IsValidID`, por eso la selección debe
/// ordenar por `instance_id` antes de aplicar el ordinal de `RandomRange`.
fn random_industry_pool_index(state: &mut GameState) -> Option<usize> {
    let count = state.industries.len();
    if count == 0 {
        return None;
    }
    let ordinal = usize::try_from(
        state
            .random
            .random_range(u32::try_from(count).unwrap_or(u32::MAX)),
    )
    .ok()?;
    let mut pool_indices: Vec<_> = (0..count).collect();
    // El segundo componente conserva un resultado total aun en fixtures
    // legacy donde más de una entidad todavía tiene el ID cero.
    pool_indices.sort_unstable_by_key(|&index| (state.industries[index].instance_id, index));
    pool_indices.get(ordinal).copied()
}

/// Cuenta los tipos vanilla presentes, exactamente en el espacio de 240
/// `IndustryType` que usa `ITBL`.
///
/// `None` marca que el pool no se puede fundar con la ruta vanilla: una
/// entidad opaca o `NewGRF` exige sus callbacks de probabilidad/producción y
/// no debe ser reinterpretada como una especie estándar.
fn vanilla_industry_type_counts(state: &GameState) -> Option<Vec<u16>> {
    let mut counts = vec![0_u16; industry_builder::INDUSTRY_BUILD_TYPE_COUNT];
    for industry in &state.industries {
        let spec = industry.spec?;
        if industry.newgrf_type_id.is_some() {
            return None;
        }
        let type_index = usize::from(spec.native_type());
        *counts.get_mut(type_index)? = counts[type_index].saturating_add(1);
    }
    Some(counts)
}

/// Indica que el runtime puede ejecutar `TryBuildNewIndustry` completo para
/// el roster vanilla. Un catálogo `NewGRF` requiere sus providers propios;
/// conservar el contador y dejar la fundación pendiente es más seguro que
/// redistribuir sus objetivos con las probabilidades vanilla.
fn can_run_vanilla_industry_builder(state: &GameState) -> bool {
    state.industry_spec_catalog.is_empty() && vanilla_industry_type_counts(state).is_some()
}

/// Ejecuta la rama `TryBuildNewIndustry` para una partida estrictamente
/// vanilla: refresca `ITBL`, elige la especie, intenta los 2.000 sitios y
/// aplica el backoff incluso cuando no hay una especie elegible. Devuelve el
/// resultado que observa el hook nativo: `None` si no hubo especie elegible,
/// o la especie elegida y si `PlaceIndustry` logró materializarla.
fn try_build_new_vanilla_industry(state: &mut GameState) -> Option<(u16, bool)> {
    if !can_run_vanilla_industry_builder(state) {
        return None;
    }
    let (map_w, map_h) = state.map.dimensions();
    let current_counts = vanilla_industry_type_counts(state)?;
    state.industry_builder.setup_vanilla_target_count(
        state.climate,
        state.calendar.year,
        false,
        &mut state.random,
    );
    let selected_type = state.industry_builder.select_automatic_build_type(
        &current_counts,
        state.global_economy.is_in_recession(),
        &mut state.random,
    );
    let succeeded = selected_type
        .and_then(crate::IndustrySpec::from_native_type)
        .is_some_and(|spec| {
            // `PlaceIndustry` reads the dimensions from the current map. Keep
            // the locals above only as a cross-check that this branch never
            // accidentally derives map scale from an entity count.
            debug_assert_eq!(state.map.dimensions(), (map_w, map_h));
            crate::world_gen::try_place_runtime_industry(state, spec)
        });
    state
        .industry_builder
        .finish_automatic_build_attempt(selected_type, succeeded);
    selected_type.map(|industry_type| (industry_type, succeeded))
}

fn industry_creation_percent(desired_count: u32, current_count: u32) -> u32 {
    if desired_count > current_count {
        3_u32
            .saturating_add(desired_count.saturating_sub(current_count))
            .min(9)
    } else {
        3
    }
}

/// Scheduler diario completo de industrias (`_economy_industries_daily`).
///
/// El contador 16.16 reparte las acciones según el tamaño de mapa. Cada
/// acción consume primero `Chance16(perc, 100)`: la rama falsa selecciona una
/// industria del pool sparse y cambia su producción; la verdadera intenta una
/// fundación. Así se evita el antiguo error de cambiar una industria cada día
/// aun en mapas 64×64.
pub(super) fn advance_industry_daily_scheduler(state: &mut GameState) {
    let trace_enabled = state.runtime.industry_scheduler_trace_enabled;
    let mut trace_actions = trace_enabled.then(Vec::new);
    let (map_w, map_h) = state.map.dimensions();
    let change_loop = industry_builder::advance_industry_daily_change_counter(
        &mut state.global_economy.industry_daily_change_counter,
        map_w,
        map_h,
    );

    if change_loop != 0 {
        let current_count = u32::try_from(state.industries.len()).unwrap_or(u32::MAX);
        let desired_count = state.industry_builder.wanted_count();
        let creation_percent = industry_creation_percent(desired_count, current_count);
        let trace_percent = u8::try_from(creation_percent).unwrap_or(u8::MAX);

        for ordinal in 0..change_loop {
            if state.random.chance16(creation_percent, 100) {
                let result = try_build_new_vanilla_industry(state);
                if let Some(actions) = &mut trace_actions {
                    actions.push(crate::IndustrySchedulerTraceAction::foundation(
                        ordinal,
                        trace_percent,
                        result,
                    ));
                }
            } else {
                let selected = random_industry_pool_index(state);
                let industry_id =
                    selected.map(|index| u32::from(state.industries[index].instance_id));
                if let Some(index) = selected {
                    change_industry_production_at(state, index);
                }
                if let Some(actions) = &mut trace_actions {
                    actions.push(crate::IndustrySchedulerTraceAction::production(
                        ordinal,
                        trace_percent,
                        industry_id,
                    ));
                }
            }
        }
    }

    if let Some(actions) = trace_actions {
        state.runtime.industry_scheduler_trace_samples.push(
            crate::IndustrySchedulerTraceSample::from_state(state, change_loop, actions),
        );
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
        // `ProduceIndustryGoods` consulta el sonido ambiental antes de
        // decrementar `Industry::counter`. Aunque este runtime todavía no
        // reproduce ese efecto, `Chance16R(1, 14)` consume el RNG global y
        // omitirlo desplaza todos los consumidores posteriores. La condición
        // usa el contador persistido previo al decremento, como OpenTTD.
        if state.industries[i].counter.is_multiple_of(64) {
            let _ = state.random.chance16(1, 14);
        }
        // `ProduceIndustryGoods` decrementa el contador persistido antes de
        // evaluar producción, callbacks y efectos. No se puede reconstruir
        // esa fase con `tick + counter` tras importar un SAV.
        let production_tick = state.industries[i].advance_production_counter();
        if state.industries[i].requires_station_inputs() {
            let processed = if callback_on_arrival || callback_on_tick {
                state.industries[i].produce_from_nearby_stations_on_production_tick(
                    &mut state.stations,
                    true,
                    newgrf_def.as_ref(),
                    &state.cargo_spec_catalog,
                    production_tick,
                )
            } else {
                state.industries[i].produce_from_nearby_stations_on_production_tick(
                    &mut state.stations,
                    false,
                    newgrf_def.as_ref(),
                    &state.cargo_spec_catalog,
                    production_tick,
                )
            };
            if processed {
                state.industries[i].was_cargo_delivered = true;
                if callback_on_arrival && let Some(def) = newgrf_def.as_ref() {
                    crate::newgrf_callback::apply_industry_production_callback_with_catalog(
                        def,
                        &mut state.industries[i],
                        0,
                        &mut state.random,
                        &state.cargo_spec_catalog,
                    );
                }
                let dirty = crate::map::trigger_industry_randomisation_at_with_catalog_and_world_and_cargo_catalog(
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
                    &state.cargo_spec_catalog,
                );
                state.runtime.industry_tile_dirty.extend(dirty);
            }
            if callback_on_tick
                && production_tick
                && let Some(def) = newgrf_def.as_ref()
            {
                crate::newgrf_callback::apply_industry_production_callback_with_catalog(
                    def,
                    &mut state.industries[i],
                    1,
                    &mut state.random,
                    &state.cargo_spec_catalog,
                );
            }
        } else if callback_on_tick
            && production_tick
            && let Some(def) = newgrf_def.as_ref()
        {
            crate::newgrf_callback::apply_industry_production_callback_with_catalog(
                def,
                &mut state.industries[i],
                1,
                &mut state.random,
                &state.cargo_spec_catalog,
            );
        } else {
            state.industries[i].produce_if_due(production_tick);
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
                        u64::from(state.industries[i].counter)
                            .is_multiple_of(INDUSTRY_CUT_TREE_TICKS)
                    })
                } else {
                    u64::from(state.industries[i].counter).is_multiple_of(INDUSTRY_CUT_TREE_TICKS)
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
            let dirty = crate::map::trigger_industry_randomisation_at_with_catalog_and_world_and_cargo_catalog(
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
                &state.cargo_spec_catalog,
            );
            state.runtime.industry_tile_dirty.extend(dirty);
            let dirty =
                crate::map::trigger_newgrf_industry_animation_group_with_world_and_cargo_catalog(
                    &mut state.map,
                    &footprint,
                    &mut state.industries,
                    &state.towns,
                    &state.industry_tile_spec_catalog,
                    &state.industry_spec_catalog,
                    state.climate,
                    &mut state.newgrf_animated_industry_tiles,
                    crate::map::IndustryAnimationTrigger::IndustryTick,
                    &state.cargo_spec_catalog,
                    &mut state.random,
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
            let dirty =
                crate::map::trigger_newgrf_industry_animation_group_with_world_and_cargo_catalog(
                    &mut state.map,
                    &footprint,
                    &mut state.industries,
                    &state.towns,
                    &state.industry_tile_spec_catalog,
                    &state.industry_spec_catalog,
                    state.climate,
                    &mut state.newgrf_animated_industry_tiles,
                    crate::map::IndustryAnimationTrigger::CargoDistributed,
                    &state.cargo_spec_catalog,
                    &mut state.random,
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cargodist::parity::Randomizer;
    use crate::industry::Industry;
    use crate::{Climate, IndustrySpec};

    fn pool_industry(instance_id: u16) -> Industry {
        Industry::with_tiles_spec(
            TileCoord::new(0, 0),
            IndustrySpec::CoalMine.kind(),
            IndustrySpec::CoalMine,
            Vec::new(),
            0,
        )
        .with_instance_id(instance_id)
    }

    #[test]
    fn sparse_pool_selection_uses_instance_id_instead_of_vector_order() {
        // El vector se invierte a propósito para comprobar que no se confunde
        // su orden de almacenamiento con el `IndustryPool`. El RNG controlado
        // toma primero la rama de producción y luego el ordinal seis.
        let mut state = GameState::new(256, 256);
        state.industries = (0..=70).rev().map(pool_industry).collect();
        state.random = Randomizer {
            state: [3_704_038_854, 2_305_091_219],
        };

        assert!(!state.random.chance16(3, 100));
        let selected =
            random_industry_pool_index(&mut state).map(|index| state.industries[index].instance_id);
        assert_eq!(selected, Some(6));
    }

    #[test]
    fn vanilla_industry_special_effect_uses_chance16_low_word() {
        // Esta palabra aparece inmediatamente después del grupo IndustryTick
        // de la industria 8 de autosave0.sav. Sus bits altos hacen que
        // `RandomRange(8)` devuelva cero, mientras que `Chance16(1, 8)` —la
        // regla nativa de PlantFields— la rechaza usando los bits bajos.
        let mut rng = Randomizer {
            state: [628_366_352, 2_882_224_545],
        };
        let mut random_range = rng;
        assert_eq!(random_range.random_range(8), 0);

        let mut expected = rng;
        assert!(!expected.chance16(1, 8));
        let mut industry = pool_industry(8);
        assert!(!industry_special_effect(
            &mut rng,
            &mut industry,
            None,
            0,
            Some(8),
        ));
        assert_eq!(rng, expected, "consume una sola palabra Chance16");
    }

    #[test]
    fn runtime_decrements_industry_counter_before_the_production_cycle() {
        let mut state = GameState::new(4, 4);
        state.industries.push(
            Industry::with_tiles_spec(
                TileCoord::new(1, 1),
                IndustrySpec::CoalMine.kind(),
                IndustrySpec::CoalMine,
                vec![TileCoord::new(1, 1)],
                0,
            )
            .with_persisted_counter(1),
        );

        produce_industries(&mut state, 17);
        assert_eq!(state.industries[0].counter, 0);
        assert!(state.industries[0].stock > 0, "el cero posterior produce");

        produce_industries(&mut state, 18);
        assert_eq!(state.industries[0].counter, u16::MAX);
    }

    #[test]
    fn runtime_consumes_ambient_industry_rng_before_decrementing_counter() {
        let mut state = GameState::new(4, 4);
        state.random = Randomizer {
            state: [0x1020_3040, 0x5060_7080],
        };
        for (instance_id, counter) in [(0, 64), (1, 63), (2, 0), (3, 1)] {
            state.industries.push(
                Industry::with_tiles_spec(
                    TileCoord::new(1, 1),
                    IndustrySpec::CoalMine.kind(),
                    IndustrySpec::CoalMine,
                    vec![TileCoord::new(1, 1)],
                    0,
                )
                .with_instance_id(instance_id)
                .with_persisted_counter(counter),
            );
        }

        let mut expected_rng = state.random;
        let _ = expected_rng.chance16(1, 14);
        let _ = expected_rng.chance16(1, 14);
        // El contador 1 llega a cero en esta pasada: además de los dos
        // `Chance16R` de sonido, `TriggerIndustryAnimation` toma su palabra
        // base aunque ninguna tesela del fixture declare CB25.
        let _ = expected_rng.next();

        produce_industries(&mut state, 17);

        assert_eq!(
            state.random, expected_rng,
            "64 y 0 consumen sonido; 1 activa el grupo de animación"
        );
        assert_eq!(
            state
                .industries
                .iter()
                .map(|industry| industry.counter)
                .collect::<Vec<_>>(),
            vec![63, 62, u16::MAX, 0],
            "el chequeo ocurre antes del decremento persistido"
        );
    }

    #[test]
    fn due_industry_groups_always_consume_their_global_animation_word() {
        let mut state = GameState::new(8, 8);
        state.random = Randomizer {
            state: [0x1020_3040, 0x5060_7080],
        };
        // La tesela no necesita declarar un callback: el `Random()` base de
        // `TriggerIndustryAnimation` sucede antes de examinar la huella. Es
        // el caso que cubre los cinco grupos vanilla de autosave0.sav.
        for instance_id in 0..5 {
            state.industries.push(
                Industry::with_tiles_spec(
                    TileCoord::new(i32::from(instance_id), 0),
                    IndustrySpec::CoalMine.kind(),
                    IndustrySpec::CoalMine,
                    vec![TileCoord::new(i32::from(instance_id), 0)],
                    0,
                )
                .with_instance_id(instance_id)
                .with_persisted_counter(1),
            );
        }

        let mut expected_rng = state.random;
        for _ in 0..5 {
            let _ = expected_rng.next();
        }

        produce_industries(&mut state, 17);

        assert_eq!(state.random, expected_rng);
        assert!(
            state
                .industries
                .iter()
                .all(|industry| industry.counter == 0)
        );
    }

    #[test]
    fn daily_scheduler_preserves_fraction_and_does_not_run_before_boundary() {
        let mut state = GameState::new(256, 256);
        state.climate = Climate::Temperate;
        state.industries = (0..=70).map(pool_industry).collect();
        state.industry_builder.wanted_inds = 4_670_255;
        state.random = Randomizer {
            state: [3_704_038_854, 2_305_091_219],
        };

        state.global_economy.industry_daily_change_counter = 63_290;
        let before_rng = state.random.state;
        advance_industry_daily_scheduler(&mut state);
        assert_eq!(state.global_economy.industry_daily_change_counter, 65_404);
        assert_eq!(state.random.state, before_rng, "sin loop no hay RNG diario");

        advance_industry_daily_scheduler(&mut state);
        assert_eq!(
            state.global_economy.industry_daily_change_counter, 1_982,
            "65404 + 2114 conserva la fracción de la fila nativa del tick 20553"
        );
    }

    #[test]
    fn daily_scheduler_trace_captures_zero_loop_and_foundation_decision() {
        let mut state = GameState::new(256, 256);
        state.enable_industry_scheduler_trace();
        state.global_economy.industry_daily_change_counter = 63_290;

        advance_industry_daily_scheduler(&mut state);

        let zero_loop = state.take_industry_scheduler_trace_samples();
        assert_eq!(zero_loop.len(), 1);
        assert_eq!(zero_loop[0].change_loop, 0);
        assert!(zero_loop[0].actions.is_empty());

        // El siguiente `Random()` tiene low-word cero, por lo que
        // `Chance16(3, 100)` toma la rama de fundación. El objetivo cero no
        // tiene especie elegible y replica los campos null del hook nativo.
        state.random = Randomizer { state: [8, 0] };
        state.global_economy.industry_daily_change_counter = 65_404;
        advance_industry_daily_scheduler(&mut state);

        let samples = state.take_industry_scheduler_trace_samples();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].change_loop, 1);
        assert_eq!(samples[0].actions.len(), 1);
        assert!(samples[0].actions[0].tries_foundation);
        assert_eq!(samples[0].actions[0].creation_percent, 3);
        assert_eq!(samples[0].actions[0].foundation_type, None);
        assert_eq!(samples[0].actions[0].foundation_succeeded, None);
    }

    #[test]
    fn creation_percent_is_capped_at_nine_when_builder_is_behind() {
        assert_eq!(industry_creation_percent(0, 0), 3);
        assert_eq!(industry_creation_percent(72, 71), 4);
        assert_eq!(industry_creation_percent(100, 0), 9);
    }

    #[test]
    fn daily_scheduler_founds_post_1960_oil_rig_and_roundtrips_sav() {
        // Fixture mínima de gameplay: agua abierta y un pueblo válido. La
        // semilla 13 toma Chance16(4, 100), asigna el único objetivo a Oil Rig
        // (roll 12 del peso vanilla 34) y deja que PlaceIndustry consuma sus
        // propios intentos, layout y bytes constructores.
        let mut state = GameState::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                crate::map::make_water_tile(
                    &mut state.map,
                    TileCoord::new(x, y),
                    crate::map::WaterClass::Sea,
                )
                .expect("sea fixture");
            }
        }
        state.towns.push(crate::town::Town {
            id: 0,
            pos: TileCoord::new(32, 32),
            name: "Puerto scheduler".into(),
            ..crate::town::Town::default()
        });
        state.tick = crate::news::tick_for_calendar_year(1960);
        state.sync_timers_from_tick();
        state.industry_builder.wanted_inds = 1 << 16;
        state.global_economy.industry_daily_change_counter = 65_404;
        state.economy.money = 777_000;
        let money_before = state.economy.money;
        state.random = Randomizer::new(13);

        advance_industry_daily_scheduler(&mut state);

        assert_eq!(state.global_economy.industry_daily_change_counter, 0);
        assert_eq!(state.industries.len(), 1);
        let industry = &state.industries[0];
        assert_eq!(industry.spec, Some(IndustrySpec::OilRig));
        assert_eq!(industry.founder, None);
        assert_eq!(
            industry.construction_type,
            crate::industry::INDUSTRY_CONSTRUCTION_NORMAL_GAMEPLAY
        );
        assert_eq!(industry.town_id, Some(0));
        assert_eq!(state.economy.money, money_before);
        assert_eq!(industry.tiles.len(), 6);
        assert!(industry.tiles.iter().all(|&tile| {
            state.map.get_kind(tile) == Some(crate::map::TileKind::Industry)
                && state.map.get(tile).is_some_and(|value| value.m3hi == 0)
        }));
        assert_eq!(
            state.news.items.front().map(|item| item.news_type),
            Some(crate::news::NewsType::IndustryOpen)
        );
        assert_eq!(
            state
                .industry_builder
                .type_data(5)
                .map(|data| data.max_wait),
            Some(1),
            "un éxito reduce max_wait sin generar backoff"
        );

        let bytes = crate::sav::save_to_bytes_with(&state, crate::sav::SavContainer::Ottn)
            .expect("export scheduler SAV");
        let loaded =
            GameState::from_sav_game(crate::sav::load(&bytes).expect("reload scheduler SAV"));
        assert_eq!(
            loaded.random, state.random,
            "DATE conserva el stream post-placement"
        );
        assert_eq!(
            loaded.global_economy.industry_daily_change_counter,
            state.global_economy.industry_daily_change_counter
        );
        assert_eq!(loaded.industry_builder, state.industry_builder);
        assert_eq!(loaded.industries.len(), 1);
        assert_eq!(loaded.industries[0].spec, Some(IndustrySpec::OilRig));
        assert_eq!(loaded.industries[0].town_id, Some(0));
        // El pool INDY sólo persiste el rectángulo; el orden interno de
        // `tiles` no forma parte del SAV. Comparamos la cobertura, que es la
        // entidad semántica que OpenTTD vuelve a hidratar desde el mapa.
        let mut loaded_tiles = loaded.industries[0].tiles.clone();
        let mut original_tiles = industry.tiles.clone();
        loaded_tiles.sort_unstable_by_key(|tile| (tile.y, tile.x));
        original_tiles.sort_unstable_by_key(|tile| (tile.y, tile.x));
        assert_eq!(loaded_tiles, original_tiles);
    }
}
