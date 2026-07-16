//! Pagos de transporte inspirados en `GetTransportedGoodsIncome` de `OpenTTD` (Camino B).

pub mod build_costs;
pub mod payments;
pub mod time;
pub mod vehicle_costs;

pub use build_costs::{build_object_cost, buy_land_cost, terraform_cost_per_corner};
pub use payments::{
    ANNUAL_INTEREST_RATE_PCT, CargoPaymentSpec, DEFAULT_MAX_LOAN, LOAN_INTERVAL, cargo_time_factor,
    check_bankruptcy, decrease_loan, increase_loan, inflation_income_factor,
    inflation_prices_factor, manhattan_distance, monthly_loan_interest, transported_goods_income,
};
pub use time::{
    OTTD_MILLISECONDS_PER_TICK, SIM_TICKS_PER_SECOND, TICKS_PER_MONTH, TICKS_PER_TRANSIT_DAY,
    TICKS_PER_YEAR, ticks_to_transit_days,
};
pub use vehicle_costs::{
    vehicle_purchase_cost, vehicle_running_cost_per_tick, vehicle_sell_refund,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::map::TileCoord;

    #[test]
    fn coal_pays_more_than_passengers_on_same_route() {
        let dist = 12;
        let days = 3;
        let count = 10;
        let coal = transported_goods_income(count, dist, days, CargoType::Coal, 0);
        let pax = transported_goods_income(count, dist, days, CargoType::Passengers, 0);
        assert!(coal > pax, "carbón {coal} vs pasajeros {pax}");
    }

    #[test]
    fn longer_distance_pays_more() {
        let short = transported_goods_income(5, 4, 2, CargoType::Wood, 0);
        let long = transported_goods_income(5, 20, 2, CargoType::Wood, 0);
        assert!(long > short);
    }

    #[test]
    fn slow_transit_reduces_income() {
        let fast = transported_goods_income(8, 10, 2, CargoType::Goods, 0);
        let slow = transported_goods_income(8, 10, 80, CargoType::Goods, 0);
        assert!(fast > slow);
    }

    #[test]
    fn packet_transit_days_match_payment_formula() {
        let young = transported_goods_income(4, 12, 1, CargoType::Coal, 0);
        let aged = transported_goods_income(4, 12, 40, CargoType::Coal, 0);
        assert!(young > aged, "packet joven {young} vs envejecido {aged}");
    }

    #[test]
    fn inflation_increases_income_over_years() {
        let base = transported_goods_income(10, 8, 4, CargoType::Coal, 0);
        let later = transported_goods_income(10, 8, 4, CargoType::Coal, TICKS_PER_YEAR * 10);
        assert!(later > base);
    }

    #[test]
    fn inflation_increases_terraform_cost_over_years() {
        let base = terraform_cost_per_corner(0);
        let later = terraform_cost_per_corner(TICKS_PER_YEAR * 20);
        assert!(later > base);
    }

    #[test]
    fn price_inflation_grows_slower_than_income_inflation() {
        let income = inflation_income_factor(TICKS_PER_YEAR * 50);
        let prices = inflation_prices_factor(TICKS_PER_YEAR * 50);
        assert!(income > prices);
    }

    #[test]
    fn manhattan_distance_matches_expectation() {
        let a = TileCoord::new(3, 2);
        let b = TileCoord::new(6, 2);
        assert_eq!(manhattan_distance(a, b), 3);
    }

    #[test]
    fn monthly_interest_on_100k_loan() {
        let interest = monthly_loan_interest(100_000);
        assert_eq!(interest, 833);
    }

    #[test]
    fn bankruptcy_when_debt_exceeds_max_loan() {
        assert!(!check_bankruptcy(-200_000, 300_000));
        assert!(check_bankruptcy(-300_001, 300_000));
    }

    #[test]
    fn increase_and_decrease_loan() {
        let mut economy = crate::game_state::CompanyEconomy {
            money: 50_000,
            loan: 0,
            max_loan: DEFAULT_MAX_LOAN,
        };
        let added = increase_loan(&mut economy).expect("increase loan");
        assert_eq!(added, LOAN_INTERVAL);
        assert_eq!(economy.loan, LOAN_INTERVAL);
        assert_eq!(economy.money, 50_000 + LOAN_INTERVAL);
        decrease_loan(&mut economy).expect("decrease loan");
        assert_eq!(economy.loan, 0);
        assert_eq!(economy.money, 50_000);
    }

    #[test]
    fn increase_loan_fails_at_maximum() {
        let mut economy = crate::game_state::CompanyEconomy {
            money: 0,
            loan: DEFAULT_MAX_LOAN - 5_000,
            max_loan: DEFAULT_MAX_LOAN,
        };
        assert!(matches!(
            increase_loan(&mut economy),
            Err(crate::command::CommandError::LoanAtMaximum)
        ));
    }
}
