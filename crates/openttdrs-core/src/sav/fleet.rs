//! Pools de flota opcionales (`GRPS` y `ERNW`) del savegame nativo.

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};
use crate::autoreplace::AutoReplaceRule;
use crate::vehicle_group::VehicleGroup;

fn chunks_record(name: &str, chunks: &[RawChunk]) -> Option<Vec<(u32, super::table::SlRecord)>> {
    let chunk = find_chunk(chunks, name)?;
    parse_table_chunk(&chunk.body, false).ok()
}

#[must_use]
pub(crate) fn vehicle_groups_from_chunks(chunks: &[RawChunk]) -> Vec<VehicleGroup> {
    let Some(rows) = chunks_record("GRPS", chunks) else {
        return Vec::new();
    };
    rows.into_iter()
        .map(|(index, record)| {
            let id = record_get(&record, "number")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(index);
            let name = record_get(&record, "name")
                .and_then(SlValue::as_str)
                .unwrap_or("Grupo")
                .to_owned();
            VehicleGroup::new(id, name)
        })
        .collect()
}

#[must_use]
pub(crate) fn autoreplace_rules_from_chunks(chunks: &[RawChunk]) -> Vec<AutoReplaceRule> {
    let Some(rows) = chunks_record("ERNW", chunks) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter_map(|(_, record)| {
            let from = record_get(&record, "from")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            let to = record_get(&record, "to")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            let group_id = record_get(&record, "group_id")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value != 0xFFFE && *value != 0xFFFD);
            let only_when_old = record_get(&record, "replace_when_old")
                .and_then(SlValue::as_u64)
                .is_some_and(|value| value != 0);
            Some(AutoReplaceRule {
                from_engine_id: from,
                to_engine_id: to,
                enabled: true,
                only_when_old,
                group_id,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sav::chunks::CH_TABLE;
    use crate::sav::table::tests::build_table_body;

    #[test]
    fn parses_group_name_and_number() {
        let body = build_table_body(
            &[(0x0A | 0x10, "name"), (4, "number")],
            &[{
                let mut record = Vec::new();
                record.push(5); // gamma(5)
                record.extend_from_slice(b"Carga");
                record.extend_from_slice(&7_u16.to_be_bytes());
                record
            }],
        );
        let chunks = vec![RawChunk {
            name: *b"GRPS",
            ch_type: CH_TABLE,
            body,
        }];
        assert_eq!(
            vehicle_groups_from_chunks(&chunks),
            vec![VehicleGroup::new(7, "Carga")]
        );
    }
}
