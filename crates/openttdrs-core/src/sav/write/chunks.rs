//! Construcción de chunks RIFF y TABLE.

use super::super::SavError;
use super::super::SavOpaqueChunk;
use super::super::chunks::{CH_RIFF, CH_SPARSE_TABLE, CH_TABLE};
use super::super::table::{TableField, field_byte_ranges, parse_table_layout};
use super::codec::{write_gamma, write_str};

/// Chunk RIFF: fourcc + tamaño 28-bit big-endian + payload.
pub(super) fn riff_chunk(name: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = payload.len();
    let mut out = Vec::with_capacity(8 + size);
    out.extend_from_slice(&name);
    out.push((((size >> 24) as u8) << 4) | CH_RIFF);
    out.push((size >> 16) as u8);
    out.push((size >> 8) as u8);
    out.push(size as u8);
    out.extend_from_slice(payload);
    out
}

/// Reemite un chunk ya validado sin interpretar su header interno.
pub(super) fn raw_chunk(name: [u8; 4], ch_type: u8, body: &[u8]) -> Vec<u8> {
    if ch_type == CH_RIFF {
        return riff_chunk(name, body);
    }
    let mut out = Vec::with_capacity(5 + body.len());
    out.extend_from_slice(&name);
    out.push(ch_type);
    out.extend_from_slice(body);
    out
}

/// Chunk TABLE simple: fourcc + header con campos + records gamma.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn table_chunk(
    name: [u8; 4],
    fields: &[(u8, &str)],
    records: &[Vec<u8>],
) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    for &(ftype, key) in fields {
        header.push(ftype);
        write_str(key, &mut header)?;
    }
    header.push(0);

    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(CH_TABLE);
    write_gamma(header.len() as u32 + 1, &mut out)?;
    out.extend_from_slice(&header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out)?;
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out)?;
    Ok(out)
}

/// Chunk TABLE/SPARSE con header arbitrario + records gamma.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn raw_table_chunk(
    name: [u8; 4],
    header: &[u8],
    records: &[Vec<u8>],
    ch_type: u8,
) -> Result<Vec<u8>, SavError> {
    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(ch_type);
    write_gamma(header.len() as u32 + 1, &mut out)?;
    out.extend_from_slice(header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out)?;
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out)?;
    Ok(out)
}

/// Selecciona el chunk canónico o una variante fusionada con un chunk
/// importado, usando opcionalmente el snapshot semántico tomado al importar
/// para saber qué campos cambiaron realmente.
///
/// Los saves de `OpenTTD` suelen añadir columnas al final de una tabla sin
/// cambiar los campos que conoce el runtime. Cuando un campo conocido conserva
/// su schema y tamaño codificado, podemos copiar sus bytes y dejar intactas
/// esas columnas futuras. Las strings y listas escalares raíz admiten otra
/// longitud: se reconstruye la fila y su prefijo gamma, preservando el resto
/// de bytes. El snapshot permite además actualizar un campo presente en un
/// schema SAV antiguo sin añadir campos nuevos que no cambiaron. Si cambian filas,
/// índices, schema o tamaños incompatibles, devolvemos el writer canónico:
/// conservar un valor viejo en ese caso sería peor que perder una columna no
/// interpretada.
pub(super) fn table_chunk_with_passthrough_from_snapshot(
    raw: Option<&SavOpaqueChunk>,
    canonical: Vec<u8>,
    snapshot_records: Option<&[Vec<u8>]>,
) -> Result<Vec<u8>, SavError> {
    let Some(raw) = raw else {
        return Ok(canonical);
    };
    if raw.ch_type != CH_TABLE && raw.ch_type != CH_SPARSE_TABLE {
        return Ok(canonical);
    }
    if canonical.len() < 5 || canonical[..4] != raw.name {
        return Ok(canonical);
    }
    let canonical_type = canonical[4] & 0x0F;
    if canonical_type != raw.ch_type {
        return Ok(canonical);
    }
    let canonical_body = &canonical[5..];
    let sparse = raw.ch_type == CH_SPARSE_TABLE;
    let snapshot_body = snapshot_records
        .map(|records| table_body_with_snapshot_records(canonical_body, records))
        .transpose()?;
    let Some(body) = merge_table_body_preserving_unknown(
        &raw.body,
        canonical_body,
        snapshot_body.as_deref(),
        sparse,
    )?
    else {
        return Ok(canonical);
    };
    Ok(raw_chunk(raw.name, raw.ch_type, &body))
}

/// Reutiliza la cabecera del writer actual con los records semánticos que se
/// habían calculado durante la importación. Los records de tablas sparse ya
/// incluyen su índice gamma, igual que los records canónicos.
fn table_body_with_snapshot_records(
    canonical_body: &[u8],
    records: &[Vec<u8>],
) -> Result<Vec<u8>, SavError> {
    let (_, header_end, _) = parse_table_layout(canonical_body)?;
    let mut body = canonical_body[..header_end].to_vec();
    for record in records {
        let length = u32::try_from(record.len())
            .map_err(|_| SavError::BadFormat("registro de snapshot demasiado grande".into()))?;
        let encoded_length = length
            .checked_add(1)
            .ok_or_else(|| SavError::BadFormat("registro de snapshot overflow".into()))?;
        write_gamma(encoded_length, &mut body)?;
        body.extend_from_slice(record);
    }
    write_gamma(0, &mut body)?;
    Ok(body)
}

#[derive(Debug, Clone, Copy)]
struct TableRecordRange {
    index: u32,
    start: usize,
    payload_start: usize,
    field_start: usize,
    end: usize,
}

fn table_records(body: &[u8], sparse: bool) -> Result<(usize, Vec<TableRecordRange>), SavError> {
    let (_, header_end, _) = parse_table_layout(body)?;
    let mut offset = header_end;
    let mut dense_index = 0u32;
    let mut records = Vec::new();
    loop {
        let start = offset;
        let length = read_gamma(body, &mut offset)?;
        if length == 0 {
            break;
        }
        let payload_len = usize::try_from(length - 1)
            .map_err(|_| SavError::BadFormat("registro de tabla demasiado grande".into()))?;
        let payload_start = offset;
        let end = payload_start
            .checked_add(payload_len)
            .ok_or_else(|| SavError::BadFormat("registro de tabla overflow".into()))?;
        if end > body.len() {
            return Err(SavError::BadFormat("registro de tabla truncado".into()));
        }
        let mut field_start = payload_start;
        let index = if sparse {
            read_gamma(body, &mut field_start)?
        } else {
            dense_index
        };
        records.push(TableRecordRange {
            index,
            start,
            payload_start,
            field_start,
            end,
        });
        offset = end;
        dense_index = dense_index.saturating_add(1);
    }
    Ok((header_end, records))
}

fn read_gamma(data: &[u8], offset: &mut usize) -> Result<u32, SavError> {
    crate::tnbp_decode::read_sl_gamma(data, offset)
        .map_err(|error| SavError::BadFormat(format!("gamma de tabla inválido: {error:?}")))
}

fn field_layout_matches(raw: &TableField, canonical: &TableField) -> bool {
    raw.name == canonical.name
        && raw.base == canonical.base
        && raw.has_length == canonical.has_length
        && raw.sub.len() == canonical.sub.len()
        && raw
            .sub
            .iter()
            .zip(&canonical.sub)
            .all(|(raw_sub, canonical_sub)| field_layout_matches(raw_sub, canonical_sub))
}

fn matching_fields<'a, 'b>(
    raw_fields: &'a [TableField],
    canonical_fields: &'b [TableField],
) -> Result<Vec<(&'a TableField, &'b TableField)>, SavError> {
    let mut matches = Vec::with_capacity(canonical_fields.len());
    for canonical in canonical_fields {
        let Some(raw) = raw_fields.iter().find(|field| field.name == canonical.name) else {
            return Err(SavError::BadFormat(format!(
                "tabla sin campo conocido {}",
                canonical.name
            )));
        };
        if !field_layout_matches(raw, canonical) {
            return Err(SavError::BadFormat(format!(
                "layout incompatible para campo {}",
                canonical.name
            )));
        }
        matches.push((raw, canonical));
    }
    Ok(matches)
}

fn compatible_raw_fields<'a, 'b>(
    raw_fields: &'a [TableField],
    canonical_fields: &'b [TableField],
) -> Vec<(Option<&'a TableField>, &'b TableField)> {
    canonical_fields
        .iter()
        .map(|canonical| {
            let raw = raw_fields
                .iter()
                .find(|raw| raw.name == canonical.name && field_layout_matches(raw, canonical));
            (raw, canonical)
        })
        .collect()
}

fn field_lists_match(left: &[TableField], right: &[TableField]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| field_layout_matches(left, right))
}

fn record_indices_match(left: &[TableRecordRange], right: &[TableRecordRange]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.index == right.index)
}

fn snapshot_matches_canonical(
    snapshot_records: &[TableRecordRange],
    snapshot_fields: &[TableField],
    canonical_records: &[TableRecordRange],
    canonical_fields: &[TableField],
) -> bool {
    field_lists_match(snapshot_fields, canonical_fields)
        && record_indices_match(snapshot_records, canonical_records)
}

fn named_range(ranges: &[(String, usize, usize)], name: &str) -> Option<(usize, usize)> {
    ranges
        .iter()
        .find(|(candidate, _, _)| candidate == name)
        .map(|(_, start, end)| (*start, *end))
}

struct RecordMergeInput<'a> {
    raw_slice: &'a [u8],
    raw_ranges: &'a [(String, usize, usize)],
    canonical_slice: &'a [u8],
    canonical_ranges: &'a [(String, usize, usize)],
}

struct FieldReplacement<'a> {
    start: usize,
    end: usize,
    bytes: &'a [u8],
}

/// Los headers de tabla de `OpenTTD` codifican `SL_ARR`, `SL_VECTOR`,
/// `SL_REFVECTOR` y `SL_STRUCTLIST` con el mismo bit `HAS_LENGTH`; los writers
/// propios mantienen sus arrays fijos en tamaño nativo. La comparación del
/// descriptor ya exige recursivamente que un struct tenga exactamente el mismo
/// esquema interno antes de llegar aquí, por lo que su cantidad puede
/// reencuadrarse sin ocultar un subcampo desconocido. Esta función no
/// transforma un array fijo en vector: su writer sigue siendo responsable de
/// emitir el tamaño que `OpenTTD` acepta.
fn root_field_allows_length_change(field: &TableField) -> bool {
    const SLE_FILE_STRING: u8 = 10;
    field.base == SLE_FILE_STRING || field.has_length
}

fn replace_field_in_raw<'a>(
    replacements: &mut Vec<FieldReplacement<'a>>,
    input: &RecordMergeInput<'a>,
    raw_field: &TableField,
    canonical_field: &TableField,
) -> Option<()> {
    let (raw_start, raw_end) = named_range(input.raw_ranges, &raw_field.name)?;
    let (canonical_start, canonical_end) =
        named_range(input.canonical_ranges, &canonical_field.name)?;
    let raw_bytes = &input.raw_slice[raw_start..raw_end];
    let canonical_bytes = &input.canonical_slice[canonical_start..canonical_end];
    if raw_bytes == canonical_bytes {
        return Some(());
    }
    if raw_bytes.len() != canonical_bytes.len() && !root_field_allows_length_change(canonical_field)
    {
        // Campos sin longitud codificada no se pueden reencuadrar de forma
        // local; los structs sólo llegan aquí con sub-schema compatible.
        return None;
    }
    replacements.push(FieldReplacement {
        start: raw_start,
        end: raw_end,
        bytes: canonical_bytes,
    });
    Some(())
}

fn merge_strict_record<'a>(
    replacements: &mut Vec<FieldReplacement<'a>>,
    input: &RecordMergeInput<'a>,
    matches: &[(&TableField, &TableField)],
) -> bool {
    matches.iter().all(|(raw_field, canonical_field)| {
        replace_field_in_raw(replacements, input, raw_field, canonical_field).is_some()
    })
}

fn merge_snapshot_record<'a>(
    replacements: &mut Vec<FieldReplacement<'a>>,
    input: &RecordMergeInput<'a>,
    snapshot_slice: &[u8],
    snapshot_ranges: &[(String, usize, usize)],
    compatible_fields: &[(Option<&TableField>, &TableField)],
) -> bool {
    for (raw_field, canonical_field) in compatible_fields {
        let Some((canonical_start, canonical_end)) =
            named_range(input.canonical_ranges, &canonical_field.name)
        else {
            return false;
        };
        let Some((snapshot_start, snapshot_end)) =
            named_range(snapshot_ranges, &canonical_field.name)
        else {
            return false;
        };
        let canonical_bytes = &input.canonical_slice[canonical_start..canonical_end];
        let snapshot_bytes = &snapshot_slice[snapshot_start..snapshot_end];
        if canonical_bytes == snapshot_bytes {
            continue;
        }
        let Some(raw_field) = raw_field else {
            // El runtime cambió un campo que no existía (o no era compatible)
            // en el schema importado. Debe serializarse canónicamente para no
            // ocultar esa mutación.
            return false;
        };
        if replace_field_in_raw(replacements, input, raw_field, canonical_field).is_none() {
            return false;
        }
    }
    true
}

/// Copia las columnas originales en su orden físico y sustituye únicamente
/// los rangos elegidos. El schema canónico puede ordenar sus campos de otra
/// manera. También se preserva cualquier sufijo opaco de la fila.
fn apply_record_replacements(
    raw: &[u8],
    replacements: &mut [FieldReplacement<'_>],
) -> Option<Vec<u8>> {
    replacements.sort_unstable_by_key(|replacement| replacement.start);
    let mut output = Vec::with_capacity(raw.len());
    let mut offset = 0;
    for replacement in replacements {
        if replacement.start < offset {
            return None;
        }
        output.extend_from_slice(&raw[offset..replacement.start]);
        output.extend_from_slice(replacement.bytes);
        offset = replacement.end;
    }
    output.extend_from_slice(&raw[offset..]);
    Some(output)
}

fn append_merged_record(
    output: &mut Vec<u8>,
    raw_body: &[u8],
    raw_record: TableRecordRange,
    replacements: &mut [FieldReplacement<'_>],
) -> Option<()> {
    if replacements.is_empty() {
        output.extend_from_slice(&raw_body[raw_record.start..raw_record.end]);
        return Some(());
    }
    let raw_slice = &raw_body[raw_record.field_start..raw_record.end];
    let record = apply_record_replacements(raw_slice, replacements)?;
    // La longitud de fila incluye el índice sparse, cuya codificación
    // original se conserva; no incluye su propio prefijo gamma.
    let index_bytes = &raw_body[raw_record.payload_start..raw_record.field_start];
    if record.len() == raw_slice.len() {
        output.extend_from_slice(&raw_body[raw_record.start..raw_record.field_start]);
    } else {
        let length = record
            .len()
            .checked_add(index_bytes.len())?
            .checked_add(1)?;
        let length = u32::try_from(length).ok()?;
        super::codec::write_full_gamma(length, output);
        output.extend_from_slice(index_bytes);
    }
    output.extend_from_slice(&record);
    Some(())
}

fn table_fields_and_records(
    body: &[u8],
    sparse: bool,
) -> Result<(Vec<TableRecordRange>, Vec<TableField>), SavError> {
    let (_, records) = table_records(body, sparse)?;
    let (_, _, fields) = parse_table_layout(body)?;
    Ok((records, fields))
}

fn merge_table_body_preserving_unknown(
    raw_body: &[u8],
    canonical_body: &[u8],
    snapshot_body: Option<&[u8]>,
    sparse: bool,
) -> Result<Option<Vec<u8>>, SavError> {
    let (raw_header_end, raw_records) = table_records(raw_body, sparse)?;
    let (_, canonical_records) = table_records(canonical_body, sparse)?;
    let (_, _, raw_fields) = parse_table_layout(raw_body)?;
    let (_, _, canonical_fields) = parse_table_layout(canonical_body)?;
    let snapshot = snapshot_body
        .map(|body| table_fields_and_records(body, sparse))
        .transpose()?;

    if let Some((snapshot_records, snapshot_fields)) = &snapshot
        && !snapshot_matches_canonical(
            snapshot_records,
            snapshot_fields,
            &canonical_records,
            &canonical_fields,
        )
    {
        return Ok(None);
    }

    let matches = if snapshot.is_some() {
        None
    } else {
        let Ok(matches) = matching_fields(&raw_fields, &canonical_fields) else {
            return Ok(None);
        };
        Some(matches)
    };
    let compatible_fields = snapshot
        .as_ref()
        .map(|_| compatible_raw_fields(&raw_fields, &canonical_fields));

    // No insertar/retirar filas durante la fusión. Esto también cubre
    // los huecos densos de pools y evita dejar entidades huérfanas.
    if !record_indices_match(&raw_records, &canonical_records) {
        return Ok(None);
    }

    let mut merged = raw_body[..raw_header_end].to_vec();
    for (record_index, (raw_record, canonical_record)) in
        raw_records.iter().zip(&canonical_records).enumerate()
    {
        let raw_slice = &raw_body[raw_record.field_start..raw_record.end];
        let canonical_slice = &canonical_body[canonical_record.field_start..canonical_record.end];
        let snapshot_slice = match (&snapshot, snapshot_body) {
            (Some((records, _)), Some(body)) => records
                .get(record_index)
                .map(|record| &body[record.field_start..record.end]),
            (None, _) => None,
            _ => return Ok(None),
        };
        if raw_slice.is_empty()
            && canonical_slice.is_empty()
            && snapshot_slice.is_none_or(<[u8]>::is_empty)
        {
            merged.extend_from_slice(&raw_body[raw_record.start..raw_record.end]);
            continue;
        }
        if raw_slice.is_empty()
            || canonical_slice.is_empty()
            || snapshot_slice.is_some_and(<[u8]>::is_empty)
        {
            return Ok(None);
        }
        let Ok(raw_ranges) = field_byte_ranges(&raw_fields, raw_slice) else {
            return Ok(None);
        };
        let Ok(canonical_ranges) = field_byte_ranges(&canonical_fields, canonical_slice) else {
            return Ok(None);
        };
        let input = RecordMergeInput {
            raw_slice,
            raw_ranges: &raw_ranges,
            canonical_slice,
            canonical_ranges: &canonical_ranges,
        };

        let mut replacements = Vec::new();
        let merged_record = if let Some(matches) = &matches {
            merge_strict_record(&mut replacements, &input, matches)
        } else if let (Some(compatible_fields), Some(snapshot_slice)) =
            (&compatible_fields, snapshot_slice)
        {
            let Ok(snapshot_ranges) = field_byte_ranges(&canonical_fields, snapshot_slice) else {
                return Ok(None);
            };
            merge_snapshot_record(
                &mut replacements,
                &input,
                snapshot_slice,
                &snapshot_ranges,
                compatible_fields,
            )
        } else {
            false
        };
        if !merged_record {
            return Ok(None);
        }
        if append_merged_record(&mut merged, raw_body, *raw_record, &mut replacements).is_none() {
            return Ok(None);
        }
    }

    let tail_start = raw_records
        .last()
        .map_or(raw_header_end, |record| record.end);
    merged.extend_from_slice(&raw_body[tail_start..]);
    Ok(Some(merged))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::super::super::SavOpaqueChunk;
    use super::*;
    use crate::sav::chunks::CH_TABLE;
    use crate::sav::table::SlValue;

    #[test]
    fn passthrough_patches_known_scalar_without_losing_future_column() {
        let raw = table_chunk(
            *b"TEST",
            &[(2, "known"), (4, "future")],
            &[vec![7, 0xCA, 0xFE]],
        )
        .expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };
        let canonical =
            table_chunk(*b"TEST", &[(2, "known")], &[vec![9]]).expect("canonical table");

        let merged = table_chunk_with_passthrough_from_snapshot(Some(&raw_chunk), canonical, None)
            .expect("merge");
        let chunks = crate::sav::chunks::parse_chunks(&merged).expect("parse merged");
        let body = &chunks[0].body;
        let (_, _, fields) = parse_table_layout(body).expect("merged header");
        assert_eq!(fields.len(), 2);
        let rows = crate::sav::table::parse_table_chunk(body, false).expect("merged row");
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "known").and_then(SlValue::as_u64),
            Some(9)
        );
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "future").and_then(SlValue::as_u64),
            Some(0xCAFE)
        );
    }

    #[test]
    fn snapshot_passthrough_patches_compatible_field_when_old_schema_omits_new_fields() {
        let raw = table_chunk(
            *b"TEST",
            &[(2, "known"), (4, "future")],
            &[vec![7, 0xCA, 0xFE]],
        )
        .expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };
        let snapshot_records = vec![vec![7, 0]];
        let canonical = table_chunk(*b"TEST", &[(2, "known"), (2, "newer")], &[vec![9, 0]])
            .expect("canonical table");

        let merged = table_chunk_with_passthrough_from_snapshot(
            Some(&raw_chunk),
            canonical,
            Some(&snapshot_records),
        )
        .expect("merge");
        let chunks = crate::sav::chunks::parse_chunks(&merged).expect("parse merged");
        let body = &chunks[0].body;
        let (_, _, fields) = parse_table_layout(body).expect("merged header");
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            vec!["known", "future"]
        );
        let rows = crate::sav::table::parse_table_chunk(body, false).expect("merged row");
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "known").and_then(SlValue::as_u64),
            Some(9)
        );
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "future").and_then(SlValue::as_u64),
            Some(0xCAFE)
        );
    }

    #[test]
    fn snapshot_passthrough_falls_back_when_omitted_field_changes() {
        let raw = table_chunk(
            *b"TEST",
            &[(2, "known"), (4, "future")],
            &[vec![7, 0xCA, 0xFE]],
        )
        .expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };
        let snapshot_records = vec![vec![7, 0]];
        let canonical = table_chunk(*b"TEST", &[(2, "known"), (2, "newer")], &[vec![9, 1]])
            .expect("canonical table");

        let merged = table_chunk_with_passthrough_from_snapshot(
            Some(&raw_chunk),
            canonical.clone(),
            Some(&snapshot_records),
        )
        .expect("merge");
        assert_eq!(merged, canonical);
    }

    #[test]
    fn passthrough_patches_equal_sized_string_and_list_without_losing_future_column() {
        let mut raw_record = Vec::new();
        write_str("old", &mut raw_record).expect("raw name");
        write_gamma(2, &mut raw_record).expect("raw list length");
        raw_record.extend_from_slice(&[1, 2, 0xCA, 0xFE]);
        let raw = table_chunk(
            *b"TEST",
            &[(0x0A, "name"), (0x12, "values"), (4, "future")],
            &[raw_record],
        )
        .expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };

        let mut canonical_record = Vec::new();
        write_str("new", &mut canonical_record).expect("canonical name");
        write_gamma(2, &mut canonical_record).expect("canonical list length");
        canonical_record.extend_from_slice(&[3, 4]);
        let canonical = table_chunk(
            *b"TEST",
            &[(0x0A, "name"), (0x12, "values")],
            &[canonical_record],
        )
        .expect("canonical table");

        let merged = table_chunk_with_passthrough_from_snapshot(Some(&raw_chunk), canonical, None)
            .expect("merge");
        let chunks = crate::sav::chunks::parse_chunks(&merged).expect("parse merged");
        let rows =
            crate::sav::table::parse_table_chunk(&chunks[0].body, false).expect("merged row");
        let row = &rows[0].1;
        assert_eq!(
            crate::sav::table::record_get(row, "name").and_then(SlValue::as_str),
            Some("new")
        );
        assert_eq!(
            crate::sav::table::record_get(row, "values"),
            Some(&SlValue::List(vec![SlValue::Uint(3), SlValue::Uint(4)]))
        );
        assert_eq!(
            crate::sav::table::record_get(row, "future").and_then(SlValue::as_u64),
            Some(0xCAFE)
        );
    }

    #[test]
    fn passthrough_preserves_future_column_when_scalar_list_changes_length() {
        let old_values = vec![0x11; 127];
        let new_values = vec![0x22; 128];
        let mut raw_record = Vec::new();
        write_gamma(
            u32::try_from(old_values.len()).expect("old length"),
            &mut raw_record,
        )
        .expect("raw list length");
        raw_record.extend_from_slice(&old_values);
        raw_record.extend_from_slice(&0xCAFE_u16.to_be_bytes());
        let raw = table_chunk(*b"TEST", &[(0x12, "values"), (4, "future")], &[raw_record])
            .expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };

        let mut canonical_record = Vec::new();
        write_gamma(
            u32::try_from(new_values.len()).expect("new length"),
            &mut canonical_record,
        )
        .expect("canonical list length");
        canonical_record.extend_from_slice(&new_values);
        let canonical = table_chunk(*b"TEST", &[(0x12, "values")], &[canonical_record])
            .expect("canonical table");

        let merged = table_chunk_with_passthrough_from_snapshot(Some(&raw_chunk), canonical, None)
            .expect("merge");
        let chunks = crate::sav::chunks::parse_chunks(&merged).expect("parse merged");
        let body = &chunks[0].body;
        let (_, _, fields) = parse_table_layout(body).expect("merged header");
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            vec!["values", "future"]
        );
        let rows = crate::sav::table::parse_table_chunk(body, false).expect("merged row");
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "values"),
            Some(&SlValue::List(
                new_values
                    .into_iter()
                    .map(|value| SlValue::Uint(u64::from(value)))
                    .collect()
            ))
        );
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "future").and_then(SlValue::as_u64),
            Some(0xCAFE)
        );
    }

    #[test]
    fn passthrough_patches_equal_sized_nested_struct_without_losing_future_column() {
        let mut raw_header = Vec::new();
        raw_header.push(0x0B);
        write_str("stats", &mut raw_header).expect("struct field");
        raw_header.push(4);
        write_str("future", &mut raw_header).expect("future field");
        raw_header.push(0);
        raw_header.push(2);
        write_str("level", &mut raw_header).expect("nested field");
        raw_header.push(0);
        let mut raw_record = Vec::new();
        write_gamma(1, &mut raw_record).expect("struct count");
        raw_record.extend_from_slice(&[7, 0xCA, 0xFE]);
        let raw =
            raw_table_chunk(*b"TEST", &raw_header, &[raw_record], CH_TABLE).expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };

        let mut canonical_header = Vec::new();
        canonical_header.push(0x0B);
        write_str("stats", &mut canonical_header).expect("canonical struct field");
        canonical_header.push(0);
        canonical_header.push(2);
        write_str("level", &mut canonical_header).expect("canonical nested field");
        canonical_header.push(0);
        let mut canonical_record = Vec::new();
        write_gamma(1, &mut canonical_record).expect("canonical struct count");
        canonical_record.push(9);
        let canonical = raw_table_chunk(*b"TEST", &canonical_header, &[canonical_record], CH_TABLE)
            .expect("canonical table");

        let merged = table_chunk_with_passthrough_from_snapshot(Some(&raw_chunk), canonical, None)
            .expect("merge");
        let chunks = crate::sav::chunks::parse_chunks(&merged).expect("parse merged");
        let rows =
            crate::sav::table::parse_table_chunk(&chunks[0].body, false).expect("merged row");
        let row = &rows[0].1;
        let Some(SlValue::Structs(stats)) = crate::sav::table::record_get(row, "stats") else {
            panic!("stats debería decodificar como struct");
        };
        assert_eq!(
            crate::sav::table::record_get(&stats[0], "level").and_then(SlValue::as_u64),
            Some(9)
        );
        assert_eq!(
            crate::sav::table::record_get(row, "future").and_then(SlValue::as_u64),
            Some(0xCAFE)
        );
    }

    #[test]
    fn passthrough_preserves_future_column_when_string_changes_length() {
        let mut raw_record = Vec::new();
        write_str("old", &mut raw_record).expect("string");
        raw_record.push(0xAA);
        let raw = table_chunk(*b"TEST", &[(0x0A, "name"), (2, "future")], &[raw_record])
            .expect("raw table");
        let raw_chunk = SavOpaqueChunk {
            name: *b"TEST",
            ch_type: CH_TABLE,
            body: raw[5..].to_vec(),
        };
        let mut canonical_name = Vec::new();
        write_str("new name", &mut canonical_name).expect("string");
        let canonical =
            table_chunk(*b"TEST", &[(0x0A, "name")], &[canonical_name]).expect("canonical table");

        let merged =
            table_chunk_with_passthrough_from_snapshot(Some(&raw_chunk), canonical.clone(), None)
                .expect("merge");
        let rows = crate::sav::table::parse_table_chunk(&merged[5..], false).expect("merged rows");
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "name").and_then(SlValue::as_str),
            Some("new name")
        );
        assert_eq!(
            crate::sav::table::record_get(&rows[0].1, "future").and_then(SlValue::as_u64),
            Some(0xAA)
        );
    }
}
