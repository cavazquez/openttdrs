//! Pagos de transporte inspirados en `GetTransportedGoodsIncome` de `OpenTTD` (Camino B).

use crate::cargo::CargoType;
use crate::map::TileCoord;

use super::time::TICKS_PER_YEAR;

/// Tasas base del clima templado de `OpenTTD` (`cargo_const.h`), sin inflación.
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
            Self::Livestock => CargoPaymentSpec {
                base_rate: 4322,
                transit_fast_days: 4,
                transit_slow_days: 18,
            },
            Self::Goods => CargoPaymentSpec {
                base_rate: 6144,
                transit_fast_days: 5,
                transit_slow_days: 28,
            },
            Self::Grain => CargoPaymentSpec {
                base_rate: 4778,
                transit_fast_days: 4,
                transit_slow_days: 40,
            },
            Self::Wood => CargoPaymentSpec {
                base_rate: 5005,
                transit_fast_days: 15,
                transit_slow_days: 255,
            },
            Self::IronOre => CargoPaymentSpec {
                base_rate: 5120,
                transit_fast_days: 9,
                transit_slow_days: 255,
            },
            Self::Steel => CargoPaymentSpec {
                base_rate: 5688,
                transit_fast_days: 7,
                transit_slow_days: 255,
            },
            Self::Valuables => CargoPaymentSpec {
                base_rate: 7509,
                transit_fast_days: 1,
                transit_slow_days: 32,
            },
        }
    }
}

#[must_use]
pub const fn manhattan_distance(a: TileCoord, b: TileCoord) -> u32 {
    (a.x - b.x).unsigned_abs() + (a.y - b.y).unsigned_abs()
}

/// Factor de inflación de ingresos (punto fijo /1024). Crece ~0,4 % por año simulado.
#[must_use]
pub fn inflation_income_factor(tick: u64) -> u32 {
    let years = tick / TICKS_PER_YEAR;
    1024 + u32::try_from(years).unwrap_or(u32::MAX).saturating_mul(4)
}

/// Factor de inflación de precios de construcción (/1024), al estilo `inflation_prices`.
///
/// `OpenTTD` usa `infl_amount_pr = max(0, initial_interest - 1)` (por defecto 1 %/año
/// frente a ~2 % en ingresos). Aquí ~0,3 %/año (`+3` por 1024).
#[must_use]
pub fn inflation_prices_factor(tick: u64) -> u32 {
    let years = tick / TICKS_PER_YEAR;
    1024 + u32::try_from(years).unwrap_or(u32::MAX).saturating_mul(3)
}

const MIN_TIME_FACTOR: i32 = 31;
const MAX_TIME_FACTOR: i32 = 255;

#[must_use]
pub fn cargo_time_factor(transit_days: u16, spec: CargoPaymentSpec) -> i32 {
    let tp = i32::from(transit_days);
    let periods1 = i32::from(spec.transit_fast_days);
    let periods2 = i32::from(spec.transit_slow_days);
    let over1 = (tp - periods1).max(0);
    let over2 = (over1 - periods2).max(0);
    (MAX_TIME_FACTOR - over1 - over2).max(MIN_TIME_FACTOR)
}

/// Ingreso por entrega final (simplificación de `OpenTTD` `GetTransportedGoodsIncome`).
#[must_use]
pub fn transported_goods_income(
    count: u32,
    distance: u32,
    transit_days: u16,
    cargo: CargoType,
    tick: u64,
) -> i64 {
    if count == 0 {
        return 0;
    }
    let dist = distance.max(1);
    let spec = cargo.payment_spec();
    let time_factor = cargo_time_factor(transit_days, spec);
    let mut income =
        i64::from(dist) * i64::from(time_factor) * i64::from(count) * i64::from(spec.base_rate);
    income >>= 21;
    income = income.saturating_mul(i64::from(inflation_income_factor(tick))) / 1024;
    income.max(1)
}

/// Préstamo máximo por defecto (`_settings_game.economy.max_loan`).
pub const DEFAULT_MAX_LOAN: i64 = 300_000;
/// Incremento/decremento por comando de préstamo (`LOAN_INTERVAL`).
pub const LOAN_INTERVAL: i64 = 10_000;
/// Tasa de interés anual aproximada (~10 % en dificultad media).
pub const ANNUAL_INTEREST_RATE_PCT: i64 = 10;

/// Interés mensual sobre el préstamo actual (~10 % anual / 12).
#[must_use]
pub fn monthly_loan_interest(loan: i64) -> i64 {
    if loan <= 0 {
        return 0;
    }
    loan.saturating_mul(ANNUAL_INTEREST_RATE_PCT) / 100 / 12
}

/// `true` si la compañía superó el límite de deuda (`CompanyCheckBankrupt`).
///
/// `OpenTTD` (`economy.cpp:556`) sobrevive mientras `money - current_loan >= -GetMaxLoan()`:
/// el préstamo pendiente cuenta como deuda, así que tener caja no basta para librarse.
#[must_use]
pub const fn check_bankruptcy(money: i64, loan: i64, max_loan: i64) -> bool {
    money.saturating_sub(loan) < -max_loan
}

/// Solicita más préstamo hasta `max_loan`. Devuelve el importe añadido.
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

/// Devuelve parte del préstamo si hay fondos.
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
