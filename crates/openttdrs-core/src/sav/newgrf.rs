//! Lectura del stack `NewGRF` persistido en el chunk `NGRF`.
//!
//! `OpenTTD` guarda aquí sólo la configuración activa (los GRF estáticos y
//! `InitOnly` se agregan al arrancar y no forman parte del save). El modelo de
//! `openttdrs` no necesita el digest MD5 ni la paleta para volver a ejecutar
//! sus acciones, pero sí conserva el nombre, GRFID, versión y parámetros para
//! que el catálogo runtime pueda reconstruirse después de cargar un `.sav`.

use super::chunks::{CH_SPARSE_TABLE, CH_TABLE, RawChunk, find_chunk};
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};
use crate::newgrf_config::{MAX_NEWGRF_PARAMS, NewGrfEntry};

/// Decodifica el stack activo del chunk `NGRF`.
///
/// El parser de tablas ya salta campos desconocidos, por lo que esta función
/// es compatible con versiones que agreguen columnas al registro. Un chunk
/// ausente, vacío o corrupto se trata como un stack vacío; el llamador aplica
/// entonces la configuración vanilla por defecto.
pub(crate) fn newgrf_stack_from_chunks(chunks: &[RawChunk]) -> Vec<NewGrfEntry> {
    let Some(chunk) = find_chunk(chunks, "NGRF") else {
        return Vec::new();
    };
    if !matches!(chunk.ch_type, CH_TABLE | CH_SPARSE_TABLE) {
        return Vec::new();
    }
    let sparse = chunk.ch_type == CH_SPARSE_TABLE;
    let Ok(rows) = parse_table_chunk(&chunk.body, sparse) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter_map(|(_, record)| entry_from_record(&record))
        .collect()
}

fn entry_from_record(record: &SlRecord) -> Option<NewGrfEntry> {
    let filename = record_get(record, "filename")?.as_str()?.to_owned();
    if filename.is_empty() {
        return None;
    }
    let grfid = record_get(record, "ident.grfid")
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    let mut entry = NewGrfEntry::new(filename, grfid);
    entry.grf_version = record_get(record, "version")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0);

    if let Some(SlValue::List(values)) = record_get(record, "param") {
        let declared = record_get(record, "num_params")
            .and_then(SlValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(values.len());
        entry.params = values
            .iter()
            .take(declared.min(MAX_NEWGRF_PARAMS))
            .filter_map(SlValue::as_u64)
            .filter_map(|value| u32::try_from(value).ok())
            .collect();
    }
    Some(entry)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::sav::chunks::RawChunk;
    use crate::sav::table::tests::{build_table_body, write_gamma, write_str};

    #[test]
    fn parses_active_stack_and_truncates_to_num_params() {
        let mut record = Vec::new();
        write_str("example.grf", &mut record);
        record.extend_from_slice(&0x1234_5678u32.to_be_bytes());
        write_gamma(16, &mut record);
        record.extend(std::iter::repeat_n(0, 16));
        record.extend_from_slice(&8u32.to_be_bytes());
        write_gamma(3, &mut record);
        record.extend_from_slice(&11u32.to_be_bytes());
        record.extend_from_slice(&22u32.to_be_bytes());
        record.extend_from_slice(&33u32.to_be_bytes());
        record.push(2); // num_params
        record.push(1); // palette (not represented by NewGrfEntry)

        let body = build_table_body(
            &[
                (0x0A, "filename"),
                (6, "ident.grfid"),
                (2 | 0x10, "ident.md5sum"),
                (6, "version"),
                (6 | 0x10, "param"),
                (2, "num_params"),
                (2, "palette"),
            ],
            &[record],
        );
        let chunks = [RawChunk {
            name: *b"NGRF",
            ch_type: CH_TABLE,
            body,
        }];

        let stack = newgrf_stack_from_chunks(&chunks);
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0].filename, "example.grf");
        assert_eq!(stack[0].grfid, 0x1234_5678);
        assert_eq!(stack[0].grf_version, 8);
        assert_eq!(stack[0].params, vec![11, 22]);
        assert!(stack[0].enabled);
        assert!(!stack[0].is_static);
    }

    #[test]
    fn malformed_or_non_table_chunk_is_best_effort_empty() {
        let chunks = [RawChunk {
            name: *b"NGRF",
            ch_type: super::super::chunks::CH_RIFF,
            body: vec![1, 2, 3],
        }];
        assert!(newgrf_stack_from_chunks(&chunks).is_empty());
    }
}
