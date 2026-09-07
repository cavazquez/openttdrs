//! Relojes de calendario y economía (`TimerGameCalendar` / `TimerGameEconomy` en `OpenTTD`).
//!
//! Por defecto ambos avanzan alineados (sin wallclock). El tick de simulación sigue siendo
//! independiente; estos timers mantienen `date_fract` 0..=73 y el contador de días.
//! Con [`EconomyTimer::using_wallclock`] los meses económicos son siempre 30 días
//! (`EconomyTime::DAYS_IN_ECONOMY_MONTH`) y el año económico 360 días, desacoplados
//! del calendario gregoriano.

use crate::economy::{TICKS_PER_DAY, calendar_month_index};
use crate::news::{
    CALENDAR_BASE_YEAR, calendar_day_index_from_openttd_date, calendar_day_of_month_from_day_index,
    calendar_year_day, openttd_date_from_calendar_day_index,
};

/// Ticks de simulación en un día de calendario (`Ticks::DAY_TICKS`).
#[allow(clippy::cast_possible_truncation)]
pub const DAY_TICKS: u16 = TICKS_PER_DAY as u16;

/// Días por mes económico en modo wallclock (`EconomyTime::DAYS_IN_ECONOMY_MONTH`).
pub const DAYS_IN_ECONOMY_MONTH: u32 = 30;
/// Días por año económico en modo wallclock (`EconomyTime::DAYS_IN_ECONOMY_YEAR`).
pub const DAYS_IN_ECONOMY_YEAR: u32 = 360;

/// Fecha absoluta de `OpenTTD` correspondiente al inicio del año económico 1950
/// cuando las unidades son wallclock (360 días por año).
pub const OPENTTD_WALLCLOCK_ECONOMY_BASE_DATE: i32 = 702_000;

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
    /// Subparte de `date_fract` para progresión de calendario no predeterminada.
    #[serde(default)]
    pub sub_date_fract: u16,
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
            sub_date_fract: 0,
            year,
            month,
        }
    }

    /// Rehidrata el reloj desde `TimerGameCalendar::{date,date_fract,sub_date_fract}`.
    #[must_use]
    pub fn from_openttd_date(date: i32, date_fract: u16, sub_date_fract: u16) -> Self {
        let date = calendar_day_index_from_openttd_date(date);
        let (year, _) = calendar_year_day(u64::from(date));
        let month = calendar_month_index(u64::from(date));
        Self {
            date,
            date_fract: normalized_date_fract(date_fract),
            sub_date_fract,
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
        self.sub_date_fract = 0;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EconomyTimer {
    pub date: u32,
    pub date_fract: u16,
    pub year: u32,
    pub month: u8,
    pub days_since_last_month: u32,
    /// `TimerGameEconomy::UsingWallclockUnits` — meses de 30 días fijos.
    #[serde(default)]
    pub using_wallclock: bool,
}

impl Default for EconomyTimer {
    fn default() -> Self {
        Self {
            date: 0,
            date_fract: 0,
            year: CALENDAR_BASE_YEAR,
            month: 0,
            days_since_last_month: 0,
            using_wallclock: false,
        }
    }
}

impl EconomyTimer {
    #[must_use]
    pub fn from_tick(tick: u64) -> Self {
        Self::from_tick_with_wallclock(tick, false)
    }

    #[must_use]
    pub fn from_tick_with_wallclock(tick: u64, using_wallclock: bool) -> Self {
        let cal = CalendarTimer::from_tick(tick);
        let mut timer = Self {
            date: cal.date,
            date_fract: cal.date_fract,
            year: cal.year,
            month: cal.month,
            days_since_last_month: 0,
            using_wallclock,
        };
        timer.sync_ymd();
        timer.days_since_last_month = if using_wallclock {
            days_in_economy_month(timer.date)
        } else {
            days_in_current_month(timer.date)
        };
        timer
    }

    /// Rehidrata el reloj desde los campos económicos del chunk `DATE`.
    #[must_use]
    pub fn from_openttd_date(
        date: i32,
        date_fract: u16,
        days_since_last_month: u32,
        using_wallclock: bool,
    ) -> Self {
        let date = economy_day_index_from_openttd_date(date, using_wallclock);
        let mut timer = Self {
            date,
            date_fract: normalized_date_fract(date_fract),
            year: CALENDAR_BASE_YEAR,
            month: 0,
            days_since_last_month,
            using_wallclock,
        };
        timer.sync_ymd();
        timer
    }

    /// `TimerGameEconomy::UsingWallclockUnits`.
    #[must_use]
    pub const fn using_wallclock_units(self) -> bool {
        self.using_wallclock
    }

    #[must_use]
    pub const fn day_index(self) -> u64 {
        self.date as u64
    }

    fn sync_ymd(&mut self) {
        if self.using_wallclock {
            // Meses de 30 días / años de 360 (`ConvertDateToYMD` wallclock).
            self.year = CALENDAR_BASE_YEAR + self.date / DAYS_IN_ECONOMY_YEAR;
            self.month = ((self.date % DAYS_IN_ECONOMY_YEAR) / DAYS_IN_ECONOMY_MONTH) as u8;
        } else {
            let (year, _) = calendar_year_day(u64::from(self.date));
            self.year = year;
            self.month = calendar_month_index(u64::from(self.date));
        }
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
fn days_in_current_month(date: u32) -> u32 {
    u32::from(calendar_day_of_month_from_day_index(u64::from(date)))
}

/// Día dentro del mes económico wallclock (1..=30).
fn days_in_economy_month(date: u32) -> u32 {
    (date % DAYS_IN_ECONOMY_MONTH) + 1
}

/// Convierte una fecha económica de `OpenTTD` al contador relativo del core.
#[must_use]
pub fn economy_day_index_from_openttd_date(date: i32, using_wallclock: bool) -> u32 {
    if !using_wallclock {
        return calendar_day_index_from_openttd_date(date);
    }
    let relative = i64::from(date).saturating_sub(i64::from(OPENTTD_WALLCLOCK_ECONOMY_BASE_DATE));
    u32::try_from(relative.max(0)).unwrap_or(u32::MAX)
}

/// Convierte el contador económico relativo al `Date` que `OpenTTD` guarda.
#[must_use]
pub fn openttd_economy_date_from_day_index(date: u32, using_wallclock: bool) -> i32 {
    if !using_wallclock {
        return openttd_date_from_calendar_day_index(u64::from(date));
    }
    i32::try_from(i64::from(OPENTTD_WALLCLOCK_ECONOMY_BASE_DATE).saturating_add(i64::from(date)))
        .unwrap_or(i32::MAX)
}

fn normalized_date_fract(date_fract: u16) -> u16 {
    date_fract.min(DAY_TICKS.saturating_sub(1))
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
    fn wallclock_economy_months_are_30_days() {
        // Día 29 → al cerrar entra el día 30 = primer día del mes 1.
        let mut eco = EconomyTimer::from_tick_with_wallclock(tick_at_end_of_day(29), true);
        assert!(eco.using_wallclock_units());
        assert_eq!(eco.month, 0);
        let triggers = eco.elapsed_tick();
        assert!(triggers.new_day);
        assert!(triggers.new_month);
        assert_eq!(eco.month, 1);
        assert_eq!(eco.days_since_last_month, 0);
        // Calendario independiente: enero tiene 31 días.
        let mut cal = CalendarTimer::from_tick(tick_at_end_of_day(29));
        let cal_t = cal.elapsed_tick();
        assert!(cal_t.new_day);
        assert!(!cal_t.new_month);
    }

    #[test]
    fn wallclock_economy_year_is_360_days() {
        let mut eco = EconomyTimer::from_tick_with_wallclock(tick_at_end_of_day(359), true);
        let triggers = eco.elapsed_tick();
        assert!(triggers.new_year);
        assert!(triggers.new_month);
        assert_eq!(eco.year, crate::news::CALENDAR_BASE_YEAR + 1);
        assert_eq!(eco.month, 0);
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
