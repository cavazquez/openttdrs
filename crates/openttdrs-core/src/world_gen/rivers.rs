//! Generación de ríos y costas.

use crate::map::slope::SLOPE_STEEP;
use crate::map::water_flood::{DIR_OFFSETS, FLOOD_FROM_DIRS, is_slope_one_corner_raised};
use crate::map::{
    Map, MapError, TileCoord, TileKind, WaterClass, make_shore_tile, make_water_tile,
    set_water_class_m1, tile_slope_and_z,
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
    if amount == 0 {
        return Ok(());
    }
    let wells = super::population::scale_by_size(4u32 << amount, map_w as u32, map_h as u32);
    // C++ prueba primero `max(1, wells / 10)` ríos largos; el resto son
    // cortos. En 64² y cantidad predeterminada hay exactamente un pozo largo
    // (mínimo 16 × 4), no un río corto que pueda cruzar el mapa de cualquier
    // modo. Esta clasificación debe ocurrir antes de pintar una tesela.
    let long_wells = (wells / 10).max(1).min(wells);
    for i in 0..wells as usize {
        let Some(start) = pick_river_source(map, config, map_w, map_h, preserve, i as u64) else {
            continue;
        };
        let min_river_length = u32::from(config.min_river_length)
            .saturating_mul(if i < long_wells as usize { 4 } else { 1 });
        let _ = flow_river(
            map,
            start,
            config.seed ^ (i as u64).wrapping_mul(0x9E37),
            preserve,
            min_river_length,
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
    min_river_length: u32,
) -> Result<bool, MapError> {
    let mut cur = start;
    let mut steps = 0u32;
    let mut rng = seed;
    let mut path = Vec::new();
    let mut reached_water = false;
    while steps < 200 {
        steps += 1;
        if preserve.iter().any(|r| r.contains(cur.x, cur.y)) {
            break;
        }
        match map.get_kind(cur) {
            Some(TileKind::Grass | TileKind::Forest | TileKind::CoalField) => {
                // `FlowRiver`/YAPF primero prueba que el flujo alcanza una
                // terminación válida; no materializa un río parcial mientras
                // explora. Guardar el trayecto evita que un pozo largo fallido
                // deje agua sintética en mapas pequeños.
                path.push(cur);
            }
            Some(TileKind::Water) => {
                reached_water = true;
                break;
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

    let distance = start.x.abs_diff(cur.x) + start.y.abs_diff(cur.y);
    if !reached_water || distance <= min_river_length {
        return Ok(false);
    }

    for tile in path {
        make_water_tile(map, tile, WaterClass::River)?;
    }
    Ok(true)
}

fn hash2(a: u64, b: u64) -> u64 {
    a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b)
        .wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

/// Port de `ConvertGroundTilesIntoWaterTiles` de `water_cmd.cpp`.
///
/// El generador original no transforma todo terreno a altura cero en agua:
/// las pendientes complejas se conservan como `MP_CLEAR` salvo que alguna de
/// sus direcciones de inundación llegue a una pendiente plana, de una esquina
/// elevada o a `MP_VOID`. Esta distinción determina tanto el tipo de tesela
/// como `MAP1`/`MAP5`, y por extensión el comportamiento posterior de
/// `TileLoop_Water`.
pub(super) fn convert_ground_tiles_into_water_tiles(
    map: &mut Map,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    let (width, height) = map.dimensions();
    let width = i32::try_from(width).expect("map width fits i32");
    let height = i32::try_from(height).expect("map height fits i32");

    for y in 0..height {
        for x in 0..width {
            if preserve.iter().any(|rect| rect.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            if map.get_kind(c) != Some(TileKind::Grass) {
                continue;
            }
            let Some((slope, z)) = tile_slope_and_z(map, c) else {
                continue;
            };
            if z != 0 {
                continue;
            }

            match slope {
                0 => make_water_tile(map, c, WaterClass::Sea)?,
                1 | 2 | 4 | 8 => make_shore_tile(map, c)?,
                _ => {
                    let slope_index = usize::from((slope & !SLOPE_STEEP) & 0x0F);
                    let mut directions = FLOOD_FROM_DIRS[slope_index];
                    let mut make_shore = false;
                    while directions != 0 {
                        let direction = directions.trailing_zeros() as usize;
                        directions &= directions - 1;
                        let (dx, dy) = DIR_OFFSETS[direction];
                        let dest = TileCoord::new(c.x + dx, c.y + dy);
                        if map.get(dest).is_none() {
                            continue;
                        }
                        let Some((dest_slope, _)) = tile_slope_and_z(map, dest) else {
                            continue;
                        };
                        let dest_slope = dest_slope & !SLOPE_STEEP;
                        if dest_slope == 0
                            || is_slope_one_corner_raised(dest_slope)
                            || map.get_kind(dest) == Some(TileKind::Void)
                        {
                            make_shore = true;
                            break;
                        }
                    }
                    if make_shore {
                        make_shore_tile(map, c)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Heurística heredada para heightmaps externos.
///
/// El pipeline procedural usa [`convert_ground_tiles_into_water_tiles`], que
/// replica el contrato por pendiente del original. Los heightmaps ya llegan
/// con el agua materializada y conservan esta pasada visual independiente.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::company::{OWNER_NONE_M1, OWNER_WATER_M1};
    use crate::map::is_river_tile;

    #[test]
    fn converter_makes_flat_ground_sea_and_one_corner_slope_shore() {
        let mut map = Map::new_flat(8, 8, 0);
        let flat = TileCoord::new(2, 2);
        let shore = TileCoord::new(5, 2);
        // `hwest` de `shore`: slope W (una única esquina elevada).
        map.set_height(TileCoord::new(shore.x + 1, shore.y), 1)
            .expect("raise west corner");

        convert_ground_tiles_into_water_tiles(&mut map, &[]).expect("convert ground");

        let sea = map.get(flat).expect("flat sea");
        assert_eq!(sea.kind, TileKind::Water);
        assert_eq!(sea.m1, OWNER_WATER_M1);
        assert_eq!(sea.m5, 0);

        let coast = map.get(shore).expect("shore");
        assert_eq!(coast.kind, TileKind::Water);
        assert_eq!(coast.m1, OWNER_WATER_M1);
        assert_eq!(coast.m5, 0x10);
    }

    #[test]
    fn converter_keeps_slope_without_flood_directions_as_clear_ground() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        // `SLOPE_EW` (bits 1|4) no tiene entradas en `_flood_from_dirs`.
        map.set_height(TileCoord::new(c.x + 1, c.y), 1)
            .expect("raise west corner");
        map.set_height(TileCoord::new(c.x, c.y + 1), 1)
            .expect("raise east corner");
        map.set_m1(c, OWNER_NONE_M1).expect("clear owner");

        convert_ground_tiles_into_water_tiles(&mut map, &[]).expect("convert ground");

        let clear = map.get(c).expect("clear slope");
        assert_eq!(clear.kind, TileKind::Grass);
        assert_eq!(clear.m1, OWNER_NONE_M1);
    }

    fn descending_river_test_map(width: i32, sea_x: i32) -> Map {
        let mut map = Map::new_flat(width as u32, 8, 0);
        for x in 0..width {
            for y in 0..8 {
                map.set_height(TileCoord::new(x, y), (96 - x).try_into().expect("height"))
                    .expect("set height");
            }
        }
        make_water_tile(&mut map, TileCoord::new(sea_x, 3), WaterClass::Sea)
            .expect("make terminal sea");
        map
    }

    #[test]
    fn long_river_rejects_a_route_shorter_than_its_manhattan_minimum() {
        let mut map = descending_river_test_map(16, 9);
        let start = TileCoord::new(1, 3);

        let built = flow_river(&mut map, start, 7, &[], 16).expect("flow river");

        assert!(!built);
        assert!(map.tiles().iter().all(|tile| !is_river_tile(*tile)));
    }

    #[test]
    fn accepted_route_is_painted_only_after_reaching_water_and_minimum_length() {
        let mut map = descending_river_test_map(40, 25);
        let start = TileCoord::new(1, 3);

        let built = flow_river(&mut map, start, 7, &[], 16).expect("flow river");

        assert!(built);
        assert!(map.tiles().iter().any(|tile| is_river_tile(*tile)));
    }

    #[test]
    fn one_well_is_a_long_attempt_like_create_rivers() {
        let wells = 1u32;
        let long_wells = (wells / 10).max(1).min(wells);
        assert_eq!(long_wells, 1);
        assert_eq!(u32::from(16u8).saturating_mul(4), 64);
    }
}
