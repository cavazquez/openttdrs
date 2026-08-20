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
            // GRPS es una tabla densa indexada por el `GroupID` de pool. El
            // campo `number` sólo es el número visible por empresa y no puede
            // usarse para enlazar `VEHS.group_id`.
            let id = index;
            let number = record_get(&record, "number")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(index);
            let name = record_get(&record, "name")
                .and_then(SlValue::as_str)
                .unwrap_or("Grupo")
                .to_owned();
            let mut group = VehicleGroup::new(id, name);
            group.number = number;
            group.owner = record_get(&record, "owner")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(group.owner);
            group.vehicle_type = record_get(&record, "vehicle_type")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(group.vehicle_type);
            group.flags = record_get(&record, "flags")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(group.flags);
            group.livery_in_use = record_get(&record, "livery.in_use")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(group.livery_in_use);
            group.livery_colour1 = record_get(&record, "livery.colour1")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(group.livery_colour1);
            group.livery_colour2 = record_get(&record, "livery.colour2")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(group.livery_colour2);
            group.parent = record_get(&record, "parent")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value != u32::from(u16::MAX));
            group
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
        let mut expected = VehicleGroup::new(0, "Carga");
        expected.number = 7;
        assert_eq!(vehicle_groups_from_chunks(&chunks), vec![expected]);
    }
}
