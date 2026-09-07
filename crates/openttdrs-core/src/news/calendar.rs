//! Funciones de calendario: conversión tick ↔ año/día, formato de fechas.
//!
//! `OpenTTD` guarda `Date` como días absolutos desde el año 0 y usa el
//! calendario gregoriano proléptico (incluido el año 0). El estado Rust usa
//! índices relativos a 1950 para mantener pequeños los contadores internos;
//! las conversiones de este módulo son el único puente entre ambas bases.

use crate::economy::TICKS_PER_DAY;
use crate::tick::GameTick;

/// Año base del calendario mostrado en la barra (Y1 del sim = 1950).
pub const CALENDAR_BASE_YEAR: u32 = 1950;
/// Año nominal de 365 días usado por intervalos que no representan una fecha.
///
/// Las conversiones de fecha usan [`is_calendar_leap_year`] y no esta
/// constante. Se conserva como parte de la API de intervalos histórica.
pub const CALENDAR_DAYS_PER_YEAR: u64 = 365;
/// `TimerGameCalendar::DateAtStartOfYear(1950)` de `OpenTTD`.
pub const OPENTTD_CALENDAR_BASE_DATE: i32 = 712_223;

const OPENTTD_CALENDAR_BASE_DATE_U64: u64 = 712_223;
const DAYS_PER_GREGORIAN_CYCLE: u64 = 146_097;
const MONTH_LENGTHS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MONTH_NAMES: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];

/// Devuelve si `year` es bisiesto con la regla usada por `OpenTTD`.
#[must_use]
pub const fn is_calendar_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// Días del año gregoriano indicado.
#[must_use]
pub const fn calendar_days_in_year(year: u32) -> u16 {
    if is_calendar_leap_year(year) {
        366
    } else {
        365
    }
}

/// `DateAtStartOfYear(year)` de `TimerGame` en `OpenTTD`.
#[must_use]
pub fn openttd_date_at_start_of_year(year: u32) -> u64 {
    let year = u64::from(year);
    let leap_years = if year == 0 {
        0
    } else {
        let previous = year - 1;
        previous / 4 - previous / 100 + previous / 400 + 1
    };
    year.saturating_mul(365).saturating_add(leap_years)
}

/// Convierte un índice de día relativo a 1950 al `Date` absoluto de `OpenTTD`.
#[must_use]
pub fn openttd_date_from_calendar_day_index(day_index: u64) -> i32 {
    let absolute = OPENTTD_CALENDAR_BASE_DATE_U64.saturating_add(day_index);
    i32::try_from(absolute).unwrap_or(i32::MAX)
}

/// Convierte un `Date` absoluto de `OpenTTD` al índice de día interno relativo
/// a 1950. Las partidas anteriores al año base se saturan al día 0: el modelo
/// persistente actual no representa fechas negativas.
#[must_use]
pub fn calendar_day_index_from_openttd_date(date: i32) -> u32 {
    let relative = i64::from(date).saturating_sub(i64::from(OPENTTD_CALENDAR_BASE_DATE));
    u32::try_from(relative.max(0)).unwrap_or(u32::MAX)
}

/// Año, mes (0..=11) y día (1..=31) para un índice relativo a 1950.
#[must_use]
pub fn calendar_ymd_from_day_index(day_index: u64) -> (u32, u8, u8) {
    ymd_from_absolute_date(OPENTTD_CALENDAR_BASE_DATE_U64.saturating_add(day_index))
}

/// Índice de día relativo a 1950 correspondiente al primer día de `year`.
#[must_use]
pub fn calendar_day_index_at_start_of_year(year: u32) -> u64 {
    openttd_date_at_start_of_year(year).saturating_sub(OPENTTD_CALENDAR_BASE_DATE_U64)
}

#[must_use]
pub fn tick_for_calendar_year(year: u32) -> GameTick {
    GameTick::new(
        calendar_day_index_at_start_of_year(year).saturating_mul(u64::from(TICKS_PER_DAY)),
    )
}

/// Índice de día de calendario a partir del tick de simulación (compatibilidad).
///
/// Preferir [`calendar_day_index_from_state`] cuando se disponga del [`crate::GameState`].
#[must_use]
pub fn calendar_day_index(tick: GameTick) -> u64 {
    tick.get() / u64::from(TICKS_PER_DAY)
}

/// Índice de día de calendario desde el reloj autoritativo del estado.
#[must_use]
pub fn calendar_day_index_from_state(state: &crate::GameState) -> u64 {
    state.calendar.day_index()
}

/// Año y día del año (1..=365/366) de un índice relativo a 1950.
#[must_use]
pub fn calendar_year_day(day_index: u64) -> (u32, u64) {
    let (year, month, day) = calendar_ymd_from_day_index(day_index);
    let before_month = MONTH_LENGTHS
        .iter()
        .take(usize::from(month))
        .map(|length| u64::from(*length))
        .sum::<u64>();
    let leap_day = u64::from(is_calendar_leap_year(year) && month >= 2);
    (year, before_month + leap_day + u64::from(day))
}

/// Mes 0..=11 para un índice de día relativo a 1950.
#[must_use]
pub fn calendar_month_from_day_index(day_index: u64) -> u8 {
    calendar_ymd_from_day_index(day_index).1
}

/// Día del mes (1..=31) para un índice de día relativo a 1950.
#[must_use]
pub fn calendar_day_of_month_from_day_index(day_index: u64) -> u8 {
    calendar_ymd_from_day_index(day_index).2
}

/// Nombre corto localizado internamente para un índice de mes válido.
#[must_use]
pub fn calendar_month_name(month: u8) -> &'static str {
    MONTH_NAMES
        .get(usize::from(month))
        .copied()
        .unwrap_or("dic")
}

#[must_use]
pub fn format_calendar_date(tick: GameTick) -> String {
    format_calendar_day_index(calendar_day_index(tick))
}

/// Formatea la fecha de calendario del estado (reloj autoritativo).
#[must_use]
pub fn format_calendar_date_from_state(state: &crate::GameState) -> String {
    format_calendar_day_index(calendar_day_index_from_state(state))
}

/// Formatea un índice de día de calendario (p. ej. `NewsItem.calendar_day`).
#[must_use]
pub fn format_calendar_day_index(day_index: u64) -> String {
    let (year, month, day) = calendar_ymd_from_day_index(day_index);
    format!("{day} {} {year}", calendar_month_name(month))
}

fn ymd_from_absolute_date(date: u64) -> (u32, u8, u8) {
    let cycles = date / DAYS_PER_GREGORIAN_CYCLE;
    let mut year = cycles.saturating_mul(400);
    let mut remaining = date % DAYS_PER_GREGORIAN_CYCLE;

    // Después de extraer ciclos completos sólo quedan como máximo 400 años.
    loop {
        let current_year = u32::try_from(year).unwrap_or(u32::MAX);
        let year_days = u64::from(calendar_days_in_year(current_year));
        if remaining < year_days || current_year == u32::MAX {
            break;
        }
        remaining -= year_days;
        year = year.saturating_add(1);
    }

    let current_year = u32::try_from(year).unwrap_or(u32::MAX);
    let mut month = 0u8;
    for common_length in MONTH_LENGTHS {
        let length = if month == 1 && is_calendar_leap_year(current_year) {
            29
        } else {
            common_length
        };
        if remaining < u64::from(length) {
            return (
                current_year,
                month,
                u8::try_from(remaining.saturating_add(1)).unwrap_or(u8::MAX),
            );
        }
        remaining -= u64::from(length);
        month = month.saturating_add(1);
    }

    (current_year, 11, 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_openttd_gregorian_date_epoch_and_leap_rules() {
        assert_eq!(openttd_date_at_start_of_year(0), 0);
        assert_eq!(openttd_date_at_start_of_year(1), 366);
        assert_eq!(openttd_date_at_start_of_year(1950), 712_223);
        assert_eq!(openttd_date_at_start_of_year(2004), 731_946);
        assert!(is_calendar_leap_year(2004));
        assert!(!is_calendar_leap_year(2100));
        assert!(is_calendar_leap_year(2000));
    }

    #[test]
    fn maps_native_save_date_to_its_real_month() {
        let relative = calendar_day_index_from_openttd_date(732_111);
        assert_eq!(
            calendar_ymd_from_day_index(u64::from(relative)),
            (2004, 5, 14)
        );
        assert_eq!(
            openttd_date_from_calendar_day_index(u64::from(relative)),
            732_111
        );
    }

    #[test]
    fn crosses_leap_day_when_formatting_relative_days() {
        let feb_28 = calendar_day_index_at_start_of_year(1952) + 58;
        assert_eq!(format_calendar_day_index(feb_28), "28 feb 1952");
        assert_eq!(format_calendar_day_index(feb_28 + 1), "29 feb 1952");
        assert_eq!(format_calendar_day_index(feb_28 + 2), "1 mar 1952");
    }
}
