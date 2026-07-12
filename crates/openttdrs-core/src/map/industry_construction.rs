//! Obra de industrias en mapa — paridad con `MakeIndustryTileBigger` (`industry_cmd.cpp`).

use super::tile_loop::for_each_map_tile_loop;
use super::{Map, TileCoord, TileKind};

/// Etapa de obra 0–2 en `m1` bits 0–1; 3 = terminada (`INDUSTRY_COMPLETED`).
pub const INDUSTRY_CONSTRUCTION_COMPLETED: u8 = 3;

/// OpenTTD `GetIndustryConstructionStage`.
#[must_use]
pub fn industry_construction_stage(m1: u8) -> u8 {
    if m1 & 0x80 != 0 {
        INDUSTRY_CONSTRUCTION_COMPLETED
    } else {
        m1 & 0x03
    }
}

/// OpenTTD `GetIndustryConstructionCounter` (bits 2–3 de `m1`).
#[must_use]
pub fn industry_construction_counter(m1: u8) -> u8 {
    (m1 >> 2) & 0x03
}

/// OpenTTD `IsIndustryCompleted`.
#[must_use]
pub fn is_industry_completed(m1: u8) -> bool {
    m1 & 0x80 != 0
}

/// Un paso de `MakeIndustryTileBigger` sobre el byte `m1`.
#[must_use]
pub fn make_industry_tile_bigger(m1: u8) -> u8 {
    if is_industry_completed(m1) {
        return m1;
    }
    let counter = industry_construction_counter(m1) + 1;
    if counter != 4 {
        return (m1 & 0xF3) | ((counter & 0x03) << 2);
    }
    let stage = industry_construction_stage(m1) + 1;
    if stage >= INDUSTRY_CONSTRUCTION_COMPLETED {
        m1 | 0x83
    } else {
        (m1 & 0xF0) | (stage & 0x03)
    }
}

/// Obra + animaciones de industria en un tick de sim (P6 + P7).
pub fn step_industry_tiles(map: &mut Map, tick: u64) -> Vec<TileCoord> {
    let mut dirty = advance_industry_construction(map, tick);
    dirty.extend(super::industry_tile_anim::advance_industry_tile_animations(
        map, tick,
    ));
    dirty.sort_by_key(|c| (c.x, c.y));
    dirty.dedup();
    dirty
}

/// Avanza obra en teselas `MP_INDUSTRY` incompletas (una franja de tile loop por tick).
pub fn advance_industry_construction(map: &mut Map, tick: u64) -> Vec<TileCoord> {
    let mut candidates = Vec::new();
    for_each_map_tile_loop(map, tick, |coord, tile| {
        if tile.kind == TileKind::Industry && !is_industry_completed(tile.m1) {
            candidates.push(coord);
        }
    });
    let mut dirty = Vec::new();
    for coord in candidates {
        let Some(mut tile) = map.get(coord) else {
            continue;
        };
        if tile.kind != TileKind::Industry || is_industry_completed(tile.m1) {
            continue;
        }
        let next = make_industry_tile_bigger(tile.m1);
        if next == tile.m1 {
            continue;
        }
        tile.m1 = next;
        if map.set_tile(coord, tile).is_ok() {
            dirty.push(coord);
        }
    }
    dirty
}

#[cfg(test)]
mod tests {
    use super::super::Tile;
    use super::*;
    use crate::map::Map;

    #[test]
    fn construction_advances_counter_then_stage() {
        let mut m1 = 0u8;
        for expected_counter in 1..=3 {
            m1 = make_industry_tile_bigger(m1);
            assert!(!is_industry_completed(m1));
            assert_eq!(industry_construction_stage(m1), 0);
            assert_eq!(industry_construction_counter(m1), expected_counter);
        }
        m1 = make_industry_tile_bigger(m1);
        assert_eq!(industry_construction_stage(m1), 1);
        assert_eq!(industry_construction_counter(m1), 0);
    }

    #[test]
    fn construction_counter_bits_round_trip_in_m1() {
        let m1 = make_industry_tile_bigger(0);
        assert_eq!(m1 & 0x0F, 0x04); // stage 0, counter 1
        let m1 = make_industry_tile_bigger(m1);
        assert_eq!(industry_construction_counter(m1), 2);
    }

    #[test]
    fn construction_completes_after_four_stages() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            Tile {
                height: 0,
                kind: TileKind::Industry,
                mapt: 0x80,
                m5: 0,
                m1: 0,
                m6: 0,
                m8: 0,
                m3: 0,
                m2: 1,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            },
        )
        .unwrap();
        for tick in 0..64 {
            advance_industry_construction(&mut map, tick);
        }
        let tile = map.get(TileCoord::new(0, 0)).unwrap();
        assert!(is_industry_completed(tile.m1));
    }
}
