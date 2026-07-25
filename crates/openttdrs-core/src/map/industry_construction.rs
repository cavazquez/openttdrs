//! Obra de industrias en mapa — paridad con `MakeIndustryTileBigger` (`industry_cmd.cpp`).
//!
//! OpenTTD avanza cada tesela en su franja del tile loop (pueden desfasarse).
//! Aquí el footprint de una industria avanza **sincronizado**: etapa 0 → 1 → 2 → fin
//! en todas las teselas a la vez (pedido de UX / obra continua).

use std::collections::HashSet;

use super::tile_loop::TileLoopState;
use super::{Map, TileCoord, TileKind};
use crate::Industry;

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

/// Conserva `WaterClass` (bits 5–6) al aplicar el progreso de obra compartido.
#[must_use]
pub fn merge_industry_construction_m1(existing: u8, progress: u8) -> u8 {
    (existing & 0x60) | (progress & !0x60)
}

fn construction_progress_key(m1: u8) -> u16 {
    if is_industry_completed(m1) {
        return u16::MAX;
    }
    u16::from(industry_construction_stage(m1)) * 4 + u16::from(industry_construction_counter(m1))
}

/// Obra + animaciones + random triggers de industria en un tick de sim (P6 + P7).
pub fn step_industry_tiles(
    map: &mut Map,
    tick: u64,
    visits: &[(TileCoord, super::Tile)],
) -> Vec<TileCoord> {
    step_industry_tiles_with_seed(map, tick, visits, 0, &[])
}

/// Como [`step_industry_tiles`], con `world_seed` y footprints para `AnimateTile`.
pub fn step_industry_tiles_with_seed(
    map: &mut Map,
    tick: u64,
    visits: &[(TileCoord, super::Tile)],
    world_seed: u64,
    industries: &[Industry],
) -> Vec<TileCoord> {
    let mut dirty = advance_industry_construction_from_visits(map, visits, industries);
    dirty.extend(
        super::industry_tile_anim::advance_industry_tile_loop_events_from_visits(map, tick, visits),
    );
    let anim_coords = industry_animation_coords(industries);
    dirty.extend(super::industry_tile_anim::advance_industry_animated_tiles(
        map,
        tick,
        &anim_coords,
    ));
    dirty.extend(
        super::industry_random::advance_industry_tile_randomisation_from_visits(
            map, tick, world_seed, visits,
        ),
    );
    dirty.sort_by_key(|c| (c.x, c.y));
    dirty.dedup();
    dirty
}

fn industry_animation_coords(industries: &[Industry]) -> Vec<TileCoord> {
    let mut coords = Vec::new();
    for ind in industries {
        if ind.tiles.is_empty() {
            coords.push(ind.pos);
        } else {
            coords.extend(ind.tiles.iter().copied());
        }
    }
    coords.sort_by_key(|c| (c.x, c.y));
    coords.dedup();
    coords
}

/// Avanza obra incompleta a partir de teselas visitadas por `RunTileLoop`.
pub fn advance_industry_construction_from_visits(
    map: &mut Map,
    visits: &[(TileCoord, super::Tile)],
    industries: &[Industry],
) -> Vec<TileCoord> {
    let mut triggered_ids = HashSet::new();
    let mut orphan_coords = Vec::new();
    for &(coord, tile) in visits {
        if tile.kind != TileKind::Industry || is_industry_completed(tile.m1) {
            continue;
        }
        if tile.m2 != 0
            && let Some(ind) = industries.iter().find(|i| i.instance_id == tile.m2)
        {
            if coord == ind.pos {
                triggered_ids.insert(tile.m2);
            }
            continue;
        }
        orphan_coords.push(coord);
    }

    let mut dirty = Vec::new();
    for id in triggered_ids {
        let Some(ind) = industries.iter().find(|i| i.instance_id == id) else {
            continue;
        };
        let tiles = if ind.tiles.is_empty() {
            std::slice::from_ref(&ind.pos)
        } else {
            ind.tiles.as_slice()
        };
        advance_synced_footprint(map, tiles, &mut dirty);
    }
    for coord in orphan_coords {
        advance_single_tile(map, coord, &mut dirty);
    }
    dirty
}

/// Avanza obra incompleta (tests: ejecuta su propio tile loop).
pub fn advance_industry_construction(
    map: &mut Map,
    tick: u64,
    industries: &[Industry],
    loop_state: &mut TileLoopState,
) -> Vec<TileCoord> {
    let visits =
        super::tile_loop::collect_tile_loop_visits(map, tick, &mut loop_state.cur_tileloop_tile);
    advance_industry_construction_from_visits(map, &visits, industries)
}

fn advance_synced_footprint(map: &mut Map, tiles: &[TileCoord], dirty: &mut Vec<TileCoord>) {
    let mut incomplete = Vec::new();
    let mut shared = None::<u8>;
    for &coord in tiles {
        let Some(tile) = map.get(coord) else {
            continue;
        };
        if tile.kind != TileKind::Industry || is_industry_completed(tile.m1) {
            continue;
        }
        incomplete.push(coord);
        shared = Some(match shared {
            None => tile.m1,
            Some(prev) if construction_progress_key(tile.m1) < construction_progress_key(prev) => {
                tile.m1
            }
            Some(prev) => prev,
        });
    }
    let Some(shared_m1) = shared else {
        return;
    };
    let next = make_industry_tile_bigger(shared_m1);
    if next == shared_m1 {
        return;
    }
    for coord in incomplete {
        let Some(mut tile) = map.get(coord) else {
            continue;
        };
        tile.m1 = merge_industry_construction_m1(tile.m1, next);
        if map.set_tile(coord, tile).is_ok() {
            dirty.push(coord);
        }
    }
}

fn advance_single_tile(map: &mut Map, coord: TileCoord, dirty: &mut Vec<TileCoord>) {
    let Some(mut tile) = map.get(coord) else {
        return;
    };
    if tile.kind != TileKind::Industry || is_industry_completed(tile.m1) {
        return;
    }
    let next = make_industry_tile_bigger(tile.m1);
    if next == tile.m1 {
        return;
    }
    tile.m1 = next;
    if map.set_tile(coord, tile).is_ok() {
        dirty.push(coord);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::Tile;
    use super::super::tile_loop::MAP_TILE_LOOP_STRIDE;
    use super::*;
    use crate::IndustryKind;
    use crate::map::{Map, WaterClass, set_water_class_m1};

    fn industry_tile(m1: u8, m2: u8, m5: u8) -> Tile {
        Tile {
            height: 0,
            kind: TileKind::Industry,
            mapt: 0x80,
            m5,
            m1,
            m6: 0,
            m8: 0,
            m3: 0,
            m2,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

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
    fn construction_completes_after_tile_loop_visits() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(TileCoord::new(0, 0), industry_tile(0, 1, 0))
            .unwrap();
        // ~12 visitas al tile loop (counter×stages) × 256 ticks.
        let mut loop_state = TileLoopState::default();
        for visit in 0..16u64 {
            advance_industry_construction(
                &mut map,
                visit * u64::from(MAP_TILE_LOOP_STRIDE),
                &[],
                &mut loop_state,
            );
        }
        let tile = map.get(TileCoord::new(0, 0)).unwrap();
        assert!(is_industry_completed(tile.m1));
    }

    #[test]
    fn construction_does_not_finish_in_sixty_four_ticks_on_small_map() {
        let mut map = Map::new_flat(4, 4, 0);
        map.set_tile(TileCoord::new(0, 0), industry_tile(0, 1, 0))
            .unwrap();
        let mut loop_state = TileLoopState::default();
        for tick in 0..64u64 {
            advance_industry_construction(&mut map, tick, &[], &mut loop_state);
        }
        let tile = map.get(TileCoord::new(0, 0)).unwrap();
        assert!(
            !is_industry_completed(tile.m1),
            "full-scan rápido no debe completar obra en 64 ticks"
        );
    }

    #[test]
    fn footprint_tiles_share_the_same_construction_stage() {
        let mut map = Map::new_flat(8, 8, 0);
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(3, 2);
        let c = TileCoord::new(2, 3);
        let m1 = set_water_class_m1(0, WaterClass::Invalid);
        for &coord in &[a, b, c] {
            map.set_tile(coord, industry_tile(m1, 1, 0)).unwrap();
        }
        let industry =
            Industry::with_tiles(a, IndustryKind::Factory, vec![a, b, c]).with_instance_id(1);

        // Varias visitas al ancla `pos` (= a).
        let mut loop_state = TileLoopState::default();
        for visit in 0..5u64 {
            advance_industry_construction(
                &mut map,
                visit * u64::from(MAP_TILE_LOOP_STRIDE),
                std::slice::from_ref(&industry),
                &mut loop_state,
            );
        }

        let stages: Vec<_> = [a, b, c]
            .into_iter()
            .map(|coord| industry_construction_stage(map.get(coord).unwrap().m1))
            .collect();
        assert!(
            stages.iter().all(|&s| s == stages[0]),
            "todas las teselas deben compartir etapa: {stages:?}"
        );
        let counters: Vec<_> = [a, b, c]
            .into_iter()
            .map(|coord| industry_construction_counter(map.get(coord).unwrap().m1))
            .collect();
        assert!(
            counters.iter().all(|&n| n == counters[0]),
            "todas las teselas deben compartir contador: {counters:?}"
        );
        // WaterClass Intacta.
        for coord in [a, b, c] {
            assert_eq!(
                crate::map::water_class_from_m1(map.get(coord).unwrap().m1),
                WaterClass::Invalid
            );
        }
    }

    #[test]
    fn construction_completes_on_16x16_with_lfsr() {
        use crate::{Command, GameState, IndustrySpec, apply_command};
        let mut state = GameState::new(16, 16);
        let origin = TileCoord::new(5, 5);
        apply_command(
            &mut state,
            &Command::PlaceIndustrySpec(origin, IndustrySpec::Sawmill),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        for tick in 0..20_000u64 {
            let visits = crate::map::collect_tile_loop_visits(
                &state.map,
                tick,
                &mut loop_state.cur_tileloop_tile,
            );
            advance_industry_construction_from_visits(&mut state.map, &visits, &state.industries);
            if state.map.get(origin).unwrap().m1 & 0x80 != 0 {
                return;
            }
        }
        panic!("obra no terminó en 20k ticks LFSR");
    }

    #[test]
    fn merge_construction_preserves_water_class() {
        let land = set_water_class_m1(0, WaterClass::Invalid);
        let progressed = make_industry_tile_bigger(land);
        let merged = merge_industry_construction_m1(land, progressed);
        assert_eq!(crate::map::water_class_from_m1(merged), WaterClass::Invalid);
        assert_eq!(industry_construction_counter(merged), 1);
    }
}
