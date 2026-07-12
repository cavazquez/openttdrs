//! Iteración por franjas del mapa (estilo OpenTTD tile loop).

use super::{Map, Tile, TileCoord};

/// Paso entre teselas procesadas en un tick (análogo al tile loop de OpenTTD).
pub const MAP_TILE_LOOP_STRIDE: u32 = 256;

/// Por debajo de este tamaño se barre el mapa completo (tests y mapas chicos).
pub const MAP_FULL_SCAN_TILE_LIMIT: u32 = 65_536;

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
}
