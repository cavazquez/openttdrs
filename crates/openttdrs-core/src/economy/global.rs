//! Estado económico global (`_economy` en `OpenTTD`: inflación, recesiones, préstamo máximo).

use crate::linkgraph_parity::Randomizer;

use super::payments::{DEFAULT_MAX_LOAN, LOAN_INTERVAL};

/// Uno en punto fijo 16.16 (`InitializeEconomy`, `economy.cpp:923`).
pub const INFLATION_FRAC_ONE: u64 = 1 << 16;
/// Tope de inflación acumulada (`economy_type.h:225`).
pub const MAX_INFLATION: u64 = (1_u64 << (63 - 32)) - 1;
/// Inflación solo entre estos años de calendario (`economy.cpp:712`).
pub const ORIGINAL_BASE_YEAR: u32 = 1920;
pub const ORIGINAL_MAX_YEAR: u32 = 2090;
/// Interés/inflación inicial por defecto en dificultad media (`StartupEconomy`).
pub const DEFAULT_INITIAL_INTEREST: u8 = 2;

/// Evento mensual de fluctuación económica (`HandleEconomyFluctuations`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluctuationEvent {
    RecessionStart,
    RecessionEnd,
}

/// Economía global de la partida (inflación compuesta, recesiones, escala de `max_loan`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GlobalEconomy {
    /// Precios de construcción acumulados (16.16).
    #[serde(default = "default_inflation_frac_one")]
    pub inflation_prices: u64,
    /// Pagos de carga acumulados (16.16).
    #[serde(default = "default_inflation_frac_one")]
    pub inflation_payment: u64,
    /// Tasa anual de inflación de precios en % (`infl_amount`).
    #[serde(default = "default_initial_interest")]
    pub infl_amount: u8,
    /// Tasa anual de inflación de pagos: `max(0, interés − 1)` (`infl_amount_pr`).
    #[serde(default = "default_infl_amount_pr")]
    pub infl_amount_pr: u8,
    /// Contador de recesión / calma (`fluct`; ≤ 0 = recesión, `economy.cpp:831-851`).
    #[serde(default)]
    pub fluct: i16,
    /// `difficulty.economy` / recesiones activas (por defecto `false` = economía estable).
    #[serde(default)]
    pub recessions_enabled: bool,
    /// Si la inflación compuesta está activa (`economy.inflation`).
    #[serde(default = "default_true")]
    pub inflation_enabled: bool,
    /// Préstamo máximo base sin inflación (`difficulty.max_loan`).
    #[serde(default = "default_base_max_loan")]
    pub base_max_loan: i64,
}

const fn default_inflation_frac_one() -> u64 {
    INFLATION_FRAC_ONE
}

const fn default_initial_interest() -> u8 {
    DEFAULT_INITIAL_INTEREST
}

const fn default_infl_amount_pr() -> u8 {
    DEFAULT_INITIAL_INTEREST.saturating_sub(1)
}

const fn default_base_max_loan() -> i64 {
    DEFAULT_MAX_LOAN
}

const fn default_true() -> bool {
    true
}

impl Default for GlobalEconomy {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalEconomy {
    #[must_use]
    pub const fn new() -> Self {
        let infl_amount = DEFAULT_INITIAL_INTEREST;
        Self {
            inflation_prices: INFLATION_FRAC_ONE,
            inflation_payment: INFLATION_FRAC_ONE,
            infl_amount,
            infl_amount_pr: infl_amount.saturating_sub(1),
            fluct: 0,
            recessions_enabled: false,
            inflation_enabled: true,
            base_max_loan: DEFAULT_MAX_LOAN,
        }
    }

    /// Inicializa fluctuación y aplica inflación previa al año de arranque (`StartupEconomy`).
    pub fn startup(&mut self, rng: &mut Randomizer, start_year: u32) {
        self.fluct = i16::try_from((rng.next() & 0xFF) + 168).unwrap_or(168);
        if self.inflation_enabled {
            let end_year = start_year.min(ORIGINAL_MAX_YEAR);
            let months = usize::try_from(
                u32::try_from(end_year.saturating_sub(ORIGINAL_BASE_YEAR))
                    .unwrap_or(0)
                    .saturating_mul(12),
            )
            .unwrap_or(0);
            for _ in 0..months {
                let _ = self.add_monthly_inflation(start_year, false);
            }
        }
    }

    #[must_use]
    pub const fn is_in_recession(&self) -> bool {
        self.fluct <= 0
    }

    /// Convierte acumulador 16.16 a factor /1024 para costes legacy.
    #[must_use]
    pub const fn inflation_factor_from_accumulator(accumulator: u64) -> u32 {
        let scaled = accumulator.saturating_mul(1024) >> 16;
        if scaled > u32::MAX as u64 {
            u32::MAX
        } else {
            scaled as u32
        }
    }

    /// `AddInflation` (`economy.cpp:704-727`). Devuelve `true` si no hay más inflación.
    pub fn add_monthly_inflation(&mut self, calendar_year: u32, check_year: bool) -> bool {
        if check_year
            && (calendar_year < ORIGINAL_BASE_YEAR || calendar_year >= ORIGINAL_MAX_YEAR)
        {
            return true;
        }
        if !self.inflation_enabled {
            return true;
        }
        if self.inflation_prices == MAX_INFLATION || self.inflation_payment == MAX_INFLATION {
            return true;
        }
        self.inflation_prices = self
            .inflation_prices
            .saturating_add((self.inflation_prices * u64::from(self.infl_amount) * 54) >> 16);
        self.inflation_payment = self.inflation_payment.saturating_add(
            (self.inflation_payment * u64::from(self.infl_amount_pr) * 54) >> 16,
        );
        if self.inflation_prices > MAX_INFLATION {
            self.inflation_prices = MAX_INFLATION;
        }
        if self.inflation_payment > MAX_INFLATION {
            self.inflation_payment = MAX_INFLATION;
        }
        false
    }

    /// `RecomputePrices` → escala de `max_loan` (`economy.cpp:736`).
    #[must_use]
    pub fn scaled_max_loan(&self) -> i64 {
        let scaled = (self.base_max_loan as u64).saturating_mul(self.inflation_prices) >> 16;
        let interval = LOAN_INTERVAL as u64;
        i64::try_from((scaled / interval) * interval).unwrap_or(i64::MAX)
    }

    /// `HandleEconomyFluctuations` (`economy.cpp:831-851`).
    pub fn handle_monthly_fluctuations(&mut self, rng: &mut Randomizer) -> Option<FluctuationEvent> {
        if self.recessions_enabled {
            self.fluct -= 1;
        } else if self.is_in_recession() {
            self.fluct = -12;
        } else {
            return None;
        }

        if self.fluct == 0 {
            let bits = rng.next() & 3;
            self.fluct = -i16::try_from(bits).unwrap_or(1);
            if self.fluct == 0 {
                self.fluct = -1;
            }
            Some(FluctuationEvent::RecessionStart)
        } else if self.fluct == -12 {
            self.fluct = i16::try_from((rng.next() & 0xFF) + 312).unwrap_or(312);
            Some(FluctuationEvent::RecessionEnd)
        } else {
            None
        }
    }
}
