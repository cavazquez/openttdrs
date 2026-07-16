//! Subsidios de transporte (simplificación de `subsidy.cpp`).

use crate::GameState;
use crate::cargo::CargoType;
use crate::company::CompanyId;
use crate::economy::{TICKS_PER_MONTH, TICKS_PER_YEAR};
use crate::map::TileCoord;
use crate::sim_events::SimEvent;
use crate::station::{self, STATION_COVERAGE_RADIUS};

/// Multiplicador de pago mientras el subsidio está activo (`difficulty.subsidy_multiplier` = 2).
pub const SUBSIDY_PAYMENT_MULTIPLIER: i64 = 2;
/// Meses de validez de la oferta antes de caducar.
pub const SUBSIDY_OFFER_MONTHS: u32 = 12;
/// Años de bonificación tras adjudicar el subsidio.
pub const SUBSIDY_AWARDED_YEARS: u32 = 1;

/// Subsidio activo u ofrecido.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subsidy {
    pub id: u32,
    pub cargo: CargoType,
    pub source_industry_pos: TileCoord,
    pub dest_station_pos: TileCoord,
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
    ) -> bool {
        if self.cargo != cargo || self.dest_station_pos != dest_station {
            return false;
        }
        if self.awarded {
            return self.is_award_active(tick)
                && station::industry_in_station_coverage_by_pos(
                    self.source_industry_pos,
                    dest_station,
                    STATION_COVERAGE_RADIUS,
                )
                && (source == self.source_industry_pos
                    || station::industry_in_station_coverage_by_pos(
                        self.source_industry_pos,
                        source,
                        STATION_COVERAGE_RADIUS,
                    ));
        }
        self.is_offer_active(tick)
            && (source == self.source_industry_pos
                || station::industry_in_station_coverage_by_pos(
                    self.source_industry_pos,
                    source,
                    STATION_COVERAGE_RADIUS,
                ))
    }
}

/// Purga ofertas caducadas y genera nuevas periódicamente.
pub fn tick_subsidies(state: &mut GameState) {
    let tick = state.tick.get();
    state
        .subsidies
        .retain(|s| s.awarded || s.is_offer_active(tick));

    if tick > 0 && tick.is_multiple_of(TICKS_PER_MONTH * 8) {
        let _ = try_create_subsidy(state);
    }
}

/// Intenta crear un subsidio industria → estación para el cargo primario.
#[must_use]
pub fn try_create_subsidy(state: &mut GameState) -> bool {
    if state.industries.is_empty() || state.stations.is_empty() {
        return false;
    }

    let tick = state.tick.get();
    let industry_idx =
        (usize::try_from(tick).unwrap_or(0) + state.industries.len()) % state.industries.len();
    let industry = &state.industries[industry_idx];
    let cargo = industry.output_cargo();
    if cargo.is_town_cargo() {
        return false;
    }

    let stations: Vec<(usize, TileCoord)> = state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, st)| {
            !st.is_waypoint()
                && st.accepts_cargo(cargo)
                && station::industry_in_station_coverage(industry, st.pos, STATION_COVERAGE_RADIUS)
                && st.pos != industry.pos
        })
        .map(|(i, st)| (i, st.pos))
        .collect();

    if stations.is_empty() {
        return false;
    }

    let (station_idx, dest) = stations[usize::try_from(tick / 17).unwrap_or(0) % stations.len()];
    let source = industry.pos;

    if state.subsidies.iter().any(|s| {
        !s.awarded
            && s.cargo == cargo
            && s.source_industry_pos == source
            && s.dest_station_pos == dest
    }) {
        return false;
    }

    let id = state.next_subsidy_id;
    state.next_subsidy_id = state.next_subsidy_id.saturating_add(1);
    let offer_expires_tick = tick.saturating_add(u64::from(SUBSIDY_OFFER_MONTHS) * TICKS_PER_MONTH);
    state.subsidies.push(Subsidy {
        id,
        cargo,
        source_industry_pos: source,
        dest_station_pos: dest,
        offer_expires_tick,
        awarded: false,
        award_expires_tick: 0,
        awarded_company: None,
    });
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::SubsidyCreated {
            industry_pos: source,
            station_pos: dest,
            cargo,
        });
    crate::news::push_subsidy_offer_news(state, cargo, source, dest);
    let _ = station_idx;
    true
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
    let Some(idx) = state.subsidies.iter().position(|s| {
        !s.awarded
            && s.matches_delivery(cargo, dest_station, source, tick)
            && s.is_offer_active(tick)
    }) else {
        return false;
    };

    let award_expires_tick = tick.saturating_add(u64::from(SUBSIDY_AWARDED_YEARS) * TICKS_PER_YEAR);
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

/// Multiplicador de ingreso por entrega (`1` o [`SUBSIDY_PAYMENT_MULTIPLIER`]).
///
/// Solo la compañía adjudicada recibe el ×2.
#[must_use]
pub fn delivery_income_multiplier(
    state: &GameState,
    dest_station: TileCoord,
    cargo: CargoType,
    source: TileCoord,
    company: CompanyId,
) -> i64 {
    let tick = state.tick.get();
    if state.subsidies.iter().any(|s| {
        s.awarded
            && s.awarded_company == Some(company)
            && s.is_award_active(tick)
            && s.matches_delivery(cargo, dest_station, source, tick)
    }) {
        SUBSIDY_PAYMENT_MULTIPLIER
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
    fn award_on_first_delivery_doubles_income_for_winner() {
        let mut state = setup_subsidy_route();
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
        assert_eq!(state.subsidies[0].awarded_company, Some(CompanyId::PLAYER));
        assert_eq!(
            delivery_income_multiplier(&state, dest, CargoType::Coal, source, CompanyId::PLAYER),
            SUBSIDY_PAYMENT_MULTIPLIER
        );
        assert_eq!(
            delivery_income_multiplier(&state, dest, CargoType::Coal, source, CompanyId(1)),
            1
        );
    }

    #[test]
    fn periodic_tick_creates_subsidy() {
        let mut state = setup_subsidy_route();
        state.tick = crate::GameTick::new(TICKS_PER_MONTH * 8);
        tick_subsidies(&mut state);
        assert!(!state.subsidies.is_empty());
    }
}
