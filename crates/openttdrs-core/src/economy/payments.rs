//! Pagos de transporte inspirados en `GetTransportedGoodsIncome` de `OpenTTD` (Camino B).

use crate::cargo::CargoType;
use crate::map::TileCoord;

use super::global::GlobalEconomy;

/// Tasas base de `OpenTTD` (`cargo_const.h`), sin inflación.
#[derive(Debug, Clone, Copy)]
pub struct CargoPaymentSpec {
    /// `CargoSpec::initial_payment`.
    pub base_rate: i32,
    /// `transit_periods[0]`: entrega rápida sin penalización.
    pub transit_fast_days: u16,
    /// `transit_periods[1]`: tramo lineal extra antes del mínimo.
    pub transit_slow_days: u16,
}

impl CargoType {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn payment_spec(self) -> CargoPaymentSpec {
        match self {
            Self::Passengers => CargoPaymentSpec {
                base_rate: 3185,
                transit_fast_days: 0,
                transit_slow_days: 24,
            },
            Self::Mail => CargoPaymentSpec {
                base_rate: 4550,
                transit_fast_days: 20,
                transit_slow_days: 90,
            },
            Self::Coal => CargoPaymentSpec {
                base_rate: 5916,
                transit_fast_days: 7,
                transit_slow_days: 255,
            },
            Self::Oil => CargoPaymentSpec {
                base_rate: 4437,
                transit_fast_days: 25,
                transit_slow_days: 255,
            },
            Self::Livestock | Self::Batteries => CargoPaymentSpec {
                base_rate: 4322,
                transit_fast_days: 4,
                transit_slow_days: 18,
            },
            Self::Fruit => CargoPaymentSpec {
                base_rate: 4209,
                transit_fast_days: 0,
                transit_slow_days: 15,
            },
            Self::Goods | Self::Candy => CargoPaymentSpec {
                base_rate: 6144,
                transit_fast_days: 5,
                transit_slow_days: 28,
            },
            Self::Grain | Self::Wheat | Self::Toffee => CargoPaymentSpec {
                base_rate: 4778,
                transit_fast_days: 4,
                transit_slow_days: 40,
            },
            Self::Maize => CargoPaymentSpec {
                base_rate: 4322,
                transit_fast_days: 4,
                transit_slow_days: 40,
            },
            Self::Wood => CargoPaymentSpec {
                base_rate: 5005,
                transit_fast_days: 15,
                transit_slow_days: 255,
            },
            Self::CottonCandy => CargoPaymentSpec {
                base_rate: 5005,
                transit_fast_days: 10,
                transit_slow_days: 25,
            },
            Self::IronOre => CargoPaymentSpec {
                base_rate: 5120,
                transit_fast_days: 9,
                transit_slow_days: 255,
            },
            Self::Steel | Self::Food => CargoPaymentSpec {
                base_rate: 5688,
                transit_fast_days: 7,
                transit_slow_days: 255,
            },
            Self::Valuables => CargoPaymentSpec {
                base_rate: 7509,
                transit_fast_days: 1,
                transit_slow_days: 32,
            },
            Self::Paper => CargoPaymentSpec {
                base_rate: 5461,
                transit_fast_days: 7,
                transit_slow_days: 60,
            },
            Self::Gold => CargoPaymentSpec {
                base_rate: 5802,
                transit_fast_days: 10,
                transit_slow_days: 40,
            },
            Self::Diamonds => CargoPaymentSpec {
                base_rate: 5802,
                transit_fast_days: 10,
                transit_slow_days: 255,
            },
            Self::CopperOre => CargoPaymentSpec {
                base_rate: 4892,
                transit_fast_days: 12,
                transit_slow_days: 255,
            },
            Self::Cola => CargoPaymentSpec {
                base_rate: 4892,
                transit_fast_days: 5,
                transit_slow_days: 75,
            },
            Self::Water => CargoPaymentSpec {
                base_rate: 4664,
                transit_fast_days: 20,
                transit_slow_days: 80,
            },
            Self::Plastic => CargoPaymentSpec {
                base_rate: 4664,
                transit_fast_days: 30,
                transit_slow_days: 255,
            },
            Self::Rubber => CargoPaymentSpec {
                base_rate: 4437,
                transit_fast_days: 2,
                transit_slow_days: 20,
            },
            Self::Sugar => CargoPaymentSpec {
                base_rate: 4437,
                transit_fast_days: 20,
                transit_slow_days: 255,
            },
            Self::Toys => CargoPaymentSpec {
                base_rate: 5574,
                transit_fast_days: 25,
                transit_slow_days: 255,
            },
            Self::Bubbles => CargoPaymentSpec {
                base_rate: 5077,
                transit_fast_days: 20,
                transit_slow_days: 80,
            },
            Self::FizzyDrinks => CargoPaymentSpec {
                base_rate: 6250,
                transit_fast_days: 30,
                transit_slow_days: 50,
            },
        }
    }
}

#[must_use]
pub const fn manhattan_distance(a: TileCoord, b: TileCoord) -> u32 {
    (a.x - b.x).unsigned_abs() + (a.y - b.y).unsigned_abs()
}

#[must_use]
pub fn inflation_income_factor(inflation_payment: u64) -> u32 {
    GlobalEconomy::inflation_factor_from_accumulator(inflation_payment)
}

#[must_use]
pub fn inflation_prices_factor(inflation_prices: u64) -> u32 {
    GlobalEconomy::inflation_factor_from_accumulator(inflation_prices)
}

const MIN_TIME_FACTOR: i32 = 31;
const MAX_TIME_FACTOR: i32 = 255;
const TIME_FACTOR_FRAC_BITS: i32 = 4;
const TIME_FACTOR_FRAC: i32 = 1 << TIME_FACTOR_FRAC_BITS;

#[must_use]
pub fn cargo_time_factor(transit_days: u16, spec: CargoPaymentSpec) -> (i32, bool) {
    let tp = i32::from(transit_days);
    let periods1 = i32::from(spec.transit_fast_days);
    let periods2 = i32::from(spec.transit_slow_days);
    let over1 = (tp - periods1).max(0);
    let over2 = (over1 - periods2).max(0);

    let mut periods_over_max = MIN_TIME_FACTOR - MAX_TIME_FACTOR;
    if periods2 > -periods_over_max {
        periods_over_max += tp - periods1;
    } else {
        periods_over_max += 2 * (tp - periods1) - periods2;
    }

    if periods_over_max > 0 {
        let time_factor = (2 * MIN_TIME_FACTOR * TIME_FACTOR_FRAC * TIME_FACTOR_FRAC
            / (periods_over_max + 2 * TIME_FACTOR_FRAC))
            .max(1);
        (time_factor, true)
    } else {
        let time_factor = (MAX_TIME_FACTOR - over1 - over2).max(MIN_TIME_FACTOR);
        (time_factor, false)
    }
}

#[must_use]
pub fn transported_goods_income(
    count: u32,
    distance: u32,
    transit_days: u16,
    cargo: CargoType,
    inflation_payment: u64,
) -> i64 {
    transported_goods_income_with_spec(
        count,
        distance,
        transit_days,
        cargo.payment_spec(),
        inflation_payment,
    )
}

#[must_use]
pub fn transported_goods_income_with_spec(
    count: u32,
    distance: u32,
    transit_days: u16,
    spec: CargoPaymentSpec,
    inflation_payment: u64,
) -> i64 {
    if count == 0 {
        return 0;
    }
    let dist = distance.max(1);
    let (time_factor, asymptotic) = cargo_time_factor(transit_days, spec);
    let effective_rate =
        (i64::from(spec.base_rate) * i64::try_from(inflation_payment).unwrap_or(i64::MAX)) >> 16;
    let mut income = i64::from(dist) * i64::from(time_factor) * i64::from(count) * effective_rate;
    let shift = if asymptotic {
        21 + TIME_FACTOR_FRAC_BITS
    } else {
        21
    };
    income >>= shift;
    income.max(1)
}

pub const DEFAULT_MAX_LOAN: i64 = 300_000;
pub const LOAN_INTERVAL: i64 = 10_000;
pub const ANNUAL_INTEREST_RATE_PCT: i64 = 10;

#[must_use]
pub fn monthly_loan_interest(loan: i64) -> i64 {
    monthly_loan_interest_with_rate(loan, ANNUAL_INTEREST_RATE_PCT)
}

#[must_use]
pub fn monthly_loan_interest_with_rate(loan: i64, annual_rate_pct: i64) -> i64 {
    if loan <= 0 {
        return 0;
    }
    loan.saturating_mul(annual_rate_pct) / 100 / 12
}

#[must_use]
pub fn monthly_company_interest(loan: i64, money: i64, annual_rate_pct: i64, month: u8) -> i64 {
    let mut yearly_fee = loan.saturating_mul(annual_rate_pct) / 100;
    if money < 0 {
        yearly_fee = yearly_fee.saturating_add((-money).saturating_mul(annual_rate_pct) / 100);
    }
    let m = i64::from(month.min(11));
    let up_to_previous = yearly_fee.saturating_mul(m) / 12;
    let up_to_this = yearly_fee.saturating_mul(m + 1) / 12;
    up_to_this.saturating_sub(up_to_previous)
}

#[must_use]
pub fn monthly_station_maintenance_fee(ge: &super::global::GlobalEconomy) -> i64 {
    super::pricebase::get_price(ge, super::pricebase::PriceIndex::StationValue, 1, 0) >> 2
}

#[must_use]
pub const fn check_bankruptcy(money: i64, loan: i64, max_loan: i64) -> bool {
    money.saturating_sub(loan) < -max_loan
}

pub fn increase_loan(
    economy: &mut crate::game_state::CompanyEconomy,
) -> Result<i64, crate::command::CommandError> {
    let room = economy.max_loan.saturating_sub(economy.loan);
    if room < LOAN_INTERVAL {
        return Err(crate::command::CommandError::LoanAtMaximum);
    }
    economy.loan += LOAN_INTERVAL;
    economy.money += LOAN_INTERVAL;
    Ok(LOAN_INTERVAL)
}

pub fn decrease_loan(
    economy: &mut crate::game_state::CompanyEconomy,
) -> Result<i64, crate::command::CommandError> {
    if economy.loan < LOAN_INTERVAL {
        return Err(crate::command::CommandError::NoLoanToRepay);
    }
    if economy.money < LOAN_INTERVAL {
        return Err(crate::command::CommandError::InsufficientFunds);
    }
    economy.loan -= LOAN_INTERVAL;
    economy.money -= LOAN_INTERVAL;
    Ok(LOAN_INTERVAL)
}
