//! Estado persistente del pool nativo `Engine` (`ENGN`).
//!
//! El catálogo del port usa IDs normalizados por clase de vehículo, mientras
//! que `OpenTTD` guarda los slots globales del pool. `EIDS` permite resolver
//! motores `NewGRF` por `(GRFID, ID local)` cuando el catálogo está cargado;
//! los slots sin correspondencia y las columnas que todavía no modelamos
//! siguen pasando por `SavOpaqueChunk`.

use crate::CompanyId;

use super::SavOpaqueChunk;
use super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};

const INVALID_GRFID: u32 = u32::MAX;

/// Correspondencia persistente entre un slot del pool nativo y el `(GRFID,
/// ID local)` que lo ocupaba cuando se guardó la partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SavEngineMapping {
    pub(crate) engine_id: u16,
    pub(crate) grfid: u32,
    pub(crate) internal_id: u16,
    pub(crate) vehicle_type: u8,
    pub(crate) substitute_id: u8,
}

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

fn value_u32(record: &SlRecord, field: &str) -> Option<u32> {
    record_get(record, field)
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
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

/// Lee `EIDS`, que `OpenTTD` guarda como una tabla densa indexada por
/// `EngineID`. El chunk puede faltar en saves viejos; en ese caso el puente
/// vanilla de abajo sigue disponible.
pub(crate) fn mappings_from_opaque(chunks: &[SavOpaqueChunk]) -> Vec<SavEngineMapping> {
    let Some(chunk) = chunks.iter().find(|chunk| chunk.name == *b"EIDS") else {
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
            Some(SavEngineMapping {
                engine_id: u16::try_from(engine_id).ok()?,
                grfid: value_u32(&record, "grfid")?,
                internal_id: value_u16(&record, "internal_id")?,
                vehicle_type: value_u8(&record, "type")?,
                substitute_id: value_u8(&record, "substitute_id")?,
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

fn vehicle_kind_matches_native_type(kind: crate::vehicle::VehicleKind, native_type: u8) -> bool {
    match native_type {
        0 => kind == crate::vehicle::VehicleKind::Train,
        1 => matches!(
            kind,
            crate::vehicle::VehicleKind::Bus
                | crate::vehicle::VehicleKind::Truck
                | crate::vehicle::VehicleKind::Tram
        ),
        2 => kind == crate::vehicle::VehicleKind::Ship,
        3 => kind == crate::vehicle::VehicleKind::Aircraft,
        _ => false,
    }
}

/// Traduce un slot nativo usando `EIDS` cuando la entrada pertenece a un GRF
/// activo. La correspondencia vanilla sigue siendo el fallback para saves sin
/// `EIDS` y para las filas cuyo GRFID es el centinela nativo.
pub(crate) fn catalog_engine_id_for_native_in(
    native_id: u16,
    mappings: &[SavEngineMapping],
    catalog: &[crate::engine::EngineDef],
) -> Option<u16> {
    if let Some(mapping) = mappings
        .iter()
        .find(|mapping| mapping.engine_id == native_id)
        && mapping.grfid != INVALID_GRFID
    {
        return catalog
            .iter()
            .find(|engine| {
                engine.newgrf_grfid == mapping.grfid
                    && engine.newgrf_local_id == mapping.internal_id
                    && vehicle_kind_matches_native_type(engine.kind, mapping.vehicle_type)
            })
            .map(|engine| engine.id);
    }
    catalog_engine_id_for_native(native_id)
}

/// Rehidrata las excepciones de disponibilidad y las previews activas del
/// pool nativo en las estructuras que consume el runtime propio.
pub(crate) fn hydrate_state_from_pool(
    state: &mut crate::GameState,
    engine_states: &[SavEngineState],
    mappings: &[SavEngineMapping],
) {
    for saved in engine_states {
        let Some(catalog_id) =
            catalog_engine_id_for_native_in(saved.engine_id, mappings, &state.engine_catalog)
        else {
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
    fn resolves_newgrf_engine_from_eids_mapping() {
        let mut custom = crate::engine::engine_for_vehicle(
            crate::vehicle::VehicleKind::Train,
            crate::engine::ENGINE_TRAIN_KIRBY,
        )
        .clone();
        custom.id = 60_000;
        custom.newgrf_grfid = 0x4355_5301;
        custom.newgrf_local_id = 42;
        custom.from_newgrf = true;
        let record = {
            let mut record = Vec::new();
            record.extend_from_slice(&custom.newgrf_grfid.to_be_bytes());
            record.extend_from_slice(&custom.newgrf_local_id.to_be_bytes());
            record.push(0); // VEH_TRAIN
            record.push(0); // substitute_id
            record
        };
        let chunk = SavOpaqueChunk {
            name: *b"EIDS",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (6, "grfid"),
                    (4, "internal_id"),
                    (2, "type"),
                    (2, "substitute_id"),
                ],
                &[record],
            ),
        };
        let mappings = mappings_from_opaque(&[chunk]);
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].engine_id, 0);
        assert_eq!(
            catalog_engine_id_for_native_in(0, &mappings, &[custom]),
            Some(60_000)
        );
    }

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
