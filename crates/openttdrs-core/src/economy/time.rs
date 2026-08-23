//! Constantes de tiempo y conversiones de ticks de simulación.

/// Ticks de simulación de un día de calendario (`Ticks::DAY_TICKS`).
pub const TICKS_PER_DAY: u32 = 74;
/// `OpenTTD` `timer_game_tick.h`: 1 tick de juego ≈ 27 ms (`74 * 27 ms` ≈ 2 s/día).
pub const OTTD_MILLISECONDS_PER_TICK: u32 = 27;
/// Frecuencia de simulación alineada con `OpenTTD` a velocidad normal.
pub const SIM_TICKS_PER_SECOND: f64 = 1000.0 / OTTD_MILLISECONDS_PER_TICK as f64;
/// Año simulado en ticks (365 días).
pub const TICKS_PER_YEAR: u64 = TICKS_PER_DAY as u64 * 365;
/// Mes aproximado de calendario (30 días) para intereses de préstamo.
pub const TICKS_PER_MONTH: u64 = TICKS_PER_DAY as u64 * 30;

/// Duración de un periodo de tránsito de la carga (`Ticks::CARGO_AGING_TICKS`).
///
/// No es un día: son ~2,5 días. El pago decae por periodo, así que usar el día
/// de calendario aquí penalizaría los viajes largos (la API de scripts hace la
/// conversión inversa con `days_in_transit * 2 / 5`, `script_cargo.cpp:78`).
pub const CARGO_AGING_TICKS: u32 = 185;

/// Periodo de recálculo del rating de estación (`Ticks::STATION_RATING_TICKS`).
pub const STATION_RATING_TICKS: u32 = 185;

/// Periodo del trigger de aceptación de estación
/// (`Ticks::STATION_ACCEPTANCE_TICKS`).
///
/// Es independiente del barrido de rating: `OpenTTD` lo ejecuta cada 250 ticks
/// y lo escalona por índice de estación.
pub const STATION_ACCEPTANCE_TICKS: u32 = 250;

/// Mes de calendario 0..=11 a partir del índice de día (`calendar_day_index`).
#[must_use]
pub fn calendar_month_index(day_index: u64) -> u8 {
    const MONTH_LEN: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let doy = day_index % 365 + 1;
    let mut remaining = doy;
    for (i, len) in MONTH_LEN.iter().enumerate() {
        if remaining <= *len {
            return u8::try_from(i).unwrap_or(11);
        }
        remaining -= len;
    }
    11
}
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn ticks_to_transit_periods(ticks: u32) -> u16 {
    let periods = ticks / CARGO_AGING_TICKS;
    if periods > u16::MAX as u32 {
        u16::MAX
    } else {
        periods as u16
    }
}
