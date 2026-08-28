//! Reloj y estado RNG global desde el chunk `DATE`.

use crate::tick::GameTick;

use super::chunks::{CH_RIFF, CH_TABLE, RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

/// `SLV_U64_TICK_COUNTER` — contador de ticks pasa a u64.
const SLV_U64_TICK_COUNTER: u16 = 300;

/// Días de calendario + contador de ticks decodificados del save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavGameTime {
    /// Días absolutos de calendario (`TimerGameCalendar::date`).
    pub calendar_date: i32,
    /// Contador monotónico de ticks (`TimerGameTick::counter`).
    pub tick: u64,
}

/// Lee el registro global `DATE` (best-effort).
#[must_use]
pub(crate) fn game_time_from_chunks(chunks: &[RawChunk], save_version: u16) -> Option<SavGameTime> {
    let date = find_chunk(chunks, "DATE")?;
    let (calendar_date, tick) = if date.ch_type == CH_TABLE {
        let rows = parse_table_chunk(&date.body, false).ok()?;
        let record = &rows.first()?.1;
        let calendar_date = record_get(record, "date")
            .and_then(|v| match v {
                SlValue::Int(i) => i32::try_from(*i).ok(),
                SlValue::Uint(u) => i32::try_from(*u).ok(),
                _ => None,
            })
            .unwrap_or(0);
        let tick = tick_counter_from_record(record, save_version);
        (calendar_date, tick)
    } else if date.ch_type == CH_RIFF {
        let (calendar_date, tick) = super::array_legacy::date_from_riff(&date.body)?;
        (calendar_date, tick)
    } else {
        return None;
    };
    Some(SavGameTime {
        calendar_date,
        tick,
    })
}

/// Lee el estado de `_random` que `OpenTTD` persiste en `DATE`.
///
/// No se infiere a partir de la semilla de creación: al cargar una partida el
/// stream ya puede haber sido consumido por generación, economía o callbacks.
/// Los `CH_RIFF` antiguos no tienen nombres de columnas auto-descriptivos, por
/// eso se conservan como `None` hasta que exista un decoder específico.
#[must_use]
pub(crate) fn random_state_from_chunks(chunks: &[RawChunk]) -> Option<[u32; 2]> {
    let date = find_chunk(chunks, "DATE")?;
    if date.ch_type != CH_TABLE {
        return None;
    }
    let rows = parse_table_chunk(&date.body, false).ok()?;
    let record = &rows.first()?.1;
    let state_0 = record_get(record, "random_state[0]")
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    let state_1 = record_get(record, "random_state[1]")
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    Some([state_0, state_1])
}

fn tick_counter_from_record(record: &super::table::SlRecord, save_version: u16) -> u64 {
    let raw = record_get(record, "tick_counter")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    if save_version < SLV_U64_TICK_COUNTER {
        raw & 0xFFFF
    } else {
        raw
    }
}

/// Convierte el reloj del save a [`GameTick`] del estado jugable.
///
/// En `OpenTTD`, `tick_counter` puede envolver / no anclar el calendario; la fecha
/// jugable debe salir de `calendar_date` cuando implica un año claramente posterior
/// (#189: noticias/status en 1950 tras cargar un .sav avanzado).
#[must_use]
pub(crate) fn game_tick_from_sav_time(time: SavGameTime) -> GameTick {
    let from_tick = GameTick::new(time.tick);
    let from_calendar = tick_from_packed_calendar_date(time.calendar_date);
    // Nuestros .sav escriben tick ≈ calendar→tick; OpenTTD suele traer tick << calendar.
    if time.calendar_date > 0
        && from_calendar.get()
            > from_tick
                .get()
                .saturating_add(crate::economy::TICKS_PER_YEAR)
    {
        from_calendar
    } else {
        from_tick
    }
}

/// `calendar_date` empaquetado como en `sav/write/meta.rs`: `year * 365 + (doy - 1)`.
#[must_use]
pub(crate) fn tick_from_packed_calendar_date(calendar_date: i32) -> GameTick {
    use crate::economy::TICKS_PER_DAY;
    use crate::news::{CALENDAR_BASE_YEAR, CALENDAR_DAYS_PER_YEAR};
    let packed = u64::try_from(calendar_date.max(0)).unwrap_or(0);
    let base = u64::from(CALENDAR_BASE_YEAR).saturating_mul(CALENDAR_DAYS_PER_YEAR);
    let day_index = packed.saturating_sub(base);
    GameTick::new(day_index.saturating_mul(u64::from(TICKS_PER_DAY)))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::super::table::tests::write_str;

    use super::*;

    #[test]
    fn reads_tick_and_calendar_date() {
        let mut rec = Vec::new();
        rec.extend_from_slice(&12_345i32.to_be_bytes());
        rec.extend_from_slice(&99_000u64.to_be_bytes());
        rec.extend_from_slice(&0x1020_3040u32.to_be_bytes());
        rec.extend_from_slice(&0x5060_7080u32.to_be_bytes());

        let mut header = Vec::new();
        header.push(5);
        write_str("date", &mut header);
        header.push(8);
        write_str("tick_counter", &mut header);
        header.push(6);
        write_str("random_state[0]", &mut header);
        header.push(6);
        write_str("random_state[1]", &mut header);
        header.push(0);

        let mut body = Vec::new();
        super::super::table::tests::write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        super::super::table::tests::write_gamma(rec.len() as u32 + 1, &mut body);
        body.extend_from_slice(&rec);
        super::super::table::tests::write_gamma(0, &mut body);

        let chunk = RawChunk {
            name: *b"DATE",
            ch_type: CH_TABLE,
            body,
        };
        let time = game_time_from_chunks(std::slice::from_ref(&chunk), 310).expect("DATE");
        assert_eq!(time.calendar_date, 12_345);
        assert_eq!(time.tick, 99_000);
        assert_eq!(game_tick_from_sav_time(time).get(), 99_000);
        assert_eq!(
            random_state_from_chunks(std::slice::from_ref(&chunk)),
            Some([0x1020_3040, 0x5060_7080])
        );
    }

    #[test]
    fn random_state_requires_both_date_columns() {
        let mut rec = Vec::new();
        rec.extend_from_slice(&0x1020_3040u32.to_be_bytes());

        let mut header = Vec::new();
        header.push(6);
        write_str("random_state[0]", &mut header);
        header.push(0);

        let mut body = Vec::new();
        super::super::table::tests::write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        super::super::table::tests::write_gamma(rec.len() as u32 + 1, &mut body);
        body.extend_from_slice(&rec);
        super::super::table::tests::write_gamma(0, &mut body);

        let chunk = RawChunk {
            name: *b"DATE",
            ch_type: CH_TABLE,
            body,
        };
        assert_eq!(random_state_from_chunks(&[chunk]), None);
    }

    #[test]
    fn prefers_calendar_date_when_tick_is_stale() {
        use crate::economy::TICKS_PER_YEAR;
        use crate::news::{format_calendar_date, tick_for_calendar_year};
        // Año ~1980 empaquetado (write/meta) con tick_counter envuelto/pequeño.
        let calendar_date = i32::try_from(1980u64 * 365).unwrap();
        let time = SavGameTime {
            calendar_date,
            tick: 12_345, // << un año de sim
        };
        let tick = game_tick_from_sav_time(time);
        assert!(
            tick.get() > 12_345 + TICKS_PER_YEAR,
            "debe anclar al calendario, no al tick_counter"
        );
        assert_eq!(
            format_calendar_date(tick),
            format_calendar_date(tick_for_calendar_year(1980))
        );
    }

    #[test]
    fn keeps_tick_when_aligned_with_calendar() {
        use crate::news::tick_for_calendar_year;
        let tick = tick_for_calendar_year(1980).get();
        let calendar_date = i32::try_from(1980u64 * 365).unwrap();
        let time = SavGameTime {
            calendar_date,
            tick,
        };
        assert_eq!(game_tick_from_sav_time(time).get(), tick);
    }
}
