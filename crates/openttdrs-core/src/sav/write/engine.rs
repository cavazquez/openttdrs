//! Actualización acotada del pool `Engine` al reexportar un SAV nativo.
//!
//! `ENGN` permanece opaco para conservar columnas de versiones futuras. Sólo
//! se reemplazan los cuatro campos de preview que el runtime modela, siempre
//! que el layout y el tamaño de la fila sean compatibles.

use crate::game_state::GameState;
use crate::sav::SavError;
use crate::sav::SavOpaqueChunk;
use crate::sav::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use crate::sav::table::{field_byte_ranges, parse_table_layout};

use super::chunks::raw_chunk;

fn gamma(data: &[u8], offset: &mut usize) -> Result<u32, SavError> {
    crate::tnbp_decode::read_sl_gamma(data, offset)
        .map_err(|error| SavError::BadFormat(format!("gamma ENGN inválido: {error:?}")))
}

fn field_range(ranges: &[(String, usize, usize)], name: &str) -> Option<(usize, usize)> {
    ranges
        .iter()
        .find(|(field, _, _)| field == name)
        .map(|(_, start, end)| (*start, *end))
}

fn read_u16(record: &[u8], ranges: &[(String, usize, usize)], name: &str) -> Option<u16> {
    let (start, end) = field_range(ranges, name)?;
    (end - start == 2).then(|| u16::from_be_bytes([record[start], record[start + 1]]))
}

fn replace_field(
    record: &mut [u8],
    ranges: &[(String, usize, usize)],
    name: &str,
    bytes: &[u8],
) -> bool {
    let Some((start, end)) = field_range(ranges, name) else {
        return false;
    };
    if end - start != bytes.len() {
        return false;
    }
    if record[start..end] == *bytes {
        return false;
    }
    record[start..end].copy_from_slice(bytes);
    true
}

fn company_avail_after_runtime(state: &GameState, catalog_id: u16, raw: u16) -> u16 {
    state
        .companies
        .iter()
        .filter(|company| company.has_available_engine(catalog_id))
        .filter_map(|company| (company.id.0 < 16).then_some(company.id.0))
        .fold(raw, |mask, company| mask | (1_u16 << company))
}

fn patch_record(
    state: &GameState,
    native_engine_id: u16,
    record: &mut [u8],
    ranges: &[(String, usize, usize)],
) -> bool {
    let Some(catalog_id) = crate::sav::engine::catalog_engine_id_for_native(native_engine_id)
    else {
        return false;
    };

    let Some(raw_company_avail) = read_u16(record, ranges, "company_avail") else {
        return false;
    };
    let mut changed = false;
    let company_avail = company_avail_after_runtime(state, catalog_id, raw_company_avail);
    changed |= replace_field(
        record,
        ranges,
        "company_avail",
        &company_avail.to_be_bytes(),
    );

    let accepted = state
        .companies
        .iter()
        .any(|company| company.has_available_engine(catalog_id));
    if accepted {
        changed |= replace_field(record, ranges, "preview_asked", &u16::MAX.to_be_bytes());
        changed |= replace_field(record, ranges, "preview_company", &[u8::MAX]);
        changed |= replace_field(record, ranges, "preview_wait", &[0]);
    } else if let Some(offer) = state.runtime.engine_preview_offers.get(&catalog_id) {
        changed |= replace_field(
            record,
            ranges,
            "preview_asked",
            &offer.asked_companies.to_be_bytes(),
        );
        changed |= replace_field(
            record,
            ranges,
            "preview_company",
            &[offer.company.map_or(u8::MAX, |company| company.0)],
        );
        changed |= replace_field(record, ranges, "preview_wait", &[offer.wait_days]);
    }
    changed
}

/// Actualiza campos de preview de un `ENGN` nativo conservando header,
/// columnas desconocidas, huecos del pool y framing gamma originales.
pub(super) fn patch_engine_chunk(
    state: &GameState,
    chunk: &SavOpaqueChunk,
) -> Result<Option<Vec<u8>>, SavError> {
    let sparse = match chunk.ch_type {
        CH_TABLE => false,
        CH_SPARSE_TABLE => true,
        _ => return Ok(None),
    };
    let Ok((_, header_end, fields)) = parse_table_layout(&chunk.body) else {
        return Ok(None);
    };
    let mut offset = header_end;
    let mut dense_index = 0_u32;
    let mut changed = false;
    let mut body = chunk.body[..header_end].to_vec();

    loop {
        let record_start = offset;
        let Ok(length) = gamma(&chunk.body, &mut offset) else {
            return Ok(None);
        };
        if length == 0 {
            body.extend_from_slice(&chunk.body[record_start..offset]);
            break;
        }
        let payload_len = usize::try_from(length - 1)
            .map_err(|_| SavError::BadFormat("registro ENGN demasiado grande".into()))?;
        let payload_start = offset;
        let record_end = match payload_start.checked_add(payload_len) {
            Some(end) if end <= chunk.body.len() => end,
            _ => return Ok(None),
        };
        let mut fields_start = payload_start;
        let native_engine_id = if sparse {
            match gamma(&chunk.body, &mut fields_start) {
                Ok(index) => index,
                Err(_) => return Ok(None),
            }
        } else {
            dense_index
        };
        let Ok(native_engine_id) = u16::try_from(native_engine_id) else {
            return Ok(None);
        };
        let mut record = chunk.body[fields_start..record_end].to_vec();
        let Ok(ranges) = field_byte_ranges(&fields, &record) else {
            return Ok(None);
        };
        if !record.is_empty() {
            let record_changed = patch_record(state, native_engine_id, &mut record, &ranges);
            changed |= record_changed;
        }
        if record == chunk.body[fields_start..record_end] {
            body.extend_from_slice(&chunk.body[record_start..record_end]);
        } else {
            body.extend_from_slice(&chunk.body[record_start..fields_start]);
            body.extend_from_slice(&record);
        }
        offset = record_end;
        dense_index = dense_index.saturating_add(1);
    }

    if !changed {
        return Ok(None);
    }
    Ok(Some(raw_chunk(chunk.name, chunk.ch_type, &body)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ENGINE_TRAIN_KIRBY;
    use crate::game_state::EnginePreviewOffer;
    use crate::sav::SavOpaqueChunk;
    use crate::sav::chunks::CH_TABLE;
    use crate::sav::table::SlValue;
    use crate::sav::table::parse_table_chunk;
    use crate::sav::table::tests::build_table_body;

    fn engine_chunk(company_avail: u16, asked: u16, company: u8, wait: u8) -> SavOpaqueChunk {
        let mut record = Vec::new();
        record.extend_from_slice(&company_avail.to_be_bytes());
        record.extend_from_slice(&asked.to_be_bytes());
        record.push(company);
        record.push(wait);
        record.push(0xCA); // columna futura que debe viajar intacta.
        SavOpaqueChunk {
            name: *b"ENGN",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (4, "company_avail"),
                    (4, "preview_asked"),
                    (2, "preview_company"),
                    (2, "preview_wait"),
                    (2, "future_column"),
                ],
                &[record],
            ),
        }
    }

    #[test]
    fn accepted_preview_updates_native_pool_without_losing_future_fields() {
        let mut state = GameState::new(8, 8);
        state.companies[0].grant_engine_preview(ENGINE_TRAIN_KIRBY);
        let raw = engine_chunk(0, 1, 0, 20);
        let patched = patch_engine_chunk(&state, &raw)
            .expect("patch")
            .expect("accepted preview changes ENGN");
        let chunks = crate::sav::chunks::parse_chunks(&patched).expect("parse patched ENGN");
        let rows = parse_table_chunk(&chunks[0].body, false).expect("parse table");
        let record = &rows[0].1;
        assert_eq!(
            crate::sav::table::record_get(record, "company_avail").and_then(SlValue::as_u64),
            Some(1)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "preview_asked").and_then(SlValue::as_u64),
            Some(u64::from(u16::MAX))
        );
        assert_eq!(
            crate::sav::table::record_get(record, "preview_company").and_then(SlValue::as_u64),
            Some(u64::from(u8::MAX))
        );
        assert_eq!(
            crate::sav::table::record_get(record, "future_column").and_then(SlValue::as_u64),
            Some(0xCA)
        );
    }

    #[test]
    fn active_offer_updates_native_countdown() {
        let mut state = GameState::new(8, 8);
        state.runtime.engine_preview_offers.insert(
            ENGINE_TRAIN_KIRBY,
            EnginePreviewOffer {
                company: Some(crate::CompanyId::PLAYER),
                wait_days: 17,
                asked_companies: 3,
            },
        );
        let raw = engine_chunk(0, 0, u8::MAX, 0);
        let patched = patch_engine_chunk(&state, &raw)
            .expect("patch")
            .expect("active offer changes ENGN");
        let chunks = crate::sav::chunks::parse_chunks(&patched).expect("parse patched ENGN");
        let rows = parse_table_chunk(&chunks[0].body, false).expect("parse table");
        let record = &rows[0].1;
        assert_eq!(
            crate::sav::table::record_get(record, "preview_asked").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "preview_wait").and_then(SlValue::as_u64),
            Some(17)
        );
    }
}
