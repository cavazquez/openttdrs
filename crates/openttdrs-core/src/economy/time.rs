//! Constantes de tiempo y conversiones de ticks de simulación.

/// Ticks de simulación ≈ un día de calendario (alineado con el HUD del cliente).
pub const TICKS_PER_TRANSIT_DAY: u32 = 74;
/// `OpenTTD` `timer_game_tick.h`: 1 tick de juego ≈ 27 ms (`74 * 27 ms` ≈ 2 s/día).
pub const OTTD_MILLISECONDS_PER_TICK: u32 = 27;
/// Frecuencia de simulación alineada con `OpenTTD` a velocidad normal.
pub const SIM_TICKS_PER_SECOND: f64 = 1000.0 / OTTD_MILLISECONDS_PER_TICK as f64;
/// Año simulado en ticks (365 días).
pub const TICKS_PER_YEAR: u64 = TICKS_PER_TRANSIT_DAY as u64 * 365;
/// Mes aproximado de calendario (30 días) para intereses de préstamo.
pub const TICKS_PER_MONTH: u64 = TICKS_PER_TRANSIT_DAY as u64 * 30;

#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn ticks_to_transit_days(ticks: u32) -> u16 {
    let days = ticks / TICKS_PER_TRANSIT_DAY;
    if days > u16::MAX as u32 {
        u16::MAX
    } else {
        days as u16
    }
}
