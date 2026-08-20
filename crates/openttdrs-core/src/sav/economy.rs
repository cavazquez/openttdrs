//! Estado económico global (`ECMY`) de un savegame de OpenTTD.

use crate::economy::GlobalEconomy;

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

/// Lee el registro global `ECMY` de saves modernos.
///
/// El chunk sólo contiene los campos que OpenTTD conserva en la versión
/// actual. Los flags de dificultad que no forman parte de `ECMY` permanecen
/// con sus defaults y se hidratan desde `PATS`/`OPTS` cuando existen.
#[must_use]
pub(crate) fn global_economy_from_chunks(chunks: &[RawChunk]) -> GlobalEconomy {
    let mut economy = GlobalEconomy::new();
    let Some(chunk) = find_chunk(chunks, "ECMY") else {
        return economy;
    };
    let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
        return economy;
    };
    let Some((_, record)) = rows.first() else {
        return economy;
    };
    if let Some(value) = record_get(record, "inflation_prices").and_then(SlValue::as_u64) {
        economy.inflation_prices = value;
    }
    if let Some(value) = record_get(record, "inflation_payment").and_then(SlValue::as_u64) {
        economy.inflation_payment = value;
    }
    if let Some(value) = record_get(record, "fluct")
        .and_then(SlValue::as_i64)
        .and_then(|value| i16::try_from(value).ok())
    {
        economy.fluct = value;
    }
    if let Some(value) = record_get(record, "interest_rate")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
    {
        economy.interest_rate = value;
    }
    if let Some(value) = record_get(record, "infl_amount")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
    {
        economy.infl_amount = value;
    }
    if let Some(value) = record_get(record, "infl_amount_pr")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
    {
        economy.infl_amount_pr = value;
    }
    if let Some(value) = record_get(record, "industry_daily_change_counter")
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    {
        economy.industry_daily_change_counter = value;
    }
    economy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sav::chunks::CH_TABLE;
    use crate::sav::table::tests::build_table_body;

    #[test]
    fn reads_ecmy_fields_without_losing_signed_fluctuation() {
        let chunk = RawChunk {
            name: *b"ECMY",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (8, "inflation_prices"),
                    (8, "inflation_payment"),
                    (3, "fluct"),
                    (2, "interest_rate"),
                    (2, "infl_amount"),
                    (2, "infl_amount_pr"),
                    (6, "industry_daily_change_counter"),
                ],
                &[{
                    let mut record = Vec::new();
                    record.extend_from_slice(&123_456u64.to_be_bytes());
                    record.extend_from_slice(&234_567u64.to_be_bytes());
                    record.extend_from_slice(&(-7i16).to_be_bytes());
                    record.extend_from_slice(&[13, 4, 3]);
                    record.extend_from_slice(&77u32.to_be_bytes());
                    record
                }],
            ),
        };
        let economy = global_economy_from_chunks(&[chunk]);
        assert_eq!(economy.inflation_prices, 123_456);
        assert_eq!(economy.inflation_payment, 234_567);
        assert_eq!(economy.fluct, -7);
        assert_eq!(economy.interest_rate, 13);
        assert_eq!(economy.infl_amount, 4);
        assert_eq!(economy.infl_amount_pr, 3);
        assert_eq!(economy.industry_daily_change_counter, 77);
    }
}
