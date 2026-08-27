//! Serialización del mapping de objetos `NewGRF` (`OBID`).

use super::super::SavError;
use super::chunks::table_chunk;
use crate::game_state::GameState;
use crate::sav::SavObjectMapping;

const MAX_OBJECT_MAPPING_ROWS_TO_EXPORT: usize = 1 << 20;

/// Reconstruye `OBID` desde el mapping importado o, para estados creados en
/// Rust, desde el catálogo de objetos `NewGRF` ya aplicado.
pub(super) fn object_mappings_chunk(state: &GameState) -> Result<Option<Vec<u8>>, SavError> {
    let mappings = if state.object_mappings.is_empty() {
        state
            .object_spec_catalog
            .iter()
            .filter(|spec| spec.from_newgrf && spec.grfid != 0)
            .map(|spec| SavObjectMapping {
                object_type: spec.id,
                grfid: spec.grfid,
                entity_id: u16::from(spec.local_id),
                // `substitute_id` is the runtime ObjectType selected when
                // the original local spec cannot be loaded. The catalog ID
                // is the closest lossless value available to this port.
                substitute_id: spec.id,
            })
            .collect::<Vec<_>>()
    } else {
        state.object_mappings.clone()
    };
    if mappings.is_empty() {
        return Ok(None);
    }

    let Some(max_id) = mappings.iter().map(|mapping| mapping.object_type).max() else {
        return Ok(None);
    };
    let rows = usize::from(max_id)
        .checked_add(1)
        .ok_or(SavError::AllocationFailed {
            context: "mapping OBID",
            requested: usize::MAX,
        })?;
    if rows > MAX_OBJECT_MAPPING_ROWS_TO_EXPORT {
        return Err(SavError::AllocationFailed {
            context: "mapping OBID",
            requested: rows,
        });
    }
    let mut records = vec![Vec::new(); rows];
    for mapping in mappings {
        let mut record = Vec::with_capacity(8);
        record.extend_from_slice(&mapping.grfid.to_be_bytes());
        record.extend_from_slice(&mapping.entity_id.to_be_bytes());
        record.extend_from_slice(&mapping.substitute_id.to_be_bytes());
        records[usize::from(mapping.object_type)] = record;
    }
    Ok(Some(table_chunk(
        *b"OBID",
        &[(6, "grfid"), (4, "entity_id"), (4, "substitute_id")],
        &records,
    )?))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::sav::chunks::{find_chunk, parse_chunks};
    use crate::sav::table::{SlValue, parse_table_chunk, record_get};

    #[test]
    fn writes_dense_object_mapping_and_uses_catalog_fallback() {
        let mut state = GameState::new(64, 64);
        state.object_spec_catalog.push(crate::ObjectSpecDef {
            id: 7,
            class_label: "OBJ ".into(),
            name: "Mapped object".into(),
            size: 0x11,
            from_newgrf: true,
            local_id: 4,
            grfid: 0x4f42_0001,
            climate_mask: 0x0f,
            build_cost_factor: 1,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        });
        let chunk = object_mappings_chunk(&state)
            .expect("encode")
            .expect("OBID");
        let chunks = parse_chunks(&chunk).expect("parse chunk");
        let obid = find_chunk(&chunks, "OBID").expect("OBID");
        let rows = parse_table_chunk(&obid.body, false).expect("parse table");
        assert_eq!(rows.len(), 1);
        let (id, record) = &rows[0];
        assert_eq!(*id, 7);
        assert_eq!(
            record_get(record, "grfid").and_then(SlValue::as_u64),
            Some(0x4f42_0001)
        );
        assert_eq!(
            record_get(record, "entity_id").and_then(SlValue::as_u64),
            Some(4)
        );
        assert_eq!(
            record_get(record, "substitute_id").and_then(SlValue::as_u64),
            Some(7)
        );
    }
}
