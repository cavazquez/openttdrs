//! Subsidios de transporte (`subsidy.cpp`).

use crate::GameState;
use crate::cargo::CargoType;
use crate::company::CompanyId;
use crate::economy::{TICKS_PER_MONTH, manhattan_distance};
use crate::map::TileCoord;
use crate::sim_events::SimEvent;
use crate::station::{self, STATION_COVERAGE_RADIUS};

/// Meses de validez de la oferta antes de caducar.
pub const SUBSIDY_OFFER_MONTHS: u32 = 12;
/// Distancia máxima Manhattan entre origen y destino.
pub const SUBSIDY_MAX_DISTANCE: u32 = 70;
/// Población mínima de pueblo para subsidio de pasajeros.
pub const SUBSIDY_PAX_MIN_POPULATION: u32 = 400;
/// Población mínima de pueblo origen para carga urbana.
pub const SUBSIDY_CARGO_MIN_POPULATION: u32 = 900;
/// Máximo % transportado para ser elegible (`SUBSIDY_MAX_PCT_TRANSPORTED`).
pub const SUBSIDY_MAX_PCT_TRANSPORTED: u32 = 42;

/// Subsidio activo u ofrecido.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subsidy {
    pub id: u32,
    pub cargo: CargoType,
    pub source_industry_pos: TileCoord,
    pub dest_station_pos: TileCoord,
    /// Origen en pueblo (pasajeros / carga urbana).
    #[serde(default)]
    pub source_town_pos: Option<TileCoord>,
    /// Destino en pueblo (pasajeros / carga urbana).
    #[serde(default)]
    pub dest_town_pos: Option<TileCoord>,
    /// Tick en el que caduca la oferta si aún no se adjudicó.
    pub offer_expires_tick: u64,
    #[serde(default)]
    pub awarded: bool,
    /// Tick en el que termina la bonificación (`0` si no adjudicado).
    #[serde(default)]
    pub award_expires_tick: u64,
    /// Compañía que adjudicó el subsidio.
    #[serde(default)]
    pub awarded_company: Option<CompanyId>,
}

impl Subsidy {
    #[must_use]
    pub fn is_offer_active(&self, tick: u64) -> bool {
        !self.awarded && tick < self.offer_expires_tick
    }

    #[must_use]
    pub fn is_award_active(&self, tick: u64) -> bool {
        self.awarded && tick < self.award_expires_tick
    }

    #[must_use]
    pub fn matches_delivery(
        &self,
        cargo: CargoType,
        dest_station: TileCoord,
        source: TileCoord,
        tick: u64,
        towns: &[crate::town::Town],
    ) -> bool {
        if self.cargo != cargo {
            return false;
        }
        if let Some(dest_town) = self.dest_town_pos {
            if !town_covers_tile(towns, dest_town, dest_station) {
                return false;
            }
        } else if self.dest_station_pos != dest_station {
            return false;
        }

        let source_ok = if let Some(src_town) = self.source_town_pos {
            source == src_town || town_covers_tile(towns, src_town, source)
        } else {
            source == self.source_industry_pos
                || station::industry_in_station_coverage_by_pos(
                    self.source_industry_pos,
                    source,
                    STATION_COVERAGE_RADIUS,
                )
        };

        if self.awarded {
            return self.is_award_active(tick) && source_ok;
        }
        self.is_offer_active(tick) && source_ok
    }
}

fn town_covers_tile(towns: &[crate::town::Town], town_pos: TileCoord, tile: TileCoord) -> bool {
    towns
        .iter()
        .find(|t| t.pos == town_pos)
        .is_some_and(|town| manhattan_distance(town.pos, tile) <= STATION_COVERAGE_RADIUS as u32)
}

fn town_percent_transported(_town: &crate::town::Town, _cargo: CargoType) -> u32 {
    0
}

fn industry_percent_transported(industry: &crate::industry::Industry) -> u32 {
    if industry.produced_total == 0 {
        return 0;
    }
    let transported = industry.transported_total.min(industry.produced_total);
    u32::try_from(transported.saturating_mul(100) / industry.produced_total).unwrap_or(0)
}

/// Multiplicador de ingreso según dificultad (`economy.cpp:1124-1131`).
#[must_use]
pub const fn subsidy_payment_multiplier_from_index(index: u8) -> i64 {
    match index {
        0 | 2 => 3, // +50 % (índices 0 y 2)
        1 => 2,
        _ => 4,
    }
}

/// Mantiene compatibilidad con el hook de landscape.
///
/// La creación mensual pertenece a `TimerGameEconomy` y ocurre antes del
/// timer de pueblos. Dejarla aquí hacía que el RNG se consumiera después de
/// vehículos y landscape; el hook por tick sólo puede purgar estados vencidos.
pub fn tick_subsidies(state: &mut GameState) {
    let tick = state.tick.get();
    state
        .subsidies
        .retain(|s| s.is_offer_active(tick) || s.is_award_active(tick));
}

/// Actualiza subsidios en el borde mensual de `TimerGameEconomy`.
///
/// El original sortea una sola vez `RandomRange(16)` antes de decidir qué
/// clase de ruta intentar. Sólo la ruta elegida puede reintentar hasta 1001
/// veces; no se debe volver a sortear la clase por cada intento.
pub(crate) fn process_monthly_subsidies(state: &mut GameState) {
    tick_subsidies(state);

    if state.subsidy_duration == 0 || !has_manually_distributed_cargo(state) {
        return;
    }

    let random_chance = state.random.random_range(16);
    match random_chance {
        0 | 1 if passenger_distribution_is_manual(state) => {
            let _ = retry_subsidy_route(state, try_create_passenger_subsidy);
        }
        2 => {
            let _ = retry_subsidy_route(state, try_create_town_cargo_subsidy);
        }
        3 => {
            let _ = retry_subsidy_route(state, try_create_industry_subsidy);
        }
        _ => {}
    }
}

fn has_manually_distributed_cargo(state: &GameState) -> bool {
    use crate::flow_stat::DistributionType;

    let settings = state.cargo_dist.openttd_settings();
    [
        settings.distribution_pax,
        settings.distribution_mail,
        settings.distribution_armoured,
        settings.distribution_default,
    ]
    .into_iter()
    .any(|distribution| matches!(distribution, DistributionType::Manual))
}

fn passenger_distribution_is_manual(state: &GameState) -> bool {
    use crate::flow_stat::DistributionType;

    matches!(
        state.cargo_dist.openttd_settings().distribution_pax,
        DistributionType::Manual
    )
}

fn retry_subsidy_route(state: &mut GameState, route: fn(&mut GameState) -> bool) -> bool {
    // `do { ... } while (!route && n--)` con n=1000: una primera llamada y
    // hasta mil reintentos adicionales.
    for _ in 0..=1000 {
        if route(state) {
            return true;
        }
    }
    false
}

/// Devuelve el índice de almacenamiento elegido por `Town::GetRandom`.
///
/// El ordinal de `RandomRange` se aplica sobre el pool sparse por `TownID`, no
/// sobre el orden físico del `Vec`: una importación o una recreación puede
/// dejar ambos órdenes distintos.
fn random_town_pool_index(state: &mut GameState) -> Option<usize> {
    let count = u32::try_from(state.towns.len()).ok()?;
    if count == 0 {
        return None;
    }
    let ordinal = usize::try_from(state.random.random_range(count)).ok()?;
    let mut pool_indices: Vec<_> = (0..state.towns.len()).collect();
    // El segundo componente mantiene el orden total en fixtures legacy que
    // todavía repiten el ID cero.
    pool_indices.sort_unstable_by_key(|&index| (state.towns[index].id, index));
    pool_indices.get(ordinal).copied()
}

/// Devuelve el índice de almacenamiento elegido por `Industry::GetRandom`.
///
/// Igual que los pueblos, el `IndustryPool` nativo es sparse y se recorre por
/// ID ascendente, no por el orden físico que conserve el `Vec` importado.
fn random_industry_pool_index(state: &mut GameState) -> Option<usize> {
    let count = u32::try_from(state.industries.len()).ok()?;
    if count == 0 {
        return None;
    }
    let ordinal = usize::try_from(state.random.random_range(count)).ok()?;
    let mut pool_indices: Vec<_> = (0..state.industries.len()).collect();
    // El índice de almacenamiento resuelve de forma determinista fixtures
    // legacy con IDs de industria repetidos.
    pool_indices.sort_unstable_by_key(|&index| (state.industries[index].instance_id, index));
    pool_indices.get(ordinal).copied()
}

fn try_create_passenger_subsidy(state: &mut GameState) -> bool {
    if state.towns.is_empty() {
        return false;
    }

    // `FindSubsidyPassengerRoute` siempre llama `RandomRange` para elegir el
    // cargo TPE_PASSENGERS, aun cuando la partida vanilla sólo tenga PASS.
    // Consumirla antes de elegir el pueblo mantiene el stream global alineado.
    let _ = state.random.random_range(1);
    let Some(src_idx) = random_town_pool_index(state) else {
        return false;
    };
    let cargo = CargoType::Passengers;
    let (src_pos, src_population, src_percent_transported) = {
        let src = &state.towns[src_idx];
        (
            src.pos,
            src.population,
            town_percent_transported(src, cargo),
        )
    };
    if src_population < SUBSIDY_PAX_MIN_POPULATION
        || src_percent_transported > SUBSIDY_MAX_PCT_TRANSPORTED
    {
        return false;
    }

    let Some(dst_idx) = random_town_pool_index(state) else {
        return false;
    };
    let dst = &state.towns[dst_idx];
    if dst.population < SUBSIDY_PAX_MIN_POPULATION || src_idx == dst_idx {
        return false;
    }
    if manhattan_distance(src_pos, dst.pos) > SUBSIDY_MAX_DISTANCE {
        return false;
    }
    push_subsidy(
        state,
        cargo,
        TileCoord::new(0, 0),
        TileCoord::new(0, 0),
        Some(src_pos),
        Some(dst.pos),
    )
}

fn try_create_town_cargo_subsidy(state: &mut GameState) -> bool {
    let Some(src_idx) = random_town_pool_index(state) else {
        return false;
    };
    let cargo = CargoType::Mail;
    let (src_pos, src_population, src_percent_transported) = {
        let src = &state.towns[src_idx];
        (
            src.pos,
            src.population,
            town_percent_transported(src, cargo),
        )
    };
    if src_population < SUBSIDY_CARGO_MIN_POPULATION {
        return false;
    }
    if src_percent_transported > SUBSIDY_MAX_PCT_TRANSPORTED {
        return false;
    }
    let dest_is_town = state.random.next() & 1 == 0;
    if dest_is_town {
        let Some(dst_idx) = random_town_pool_index(state) else {
            return false;
        };
        let dst = &state.towns[dst_idx];
        if src_idx == dst_idx || dst.population < SUBSIDY_CARGO_MIN_POPULATION {
            return false;
        }
        if manhattan_distance(src_pos, dst.pos) > SUBSIDY_MAX_DISTANCE {
            return false;
        }
        return push_subsidy(
            state,
            cargo,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
            Some(src_pos),
            Some(dst.pos),
        );
    }
    find_industry_destination_for_cargo(state, cargo, src_pos, Some(src_pos))
}

fn try_create_industry_subsidy(state: &mut GameState) -> bool {
    let Some(idx) = random_industry_pool_index(state) else {
        return false;
    };
    let industry = &state.industries[idx];
    let cargo = industry.output_cargo();
    if cargo.is_town_cargo() {
        return false;
    }
    if industry.produced_total > 0
        && industry_percent_transported(industry) > SUBSIDY_MAX_PCT_TRANSPORTED
    {
        return false;
    }
    find_industry_destination_for_cargo(state, cargo, industry.pos, None)
}

fn find_industry_destination_for_cargo(
    state: &mut GameState,
    cargo: CargoType,
    source: TileCoord,
    source_town: Option<TileCoord>,
) -> bool {
    let dest_is_town = state.random.next() & 1 == 0;
    if dest_is_town {
        let Some(dst_idx) = random_town_pool_index(state) else {
            return false;
        };
        let dst = &state.towns[dst_idx];
        if dst.population < SUBSIDY_CARGO_MIN_POPULATION {
            return false;
        }
        if manhattan_distance(source, dst.pos) > SUBSIDY_MAX_DISTANCE {
            return false;
        }
        return push_subsidy(
            state,
            cargo,
            source,
            TileCoord::new(0, 0),
            source_town,
            Some(dst.pos),
        );
    }
    if state.stations.is_empty() {
        return false;
    }
    let stations: Vec<TileCoord> = state
        .stations
        .iter()
        .filter(|st| {
            !st.is_waypoint()
                && st.accepts_cargo(cargo)
                && manhattan_distance(source, st.pos) <= SUBSIDY_MAX_DISTANCE
        })
        .map(|st| st.pos)
        .collect();
    if stations.is_empty() {
        return false;
    }
    let dest = stations[(state.random.next() as usize) % stations.len()];
    push_subsidy(state, cargo, source, dest, source_town, None)
}

fn duplicate_subsidy(
    state: &GameState,
    cargo: CargoType,
    source_industry: TileCoord,
    dest_station: TileCoord,
    source_town: Option<TileCoord>,
    dest_town: Option<TileCoord>,
) -> bool {
    state.subsidies.iter().any(|s| {
        !s.awarded
            && s.cargo == cargo
            && s.source_industry_pos == source_industry
            && s.dest_station_pos == dest_station
            && s.source_town_pos == source_town
            && s.dest_town_pos == dest_town
    })
}

fn push_subsidy(
    state: &mut GameState,
    cargo: CargoType,
    source_industry: TileCoord,
    dest_station: TileCoord,
    source_town: Option<TileCoord>,
    dest_town: Option<TileCoord>,
) -> bool {
    if duplicate_subsidy(
        state,
        cargo,
        source_industry,
        dest_station,
        source_town,
        dest_town,
    ) {
        return false;
    }
    let tick = state.tick.get();
    let id = state.next_subsidy_id;
    state.next_subsidy_id = state.next_subsidy_id.saturating_add(1);
    let offer_expires_tick = tick.saturating_add(u64::from(SUBSIDY_OFFER_MONTHS) * TICKS_PER_MONTH);
    let industry_pos = source_town.unwrap_or(source_industry);
    let station_pos = dest_town.unwrap_or(dest_station);
    state.subsidies.push(Subsidy {
        id,
        cargo,
        source_industry_pos: industry_pos,
        dest_station_pos: station_pos,
        source_town_pos: source_town,
        dest_town_pos: dest_town,
        offer_expires_tick,
        awarded: false,
        award_expires_tick: 0,
        awarded_company: None,
    });
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::SubsidyCreated {
            industry_pos,
            station_pos,
            cargo,
        });
    crate::news::push_subsidy_offer_news(state, cargo, industry_pos, station_pos);
    true
}

/// Intenta crear un subsidio industria → estación (tests / compat).
#[must_use]
pub fn try_create_subsidy(state: &mut GameState) -> bool {
    try_create_industry_subsidy(state)
}

/// Adjudica el subsidio en la primera entrega válida.
#[must_use]
pub fn try_award_subsidy(
    state: &mut GameState,
    dest_station: TileCoord,
    cargo: CargoType,
    source: TileCoord,
    company: CompanyId,
) -> bool {
    let tick = state.tick.get();
    let towns = state.towns.clone();
    let Some(idx) = state.subsidies.iter().position(|s| {
        !s.awarded
            && s.matches_delivery(cargo, dest_station, source, tick, &towns)
            && s.is_offer_active(tick)
    }) else {
        return false;
    };

    let award_months = u64::from(state.subsidy_duration) * 12;
    let award_expires_tick = tick.saturating_add(award_months * TICKS_PER_MONTH);
    state.subsidies[idx].awarded = true;
    state.subsidies[idx].award_expires_tick = award_expires_tick;
    state.subsidies[idx].awarded_company = Some(company);
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::SubsidyAwarded { cargo, company });
    let company_name = state
        .companies
        .iter()
        .find(|c| c.id == company)
        .map_or_else(|| format!("Compañía {}", company.0), |c| c.name.clone());
    crate::news::push_subsidy_awarded_news(state, cargo, &company_name, dest_station);
    true
}

/// Multiplicador de ingreso por entrega (`1` o bonificación por dificultad).
#[must_use]
pub fn delivery_income_multiplier(
    state: &GameState,
    dest_station: TileCoord,
    cargo: CargoType,
    source: TileCoord,
    company: CompanyId,
) -> i64 {
    let tick = state.tick.get();
    let towns = &state.towns;
    let multiplier = subsidy_payment_multiplier_from_index(state.subsidy_multiplier);
    if state.subsidies.iter().any(|s| {
        s.awarded
            && s.awarded_company == Some(company)
            && s.is_award_active(tick)
            && s.matches_delivery(cargo, dest_station, source, tick, towns)
    }) {
        multiplier
    } else {
        1
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::industry::{Industry, IndustryKind};
    use crate::station::Station;
    use crate::{Command, GameState, apply_command};

    fn setup_subsidy_route() -> GameState {
        let mut state = GameState::new(16, 16);
        let mine = TileCoord::new(2, 2);
        let stop = TileCoord::new(6, 2);
        for x in 2..=8 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 3), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(stop, 1)).unwrap();
        let mut industry = Industry::new(mine, IndustryKind::CoalMine);
        industry.stock = 40;
        industry.produced_total = 100;
        state.industries.push(industry);
        state.stations.push(Station::new(stop));
        state
    }

    #[test]
    fn create_subsidy_emits_event_and_news() {
        let mut state = setup_subsidy_route();
        assert!(try_create_subsidy(&mut state));
        assert_eq!(state.subsidies.len(), 1);
        assert_eq!(state.subsidies[0].cargo, CargoType::Coal);
        let events = state.runtime.pending_sim_events.drain();
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::SubsidyCreated {
                cargo: CargoType::Coal,
                ..
            }
        )));
        assert!(!state.news.items.is_empty());
    }

    #[test]
    fn award_on_first_delivery_uses_difficulty_multiplier() {
        let mut state = setup_subsidy_route();
        state.subsidy_multiplier = 2;
        let _ = try_create_subsidy(&mut state);
        let dest = state.subsidies[0].dest_station_pos;
        let source = state.subsidies[0].source_industry_pos;
        assert!(try_award_subsidy(
            &mut state,
            dest,
            CargoType::Coal,
            source,
            CompanyId::PLAYER
        ));
        assert!(state.subsidies[0].awarded);
        assert_eq!(
            delivery_income_multiplier(&state, dest, CargoType::Coal, source, CompanyId::PLAYER),
            3
        );
        assert_eq!(
            delivery_income_multiplier(&state, dest, CargoType::Coal, source, CompanyId(1)),
            1
        );
    }

    #[test]
    fn monthly_tick_can_create_subsidy() {
        let mut state = setup_subsidy_route();
        // La primera palabra escala a 3 con RandomRange(16), por lo que la
        // pasada mensual elige la ruta industria → destino.
        state.random = crate::linkgraph_parity::Randomizer {
            state: [0x8000_0009, 0],
        };
        process_monthly_subsidies(&mut state);
        assert!(!state.subsidies.is_empty());
    }

    #[test]
    fn monthly_selection_uses_one_random_range_before_route_choice() {
        let mut state = setup_subsidy_route();
        // La siguiente palabra es 0x7FFF_FFFF, por lo que RandomRange(16)
        // elige 7: no hay ruta que buscar ni otra palabra que consumir.
        state.random = crate::linkgraph_parity::Randomizer { state: [4, 0] };
        let mut expected = state.random;
        assert_eq!(expected.random_range(16), 7);

        process_monthly_subsidies(&mut state);

        assert_eq!(state.random, expected);
        assert!(state.subsidies.is_empty());
    }

    #[test]
    fn passenger_subsidy_respects_distance_and_population() {
        let mut state = GameState::new(32, 32);
        state.towns.push(crate::town::Town {
            id: 1,
            pos: TileCoord::new(2, 2),
            name: "A".into(),
            population: 500,
            ..crate::town::Town::default()
        });
        state.towns.push(crate::town::Town {
            id: 2,
            pos: TileCoord::new(80, 80),
            name: "B".into(),
            population: 500,
            ..crate::town::Town::default()
        });
        // Tras el selector TPE, esta semilla elige B como origen y A como
        // destino; así la fixture prueba distancia sin depender del orden
        // antiguo que elegía ambos pueblos antes de validar el origen.
        state.random = crate::linkgraph_parity::Randomizer::new(1);
        assert!(!try_create_passenger_subsidy(&mut state));
        state.towns[1].pos = TileCoord::new(10, 2);
        state.random = crate::linkgraph_parity::Randomizer::new(1);
        assert!(try_create_passenger_subsidy(&mut state));
        assert_eq!(state.subsidies[0].cargo, CargoType::Passengers);
    }

    #[test]
    fn passenger_subsidy_draws_cargo_before_sparse_pool_source_and_destination() {
        let mut state = GameState::new(32, 32);
        // El orden físico se invierte respecto del `TownPool`: el sorteo
        // nativo recorre IDs válidos crecientes, no posiciones del `Vec`.
        for (id, x) in [(2, 10), (0, 2), (1, 6)] {
            state.towns.push(crate::town::Town {
                id,
                pos: TileCoord::new(x, 2),
                name: format!("T{id}"),
                population: SUBSIDY_PAX_MIN_POPULATION,
                ..crate::town::Town::default()
            });
        }
        state.random = crate::linkgraph_parity::Randomizer::new(1);
        let mut expected = state.random;
        assert_eq!(expected.random_range(1), 0, "selector TPE_PASSENGERS");
        let source_ordinal =
            usize::try_from(expected.random_range(3)).expect("ordinal de pueblo origen");
        let destination_ordinal =
            usize::try_from(expected.random_range(3)).expect("ordinal de pueblo destino");
        assert_ne!(source_ordinal, destination_ordinal, "semilla de la fixture");
        let mut town_pool: Vec<_> = (0..state.towns.len()).collect();
        town_pool.sort_unstable_by_key(|&index| (state.towns[index].id, index));
        let source = town_pool[source_ordinal];
        let destination = town_pool[destination_ordinal];

        assert!(try_create_passenger_subsidy(&mut state));

        assert_eq!(state.random, expected);
        assert_eq!(
            state.subsidies[0].source_town_pos,
            Some(state.towns[source].pos)
        );
        assert_eq!(
            state.subsidies[0].dest_town_pos,
            Some(state.towns[destination].pos)
        );
    }

    #[test]
    fn town_cargo_subsidy_uses_sparse_pool_for_source() {
        let mut state = GameState::new(32, 32);
        // El primer `RandomRange(3)` de esta semilla da ordinal cero. La
        // ciudad 0 está primero en el pool, pero no es la segunda entrada
        // física que habría elegido el antiguo operador módulo.
        for (id, x, population) in [
            (0, 2, SUBSIDY_CARGO_MIN_POPULATION.saturating_sub(1)),
            (2, 6, SUBSIDY_CARGO_MIN_POPULATION),
            (1, 10, SUBSIDY_CARGO_MIN_POPULATION),
        ] {
            state.towns.push(crate::town::Town {
                id,
                pos: TileCoord::new(x, 2),
                name: format!("T{id}"),
                population,
                ..crate::town::Town::default()
            });
        }
        state.random = crate::linkgraph_parity::Randomizer::new(1);
        let mut expected = state.random;
        assert_eq!(expected.random_range(3), 0, "ordinal del TownPool");

        assert!(!try_create_town_cargo_subsidy(&mut state));

        assert_eq!(state.random, expected);
        assert!(state.subsidies.is_empty());
    }

    #[test]
    fn industry_pool_selector_uses_id_not_storage_order() {
        let mut state = GameState::new(32, 32);
        state.industries = [(2, 2), (0, 6), (1, 10)]
            .into_iter()
            .map(|(id, x)| {
                Industry::new(TileCoord::new(x, 2), IndustryKind::CoalMine).with_instance_id(id)
            })
            .collect();
        state.random = crate::linkgraph_parity::Randomizer::new(1);
        let mut expected = state.random;
        assert_eq!(expected.random_range(3), 0, "ordinal del IndustryPool");

        let selected = random_industry_pool_index(&mut state).expect("industria del pool");

        assert_eq!(state.industries[selected].instance_id, 0);
        assert_eq!(state.random, expected);
    }

    #[test]
    fn subsidy_award_duration_uses_subsidy_duration_years() {
        let mut state = setup_subsidy_route();
        state.subsidy_duration = 2;
        let _ = try_create_subsidy(&mut state);
        let dest = state.subsidies[0].dest_station_pos;
        let source = state.subsidies[0].source_industry_pos;
        let tick = state.tick.get();
        assert!(try_award_subsidy(
            &mut state,
            dest,
            CargoType::Coal,
            source,
            CompanyId::PLAYER
        ));
        let expected = tick + 24_u64 * TICKS_PER_MONTH;
        assert_eq!(state.subsidies[0].award_expires_tick, expected);
    }
}
