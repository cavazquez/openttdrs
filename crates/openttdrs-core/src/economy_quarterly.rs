//! Valoración trimestral de compañía (`CompaniesGenStatistics` / `UpdateCompanyRatingAndValue`).

use crate::company::CompanyId;
use crate::economy::vehicle_purchase_cost;
use crate::game_state::{GameState, STATION_BUILD_COST, company_net_value};
use crate::vehicle::VehicleKind;

/// Trimestres retenidos (`OpenTTD` `MAX_HISTORY_QUARTERS`).
pub const ECONOMY_HISTORY_QUARTERS: usize = 24;

/// Entrada de un trimestre cerrado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuarterlyEconomyEntry {
    pub income: u64,
    pub expenses: u64,
    pub deliveries: u64,
    /// Rating 0..=1000 (`performance_history`).
    pub performance_history: i32,
    /// Valoración con activos (`CalculateCompanyValue` simplificado).
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
    #[serde(default)]
    pub cur_expenses: u64,
    #[serde(default)]
    pub cur_deliveries: u64,
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
        self.months_in_quarter = self.months_in_quarter.saturating_add(1);
        if self.months_in_quarter < 3 {
            return;
        }
        let entry = QuarterlyEconomyEntry {
            income: self.cur_income,
            expenses: self.cur_expenses,
            deliveries: self.cur_deliveries,
            performance_history: performance.clamp(0, 1000),
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
        self.months_in_quarter = 0;
    }
}

/// Valoración con activos: patrimonio líquido + estaciones×coste + vehículos×1.5×precio.
#[must_use]
pub fn calculate_company_value(state: &GameState, company_id: CompanyId) -> i64 {
    let Some(company) = state.companies.get(company_id.index()) else {
        return 0;
    };
    let liquid = company_net_value(company.economy.money, company.economy.loan);
    let stations = state
        .stations
        .iter()
        .filter(|s| s.owner == company_id)
        .count();
    let station_assets = STATION_BUILD_COST.saturating_mul(i64::try_from(stations).unwrap_or(0));
    let mut vehicle_assets = 0_i64;
    for v in &state.vehicles {
        if v.owner != company_id || v.is_wagon_unit() {
            continue;
        }
        let price = if matches!(v.kind, VehicleKind::Train) {
            v.effective_engine().price
        } else {
            vehicle_purchase_cost(v.kind)
        };
        vehicle_assets = vehicle_assets.saturating_add(price.saturating_mul(3) / 2);
    }
    liquid
        .saturating_add(station_assets)
        .saturating_add(vehicle_assets)
        .max(1)
}

/// Rating 0..=1000 a partir de flota, estaciones, entregas del trimestre y liquidez.
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
        .filter(|s| s.owner == company_id && s.income > 0)
        .count();
    let money_score = (company.economy.money.max(0) / 5_000).min(200);
    let loan_headroom = (company
        .economy
        .max_loan
        .saturating_sub(company.economy.loan)
        / 2_000)
        .clamp(0, 150);
    let vehicle_score = i64::try_from(profitable_vehicles.saturating_mul(40)).unwrap_or(0);
    let station_score = i64::try_from(active_stations.saturating_mul(30)).unwrap_or(0);
    let delivery_score = i64::try_from(quarter_deliveries.min(250)).unwrap_or(0);
    let total = vehicle_score
        .saturating_add(station_score)
        .saturating_add(delivery_score)
        .saturating_add(money_score)
        .saturating_add(loan_headroom);
    i32::try_from(total.clamp(0, 1000)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::company::CompanyId;
    use crate::game_state::GameState;
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
    fn company_value_includes_station_and_vehicle_assets() {
        let mut state = GameState::new(8, 8);
        state.ensure_companies();
        let money = state.companies[0].economy.money;
        let loan = state.companies[0].economy.loan;
        let liquid = company_net_value(money, loan);
        let mut st = Station::new_with_kind(TileCoord::new(2, 2), StopKind::RailStation);
        st.owner = CompanyId::PLAYER;
        st.income = 1;
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
        assert!(value > liquid);
    }
}
