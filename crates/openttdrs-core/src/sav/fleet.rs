//! Pools de flota opcionales (`GRPS` y `ERNW`) del savegame nativo.

use std::collections::{HashMap, HashSet};

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};
use crate::autoreplace::AutoReplaceRule;
use crate::company::CompanyId;
use crate::vehicle_group::VehicleGroup;

/// El pool `EngineRenew` de `OpenTTD` usa `PoolID<u16, ..., 64000, 0xFFFF>`.
const ENGINE_RENEW_POOL_CAPACITY: u16 = 64_000;
const ALL_GROUP: u16 = 0xFFFD;
const DEFAULT_GROUP: u16 = 0xFFFE;

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
        .filter_map(|(index, record)| {
            let sav_pool_id = u16::try_from(index)
                .ok()
                .filter(|id| *id < ENGINE_RENEW_POOL_CAPACITY)?;
            let from = record_get(&record, "from")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            let to = record_get(&record, "to")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            let raw_group_id = record_get(&record, "group_id")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            let (group_id, default_group_only) = match raw_group_id {
                Some(ALL_GROUP) | None => (None, false),
                Some(DEFAULT_GROUP) => (None, true),
                Some(group_id) => (Some(u32::from(group_id)), false),
            };
            let only_when_old = record_get(&record, "replace_when_old")
                .and_then(SlValue::as_u64)
                .is_some_and(|value| value != 0);
            let sav_next_pool_id = record_get(&record, "next")
                .and_then(SlValue::as_u64)
                .and_then(|reference| reference.checked_sub(1))
                .and_then(|id| u16::try_from(id).ok())
                .filter(|id| *id < ENGINE_RENEW_POOL_CAPACITY);
            Some(AutoReplaceRule {
                from_engine_id: from,
                to_engine_id: to,
                owner: None,
                enabled: true,
                only_when_old,
                group_id,
                default_group_only,
                sav_pool_id: Some(sav_pool_id),
                sav_next_pool_id,
            })
        })
        .collect()
}

/// Une los nodos de `ERNW` a la compañía que los referencia desde
/// `PLYR.settings.engine_renew_list`.
///
/// La lista es un pool global y las reglas no traen owner propio. `OpenTTD`
/// guarda la pertenencia exclusivamente en la cabeza de cada compañía; al
/// restaurarla aquí evitamos que una regla de otra empresa se aplique al
/// jugador durante la simulación o al reexportar.
pub(crate) fn assign_autoreplace_owners(
    rules: &mut [AutoReplaceRule],
    companies: &[super::entities::SavCompany],
) {
    let pool_indexes: HashMap<u16, usize> = rules
        .iter()
        .enumerate()
        .filter_map(|(index, rule)| rule.sav_pool_id.map(|pool_id| (pool_id, index)))
        .collect();

    for company in companies {
        let Ok(owner) = u8::try_from(company.id) else {
            continue;
        };
        let mut next = company.engine_renew_list_head;
        let mut seen = HashSet::new();
        while let Some(pool_id) = next {
            if !seen.insert(pool_id) {
                break;
            }
            let Some(&rule_index) = pool_indexes.get(&pool_id) else {
                break;
            };
            let rule = &mut rules[rule_index];
            rule.owner = Some(CompanyId(owner));
            next = rule.sav_next_pool_id;
        }
    }
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

    #[test]
    fn preserves_ernw_pool_links_group_scope_and_company_owner() {
        let mut first = Vec::new();
        first.extend_from_slice(&10_u16.to_be_bytes());
        first.extend_from_slice(&11_u16.to_be_bytes());
        first.extend_from_slice(&3_u32.to_be_bytes()); // ref -> pool index 2
        first.extend_from_slice(&ALL_GROUP.to_be_bytes());
        first.push(1);

        let mut second = Vec::new();
        second.extend_from_slice(&20_u16.to_be_bytes());
        second.extend_from_slice(&21_u16.to_be_bytes());
        second.extend_from_slice(&0_u32.to_be_bytes()); // null ref
        second.extend_from_slice(&DEFAULT_GROUP.to_be_bytes());
        second.push(0);

        let body = build_table_body(
            &[
                (4, "from"),
                (4, "to"),
                (6, "next"),
                (4, "group_id"),
                (1, "replace_when_old"),
            ],
            &[first, Vec::new(), second],
        );
        let chunks = vec![RawChunk {
            name: *b"ERNW",
            ch_type: CH_TABLE,
            body,
        }];

        let mut rules = autoreplace_rules_from_chunks(&chunks);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].sav_pool_id, Some(0));
        assert_eq!(rules[0].sav_next_pool_id, Some(2));
        assert_eq!(rules[0].group_id, None);
        assert!(!rules[0].default_group_only);
        assert!(rules[0].only_when_old);
        assert_eq!(rules[1].sav_pool_id, Some(2));
        assert_eq!(rules[1].sav_next_pool_id, None);
        assert_eq!(rules[1].group_id, None);
        assert!(rules[1].default_group_only);

        let companies = vec![super::super::entities::SavCompany {
            id: 1,
            money: 0,
            loan: None,
            max_loan: None,
            colour: 0,
            name: None,
            president_name: None,
            manager_face: None,
            manager_face_style: None,
            money_fraction: None,
            block_preview: None,
            hq_tile: None,
            last_build_tile: None,
            inaugurated_year: None,
            inaugurated_year_calendar: None,
            is_ai: None,
            bankruptcy_months: None,
            bankruptcy_asked: None,
            bankruptcy_timeout: None,
            bankruptcy_value: None,
            cur_economy: None,
            old_economy: Vec::new(),
            liveries: Vec::new(),
            engine_renew_list_head: Some(0),
            engine_renew: None,
            engine_renew_months: None,
            engine_renew_money: None,
            renew_keep_length: None,
            servint_ispercent: None,
            servint_trains: None,
            servint_roadveh: None,
            servint_aircraft: None,
            servint_ships: None,
        }];
        assign_autoreplace_owners(&mut rules, &companies);
        assert_eq!(rules[0].owner, Some(CompanyId(1)));
        assert_eq!(rules[1].owner, Some(CompanyId(1)));
    }
}
