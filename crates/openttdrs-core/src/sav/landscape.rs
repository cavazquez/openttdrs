//! Landscape / clima desde chunks de settings del `.sav` (#224).

use crate::Climate;

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

/// Lee `game_creation.landscape` de `PATS`/`OPTS` (uint 0..3).
#[must_use]
pub fn climate_from_chunks(chunks: &[RawChunk]) -> Option<Climate> {
    for name in ["PATS", "OPTS"] {
        let Some(chunk) = find_chunk(chunks, name) else {
            continue;
        };
        let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
            continue;
        };
        for (_idx, record) in rows {
            if let Some(v) =
                record_get(&record, "game_creation.landscape").and_then(SlValue::as_u64)
            {
                return climate_from_landscape_u8(u8::try_from(v).ok()?);
            }
            if let Some(v) = record_get(&record, "landscape").and_then(SlValue::as_u64) {
                return climate_from_landscape_u8(u8::try_from(v).ok()?);
            }
        }
    }
    None
}

#[must_use]
pub const fn climate_from_landscape_u8(v: u8) -> Option<Climate> {
    match v {
        0 => Some(Climate::Temperate),
        1 => Some(Climate::SubArctic),
        2 => Some(Climate::SubTropical),
        3 => Some(Climate::Toyland),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_bytes_map_to_climate() {
        assert_eq!(climate_from_landscape_u8(0), Some(Climate::Temperate));
        assert_eq!(climate_from_landscape_u8(3), Some(Climate::Toyland));
        assert_eq!(climate_from_landscape_u8(9), None);
    }
}
