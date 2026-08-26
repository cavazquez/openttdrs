//! Valoración trimestral de compañía (`CompaniesGenStatistics` / `UpdateCompanyRatingAndValue`).

use crate::cargo::ALL_CARGO_TYPES;
use crate::company::CompanyId;
use crate::economy::{get_price, pricebase::PriceIndex, vehicle_asset_value_with_catalog};
use crate::game_state::GameState;
use crate::station::{Station, StopKind};
use crate::vehicle::VehicleKind;

/// Trimestres retenidos (`OpenTTD` `MAX_HISTORY_QUARTERS`).
pub const ECONOMY_HISTORY_QUARTERS: usize = 24;
/// Slots de carga de `CompanyEconomyEntry::delivered_cargo` en saves modernos.
pub const QUARTERLY_CARGO_SLOTS: usize = 64;

/// Componentes de `_score_info` (`economy.cpp:91-102`).
#[derive(Debug, Clone, Copy)]
struct ScoreInfo {
    score: i32,
    needed: i64,
}

const SCORE_VEHICLES: ScoreInfo = ScoreInfo {
    score: 120,
    needed: 100,
};
const SCORE_STATIONS: ScoreInfo = ScoreInfo {
    score: 80,
    needed: 100,
};
const SCORE_MIN_PROFIT: ScoreInfo = ScoreInfo {
    score: 100,
    needed: 10_000,
};
const SCORE_MIN_INCOME: ScoreInfo = ScoreInfo {
    score: 50,
    needed: 50_000,
};
const SCORE_MAX_INCOME: ScoreInfo = ScoreInfo {
    score: 100,
    needed: 100_000,
};
const SCORE_DELIVERED: ScoreInfo = ScoreInfo {
    score: 400,
    needed: 40_000,
};
const SCORE_CARGO: ScoreInfo = ScoreInfo {
    score: 50,
    needed: 8,
};
const SCORE_MONEY: ScoreInfo = ScoreInfo {
    score: 50,
    needed: 10_000_000,
};
const SCORE_LOAN: ScoreInfo = ScoreInfo {
    score: 50,
    needed: 250_000,
};
const SCORE_MAX: i32 = 1000;

/// Entrada de un trimestre cerrado.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuarterlyEconomyEntry {
    pub income: u64,
    /// Costes absolutos del core; `PLYR` los serializa como `Money` negativo.
    pub expenses: u64,
    pub deliveries: u64,
    /// Entregas por `CargoID` de `OpenTTD`. El runtime sólo calcula el total,
    /// pero el vector conserva el desglose de un `.sav` importado.
    #[serde(default)]
    pub delivered_cargo: Vec<u32>,
    /// Rating 0..=1000 (`performance_history`).
    pub performance_history: i32,
    /// Valoración con activos (`CalculateCompanyValue`).
    pub company_value: i64,
}

/// Acumulador del trimestre en curso + historial.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuarterlyEconomyHistory {
    pub samples: Vec<QuarterlyEconomyEntry>,
    /// Meses acumulados en el trimestre actual (0..2).
    #[serde(default)]
    pub months_in_quarter: u8,
    #[serde(default)]
    pub cur_income: u64,
    /// Costes absolutos del trimestre abierto; el wire format usa signo negativo.
    #[serde(default)]
    pub cur_expenses: u64,
    #[serde(default)]
    pub cur_deliveries: u64,
    /// Desglose de carga del trimestre abierto importado desde `PLYR`.
    #[serde(default)]
    pub cur_delivered_cargo: Vec<u32>,
    /// Valores publicados por `OpenTTD` para el trimestre todavía abierto.
    #[serde(default)]
    pub cur_company_value: i64,
    #[serde(default)]
    pub cur_performance_history: i32,
}

impl QuarterlyEconomyHistory {
    /// Incorpora el cierre de un mes; cada 3 meses publica un trimestre.
    pub fn push_month(
        &mut self,
        month_income: u64,
        month_expenses: u64,
        month_deliveries: u64,
        performance: i32,
        company_value: i64,
    ) {
        self.cur_income = self.cur_income.saturating_add(month_income);
        self.cur_expenses = self.cur_expenses.saturating_add(month_expenses);
        self.cur_deliveries = self.cur_deliveries.saturating_add(month_deliveries);
        add_fallback_deliveries(&mut self.cur_delivered_cargo, month_deliveries);
        self.months_in_quarter = self.months_in_quarter.saturating_add(1);
        if self.months_in_quarter < 3 {
            return;
        }
        let entry = QuarterlyEconomyEntry {
            income: self.cur_income,
            expenses: self.cur_expenses,
            deliveries: self.cur_deliveries,
            delivered_cargo: std::mem::take(&mut self.cur_delivered_cargo),
            performance_history: performance.clamp(0, SCORE_MAX),
            company_value,
        };
        self.samples.push(entry);
        if self.samples.len() > ECONOMY_HISTORY_QUARTERS {
            let drop = self.samples.len() - ECONOMY_HISTORY_QUARTERS;
            self.samples.drain(0..drop);
        }
        self.cur_income = 0;
        self.cur_expenses = 0;
        self.cur_deliveries = 0;
        self.cur_company_value = 0;
        self.cur_performance_history = 0;
        self.months_in_quarter = 0;
    }
}

/// Suma segura del desglose de carga serializado por `OpenTTD`.
#[must_use]
pub(crate) fn delivered_cargo_total(delivered_cargo: &[u32]) -> u64 {
    delivered_cargo.iter().fold(0_u64, |total, &value| {
        total.saturating_add(u64::from(value))
    })
}

/// Normaliza un desglose para el array de 64 slots que espera un `.sav` moderno.
///
/// Si el runtime sólo conoce el total, lo distribuye en slots libres sin perder
/// el agregado. Los datos importados por slot siempre tienen prioridad.
#[must_use]
pub(crate) fn delivered_cargo_for_save(delivered_cargo: &[u32], total: u64) -> Vec<u32> {
    let mut slots = delivered_cargo
        .iter()
        .copied()
        .take(QUARTERLY_CARGO_SLOTS)
        .collect::<Vec<_>>();
    slots.resize(QUARTERLY_CARGO_SLOTS, 0);
    let missing = total.saturating_sub(delivered_cargo_total(&slots));
    add_fallback_deliveries(&mut slots, missing);
    slots
}

/// Añade entregas sin tipo conocido a slots con capacidad libre.
fn add_fallback_deliveries(delivered_cargo: &mut Vec<u32>, deliveries: u64) {
    if deliveries == 0 {
        return;
    }
    delivered_cargo.resize(QUARTERLY_CARGO_SLOTS, 0);
    let mut pending = deliveries;
    for slot in delivered_cargo {
        let room = u64::from(u32::MAX.saturating_sub(*slot));
        let increment = pending.min(room);
        *slot = slot.saturating_add(u32::try_from(increment).unwrap_or(u32::MAX));
        pending = pending.saturating_sub(increment);
        if pending == 0 {
            break;
        }
    }
}

fn score_component(part: i64, info: ScoreInfo) -> i32 {
    let clamped = part.clamp(0, info.needed);
    i32::try_from(clamped * i64::from(info.score) / info.needed).unwrap_or(0)
}

/// Instalaciones de estación (`facilities.Count()` simplificado).
#[must_use]
pub fn station_facility_count(station: &Station) -> u32 {
    match station.stop_kind {
        StopKind::RailWaypoint | StopKind::RoadWaypoint | StopKind::Buoy => 0,
        StopKind::Airport => u32::try_from(station.airport_tiles.len().max(1)).unwrap_or(1),
        _ => 1_u32.saturating_add(u32::try_from(station.joined_tiles.len()).unwrap_or(0)),
    }
}

fn station_recently_served(station: &Station) -> bool {
    ALL_CARGO_TYPES
        .iter()
        .any(|cargo| station.time_since_pickup.get(*cargo) <= 20)
}

/// `CalculateCompanyAssetValue` + patrimonio (`economy.cpp:115-158`).
#[must_use]
pub fn calculate_company_value(state: &GameState, company_id: CompanyId) -> i64 {
    let Some(company) = state.companies.get(company_id.index()) else {
        return 0;
    };
    let station_value = get_price(&state.global_economy, PriceIndex::StationValue, 1, 0);
    let facilities: u64 = state
        .stations
        .iter()
        .filter(|s| s.owner == company_id)
        .map(|s| u64::from(station_facility_count(s)))
        .sum();
    let station_assets = i64::try_from(facilities)
        .unwrap_or(i64::MAX)
        .saturating_mul(station_value)
        .saturating_mul(25);

    let mut vehicle_assets = 0_i64;
    for v in &state.vehicles {
        if v.owner != company_id || v.is_wagon_unit() {
            continue;
        }
        if matches!(
            v.kind,
            VehicleKind::Train
                | VehicleKind::Bus
                | VehicleKind::Truck
                | VehicleKind::Tram
                | VehicleKind::Ship
                | VehicleKind::Aircraft
        ) {
            vehicle_assets = vehicle_assets.saturating_add(
                vehicle_asset_value_with_catalog(v, &state.engine_catalog).saturating_mul(3) / 2,
            );
        }
    }

    let liquid = company.economy.money.saturating_sub(company.economy.loan);
    liquid
        .saturating_add(station_assets)
        .saturating_add(vehicle_assets)
        .max(1)
}

/// `_score_part` simplificado → 0..=1000 (`economy.cpp:202-314`).
#[must_use]
pub fn calculate_performance_rating(
    state: &GameState,
    company_id: CompanyId,
    quarter_deliveries: u64,
) -> i32 {
    let Some(company) = state.companies.get(company_id.index()) else {
        return 0;
    };

    let profitable_vehicles = state
        .vehicles
        .iter()
        .filter(|v| {
            v.owner == company_id
                && v.is_consist_head()
                && (v.profit_last_year > 0 || v.profit_this_year > 0)
        })
        .count();
    let active_stations = state
        .stations
        .iter()
        .filter(|s| s.owner == company_id && station_recently_served(s))
        .map(|s| u64::from(station_facility_count(s)))
        .sum::<u64>();

    let mut min_profit = 0_i64;
    let mut min_profit_set = false;
    for v in &state.vehicles {
        if v.owner != company_id || !v.is_consist_head() {
            continue;
        }
        if v.profit_last_year > 0 && (!min_profit_set || v.profit_last_year < min_profit) {
            min_profit = v.profit_last_year;
            min_profit_set = true;
        }
    }
    let min_profit_score = if min_profit > 0 { min_profit >> 8 } else { 0 };

    let recent = company
        .economy_history
        .samples
        .iter()
        .rev()
        .take(12)
        .map(|m| m.income.cast_signed().saturating_add(m.operating_profit()))
        .collect::<Vec<_>>();
    let (min_income, max_income) = if recent.is_empty() {
        (0, 0)
    } else {
        let min_v = recent.iter().copied().min().unwrap_or(0);
        let max_v = recent.iter().copied().max().unwrap_or(0);
        (min_v.max(0), max_v)
    };

    let delivered = i64::try_from(quarter_deliveries.min(40_000)).unwrap_or(i64::MAX);
    let cargo_variety = i64::from(quarter_deliveries > 0);
    let money = company.economy.money.max(0);
    let loan_headroom = SCORE_LOAN.needed.saturating_sub(company.economy.loan);

    let mut score = 0_i32;
    score += score_component(
        i64::try_from(profitable_vehicles).unwrap_or(0),
        SCORE_VEHICLES,
    );
    score += score_component(active_stations.cast_signed(), SCORE_STATIONS);
    score += score_component(min_profit_score, SCORE_MIN_PROFIT);
    score += score_component(min_income, SCORE_MIN_INCOME);
    score += score_component(max_income, SCORE_MAX_INCOME);
    score += score_component(delivered, SCORE_DELIVERED);
    score += score_component(cargo_variety, SCORE_CARGO);
    score += score_component(money, SCORE_MONEY);
    score += score_component(loan_headroom, SCORE_LOAN);
    score.clamp(0, SCORE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::company::CompanyId;
    use crate::game_state::{GameState, company_net_value};
    use crate::map::TileCoord;
    use crate::station::{Station, StopKind};
    use crate::vehicle::{Vehicle, VehicleKind};

    #[test]
    fn quarterly_history_publishes_every_three_months() {
        let mut q = QuarterlyEconomyHistory::default();
        q.push_month(10, 1, 1, 100, 50_000);
        q.push_month(20, 2, 2, 200, 60_000);
        assert!(q.samples.is_empty());
        q.push_month(30, 3, 3, 300, 70_000);
        assert_eq!(q.samples.len(), 1);
        assert_eq!(q.samples[0].income, 60);
        assert_eq!(q.samples[0].expenses, 6);
        assert_eq!(q.samples[0].deliveries, 6);
        assert_eq!(q.samples[0].performance_history, 300);
        assert_eq!(q.samples[0].company_value, 70_000);
        assert_eq!(q.months_in_quarter, 0);
    }

    #[test]
    fn company_value_uses_station_value_times_facilities() {
        let mut state = GameState::new(8, 8);
        state.ensure_companies();
        let money = state.companies[0].economy.money;
        let loan = state.companies[0].economy.loan;
        let liquid = company_net_value(money, loan);
        let mut st = Station::new_with_kind(TileCoord::new(2, 2), StopKind::RailStation);
        st.owner = CompanyId::PLAYER;
        state.stations.push(st);
        let mut train = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(3, 3),
        );
        train.owner = CompanyId::PLAYER;
        state.vehicles.push(train);
        let value = calculate_company_value(&state, CompanyId::PLAYER);
        let station_part = 100 * 25;
        assert!(value >= liquid + station_part);
    }

    #[test]
    fn performance_rating_includes_profit_and_stations() {
        let mut state = GameState::new(8, 8);
        state.ensure_companies();
        let mut train = Vehicle::new(
            2,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        train.owner = CompanyId::PLAYER;
        train.profit_this_year = 5_000;
        state.vehicles.push(train);
        let mut st = Station::new_with_kind(TileCoord::new(1, 1), StopKind::BusStop);
        st.owner = CompanyId::PLAYER;
        st.time_since_pickup.passengers = 5;
        state.stations.push(st);
        let rating = calculate_performance_rating(&state, CompanyId::PLAYER, 1_000);
        assert!(rating > 0);
        assert!(rating <= 1000);
    }
}
