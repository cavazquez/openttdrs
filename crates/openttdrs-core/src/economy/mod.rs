//! Pagos de transporte inspirados en `GetTransportedGoodsIncome` de `OpenTTD` (Camino B).

pub mod build_costs;
pub mod global;
pub mod payments;
pub mod pricebase;
pub mod time;
pub mod vehicle_costs;

pub use build_costs::{
    build_object_cost, build_object_cost_factored, buy_land_cost, rail_build_cost,
    rail_build_cost_factored, road_build_cost, road_build_cost_factored, station_build_cost,
    terraform_cost_per_corner, terraform_cost_per_corner_inflated, waypoint_build_cost,
};
pub use global::{
    DEFAULT_DIFFICULTY_MOD, DEFAULT_INTEREST_RATE, FluctuationEvent, GlobalEconomy,
    INFLATION_FRAC_ONE, MAX_INFLATION, ORIGINAL_BASE_YEAR, ORIGINAL_MAX_YEAR,
};
pub use payments::{
    ANNUAL_INTEREST_RATE_PCT, CargoPaymentSpec, DEFAULT_MAX_LOAN, LOAN_INTERVAL,
    cargo_current_payment, cargo_time_factor, check_bankruptcy, decrease_loan, increase_loan,
    inflation_income_factor, inflation_prices_factor, manhattan_distance, monthly_company_interest,
    monthly_loan_interest, monthly_loan_interest_with_rate, monthly_station_maintenance_fee,
    transported_goods_income, transported_goods_income_for_climate,
    transported_goods_income_with_spec,
};
pub use pricebase::{PriceIndex, base_price_at, get_price, medium_default_price};
pub use time::{
    CARGO_AGING_TICKS, OTTD_MILLISECONDS_PER_TICK, SIM_TICKS_PER_SECOND, STATION_RATING_TICKS,
    TICKS_PER_DAY, TICKS_PER_MONTH, TICKS_PER_YEAR, calendar_month_index, ticks_to_transit_periods,
};
pub use vehicle_costs::{
    YEAR_TICKS, accumulate_running_cost_for_head, accumulate_vehicle_running_cost,
    consist_running_cost_year, engine_running_cost_from_price_base, engine_running_cost_year,
    vehicle_asset_value, vehicle_counts_running_tick, vehicle_purchase_cost,
    vehicle_running_cost_per_tick, vehicle_sell_refund,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::economy::global::{GlobalEconomy, INFLATION_FRAC_ONE};
    use crate::linkgraph_parity::Randomizer;
    use crate::map::TileCoord;
    use crate::news::CALENDAR_BASE_YEAR;

    fn base_inflation_payment() -> u64 {
        let mut ge = GlobalEconomy::new();
        ge.startup(&mut Randomizer::new(42), CALENDAR_BASE_YEAR);
        ge.inflation_payment
    }

    #[test]
    fn coal_pays_more_than_passengers_on_same_route() {
        let inflation = base_inflation_payment();
        let dist = 12;
        let days = 3;
        let count = 10;
        let coal = transported_goods_income(count, dist, days, CargoType::Coal, inflation);
        let pax = transported_goods_income(count, dist, days, CargoType::Passengers, inflation);
        assert!(coal > pax, "carbón {coal} vs pasajeros {pax}");
    }

    #[test]
    fn longer_distance_pays_more() {
        let inflation = base_inflation_payment();
        let short = transported_goods_income(5, 4, 2, CargoType::Wood, inflation);
        let long = transported_goods_income(5, 20, 2, CargoType::Wood, inflation);
        assert!(long > short);
    }

    #[test]
    fn slow_transit_reduces_income() {
        let inflation = base_inflation_payment();
        let fast = transported_goods_income(8, 10, 2, CargoType::Goods, inflation);
        let slow = transported_goods_income(8, 10, 80, CargoType::Goods, inflation);
        assert!(fast > slow);
    }

    #[test]
    fn packet_transit_days_match_payment_formula() {
        let inflation = base_inflation_payment();
        let young = transported_goods_income(4, 12, 1, CargoType::Coal, inflation);
        let aged = transported_goods_income(4, 12, 40, CargoType::Coal, inflation);
        assert!(young > aged, "packet joven {young} vs envejecido {aged}");
    }

    #[test]
    fn inflation_increases_income_over_months() {
        let mut ge = GlobalEconomy::new();
        let base_payment = ge.inflation_payment;
        for _ in 0..24 {
            ge.add_monthly_inflation(CALENDAR_BASE_YEAR, true);
        }
        let base = transported_goods_income(10, 8, 4, CargoType::Coal, base_payment);
        let later = transported_goods_income(10, 8, 4, CargoType::Coal, ge.inflation_payment);
        assert!(later > base);
    }

    #[test]
    fn inflation_increases_terraform_cost_over_months() {
        let mut ge = GlobalEconomy::new();
        let base_prices = ge.inflation_prices;
        for _ in 0..24 {
            ge.add_monthly_inflation(CALENDAR_BASE_YEAR, true);
        }
        let base_ge = GlobalEconomy {
            inflation_prices: base_prices,
            ..GlobalEconomy::new()
        };
        let base = terraform_cost_per_corner(&base_ge);
        let later = terraform_cost_per_corner(&ge);
        assert!(later > base);
    }

    #[test]
    fn payment_inflation_grows_slower_than_price_inflation() {
        let mut ge = GlobalEconomy::new();
        for _ in 0..120 {
            ge.add_monthly_inflation(CALENDAR_BASE_YEAR, true);
        }
        let income = inflation_income_factor(ge.inflation_payment);
        let prices = inflation_prices_factor(ge.inflation_prices);
        assert!(prices > income, "precios {prices} vs pagos {income}");
    }

    #[test]
    fn compound_inflation_matches_openttd_monthly_step() {
        let mut ge = GlobalEconomy::new();
        let before = ge.inflation_prices;
        ge.add_monthly_inflation(CALENDAR_BASE_YEAR, true);
        let expected = before + (before * u64::from(ge.infl_amount) * 54) / (1 << 16);
        assert_eq!(ge.inflation_prices, expected);
    }

    #[test]
    fn inflation_stops_outside_original_year_window() {
        let mut ge = GlobalEconomy::new();
        let before = ge.inflation_prices;
        ge.add_monthly_inflation(1919, true);
        assert_eq!(ge.inflation_prices, before);
        ge.add_monthly_inflation(2090, true);
        assert_eq!(ge.inflation_prices, before);
    }

    #[test]
    fn max_loan_scales_with_inflation_prices() {
        let mut ge = GlobalEconomy::new();
        let base = ge.scaled_max_loan();
        for _ in 0..120 {
            ge.add_monthly_inflation(CALENDAR_BASE_YEAR, true);
        }
        assert!(ge.scaled_max_loan() > base);
        assert_eq!(ge.scaled_max_loan() % LOAN_INTERVAL, 0);
    }

    #[test]
    fn asymptotic_time_factor_for_very_long_transit() {
        let spec = CargoType::Passengers.payment_spec();
        let (medium, medium_asym) = cargo_time_factor(40, spec);
        let (very_long, is_asymptotic) = cargo_time_factor(600, spec);
        assert!(!medium_asym);
        assert!(is_asymptotic);
        assert!(very_long >= 1);
        assert!(very_long < medium);
        assert!(
            very_long < 31,
            "la rama asintótica cae por debajo del suelo fijo antiguo"
        );
    }

    #[test]
    fn recession_cycle_emits_fluctuation_events() {
        let mut ge = GlobalEconomy::new();
        ge.recessions_enabled = true;
        ge.fluct = 1;
        let mut rng = Randomizer::new(7);
        assert_eq!(
            ge.handle_monthly_fluctuations(&mut rng),
            Some(crate::economy::FluctuationEvent::RecessionStart)
        );
        assert!(ge.is_in_recession());

        ge.recessions_enabled = false;
        ge.fluct = -5;
        assert_eq!(
            ge.handle_monthly_fluctuations(&mut rng),
            Some(crate::economy::FluctuationEvent::RecessionEnd)
        );
        assert!(!ge.is_in_recession());
    }

    #[test]
    fn startup_applies_pre_1950_inflation() {
        let mut ge = GlobalEconomy::new();
        ge.startup(&mut Randomizer::new(1), CALENDAR_BASE_YEAR);
        assert!(ge.inflation_prices > INFLATION_FRAC_ONE);
        assert!(ge.inflation_payment > INFLATION_FRAC_ONE);
        assert!(ge.inflation_prices > ge.inflation_payment);
    }

    #[test]
    fn manhattan_distance_matches_expectation() {
        let a = TileCoord::new(3, 2);
        let b = TileCoord::new(6, 2);
        assert_eq!(manhattan_distance(a, b), 3);
    }

    #[test]
    fn monthly_interest_on_100k_loan() {
        let interest = monthly_company_interest(100_000, 0, 10, 0);
        assert_eq!(interest, 833);
    }

    #[test]
    fn monthly_interest_includes_negative_cash() {
        let loan_only = monthly_company_interest(0, -100_000, 10, 0);
        assert_eq!(loan_only, 833);
        let combined = monthly_company_interest(100_000, -50_000, 10, 0);
        assert_eq!(combined, 1250);
    }

    #[test]
    fn monthly_station_maintenance_uses_station_value() {
        let ge = GlobalEconomy::new();
        assert_eq!(monthly_station_maintenance_fee(&ge), 25);
    }

    #[test]
    fn bankruptcy_when_debt_exceeds_max_loan() {
        assert!(!check_bankruptcy(-200_000, 0, 300_000));
        assert!(check_bankruptcy(-300_001, 0, 300_000));
    }

    /// `OpenTTD` resta el préstamo pendiente: con caja positiva pero deuda por encima del
    /// techo, `CompanyCheckBankrupt` (`economy.cpp:556`) sí liquida la compañía.
    #[test]
    fn bankruptcy_counts_outstanding_loan() {
        assert!(check_bankruptcy(50_000, 400_000, 300_000));
        assert!(!check_bankruptcy(50_000, 300_000, 300_000));
        assert!(!check_bankruptcy(0, 300_000, 300_000));
        assert!(check_bankruptcy(-1, 300_000, 300_000));
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
