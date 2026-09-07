//! Relojes y estado RNG global desde el chunk `DATE`.

use crate::tick::GameTick;

use super::chunks::{CH_RIFF, CH_TABLE, RawChunk, find_chunk};
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};

/// `SLV_U64_TICK_COUNTER` — contador de ticks pasa a u64.
const SLV_U64_TICK_COUNTER: u16 = 300;

/// Relojes persistidos por `DATE`.
///
/// Las fechas son los `Date` absolutos de `OpenTTD` (días desde el año 0), no
/// índices relativos al reloj del core. Mantener ambos relojes junto al tick
/// evita inferir uno a partir de otro: `OpenTTD` los persiste de forma
/// independiente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavGameTime {
    /// `TimerGameCalendar::date`.
    pub calendar_date: i32,
    /// `TimerGameCalendar::date_fract`.
    pub calendar_date_fract: u16,
    /// `TimerGameCalendar::sub_date_fract`.
    pub calendar_sub_date_fract: u16,
    /// `TimerGameEconomy::date`.
    pub economy_date: i32,
    /// `TimerGameEconomy::date_fract`.
    pub economy_date_fract: u16,
    /// `TimerGameEconomy::days_since_last_month`.
    pub days_since_last_month: u32,
    /// Contador monotónico `TimerGameTick::counter`.
    pub tick: u64,
    /// Estado LFSR `_cur_tileloop_tile` de `RunTileLoop`.
    ///
    /// Este cursor es independiente del tick: restaurarlo evita que la
    /// primera franja de teselas tras un load visite otro conjunto y altere
    /// los callbacks/RNG posteriores.
    pub cur_tileloop_tile: u32,
}

/// Lee el registro global `DATE` (best-effort).
#[must_use]
pub(crate) fn game_time_from_chunks(chunks: &[RawChunk], save_version: u16) -> Option<SavGameTime> {
    let date = find_chunk(chunks, "DATE")?;
    if date.ch_type == CH_TABLE {
        let rows = parse_table_chunk(&date.body, false).ok()?;
        let record = &rows.first()?.1;
        let calendar_date = signed_i32(record, "date").unwrap_or(0);
        let calendar_date_fract = unsigned_u16(record, "date_fract").unwrap_or(0);
        let economy_date = signed_i32(record, "economy_date").unwrap_or(calendar_date);
        let economy_date_fract =
            unsigned_u16(record, "economy_date_fract").unwrap_or(calendar_date_fract);
        return Some(SavGameTime {
            calendar_date,
            calendar_date_fract,
            calendar_sub_date_fract: unsigned_u16(record, "calendar_sub_date_fract").unwrap_or(0),
            economy_date,
            economy_date_fract,
            days_since_last_month: unsigned_u32(record, "days_since_last_month").unwrap_or(0),
            tick: tick_counter_from_record(record, save_version),
            cur_tileloop_tile: unsigned_u32(record, "cur_tileloop_tile")
                .unwrap_or_else(crate::map::tile_loop::default_cur_tileloop_tile),
        });
    }

    if date.ch_type == CH_RIFF {
        let (calendar_date, tick) = super::array_legacy::date_from_riff(&date.body)?;
        return Some(SavGameTime {
            calendar_date,
            calendar_date_fract: 0,
            calendar_sub_date_fract: 0,
            economy_date: calendar_date,
            economy_date_fract: 0,
            days_since_last_month: 0,
            tick,
            cur_tileloop_tile: crate::map::tile_loop::default_cur_tileloop_tile(),
        });
    }

    None
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
    let state_0 = unsigned_u32(record, "random_state[0]")?;
    let state_1 = unsigned_u32(record, "random_state[1]")?;
    Some([state_0, state_1])
}

fn signed_i32(record: &SlRecord, field: &str) -> Option<i32> {
    record_get(record, field)
        .and_then(SlValue::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn unsigned_u16(record: &SlRecord, field: &str) -> Option<u16> {
    record_get(record, field)
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn unsigned_u32(record: &SlRecord, field: &str) -> Option<u32> {
    record_get(record, field)
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn tick_counter_from_record(record: &SlRecord, save_version: u16) -> u64 {
    let raw = record_get(record, "tick_counter")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    if save_version < SLV_U64_TICK_COUNTER {
        raw & 0xFFFF
    } else {
        raw
    }
}

/// Convierte una fecha absoluta de `OpenTTD` a un tick relativo del core para
/// campos de entidades que todavía guardan fechas (por ejemplo, servicio de
/// vehículos). No se usa para rehidratar `TimerGameTick::counter`.
#[must_use]
pub(crate) fn tick_from_packed_calendar_date(calendar_date: i32) -> GameTick {
    use crate::economy::TICKS_PER_DAY;
    let day_index = crate::news::calendar_day_index_from_openttd_date(calendar_date);
    GameTick::new(u64::from(day_index).saturating_mul(u64::from(TICKS_PER_DAY)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::super::table::tests::write_str;

    use super::*;

    fn date_chunk(fields: &[(u8, &str)], record: &[u8]) -> RawChunk {
        let mut header = Vec::new();
        for (field_type, name) in fields {
            header.push(*field_type);
            write_str(name, &mut header);
        }
        header.push(0);

        let mut body = Vec::new();
        let header_len = u32::try_from(header.len()).expect("header corto");
        super::super::table::tests::write_gamma(header_len.saturating_add(1), &mut body);
        body.extend_from_slice(&header);
        let record_len = u32::try_from(record.len()).expect("registro corto");
        super::super::table::tests::write_gamma(record_len.saturating_add(1), &mut body);
        body.extend_from_slice(record);
        super::super::table::tests::write_gamma(0, &mut body);
        RawChunk {
            name: *b"DATE",
            ch_type: CH_TABLE,
            body,
        }
    }

    #[test]
    fn reads_all_modern_date_timer_fields() {
        let mut record = Vec::new();
        record.extend_from_slice(&732_111i32.to_be_bytes());
        record.extend_from_slice(&37u16.to_be_bytes());
        record.extend_from_slice(&1_472_993u64.to_be_bytes());
        record.extend_from_slice(&732_110i32.to_be_bytes());
        record.extend_from_slice(&12u16.to_be_bytes());
        record.extend_from_slice(&29u32.to_be_bytes());
        record.extend_from_slice(&44u16.to_be_bytes());
        record.extend_from_slice(&0x89AB_CDEFu32.to_be_bytes());
        record.extend_from_slice(&0x1020_3040u32.to_be_bytes());
        record.extend_from_slice(&0x5060_7080u32.to_be_bytes());
        let chunk = date_chunk(
            &[
                (5, "date"),
                (4, "date_fract"),
                (8, "tick_counter"),
                (5, "economy_date"),
                (4, "economy_date_fract"),
                (6, "days_since_last_month"),
                (4, "calendar_sub_date_fract"),
                (6, "cur_tileloop_tile"),
                (6, "random_state[0]"),
                (6, "random_state[1]"),
            ],
            &record,
        );

        let time = game_time_from_chunks(std::slice::from_ref(&chunk), 358).expect("DATE");
        assert_eq!(time.calendar_date, 732_111);
        assert_eq!(time.calendar_date_fract, 37);
        assert_eq!(time.calendar_sub_date_fract, 44);
        assert_eq!(time.economy_date, 732_110);
        assert_eq!(time.economy_date_fract, 12);
        assert_eq!(time.days_since_last_month, 29);
        assert_eq!(time.tick, 1_472_993);
        assert_eq!(time.cur_tileloop_tile, 0x89AB_CDEF);
        assert_eq!(
            random_state_from_chunks(std::slice::from_ref(&chunk)),
            Some([0x1020_3040, 0x5060_7080])
        );
    }

    #[test]
    fn missing_modern_columns_follow_calendar_for_legacy_table() {
        let mut record = Vec::new();
        record.extend_from_slice(&12_345i32.to_be_bytes());
        record.extend_from_slice(&99_000u64.to_be_bytes());
        let chunk = date_chunk(&[(5, "date"), (8, "tick_counter")], &record);

        let time = game_time_from_chunks(&[chunk], 310).expect("DATE");
        assert_eq!(time.calendar_date, 12_345);
        assert_eq!(time.economy_date, 12_345);
        assert_eq!(time.calendar_date_fract, 0);
        assert_eq!(time.tick, 99_000);
        assert_eq!(
            time.cur_tileloop_tile,
            crate::map::tile_loop::default_cur_tileloop_tile()
        );
    }

    #[test]
    fn random_state_requires_both_date_columns() {
        let chunk = date_chunk(&[(6, "random_state[0]")], &0x1020_3040u32.to_be_bytes());
        assert_eq!(random_state_from_chunks(&[chunk]), None);
    }

    #[test]
    fn entity_service_dates_use_true_gregorian_epoch() {
        let tick = tick_from_packed_calendar_date(732_111);
        assert_eq!(tick.get(), 19_888 * 74);
    }
}
