//! Writers de los pools de grupos y autoreemplazo del save nativo.

use std::collections::{BTreeMap, BTreeSet};

use super::super::SavError;
use super::super::chunks::CH_TABLE;
use super::chunks::raw_table_chunk;
use super::codec::write_str;
use crate::autoreplace::AutoReplaceRule;
use crate::company::CompanyId;
use crate::game_state::GameState;
use crate::vehicle_group::VehicleGroup;

const ENGINE_RENEW_POOL_CAPACITY: u16 = 64_000;
const ALL_GROUP: u16 = 0xFFFD;
const DEFAULT_GROUP: u16 = 0xFFFE;

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

#[derive(Debug, Clone, Copy)]
struct ExportAutoReplaceRule {
    rule: AutoReplaceRule,
    pool_id: u16,
    next_pool_id: Option<u16>,
}

/// Vista normalizada de `ERNW` compartida por el writer del pool y `PLYR`.
///
/// Las reglas viven en un pool global, pero `OpenTTD` las asocia a cada empresa
/// exclusivamente por la cabeza `settings.engine_renew_list`. Construir ambas
/// piezas desde la misma vista evita emitir referencias a un pool distinto.
#[derive(Debug, Clone)]
pub(super) struct AutoreplaceExport {
    rules: Vec<ExportAutoReplaceRule>,
    company_heads: BTreeMap<u8, u16>,
}

impl AutoreplaceExport {
    #[must_use]
    pub(super) fn company_head(&self, company: CompanyId) -> Option<u16> {
        self.company_heads.get(&company.0).copied()
    }
}

fn next_free_pool_id(used: &BTreeSet<u16>) -> Result<u16, SavError> {
    (0..u32::from(ENGINE_RENEW_POOL_CAPACITY))
        .find_map(|candidate| {
            let id = u16::try_from(candidate).ok()?;
            (!used.contains(&id)).then_some(id)
        })
        .ok_or_else(|| SavError::BadFormat("pool ERNW agotado".into()))
}

fn effective_owner(rule: &AutoReplaceRule) -> CompanyId {
    rule.owner.unwrap_or(CompanyId::PLAYER)
}

fn normalize_export_rules(
    source_rules: &[AutoReplaceRule],
) -> Result<Vec<ExportAutoReplaceRule>, SavError> {
    let mut used_pool_ids = BTreeSet::new();
    let mut rules = Vec::with_capacity(source_rules.len());
    for rule in source_rules {
        let pool_id = match rule.sav_pool_id {
            Some(id) if id < ENGINE_RENEW_POOL_CAPACITY && used_pool_ids.insert(id) => id,
            Some(id) if id >= ENGINE_RENEW_POOL_CAPACITY => {
                return Err(SavError::BadFormat(format!(
                    "índice ERNW fuera de rango: {id}"
                )));
            }
            Some(id) => {
                return Err(SavError::BadFormat(format!("índice ERNW duplicado: {id}")));
            }
            None => {
                let id = next_free_pool_id(&used_pool_ids)?;
                used_pool_ids.insert(id);
                id
            }
        };
        rules.push(ExportAutoReplaceRule {
            rule: *rule,
            pool_id,
            next_pool_id: rule.sav_next_pool_id,
        });
    }
    Ok(rules)
}

fn rule_indices(rules: &[ExportAutoReplaceRule]) -> BTreeMap<u16, usize> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (rule.pool_id, index))
        .collect()
}

fn clear_dangling_next_links(
    rules: &mut [ExportAutoReplaceRule],
    pool_indices: &BTreeMap<u16, usize>,
) {
    for rule in rules {
        if rule
            .next_pool_id
            .is_some_and(|next| !pool_indices.contains_key(&next))
        {
            // Una regla fue eliminada en el runtime: no dejes una referencia
            // colgante que OpenTTD rechazaría durante FixPointers.
            rule.next_pool_id = None;
        }
    }
}

fn validate_rule_owners(
    companies: &[crate::company::Company],
    rules: &[ExportAutoReplaceRule],
) -> Result<(), SavError> {
    let known_companies: BTreeSet<u8> = companies.iter().map(|company| company.id.0).collect();
    for rule in rules {
        let owner = effective_owner(&rule.rule);
        if !known_companies.contains(&owner.0) {
            return Err(SavError::BadFormat(format!(
                "regla ERNW pertenece a compañía inexistente: {}",
                owner.0
            )));
        }
    }
    Ok(())
}

fn build_company_heads(
    companies: &[crate::company::Company],
    rules: &mut [ExportAutoReplaceRule],
    pool_indices: &BTreeMap<u16, usize>,
) -> BTreeMap<u8, u16> {
    let mut company_heads = BTreeMap::new();
    for company in companies {
        let company_id = company.id;
        let company_rule_indices: Vec<usize> = rules
            .iter()
            .enumerate()
            .filter_map(|(index, rule)| {
                (effective_owner(&rule.rule) == company_id).then_some(index)
            })
            .collect();
        if company_rule_indices.is_empty() {
            continue;
        }

        let mut linked = BTreeSet::new();
        let mut head = company.engine_renew_list_head.filter(|head| {
            pool_indices
                .get(head)
                .is_some_and(|index| effective_owner(&rules[*index].rule) == company_id)
        });
        let mut current = head;
        let mut previous: Option<usize> = None;
        while let Some(pool_id) = current {
            if !linked.insert(pool_id) {
                // Una lista cíclica es inválida en OpenTTD; cortar el lazo al
                // reexportar deja la parte útil de la cadena accesible.
                if let Some(index) = previous {
                    rules[index].next_pool_id = None;
                }
                break;
            }
            let Some(&index) = pool_indices.get(&pool_id) else {
                break;
            };
            if effective_owner(&rules[index].rule) != company_id {
                if let Some(previous) = previous {
                    rules[previous].next_pool_id = None;
                }
                break;
            }
            previous = Some(index);
            current = rules[index].next_pool_id;
        }

        // Las reglas creadas por openttdrs no tienen aún identidad en ERNW.
        // OpenTTD inserta sus reglas al frente de la lista; hacemos lo mismo,
        // conservando intacta una cadena importada ya válida detrás de ellas.
        for index in company_rule_indices.into_iter().rev() {
            let pool_id = rules[index].pool_id;
            if linked.contains(&pool_id) {
                continue;
            }
            rules[index].next_pool_id = head;
            head = Some(pool_id);
        }
        if let Some(head) = head {
            company_heads.insert(company_id.0, head);
        }
    }
    company_heads
}

/// Normaliza IDs y cadenas para exportar reglas creadas localmente sin perder
/// los índices y enlaces de una partida que vino de `OpenTTD`.
pub(super) fn autoreplace_export(state: &GameState) -> Result<AutoreplaceExport, SavError> {
    let mut rules = normalize_export_rules(&state.autoreplace_rules)?;
    let pool_indices = rule_indices(&rules);
    clear_dangling_next_links(&mut rules, &pool_indices);
    validate_rule_owners(&state.companies, &rules)?;
    let company_heads = build_company_heads(&state.companies, &mut rules, &pool_indices);

    Ok(AutoreplaceExport {
        rules,
        company_heads,
    })
}

pub(super) fn autoreplace_chunk(export: &AutoreplaceExport) -> Result<Option<Vec<u8>>, SavError> {
    if export.rules.is_empty() {
        return Ok(None);
    }
    let mut header = Vec::new();
    append_field(&mut header, 4, "from")?;
    append_field(&mut header, 4, "to")?;
    // Referencias de saves modernos (`SLV >= 69`) son SLE_FILE_U32 aunque el
    // índice del pool EngineRenew sea u16.
    append_field(&mut header, 6, "next")?;
    append_field(&mut header, 4, "group_id")?;
    // `SLE_BOOL` usa SLE_FILE_I8 (tipo 0x01), no U8.
    append_field(&mut header, 1, "replace_when_old")?;
    header.push(0);

    let max_pool_id = export
        .rules
        .iter()
        .map(|rule| rule.pool_id)
        .max()
        .unwrap_or(0);
    let mut records = vec![Vec::new(); usize::from(max_pool_id) + 1];
    for export_rule in &export.rules {
        let rule = export_rule.rule;
        let mut record = Vec::new();
        record.extend_from_slice(&rule.from_engine_id.to_be_bytes());
        record.extend_from_slice(&rule.to_engine_id.to_be_bytes());
        let next = export_rule.next_pool_id.map_or(0, |id| u32::from(id) + 1);
        record.extend_from_slice(&next.to_be_bytes());
        let group_id = match rule.group_id {
            Some(group_id) => u16::try_from(group_id)
                .ok()
                .filter(|id| *id < ENGINE_RENEW_POOL_CAPACITY)
                .ok_or_else(|| {
                    SavError::BadFormat(format!("GroupID inválido en ERNW: {group_id}"))
                })?,
            None if rule.default_group_only => DEFAULT_GROUP,
            None => ALL_GROUP,
        };
        record.extend_from_slice(&group_id.to_be_bytes());
        record.push(u8::from(rule.only_when_old));
        records[usize::from(export_rule.pool_id)] = record;
    }
    Ok(Some(raw_table_chunk(
        *b"ERNW", &header, &records, CH_TABLE,
    )?))
}

pub(super) fn fleet_chunks(
    state: &GameState,
    autoreplace_export: &AutoreplaceExport,
) -> Result<Vec<u8>, SavError> {
    let mut chunks = Vec::new();
    if let Some(groups) = groups_chunk(&state.vehicle_groups)? {
        chunks.extend_from_slice(&groups);
    }
    if let Some(renew) = autoreplace_chunk(autoreplace_export)? {
        chunks.extend_from_slice(&renew);
    }
    Ok(chunks)
}
