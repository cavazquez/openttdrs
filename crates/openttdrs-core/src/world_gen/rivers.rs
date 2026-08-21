//! Generación de ríos y costas.

use crate::map::{
    Map, MapError, TileCoord, TileKind, WaterClass, make_water_tile, set_water_class_m1,
    tile_slope_and_z,
};

use super::config::{PreserveRect, WorldGenConfig};

/// Flujos de río desde tierra alta hacia el mar / lagos (`WaterClass::River`).
pub(super) fn carve_rivers(
    map: &mut Map,
    config: &WorldGenConfig,
    map_w: i32,
    map_h: i32,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    // `CreateRivers` calcula los pozos con `Map::ScaleBySize(4 << amount)`;
    // el conteo anterior dependía del área y producía cuatro veces más ríos
    // que OpenTTD en 64×64. Mantener la misma escala evita que el río altere
    // alturas que ya coinciden con TGP.
    let amount = u32::from(config.amount_of_rivers.min(3));
    let wells = super::population::scale_by_size(4u32 << amount, map_w as u32, map_h as u32);
    let river_count = if amount == 0 { 0 } else { wells.max(1) } as usize;
    for i in 0..river_count {
        let Some(start) = pick_river_source(map, config, map_w, map_h, preserve, i as u64) else {
            continue;
        };
        flow_river(
            map,
            start,
            config.seed ^ (i as u64).wrapping_mul(0x9E37),
            preserve,
        )?;
    }
    Ok(())
}

pub(super) fn pick_river_source(
    map: &Map,
    config: &WorldGenConfig,
    map_w: i32,
    map_h: i32,
    preserve: &[PreserveRect],
    idx: u64,
) -> Option<TileCoord> {
    // TGP deja el mar en altura 0; `sea_level` legado (heightmaps) suele ser 1.
    let sea = config.sea_level.min(1);
    let min_z = sea.saturating_add(2);
    for attempt in 0..64u64 {
        let hash = hash2(
            config.seed ^ 0x5249_5645,
            idx.wrapping_mul(17).wrapping_add(attempt),
        );
        let pos_x = 2 + (hash % (map_w.saturating_sub(4).max(1) as u64)) as i32;
        let pos_y = 2 + ((hash >> 16) % (map_h.saturating_sub(4).max(1) as u64)) as i32;
        if preserve.iter().any(|r| r.contains(pos_x, pos_y)) {
            continue;
        }
        let coord = TileCoord::new(pos_x, pos_y);
        let kind = map.get_kind(coord)?;
        if !matches!(kind, TileKind::Grass | TileKind::Forest) {
            continue;
        }
        let (_, height) = tile_slope_and_z(map, coord)?;
        if height >= min_z {
            return Some(coord);
        }
    }
    None
}

pub(super) fn flow_river(
    map: &mut Map,
    start: TileCoord,
    seed: u64,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    let mut cur = start;
    let mut steps = 0u32;
    let mut rng = seed;
    while steps < 200 {
        steps += 1;
        if preserve.iter().any(|r| r.contains(cur.x, cur.y)) {
            break;
        }
        match map.get_kind(cur) {
            Some(TileKind::Grass | TileKind::Forest | TileKind::CoalField) => {
                make_water_tile(map, cur, WaterClass::River)?;
            }
            _ => break,
        }
        let Some((_, cz)) = tile_slope_and_z(map, cur) else {
            break;
        };
        let mut best: Option<(TileCoord, u8)> = None;
        let mut candidates = [(0i32, 0i32); 4];
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for (i, &(dx, dy)) in dirs.iter().enumerate() {
            candidates[i] = (dx, dy);
        }
        // Mezcla ligera según seed.
        if rng & 1 != 0 {
            candidates.swap(0, 2);
        }
        rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        for &(dx, dy) in &candidates {
            let n = TileCoord::new(cur.x + dx, cur.y + dy);
            if preserve.iter().any(|r| r.contains(n.x, n.y)) {
                continue;
            }
            let Some((_, nz)) = tile_slope_and_z(map, n) else {
                continue;
            };
            if nz > cz {
                continue;
            }
            match best {
                None => best = Some((n, nz)),
                Some((_, bz)) if nz < bz => best = Some((n, nz)),
                Some((_, bz)) if nz == bz && rng.trailing_zeros() >= 3 => best = Some((n, nz)),
                _ => {}
            }
        }
        let Some((next, _)) = best else {
            break;
        };
        if next == cur {
            break;
        }
        cur = next;
    }
    Ok(())
}

fn hash2(a: u64, b: u64) -> u64 {
    a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b)
        .wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

pub(super) fn mark_water_coasts(
    map: &mut Map,
    mw: i32,
    mh: i32,
    _sea_level: u8,
    preserve: &[PreserveRect],
) {
    const WATER_COAST_M5: u8 = 0x10;
    for y in 0..mh {
        for x in 0..mw {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            if map.get_kind(c) != Some(TileKind::Water) {
                continue;
            }
            let adjacent_land = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| {
                    let nc = TileCoord::new(x + dx, y + dy);
                    map.get_kind(nc)
                        .is_some_and(|k| k != TileKind::Water && k != TileKind::Void)
                });
            if adjacent_land {
                let _ = map.set_mapt_m5(c, 0x60, WATER_COAST_M5);
                if let Some(tile) = map.get(c) {
                    let _ = map.set_m1(c, set_water_class_m1(tile.m1, WaterClass::Sea));
                }
            }
        }
    }
}
