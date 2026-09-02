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
/// importado.
///
/// Los saves de `OpenTTD` suelen añadir columnas al final de una tabla sin
/// cambiar los campos que conoce el runtime. Cuando sólo cambia un escalar
/// conocido, podemos copiar sus bytes dentro del registro original y dejar
/// intactas esas columnas futuras. Si la tabla cambia de forma (índices,
/// strings, listas o structs), devolvemos el writer canónico: conservar un
/// valor viejo en ese caso sería peor que perder una columna no interpretada.
pub(super) fn table_chunk_with_passthrough(
    raw: Option<&SavOpaqueChunk>,
    canonical: Vec<u8>,
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
    let Some(body) = merge_table_body_preserving_unknown(&raw.body, canonical_body, sparse)? else {
        return Ok(canonical);
    };
    Ok(raw_chunk(raw.name, raw.ch_type, &body))
}

#[derive(Debug, Clone, Copy)]
struct TableRecordRange {
    index: u32,
    field_start: usize,
    end: usize,
}

fn table_records(body: &[u8], sparse: bool) -> Result<(usize, Vec<TableRecordRange>), SavError> {
    let (_, header_end, _) = parse_table_layout(body)?;
    let mut offset = header_end;
    let mut dense_index = 0u32;
    let mut records = Vec::new();
    loop {
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

fn fixed_scalar(field: &TableField) -> bool {
    // SLE_FILE_STRING (10), STRUCT (11) y HAS_LENGTH necesitan conocer la
    // longitud exacta del valor para reescribirse; no son parches in-place.
    field.base != 10 && field.base != 11 && !field.has_length
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
        if raw.base != canonical.base || raw.has_length != canonical.has_length {
            return Err(SavError::BadFormat(format!(
                "tipo incompatible para campo {}",
                canonical.name
            )));
        }
        matches.push((raw, canonical));
    }
    Ok(matches)
}

fn merge_table_body_preserving_unknown(
    raw_body: &[u8],
    canonical_body: &[u8],
    sparse: bool,
) -> Result<Option<Vec<u8>>, SavError> {
    let (raw_header_end, raw_records) = table_records(raw_body, sparse)?;
    let (_, canonical_records) = table_records(canonical_body, sparse)?;
    let (_, _, raw_fields) = parse_table_layout(raw_body)?;
    let (_, _, canonical_fields) = parse_table_layout(canonical_body)?;
    let Ok(matches) = matching_fields(&raw_fields, &canonical_fields) else {
        return Ok(None);
    };

    // No insertar/retirar filas con una fusión in-place. Esto también cubre
    // los huecos densos de pools y evita dejar entidades huérfanas.
    if raw_records.len() != canonical_records.len()
        || raw_records
            .iter()
            .zip(&canonical_records)
            .any(|(raw, canonical)| raw.index != canonical.index)
    {
        return Ok(None);
    }

    let mut merged = raw_body.to_vec();
    for (raw_record, canonical_record) in raw_records.iter().zip(&canonical_records) {
        let raw_slice = &raw_body[raw_record.field_start..raw_record.end];
        let canonical_slice = &canonical_body[canonical_record.field_start..canonical_record.end];
        if raw_slice.is_empty() && canonical_slice.is_empty() {
            continue;
        }
        if raw_slice.is_empty() || canonical_slice.is_empty() {
            return Ok(None);
        }
        let Ok(raw_ranges) = field_byte_ranges(&raw_fields, raw_slice) else {
            return Ok(None);
        };
        let Ok(canonical_ranges) = field_byte_ranges(&canonical_fields, canonical_slice) else {
            return Ok(None);
        };
        for (raw_field, canonical_field) in &matches {
            let Some((_, raw_start, raw_end)) = raw_ranges
                .iter()
                .find(|(name, _, _)| name == &raw_field.name)
            else {
                return Ok(None);
            };
            let Some((_, canonical_start, canonical_end)) = canonical_ranges
                .iter()
                .find(|(name, _, _)| name == &canonical_field.name)
            else {
                return Ok(None);
            };
            let raw_bytes = &raw_slice[*raw_start..*raw_end];
            let canonical_bytes = &canonical_slice[*canonical_start..*canonical_end];
            if raw_bytes == canonical_bytes {
                continue;
            }
            if !fixed_scalar(raw_field) || raw_bytes.len() != canonical_bytes.len() {
                // Strings, listas y structs sólo se pueden fusionar si no
                // cambiaron. Una mutación de longitud requiere reserializar.
                return Ok(None);
            }
            let destination_start = raw_record.field_start + *raw_start;
            let destination_end = raw_record.field_start + *raw_end;
            merged[destination_start..destination_end].copy_from_slice(canonical_bytes);
        }
    }

    // La comparación anterior trabaja sobre payloads, pero conserva el
    // header original y los bytes gamma de cada fila (incluidos los huecos).
    debug_assert_eq!(raw_header_end, parse_table_layout(raw_body)?.1);
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

        let merged = table_chunk_with_passthrough(Some(&raw_chunk), canonical).expect("merge");
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
    fn passthrough_falls_back_when_variable_field_changes_length() {
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
        canonical_name.push(0xBB);
        let canonical = table_chunk(
            *b"TEST",
            &[(0x0A, "name"), (2, "future")],
            &[canonical_name],
        )
        .expect("canonical table");

        let merged =
            table_chunk_with_passthrough(Some(&raw_chunk), canonical.clone()).expect("merge");
        assert_eq!(merged, canonical);
    }
}
