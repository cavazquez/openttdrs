//! Serialización semántica del pool `Depot` (`DEPT`).

use std::collections::BTreeMap;

use super::super::SavDepot;
use super::super::SavError;
use super::chunks::table_chunk;
use super::codec::write_str;
use crate::depot::DEPOT_POOL_SIZE;
use crate::game_state::GameState;
use crate::map::{TileCoord, TileKind, coord_to_linear_index};

/// Límite defensivo equivalente al pool nativo de `DepotID`.
const MAX_DEPOT_ROWS_TO_EXPORT: usize = DEPOT_POOL_SIZE as usize;

fn map_depot_tiles(state: &GameState) -> BTreeMap<u16, TileCoord> {
    let mut tiles = BTreeMap::new();
    let (width, height) = state.map.dimensions();
    for y in 0..height.cast_signed() {
        for x in 0..width.cast_signed() {
            let pos = TileCoord::new(x, y);
            let Some(tile) = state.map.get(pos) else {
                continue;
            };
            let Some(depot_id) = crate::depot::depot_id_from_tile(tile) else {
                continue;
            };
            let canonical = if tile.kind == TileKind::ShipDepot {
                crate::depot::ship_depot_north_tile(&state.map, pos).unwrap_or(pos)
            } else {
                pos
            };
            tiles.entry(depot_id).or_insert(canonical);
        }
    }
    tiles
}

/// Construye filas densas `DepotID → Depot`.
///
/// Los registros presentes en `GameState` son autoritativos para metadata.
/// Si un JSON antiguo sólo conserva `MAP2`, se agrega una fila mínima para
/// cada ID observado en el mapa y así no se pierde la identidad al exportar.
pub(crate) fn dept_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    let mut depots = BTreeMap::<u16, SavDepot>::new();
    for depot in &state.depots {
        if depot.depot_id >= DEPOT_POOL_SIZE {
            return Err(SavError::BadFormat(format!(
                "DepotID fuera del pool nativo: {}",
                depot.depot_id
            )));
        }
        depots.insert(depot.depot_id, depot.clone());
    }
    let build_date =
        crate::news::openttd_date_from_calendar_day_index(u64::from(state.calendar.date));
    for (depot_id, tile) in map_depot_tiles(state) {
        depots.entry(depot_id).or_insert_with(|| SavDepot {
            depot_id,
            tile,
            town_id: None,
            town_cn: 0,
            name: String::new(),
            build_date,
        });
    }
    let Some(max_id) = depots.keys().next_back().copied() else {
        return Ok(Vec::new());
    };
    let rows = usize::from(max_id) + 1;
    if rows > MAX_DEPOT_ROWS_TO_EXPORT {
        return Err(SavError::AllocationFailed {
            context: "pool DEPT",
            requested: rows,
        });
    }

    // `DEPT` es una tabla densa: los slots libres se representan con una fila
    // vacía para que el índice siga coincidiendo con `DepotID`.
    let mut records = vec![Vec::new(); rows];
    for (depot_id, depot) in depots {
        let tile = coord_to_linear_index(depot.tile, map_w).ok_or_else(|| {
            SavError::BadFormat(format!(
                "tesela de depósito fuera del mapa: ({}, {})",
                depot.tile.x, depot.tile.y
            ))
        })?;
        let town = depot
            .town_id
            .map(|town_id| {
                town_id
                    .checked_add(1)
                    .ok_or_else(|| SavError::BadFormat(format!("TownID fuera de rango: {town_id}")))
            })
            .transpose()?
            .unwrap_or(0);
        let mut record = Vec::new();
        record.extend_from_slice(&tile.to_be_bytes());
        record.extend_from_slice(&town.to_be_bytes());
        record.extend_from_slice(&depot.town_cn.to_be_bytes());
        write_str(&depot.name, &mut record)?;
        record.extend_from_slice(&depot.build_date.to_be_bytes());
        records[usize::from(depot_id)] = record;
    }
    Ok(records)
}

/// Construye el chunk nativo `DEPT` (`CH_TABLE`).
pub(crate) fn dept_chunk(records: &[Vec<u8>]) -> Result<Option<Vec<u8>>, SavError> {
    if records.is_empty() {
        return Ok(None);
    }
    Ok(Some(table_chunk(
        *b"DEPT",
        &[
            (6, "xy"),
            (6, "town"),
            (4, "town_cn"),
            (0x1A, "name"),
            (5, "build_date"),
        ],
        records,
    )?))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::sav::chunks::parse_chunks;
    use crate::sav::table::{SlValue, parse_table_chunk, record_get};

    fn depot(tile: TileCoord, depot_id: u16) -> SavDepot {
        SavDepot {
            depot_id,
            tile,
            town_id: Some(7),
            town_cn: 12,
            name: "Depósito Central".into(),
            build_date: 702_345,
        }
    }

    #[test]
    fn dense_dept_rows_keep_pool_holes_and_native_fields() {
        let mut state = GameState::new(32, 32);
        state.depots.push(depot(TileCoord::new(4, 5), 2));

        let records = dept_records(&state, 32).expect("DEPT records");
        assert_eq!(records.len(), 3);
        assert!(records[0].is_empty());
        assert!(records[1].is_empty());
        let chunk = dept_chunk(&records).expect("chunk result").expect("DEPT");
        let parsed = parse_table_chunk(&parse_chunks(&chunk).expect("parse chunks")[0].body, false)
            .expect("parse DEPT");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, 2);
        assert_eq!(
            record_get(&parsed[0].1, "xy").and_then(SlValue::as_u64),
            Some(5 * 32 + 4)
        );
        assert_eq!(
            record_get(&parsed[0].1, "town").and_then(SlValue::as_u64),
            Some(8)
        );
        assert_eq!(
            record_get(&parsed[0].1, "town_cn").and_then(SlValue::as_u64),
            Some(12)
        );
        assert_eq!(
            record_get(&parsed[0].1, "name").and_then(SlValue::as_str),
            Some("Depósito Central")
        );
        assert_eq!(
            record_get(&parsed[0].1, "build_date").and_then(SlValue::as_i64),
            Some(702_345)
        );
    }
}
