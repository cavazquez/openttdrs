//! Serialización del mapping de objetos `NewGRF` (`OBID`).

use super::super::SavError;
use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::super::table::{field_byte_ranges, parse_table_layout};
use super::chunks::{raw_table_chunk, table_chunk};
use crate::game_state::GameState;
use crate::sav::SavObjectMapping;
use crate::tnbp_decode::read_sl_gamma;

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

    // Si el mapping proviene de un save y una mutación sólo cambió valores
    // conocidos, fusionar las filas sobre la tabla original conserva campos
    // añadidos por versiones futuras de OpenTTD. Si aparecen/desaparecen IDs o
    // la cabecera usa una forma que no podemos indexar, se cae al formato
    // canónico de abajo para no emitir una tabla inconsistente.
    if !state.object_mappings.is_empty()
        && let Some(merged) = merge_original_mapping_columns(state, &mappings)?
    {
        return Ok(Some(merged));
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

#[derive(Debug)]
struct RawMappingRecord {
    index: u32,
    bytes: Vec<u8>,
    payload_offset: usize,
}

/// Fusiona `grfid/entity_id/substitute_id` sobre un `OBID` importado.
///
/// Se conserva el header original y cada registro, incluidos los campos
/// desconocidos y los huecos de una tabla densa. Sólo se acepta la fusión si
/// el conjunto de IDs no cambió; agregar/eliminar filas usa el writer
/// canónico, que es seguro para un mapping nuevo pero no puede inventar los
/// valores de columnas futuras.
fn merge_original_mapping_columns(
    state: &GameState,
    mappings: &[SavObjectMapping],
) -> Result<Option<Vec<u8>>, SavError> {
    let Some(original) = state
        .sav_opaque_chunks
        .iter()
        .find(|chunk| chunk.name == *b"OBID")
    else {
        return Ok(None);
    };
    if !matches!(original.ch_type, CH_TABLE | CH_SPARSE_TABLE) {
        return Ok(None);
    }
    let sparse = original.ch_type == CH_SPARSE_TABLE;
    let Ok((header_start, header_end, fields)) = parse_table_layout(&original.body) else {
        return Ok(None);
    };

    let mut records = Vec::new();
    let mut off = header_end;
    let mut dense_index = 0u32;
    loop {
        let Ok(length) = read_sl_gamma(&original.body, &mut off) else {
            return Ok(None);
        };
        if length == 0 {
            break;
        }
        let record_len = usize::try_from(length.saturating_sub(1)).unwrap_or(usize::MAX);
        let record_start = off;
        let record_end = record_start.saturating_add(record_len);
        if record_end > original.body.len() {
            return Ok(None);
        }
        let (index, payload_offset) = if sparse {
            let mut index_off = record_start;
            let Ok(index) = read_sl_gamma(&original.body, &mut index_off) else {
                return Ok(None);
            };
            (index, index_off.saturating_sub(record_start))
        } else {
            (dense_index, 0)
        };
        records.push(RawMappingRecord {
            index,
            bytes: original.body[record_start..record_end].to_vec(),
            payload_offset,
        });
        off = record_end;
        dense_index = dense_index.saturating_add(1);
    }

    let mut mapping_by_id = std::collections::BTreeMap::new();
    for mapping in mappings {
        mapping_by_id.insert(u32::from(mapping.object_type), mapping);
    }
    let original_ids: std::collections::BTreeSet<u32> = records
        .iter()
        .filter(|record| !record.bytes.is_empty())
        .map(|record| record.index)
        .collect();
    if original_ids != mapping_by_id.keys().copied().collect() {
        return Ok(None);
    }

    let mut patched = Vec::with_capacity(records.len());
    for mut record in records {
        let Some(mapping) = mapping_by_id.get(&record.index) else {
            patched.push(record.bytes);
            continue;
        };
        let payload = &record.bytes[record.payload_offset..];
        let Ok(ranges) = field_byte_ranges(&fields, payload) else {
            return Ok(None);
        };
        let mut found = [false; 3];
        for (name, start, end) in ranges {
            let start = start.saturating_add(record.payload_offset);
            let end = end.saturating_add(record.payload_offset);
            let width = end.saturating_sub(start);
            let value = match name.as_str() {
                "grfid" if width == 4 => {
                    found[0] = true;
                    Some(mapping.grfid.to_be_bytes().to_vec())
                }
                "entity_id" if width == 2 => {
                    found[1] = true;
                    Some(mapping.entity_id.to_be_bytes().to_vec())
                }
                "substitute_id" if width == 2 => {
                    found[2] = true;
                    Some(mapping.substitute_id.to_be_bytes().to_vec())
                }
                _ => None,
            };
            if let Some(value) = value {
                record.bytes[start..end].copy_from_slice(&value);
            }
        }
        if found != [true, true, true] {
            return Ok(None);
        }
        patched.push(record.bytes);
    }

    let header = &original.body[header_start..header_end];
    raw_table_chunk(*b"OBID", header, &patched, original.ch_type).map(Some)
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
            newgrf_grf_version: 0,
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

    #[test]
    fn merges_unknown_columns_when_mapping_changes() {
        let mut state = GameState::new(64, 64);
        state.object_mappings.push(SavObjectMapping {
            object_type: 0,
            grfid: 0xDEAD_BEEF,
            entity_id: 4,
            substitute_id: 9,
        });
        // OBID original: grfid/entity/substitute + una columna futura. El
        // último byte debe sobrevivir al parcheo de los tres campos conocidos.
        let mut fields = Vec::new();
        for (ty, name) in [
            (6, "grfid"),
            (4, "entity_id"),
            (4, "substitute_id"),
            (2, "future"),
        ] {
            fields.push(ty);
            super::super::codec::write_str(name, &mut fields).expect("name");
        }
        fields.push(0);
        let mut record = Vec::new();
        record.extend_from_slice(&[0, 0, 0, 1]);
        record.extend_from_slice(&[0, 2]);
        record.extend_from_slice(&[0, 3]);
        record.push(0xA5);
        let original = raw_table_chunk(*b"OBID", &fields, &[record], CH_TABLE).expect("chunk");
        state.sav_opaque_chunks.push(crate::SavOpaqueChunk {
            name: *b"OBID",
            ch_type: CH_TABLE,
            body: original[5..].to_vec(),
        });
        let merged = merge_original_mapping_columns(&state, &state.object_mappings)
            .expect("merge")
            .expect("OBID");
        let chunks = parse_chunks(&merged).expect("parse");
        let rows = parse_table_chunk(&chunks[0].body, false).expect("rows");
        let row = &rows[0].1;
        assert_eq!(
            record_get(row, "grfid").and_then(SlValue::as_u64),
            Some(0xDEAD_BEEF)
        );
        assert_eq!(
            record_get(row, "entity_id").and_then(SlValue::as_u64),
            Some(4)
        );
        assert_eq!(
            record_get(row, "substitute_id").and_then(SlValue::as_u64),
            Some(9)
        );
        assert_eq!(
            record_get(row, "future").and_then(SlValue::as_u64),
            Some(0xA5)
        );
    }
}
