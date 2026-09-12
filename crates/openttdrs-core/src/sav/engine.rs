//! Estado persistente del pool nativo `Engine` (`ENGN`).
//!
//! El catálogo del port usa IDs normalizados por clase de vehículo, mientras
//! que `OpenTTD` guarda los slots globales del pool. Este módulo mantiene el
//! puente sólo para los motores vanilla que el runtime identifica sin
//! ambigüedad; los motores `NewGRF` y las columnas que todavía no modelamos
//! siguen pasando por `SavOpaqueChunk`.

use crate::CompanyId;

use super::SavOpaqueChunk;
use super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};

/// Parte del registro `Engine` que afecta a previews y disponibilidad por
/// compañía. Las métricas de fiabilidad y las columnas futuras permanecen en
/// el cuerpo opaco del chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SavEngineState {
    pub(crate) engine_id: u16,
    pub(crate) company_avail: u16,
    pub(crate) preview_asked: u16,
    pub(crate) preview_company: Option<CompanyId>,
    pub(crate) preview_wait: u8,
}

fn value_u16(record: &SlRecord, field: &str) -> Option<u16> {
    record_get(record, field)
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn value_u8(record: &SlRecord, field: &str) -> Option<u8> {
    record_get(record, field)
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
}

/// Decodifica el subconjunto estable de `ENGN` que necesita el runtime.
///
/// Un chunk ausente, legacy o con un schema que no se pueda leer se deja como
/// passthrough: cargar el resto del save no debe depender de esta mejora.
pub(crate) fn states_from_opaque(chunks: &[SavOpaqueChunk]) -> Vec<SavEngineState> {
    let Some(chunk) = chunks.iter().find(|chunk| chunk.name == *b"ENGN") else {
        return Vec::new();
    };
    let sparse = match chunk.ch_type {
        CH_TABLE => false,
        CH_SPARSE_TABLE => true,
        _ => return Vec::new(),
    };
    let Ok(rows) = parse_table_chunk(&chunk.body, sparse) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter_map(|(engine_id, record)| {
            Some(SavEngineState {
                engine_id: u16::try_from(engine_id).ok()?,
                company_avail: value_u16(&record, "company_avail").unwrap_or(0),
                preview_asked: value_u16(&record, "preview_asked").unwrap_or(0),
                preview_company: value_u8(&record, "preview_company")
                    .filter(|company| *company != u8::MAX)
                    .map(CompanyId),
                preview_wait: value_u8(&record, "preview_wait").unwrap_or(0),
            })
        })
        .collect()
}

/// IDs vanilla del catálogo del port equivalentes a los slots nativos que
/// tienen una contraparte exacta en la representación actual.
pub(crate) fn catalog_engine_id_for_native(native_id: u16) -> Option<u16> {
    let id = match native_id {
        0 => crate::engine::ENGINE_TRAIN_KIRBY,
        8 => crate::engine::ENGINE_TRAIN_CHANEY_JUBILEE,
        9 => crate::engine::ENGINE_TRAIN_GINZU_A4,
        10 => crate::engine::ENGINE_TRAIN_SH_8P,
        11 => crate::engine::ENGINE_TRAIN_MANLEY_MOREL,
        12 => crate::engine::ENGINE_TRAIN_DASH,
        13 => crate::engine::ENGINE_TRAIN_SH_HENDRY_25,
        14 => crate::engine::ENGINE_TRAIN_UU_37,
        15 => crate::engine::ENGINE_TRAIN_FLOSS_47,
        22 => crate::engine::ENGINE_TRAIN_SH_125,
        23 => crate::engine::ENGINE_TRAIN_SH_30,
        24 => crate::engine::ENGINE_TRAIN_SH_40,
        25 => crate::engine::ENGINE_TRAIN_TIM,
        26 => crate::engine::ENGINE_TRAIN_ASIASTAR,
        116 => crate::engine::ENGINE_BUS_MPS,
        117 => crate::engine::ENGINE_BUS_HEREFORD,
        118 => crate::engine::ENGINE_BUS_FOSTER,
        126 => crate::engine::ENGINE_TRUCK_MPS,
        138 => crate::engine::ENGINE_TRUCK_BALOGH_GOODS,
        139 => crate::engine::ENGINE_TRUCK_CRAIGHEAD_GOODS,
        140 => crate::engine::ENGINE_TRUCK_GOSS_GOODS,
        204 => crate::engine::ENGINE_SHIP_OIL,
        206 => crate::engine::ENGINE_SHIP_MPS,
        207 => crate::engine::ENGINE_SHIP_FERRY,
        253 => crate::engine::ENGINE_AIRCRAFT_TRICARIO,
        _ => return None,
    };
    Some(id)
}

/// Rehidrata las excepciones de disponibilidad y las previews activas del
/// pool nativo en las estructuras que consume el runtime propio.
pub(crate) fn hydrate_state_from_pool(
    state: &mut crate::GameState,
    engine_states: &[SavEngineState],
) {
    for saved in engine_states {
        let Some(catalog_id) = catalog_engine_id_for_native(saved.engine_id) else {
            continue;
        };
        for company_index in 0..u16::BITS {
            if saved.company_avail & (1 << company_index) == 0 {
                continue;
            }
            let Ok(company_id) = u8::try_from(company_index) else {
                continue;
            };
            if let Some(company) = state
                .companies
                .iter_mut()
                .find(|company| company.id == CompanyId(company_id))
            {
                company.grant_engine_preview(catalog_id);
            }
        }
        if let Some(company) = saved.preview_company
            && saved.preview_wait > 0
        {
            state.runtime.engine_preview_offers.insert(
                catalog_id,
                crate::game_state::EnginePreviewOffer {
                    company: Some(company),
                    wait_days: saved.preview_wait,
                    asked_companies: saved.preview_asked,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sav::SavOpaqueChunk;
    use crate::sav::chunks::CH_TABLE;
    use crate::sav::table::tests::build_table_body;

    #[test]
    fn reads_engine_preview_fields_from_native_table() {
        let mut record = Vec::new();
        record.extend_from_slice(&0x0003_u16.to_be_bytes());
        record.push(1);
        record.push(19);
        record.extend_from_slice(&7_u16.to_be_bytes());
        let chunk = SavOpaqueChunk {
            name: *b"ENGN",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (4, "preview_asked"),
                    (2, "preview_company"),
                    (2, "preview_wait"),
                    (4, "company_avail"),
                ],
                &[record],
            ),
        };
        let states = states_from_opaque(&[chunk]);
        assert_eq!(
            states,
            vec![SavEngineState {
                engine_id: 0,
                company_avail: 7,
                preview_asked: 3,
                preview_company: Some(CompanyId(1)),
                preview_wait: 19,
            }]
        );
    }

    #[test]
    fn leaves_invalid_preview_company_without_an_offer() {
        let record = vec![u8::MAX, 0];
        let chunk = SavOpaqueChunk {
            name: *b"ENGN",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "preview_company"), (2, "preview_wait")], &[record]),
        };
        let states = states_from_opaque(&[chunk]);
        assert_eq!(states[0].preview_company, None);
        assert_eq!(states[0].preview_wait, 0);
    }
}
