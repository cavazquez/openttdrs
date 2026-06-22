//! Reloj de simulación desde el chunk global `DATE`.

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

/// Convierte ticks del save a [`GameTick`] del estado jugable.
#[must_use]
pub(crate) fn game_tick_from_sav_time(time: SavGameTime) -> GameTick {
    GameTick::new(time.tick)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::table::tests::write_str;

    use super::*;

    #[test]
    fn reads_tick_and_calendar_date() {
        let mut rec = Vec::new();
        rec.extend_from_slice(&12_345i32.to_be_bytes());
        rec.extend_from_slice(&99_000u64.to_be_bytes());

        let mut header = Vec::new();
        header.push(5);
        write_str("date", &mut header);
        header.push(8);
        write_str("tick_counter", &mut header);
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
        let time = game_time_from_chunks(&[chunk], 310).expect("DATE");
        assert_eq!(time.calendar_date, 12_345);
        assert_eq!(time.tick, 99_000);
        assert_eq!(game_tick_from_sav_time(time).get(), 99_000);
    }
}
