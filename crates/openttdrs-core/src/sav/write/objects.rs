//! Serialización semántica del pool `Object` (`OBJS`).

use super::super::SavError;
use super::chunks::table_chunk;
use crate::game_state::GameState;
use crate::map::coord_to_linear_index;

/// Límite defensivo para no reservar decenas de gigabytes si un estado JSON
/// contiene un `ObjectID` corrupto. El pool nativo tiene IDs de 24 bits, pero
/// una partida normal mantiene los objetos muy por debajo de este límite.
const MAX_OBJECT_ROWS_TO_EXPORT: usize = 1 << 20;

/// Reconstruye `OBJS` sólo cuando el estado ya no puede usar el passthrough
/// original (por ejemplo después de construir o demoler un objeto).
pub(super) fn objects_chunk(
    state: &GameState,
    map_w: u32,
    map_h: u32,
) -> Result<Option<Vec<u8>>, SavError> {
    if state.objects.is_empty() {
        return Ok(None);
    }

    let Some(max_id) = state.objects.iter().map(|object| object.object_id).max() else {
        return Ok(None);
    };
    let rows = usize::try_from(max_id)
        .ok()
        .and_then(|id| id.checked_add(1))
        .ok_or(SavError::AllocationFailed {
            context: "pool OBJS",
            requested: usize::MAX,
        })?;
    if rows > MAX_OBJECT_ROWS_TO_EXPORT {
        return Err(SavError::AllocationFailed {
            context: "pool OBJS",
            requested: rows,
        });
    }

    // `OBJS` es una tabla densa: los huecos se codifican como registros de
    // longitud cero y conservan así el ObjectID que aparece en MAP2/MAP5.
    let mut records = vec![Vec::new(); rows];
    for object in &state.objects {
        let id = usize::try_from(object.object_id).map_err(|_| {
            SavError::BadFormat("ObjectID fuera del rango direccionable por el exportador".into())
        })?;
        let x = u32::try_from(object.tile.x).map_err(|_| {
            SavError::BadFormat("origen de objeto fuera del mapa (x negativo)".into())
        })?;
        let y = u32::try_from(object.tile.y).map_err(|_| {
            SavError::BadFormat("origen de objeto fuera del mapa (y negativo)".into())
        })?;
        if x >= map_w || y >= map_h {
            return Err(SavError::BadFormat(
                "origen de objeto fuera de las dimensiones del mapa".into(),
            ));
        }
        let tile = coord_to_linear_index(object.tile, map_w)
            .ok_or_else(|| SavError::BadFormat("origen de objeto inválido".into()))?;
        let mut record = Vec::with_capacity(24);
        record.extend_from_slice(&tile.to_be_bytes());
        record.extend_from_slice(&object.width.max(1).to_be_bytes());
        record.extend_from_slice(&object.height.max(1).to_be_bytes());
        record.extend_from_slice(&object.town.to_be_bytes());
        record.extend_from_slice(&object.build_date.to_be_bytes());
        record.push(object.colour);
        record.push(object.view);
        record.extend_from_slice(&object.object_type.to_be_bytes());
        records[id] = record;
    }

    let chunk = table_chunk(
        *b"OBJS",
        &[
            (6, "location.tile"),
            (4, "location.w"),
            (4, "location.h"),
            (6, "town"),
            (6, "build_date"),
            (2, "colour"),
            (2, "view"),
            (4, "type"),
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
    use crate::TileCoord;
    use crate::sav::chunks::{find_chunk, parse_chunks};
    use crate::sav::table::{SlValue, parse_table_chunk, record_get};

    #[test]
    fn writes_dense_object_pool_with_holes_and_metadata() {
        let mut state = GameState::new(64, 64);
        state.objects.push(crate::sav::SavObject {
            object_id: 2,
            tile: TileCoord::new(3, 4),
            width: 2,
            height: 3,
            town: 7,
            build_date: 1234,
            colour: 5,
            view: 1,
            object_type: 512,
        });
        let chunk = objects_chunk(&state, 64, 64)
            .expect("encode")
            .expect("OBJS");
        let chunks = parse_chunks(&chunk).expect("parse chunk");
        let objs = find_chunk(&chunks, "OBJS").expect("OBJS");
        let rows = parse_table_chunk(&objs.body, false).expect("parse table");
        assert_eq!(rows.len(), 1);
        let (id, record) = &rows[0];
        assert_eq!(*id, 2);
        assert_eq!(
            record_get(record, "location.tile").and_then(SlValue::as_u64),
            Some(259)
        );
        assert_eq!(
            record_get(record, "location.w").and_then(SlValue::as_u64),
            Some(2)
        );
        assert_eq!(
            record_get(record, "location.h").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            record_get(record, "town").and_then(SlValue::as_u64),
            Some(7)
        );
        assert_eq!(
            record_get(record, "type").and_then(SlValue::as_u64),
            Some(512)
        );
    }
}
