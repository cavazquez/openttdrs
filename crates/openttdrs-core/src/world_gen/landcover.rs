//! Cobertura de terreno: densidad de hierba, bosques y zonas tropicales.

use crate::map::tree_tile_loop::clear_ground_type;
use crate::map::{Map, TileCoord, TileKind};

use super::PreserveRect;
use super::config::CLEAR_GROUND_DESERT;

const TROPIC_ZONE_MASK: u8 = 0x03;
const TROPIC_ZONE_DESERT: u8 = 1;
const TROPIC_ZONE_RAINFOREST: u8 = 2;

/// Offsets de `_make_desert_or_rainforest_data` en `table/genland.h`.
///
/// El bloque cuadrado de radio cinco se completa primero y luego se añaden
/// los cuatro lados del anillo de radio seis (coordenadas -3..=3). El orden
/// no afecta al predicado `all_of`, pero mantener la misma forma evita que
/// futuras trazas mezclen reglas de vecindad distintas.
const fn tropic_zone_offsets() -> [(i32, i32); 149] {
    let mut offsets = [(0_i32, 0_i32); 149];
    let mut index = 0;
    let mut y = -5;
    while y <= 5 {
        let mut x = -5;
        while x <= 5 {
            offsets[index] = (x, y);
            index += 1;
            x += 1;
        }
        y += 1;
    }
    let mut edge = -3;
    while edge <= 3 {
        offsets[index] = (6, edge);
        offsets[index + 1] = (-6, edge);
        offsets[index + 2] = (edge, 6);
        offsets[index + 3] = (edge, -6);
        index += 4;
        edge += 1;
    }
    offsets
}

const TROPIC_OFFSETS: [(i32, i32); 149] = tropic_zone_offsets();

fn set_zone(map: &mut Map, c: TileCoord, zone: u8) {
    let Some(mut tile) = map.get(c) else {
        return;
    };
    tile.mapt = (tile.mapt & !TROPIC_ZONE_MASK) | (zone & TROPIC_ZONE_MASK);
    let _ = map.set_tile(c, tile);
}

fn offset_tile(map: &Map, c: TileCoord, dx: i32, dy: i32) -> Option<crate::map::Tile> {
    map.get(TileCoord::new(
        c.x.saturating_add(dx),
        c.y.saturating_add(dy),
    ))
}

/// Marca las zonas desérticas de `CreateDesertOrRainForest`.
///
/// `OpenTTD` exige que las 145 teselas de la ventana estén por debajo de la
/// línea y no sean agua. Un desplazamiento fuera del mapa satisface el
/// predicado (`INVALID_TILE`), mientras que una tesela `MP_VOID` dentro del
/// mapa se evalúa por su altura como en `AddTileIndexDiffCWrap`.
pub(crate) fn mark_tropic_desert_zones(map: &mut Map, desert_line: u8, preserve: &[PreserveRect]) {
    let (width, height) = map.dimensions();
    for y in 0..height {
        for x in 0..width {
            let c = TileCoord::new(x as i32, y as i32);
            if preserve.iter().any(|rect| rect.contains(c.x, c.y)) {
                continue;
            }
            let Some(tile) = map.get(c) else {
                continue;
            };
            if tile.kind == TileKind::Void {
                continue;
            }
            let allows_desert = TROPIC_OFFSETS.iter().all(|&(dx, dy)| {
                offset_tile(map, c, dx, dy).is_none_or(|neighbour| {
                    neighbour.height < desert_line && neighbour.kind != TileKind::Water
                })
            });
            if allows_desert {
                set_zone(map, c, TROPIC_ZONE_DESERT);
            }
        }
    }
}

/// Marca las zonas de selva después de la primera ronda de tile loops.
pub(crate) fn mark_tropic_rainforest_zones(map: &mut Map, preserve: &[PreserveRect]) {
    let (width, height) = map.dimensions();
    for y in 0..height {
        for x in 0..width {
            let c = TileCoord::new(x as i32, y as i32);
            if preserve.iter().any(|rect| rect.contains(c.x, c.y)) {
                continue;
            }
            let Some(tile) = map.get(c) else {
                continue;
            };
            if tile.kind == TileKind::Void {
                continue;
            }
            let allows_rainforest = TROPIC_OFFSETS.iter().all(|&(dx, dy)| {
                offset_tile(map, c, dx, dy).is_none_or(|neighbour| {
                    neighbour.kind != TileKind::Grass
                        || clear_ground_type(neighbour.m5) != CLEAR_GROUND_DESERT
                })
            });
            if allows_rainforest {
                set_zone(map, c, TROPIC_ZONE_RAINFOREST);
            }
        }
    }
}

fn hash_u64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn i64_pair_hash(x: i32, y: i32) -> u64 {
    u64::from(x as u32)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(y as u32).wrapping_mul(0x6C62_272E_07BB_0142))
}

pub(super) fn grass_density(x: i32, y: i32, seed: u64) -> u8 {
    let n = hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(3), y.wrapping_mul(5))));
    // Variación suave: mayoría hierba completa; sin `bare` (m5==0) para no confundir con default.
    match n % 10 {
        0..=1 => 1,
        2..=4 => 2,
        _ => 3,
    }
}

pub fn desert_patch(x: i32, y: i32, seed: u64) -> bool {
    hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(13), y.wrapping_mul(17)))) % 5 == 0
}
