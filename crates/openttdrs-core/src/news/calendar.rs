//! Funciones de calendario: conversión tick ↔ año/día, formato de fechas.

use crate::economy::TICKS_PER_TRANSIT_DAY;
use crate::tick::GameTick;

/// Año base del calendario mostrado en la barra (Y1 del sim = 1950).
pub const CALENDAR_BASE_YEAR: u32 = 1950;
pub const CALENDAR_DAYS_PER_YEAR: u64 = 365;

#[must_use]
pub fn tick_for_calendar_year(year: u32) -> GameTick {
    let years = u64::from(year.saturating_sub(CALENDAR_BASE_YEAR));
    GameTick::new(years * crate::economy::TICKS_PER_YEAR)
}

#[must_use]
pub fn calendar_day_index(tick: GameTick) -> u64 {
    tick.get() / u64::from(TICKS_PER_TRANSIT_DAY)
}

#[must_use]
pub fn calendar_year_day(day_index: u64) -> (u32, u64) {
    let years = day_index / CALENDAR_DAYS_PER_YEAR;
    let year = CALENDAR_BASE_YEAR.saturating_add(u32::try_from(years).unwrap_or(u32::MAX));
    let doy = day_index % CALENDAR_DAYS_PER_YEAR + 1;
    (year, doy)
}

#[must_use]
pub fn format_calendar_date(tick: GameTick) -> String {
    format_calendar_day_index(calendar_day_index(tick))
}

/// Formatea un índice de día de calendario (p. ej. `NewsItem.calendar_day`).
#[must_use]
pub fn format_calendar_day_index(day_index: u64) -> String {
    let (year, doy) = calendar_year_day(day_index);
    let (day, month) = doy_to_month_day(doy);
    format!("{day} {month} {year}")
}

pub(super) fn doy_to_month_day(doy: u64) -> (u64, &'static str) {
    const MONTHS: [(&str, u64); 12] = [
        ("ene", 31),
        ("feb", 28),
        ("mar", 31),
        ("abr", 30),
        ("may", 31),
        ("jun", 30),
        ("jul", 31),
        ("ago", 31),
        ("sep", 30),
        ("oct", 31),
        ("nov", 30),
        ("dic", 31),
    ];
    let mut remaining = doy;
    for (name, len) in MONTHS {
        if remaining <= len {
            return (remaining, name);
        }
        remaining -= len;
    }
    (31, "dic")
}
