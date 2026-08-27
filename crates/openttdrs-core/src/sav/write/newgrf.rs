//! Escritura del stack `NewGRF` en el chunk `NGRF` de un `.sav`.

use super::super::SavError;
use super::chunks::table_chunk;
use super::codec::{write_gamma, write_str};
use crate::game_state::GameState;
use crate::newgrf_config::MAX_NEWGRF_PARAMS;

/// Serializa la configuración activa que `OpenTTD` debe restaurar al cargar.
///
/// El formato nativo incluye un digest MD5 y una paleta elegida por el
/// cargador. El runtime actual no los modela, así que se emite el digest en
/// cero y la paleta por defecto; los campos que sí afectan a la ejecución
/// (`filename`, GRFID, versión y parámetros) se conservan exactamente.
pub(super) fn newgrf_chunk(state: &GameState) -> Result<Option<Vec<u8>>, SavError> {
    let entries: Vec<_> = state
        .newgrf_stack
        .iter()
        .filter(|entry| entry.enabled && !entry.is_static && !entry.filename.is_empty())
        .collect();
    if entries.is_empty() {
        return Ok(None);
    }

    let mut records = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut record = Vec::new();
        write_str(&entry.filename, &mut record)?;
        record.extend_from_slice(&entry.grfid.to_be_bytes());
        write_gamma(16, &mut record)?;
        record.extend(std::iter::repeat_n(0, 16));
        record.extend_from_slice(&u32::from(entry.grf_version).to_be_bytes());

        let num_params = entry.params.len().min(MAX_NEWGRF_PARAMS);
        write_gamma(MAX_NEWGRF_PARAMS as u32, &mut record)?;
        for index in 0..MAX_NEWGRF_PARAMS {
            let value = entry.params.get(index).copied().unwrap_or(0);
            record.extend_from_slice(&value.to_be_bytes());
        }
        record.push(num_params as u8);
        // `GRFConfig::palette` se guarda como U8. Cero es la paleta por
        // defecto y coincide con la selección que hace OpenTTD para un GRF
        // cuyo digest/paleta no se conoce al exportar.
        record.push(0);
        records.push(record);
    }

    let chunk = table_chunk(
        *b"NGRF",
        &[
            (0x0A, "filename"),
            (6, "ident.grfid"),
            (2 | 0x10, "ident.md5sum"),
            (6, "version"),
            (6 | 0x10, "param"),
            (2, "num_params"),
            (2, "palette"),
        ],
        &records,
    )?;
    Ok(Some(chunk))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::newgrf_config::NewGrfEntry;
    use crate::sav::chunks::{find_chunk, parse_chunks};
    use crate::sav::table::{SlValue, parse_table_chunk, record_get};

    #[test]
    fn writes_fixed_param_array_and_skips_disabled_or_static_entries() {
        let mut state = GameState::new(64, 64);
        let mut active = NewGrfEntry::new("active.grf", 0x4142_4301);
        active.grf_version = 8;
        active.set_param(2, 0xDEAD_BEEF);
        state.newgrf_stack = vec![
            active,
            {
                let mut entry = NewGrfEntry::new("disabled.grf", 2);
                entry.enabled = false;
                entry
            },
            {
                let mut entry = NewGrfEntry::new("static.grf", 3);
                entry.is_static = true;
                entry
            },
        ];

        let chunk = newgrf_chunk(&state).expect("encode").expect("active chunk");
        let chunks = parse_chunks(&chunk).expect("parse chunk");
        let ngrf = find_chunk(&chunks, "NGRF").expect("NGRF");
        let rows = parse_table_chunk(&ngrf.body, false).expect("parse table");
        assert_eq!(rows.len(), 1);
        let record = &rows[0].1;
        assert_eq!(
            record_get(record, "filename").and_then(SlValue::as_str),
            Some("active.grf")
        );
        assert_eq!(
            record_get(record, "ident.grfid").and_then(SlValue::as_u64),
            Some(0x4142_4301)
        );
        assert_eq!(
            record_get(record, "version").and_then(SlValue::as_u64),
            Some(8)
        );
        assert_eq!(
            record_get(record, "num_params").and_then(SlValue::as_u64),
            Some(3)
        );
        let params = match record_get(record, "param") {
            Some(SlValue::List(values)) => values,
            other => panic!("param inesperado: {other:?}"),
        };
        assert_eq!(params.len(), MAX_NEWGRF_PARAMS);
        assert_eq!(params[2].as_u64(), Some(0xDEAD_BEEF));
        assert!(params[..2].iter().all(|value| value.as_u64() == Some(0)));
    }
}
