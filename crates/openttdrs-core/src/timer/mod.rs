//! Relojes de calendario y economía (`TimerGameCalendar` / `TimerGameEconomy` en `OpenTTD`).
//!
//! Por defecto ambos avanzan alineados (sin wallclock). El tick de simulación sigue siendo
//! independiente; estos timers mantienen `date_fract` 0..=73 y el contador de días.

use crate::economy::{TICKS_PER_DAY, calendar_month_index};
use crate::news::calendar_year_day;

/// Ticks de simulación en un día de calendario (`Ticks::DAY_TICKS`).
#[allow(clippy::cast_possible_truncation)]
pub const DAY_TICKS: u16 = TICKS_PER_DAY as u16;

/// Longitudes de mes usadas para `days_since_last_month` (sin bisectos).
const MONTH_LEN: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Eventos de borde detectados al cerrar un día en el timer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimerTriggers {
    pub new_day: bool,
    pub new_month: bool,
    pub new_year: bool,
}

/// Reloj de calendario: edad de vehículos, noticias, introducción de tecnología.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CalendarTimer {
    pub date: u32,
    pub date_fract: u16,
    pub year: u32,
    pub month: u8,
}

impl CalendarTimer {
    #[must_use]
    pub fn from_tick(tick: u64) -> Self {
        let date = u32::try_from(tick / u64::from(DAY_TICKS)).unwrap_or(u32::MAX);
        let date_fract = u16::try_from(tick % u64::from(DAY_TICKS)).unwrap_or(0);
        let (year, _) = calendar_year_day(u64::from(date));
        let month = calendar_month_index(u64::from(date));
        Self {
            date,
            date_fract,
            year,
            month,
        }
    }

    /// Índice de día de calendario (equivalente a `tick / DAY_TICKS`).
    #[must_use]
    pub const fn day_index(self) -> u64 {
        self.date as u64
    }

    fn sync_ymd(&mut self) {
        let (year, _) = calendar_year_day(u64::from(self.date));
        self.year = year;
        self.month = calendar_month_index(u64::from(self.date));
    }

    /// Avanza un tick de simulación en este reloj.
    pub fn elapsed_tick(&mut self) -> TimerTriggers {
        self.date_fract = self.date_fract.saturating_add(1);
        if self.date_fract < DAY_TICKS {
            return TimerTriggers::default();
        }
        self.date_fract = 0;
        self.date = self.date.saturating_add(1);
        let old_month = self.month;
        let old_year = self.year;
        self.sync_ymd();
        TimerTriggers {
            new_day: true,
            new_month: self.month != old_month,
            new_year: self.year != old_year,
        }
    }
}

/// Reloj de economía: intereses, inflación mensual, subsidios, producción industrial.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EconomyTimer {
    pub date: u32,
    pub date_fract: u16,
    pub year: u32,
    pub month: u8,
    pub days_since_last_month: u16,
}

impl EconomyTimer {
    #[must_use]
    pub fn from_tick(tick: u64) -> Self {
        let cal = CalendarTimer::from_tick(tick);
        let mut timer = Self {
            date: cal.date,
            date_fract: cal.date_fract,
            year: cal.year,
            month: cal.month,
            days_since_last_month: 0,
        };
        timer.days_since_last_month = days_in_current_month(timer.date);
        timer
    }

    #[must_use]
    pub const fn day_index(self) -> u64 {
        self.date as u64
    }

    fn sync_ymd(&mut self) {
        let (year, _) = calendar_year_day(u64::from(self.date));
        self.year = year;
        self.month = calendar_month_index(u64::from(self.date));
    }

    /// Avanza un tick de simulación en este reloj.
    pub fn elapsed_tick(&mut self) -> TimerTriggers {
        self.date_fract = self.date_fract.saturating_add(1);
        if self.date_fract < DAY_TICKS {
            return TimerTriggers::default();
        }
        self.date_fract = 0;
        self.date = self.date.saturating_add(1);
        self.days_since_last_month = self.days_since_last_month.saturating_add(1);
        let old_month = self.month;
        let old_year = self.year;
        self.sync_ymd();
        let new_month = self.month != old_month;
        let new_year = self.year != old_year;
        if new_month {
            self.days_since_last_month = 0;
        }
        TimerTriggers {
            new_day: true,
            new_month,
            new_year,
        }
    }
}

/// Días transcurridos en el mes de calendario actual (1..=31) para `days_since_last_month`.
fn days_in_current_month(date: u32) -> u16 {
    let (_, doy) = calendar_year_day(u64::from(date));
    let mut remaining = doy;
    for len in MONTH_LEN {
        if remaining <= len {
            return u16::try_from(remaining).unwrap_or(u16::MAX);
        }
        remaining -= len;
    }
    1
}

/// Tick de simulación al final del día `day_index` (último fract antes del rollover).
#[must_use]
pub fn tick_at_end_of_day(day_index: u32) -> u64 {
    u64::from(day_index + 1) * u64::from(DAY_TICKS) - 1
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::GameState;

    #[test]
    fn date_fract_wraps_at_day_ticks() {
        let mut cal = CalendarTimer::from_tick(u64::from(DAY_TICKS) - 1);
        assert_eq!(cal.date_fract, DAY_TICKS - 1);
        let triggers = cal.elapsed_tick();
        assert!(triggers.new_day);
        assert_eq!(cal.date_fract, 0);
        assert_eq!(cal.date, 1);
    }

    #[test]
    fn calendar_day_increments_on_rollover() {
        let mut cal = CalendarTimer::from_tick(0);
        for _ in 0..i32::from(DAY_TICKS) - 1 {
            assert!(!cal.elapsed_tick().new_day);
        }
        assert!(cal.elapsed_tick().new_day);
        assert_eq!(cal.day_index(), 1);
    }

    #[test]
    fn economy_timer_month_year_triggers() {
        // Fin de enero (día 30 = 31 ene): el siguiente tick abre febrero.
        let mut eco = EconomyTimer::from_tick(tick_at_end_of_day(30));
        let triggers = eco.elapsed_tick();
        assert!(triggers.new_day);
        assert!(triggers.new_month);
        assert!(!triggers.new_year);
        assert_eq!(eco.month, 1);
        assert_eq!(eco.days_since_last_month, 0);
        assert_eq!(eco.year, crate::news::CALENDAR_BASE_YEAR);
    }

    #[test]
    fn economy_timer_year_trigger_on_new_calendar_year() {
        let last_day = u32::try_from(365 - 1).unwrap();
        let mut eco = EconomyTimer::from_tick(tick_at_end_of_day(last_day));
        let triggers = eco.elapsed_tick();
        assert!(triggers.new_day);
        assert!(triggers.new_month);
        assert!(triggers.new_year);
        assert_eq!(eco.year, crate::news::CALENDAR_BASE_YEAR + 1);
    }

    #[test]
    fn save_roundtrip_preserves_timers() {
        let mut state = GameState::new(8, 8);
        for _ in 0..500 {
            state.step();
        }
        let json = state.save_json().unwrap();
        let loaded = GameState::load_json(&json).unwrap();
        assert_eq!(loaded.calendar, state.calendar);
        assert_eq!(loaded.economy_timer, state.economy_timer);
        assert_eq!(loaded.tick, state.tick);
    }

    #[test]
    fn save_migration_derives_timers_from_tick_when_missing() {
        let mut state = GameState::new(4, 4);
        for _ in 0..200 {
            state.step();
        }
        let json = state.save_json().unwrap();
        let legacy = json
            .replace("\"calendar\":", "\"calendar_removed\":")
            .replace("\"economy_timer\":", "\"economy_timer_removed\":");
        let loaded = GameState::load_json(&legacy).unwrap();
        assert_eq!(loaded.tick, state.tick);
        assert_eq!(loaded.calendar.date_fract, state.calendar.date_fract);
        assert_eq!(loaded.calendar.date, state.calendar.date);
        assert_eq!(loaded.economy_timer.date, state.economy_timer.date);
    }
}
