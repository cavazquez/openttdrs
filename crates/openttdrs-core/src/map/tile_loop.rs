//! Iteración por franjas del mapa (estilo OpenTTD tile loop).

use super::{Map, Tile, TileCoord};

/// Paso entre teselas procesadas en un tick (`TILE_UPDATE_FREQUENCY` = 256 en OpenTTD).
pub const MAP_TILE_LOOP_STRIDE: u32 = 256;

/// Por debajo de este tamaño `for_each_map_tile_loop` barre el mapa completo
/// (industrias / animación en tests). El crecimiento de paisaje usa siempre franjas.
pub const MAP_FULL_SCAN_TILE_LIMIT: u32 = 65_536;

fn visit_stripe(map: &Map, tick: u64, mut visit: impl FnMut(TileCoord, Tile)) {
    let (w, h) = map.dimensions();
    let Some(n) = w.checked_mul(h).filter(|&n| n > 0) else {
        return;
    };
    let start = u32::try_from(tick % u64::from(MAP_TILE_LOOP_STRIDE)).unwrap_or(0);
    let mut i = start;
    while i < n {
        let ux = i % w;
        let uy = i / w;
        let coord = TileCoord::new(
            i32::try_from(ux).unwrap_or(0),
            i32::try_from(uy).unwrap_or(0),
        );
        if let Some(tile) = map.get(coord) {
            visit(coord, tile);
        }
        let Some(next) = i.checked_add(MAP_TILE_LOOP_STRIDE) else {
            break;
        };
        i = next;
    }
}

/// Visita una franja `tick % 256` (cada tesela ~cada 256 ticks), como `RunTileLoop`.
pub fn for_each_map_tile_loop_stripe(map: &Map, tick: u64, visit: impl FnMut(TileCoord, Tile)) {
    visit_stripe(map, tick, visit);
}

/// Visita teselas del mapa: completo si es pequeño; si no, una franja `tick % 256`.
pub fn for_each_map_tile_loop(map: &Map, tick: u64, mut visit: impl FnMut(TileCoord, Tile)) {
    let (w, h) = map.dimensions();
    let Some(n) = w.checked_mul(h).filter(|&n| n > 0) else {
        return;
    };

    if n <= MAP_FULL_SCAN_TILE_LIMIT {
        for uy in 0..h {
            for ux in 0..w {
                let coord = TileCoord::new(
                    i32::try_from(ux).unwrap_or(0),
                    i32::try_from(uy).unwrap_or(0),
                );
                if let Some(tile) = map.get(coord) {
                    visit(coord, tile);
                }
            }
        }
        return;
    }

    visit_stripe(map, tick, visit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_maps_visit_every_tile() {
        let map = Map::new_flat(4, 4, 0);
        let mut n = 0;
        for_each_map_tile_loop(&map, 0, |_, _| n += 1);
        assert_eq!(n, 16);
    }

    #[test]
    fn large_maps_visit_one_stripe_per_tick() {
        const _: () = assert!(512 * 512 > MAP_FULL_SCAN_TILE_LIMIT);
        let map = Map::new_flat(512, 512, 0);
        let mut n0 = 0;
        for_each_map_tile_loop(&map, 0, |_, _| n0 += 1);
        let mut n1 = 0;
        for_each_map_tile_loop(&map, 1, |_, _| n1 += 1);
        assert_eq!(n0, (512 * 512) / MAP_TILE_LOOP_STRIDE);
        assert_eq!(n1, n0);
    }

    #[test]
    fn stripe_visits_same_tile_every_256_ticks_even_on_small_maps() {
        let map = Map::new_flat(4, 4, 0);
        let target = TileCoord::new(1, 1);
        let mut hits = 0;
        for tick in 0..512u64 {
            for_each_map_tile_loop_stripe(&map, tick, |c, _| {
                if c == target {
                    hits += 1;
                }
            });
        }
        assert_eq!(hits, 2, "4×4: cada tesela una vez cada 256 ticks");
    }
}
