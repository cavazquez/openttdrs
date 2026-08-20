//! Writers de los pools de grupos y autoreemplazo del save nativo.

use super::super::SavError;
use super::super::chunks::CH_TABLE;
use super::chunks::raw_table_chunk;
use super::codec::write_str;
use crate::autoreplace::AutoReplaceRule;
use crate::game_state::GameState;
use crate::vehicle_group::VehicleGroup;

fn append_field(header: &mut Vec<u8>, ftype: u8, name: &str) -> Result<(), SavError> {
    header.push(ftype);
    write_str(name, header)
}

pub(super) fn groups_chunk(groups: &[VehicleGroup]) -> Result<Option<Vec<u8>>, SavError> {
    if groups.is_empty() {
        return Ok(None);
    }
    let mut header = Vec::new();
    append_field(&mut header, 0x0A | 0x10, "name")?;
    append_field(&mut header, 2, "owner")?;
    append_field(&mut header, 2, "vehicle_type")?;
    append_field(&mut header, 2, "flags")?;
    append_field(&mut header, 2, "livery.in_use")?;
    append_field(&mut header, 2, "livery.colour1")?;
    append_field(&mut header, 2, "livery.colour2")?;
    append_field(&mut header, 4, "parent")?;
    append_field(&mut header, 4, "number")?;
    header.push(0);

    // GRPS es CH_TABLE denso: la posición de la fila es el GroupID de pool.
    // Conservar huecos permite round-trippear IDs no consecutivos y evita
    // confundirlos con `number`, que es sólo el ordinal visible por empresa.
    let max_id = groups.iter().map(|group| group.id).max().unwrap_or(0);
    let max_id = usize::try_from(max_id.min(u32::from(u16::MAX))).unwrap_or(0);
    let mut records = vec![Vec::new(); max_id.saturating_add(1)];
    for group in groups {
        let mut record = Vec::new();
        write_str(&group.name, &mut record)?;
        record.push(group.owner);
        record.push(group.vehicle_type);
        record.push(group.flags);
        record.push(group.livery_in_use);
        record.push(group.livery_colour1);
        record.push(group.livery_colour2);
        let parent = group
            .parent
            .unwrap_or(u32::from(u16::MAX))
            .min(u32::from(u16::MAX)) as u16;
        record.extend_from_slice(&parent.to_be_bytes());
        let number = group.number.min(u32::from(u16::MAX)) as u16;
        record.extend_from_slice(&number.to_be_bytes());
        if let Ok(index) = usize::try_from(group.id)
            && index < records.len()
        {
            records[index] = record;
        }
    }
    Ok(Some(raw_table_chunk(
        *b"GRPS", &header, &records, CH_TABLE,
    )?))
}

pub(super) fn autoreplace_chunk(rules: &[AutoReplaceRule]) -> Result<Option<Vec<u8>>, SavError> {
    if rules.is_empty() {
        return Ok(None);
    }
    let mut header = Vec::new();
    append_field(&mut header, 4, "from")?;
    append_field(&mut header, 4, "to")?;
    append_field(&mut header, 4, "next")?;
    append_field(&mut header, 4, "group_id")?;
    append_field(&mut header, 2, "replace_when_old")?;
    header.push(0);

    let mut records = Vec::with_capacity(rules.len());
    for rule in rules {
        let mut record = Vec::new();
        record.extend_from_slice(&rule.from_engine_id.to_be_bytes());
        record.extend_from_slice(&rule.to_engine_id.to_be_bytes());
        record.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        let group_id = rule.group_id.unwrap_or(0xFFFE).min(u32::from(u16::MAX)) as u16;
        record.extend_from_slice(&group_id.to_be_bytes());
        record.push(u8::from(rule.only_when_old));
        records.push(record);
    }
    Ok(Some(raw_table_chunk(
        *b"ERNW", &header, &records, CH_TABLE,
    )?))
}

pub(super) fn fleet_chunks(state: &GameState) -> Result<Vec<u8>, SavError> {
    let mut chunks = Vec::new();
    if let Some(groups) = groups_chunk(&state.vehicle_groups)? {
        chunks.extend_from_slice(&groups);
    }
    if let Some(renew) = autoreplace_chunk(&state.autoreplace_rules)? {
        chunks.extend_from_slice(&renew);
    }
    Ok(chunks)
}
