//! Iteración del mapa al estilo OpenTTD `RunTileLoop` (LFSR de Galois).

use super::{Map, Tile, TileCoord};
use crate::map::index::{coord_from_linear_index, openttd_tile_index_to_coord};

/// Paso entre visitas completas a la misma tesela (`TILE_UPDATE_FREQUENCY` = 256).
pub const MAP_TILE_LOOP_STRIDE: u32 = 256;

/// `TILE_UPDATE_FREQUENCY_LOG` en `landscape.cpp`.
pub const TILE_UPDATE_FREQUENCY_LOG: u32 = 8;

/// `MIN_MAP_SIZE_BITS` en `map_type.h`.
pub const MIN_MAP_SIZE_BITS: u32 = 6;

/// Por debajo de este tamaño `for_each_map_tile_loop` barre el mapa completo
/// (industrias / animación en tests). El crecimiento de paisaje usa siempre LFSR.
pub const MAP_FULL_SCAN_TILE_LIMIT: u32 = 65_536;

/// Estado persistente del tile loop (`_cur_tileloop_tile` en OpenTTD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TileLoopState {
    #[serde(default = "default_cur_tileloop_tile")]
    pub cur_tileloop_tile: u32,
}

impl Default for TileLoopState {
    fn default() -> Self {
        Self {
            cur_tileloop_tile: default_cur_tileloop_tile(),
        }
    }
}

impl TileLoopState {
    /// El LFSR no puede quedar en cero (`afterload.cpp` en OpenTTD).
    pub fn sanitize(&mut self) {
        if self.cur_tileloop_tile == 0 {
            self.cur_tileloop_tile = 1;
        }
    }
}

#[must_use]
pub const fn default_cur_tileloop_tile() -> u32 {
    1
}

/// Términos de feedback LFSR (12–24 bits) extraídos de OpenTTD `landscape.cpp`.
const LFSR_FEEDBACK: [u32; 13] = [
    0xD8F, 0x1296, 0x2496, 0x4357, 0x8679, 0x1_030E, 0x2_06CD, 0x4_03FE, 0x8_07B8, 0x10_04B2,
    0x20_06A8, 0x40_04B2, 0x80_0B87,
];

/// Bits del índice de tesela (≈ `Map::LogX() + Map::LogY()` en mapas Po2).
#[must_use]
pub fn tile_index_bits(map: &Map) -> u32 {
    let (w, h) = map.dimensions();
    let size = w.saturating_mul(h).max(1);
    (size - 1).next_power_of_two().trailing_zeros()
}

#[must_use]
pub fn map_log_x(map: &Map) -> u32 {
    let w = map.dimensions().0.max(1);
    if w.is_power_of_two() {
        w.trailing_zeros()
    } else {
        (w - 1).next_power_of_two().trailing_zeros()
    }
}

#[must_use]
pub fn map_log_y(map: &Map) -> u32 {
    let h = map.dimensions().1.max(1);
    if h.is_power_of_two() {
        h.trailing_zeros()
    } else {
        (h - 1).next_power_of_two().trailing_zeros()
    }
}

/// Suma de bits de mapa con mínimo OpenTTD (64×64 → log 6 por eje).
#[must_use]
fn effective_log_sum(map: &Map) -> u32 {
    map_log_x(map)
        .max(MIN_MAP_SIZE_BITS)
        .saturating_add(map_log_y(map).max(MIN_MAP_SIZE_BITS))
}

#[must_use]
fn lfsr_feedback(map: &Map) -> u32 {
    let idx = effective_log_sum(map).saturating_sub(2 * MIN_MAP_SIZE_BITS) as usize;
    LFSR_FEEDBACK[idx.min(LFSR_FEEDBACK.len() - 1)]
}

#[must_use]
fn lfsr_visit_count(map: &Map) -> u32 {
    let bits = effective_log_sum(map);
    debug_assert!(bits >= TILE_UPDATE_FREQUENCY_LOG);
    1u32 << (bits - TILE_UPDATE_FREQUENCY_LOG)
}

#[must_use]
fn mask_tile_index(tile: u32, map: &Map) -> u32 {
    let (w, h) = map.dimensions();
    let size = w.saturating_mul(h).max(1);
    if w.is_power_of_two() && h.is_power_of_two() && w == h {
        tile & (size - 1)
    } else {
        tile % size
    }
}

#[must_use]
fn lfsr_next(mut tile: u32, feedback: u32) -> u32 {
    let lsb = tile & 1;
    tile >>= 1;
    if lsb != 0 {
        tile ^= feedback;
    }
    tile
}

#[must_use]
pub fn tile_index_to_coord(tile: u32, map: &Map) -> Option<TileCoord> {
    let (w, h) = map.dimensions();
    if w.is_power_of_two()
        && h.is_power_of_two()
        && let Some(coord) = openttd_tile_index_to_coord(tile, w, h)
    {
        return Some(coord);
    }
    coord_from_linear_index(u64::from(tile), w)
}

/// OpenTTD `RunTileLoop`: visita `map_size / 256` teselas y actualiza el estado LFSR.
pub fn run_tile_loop(
    map: &Map,
    tick: u64,
    loop_state: &mut TileLoopState,
    mut visit: impl FnMut(TileCoord, Tile),
) {
    loop_state.sanitize();
    let feedback = lfsr_feedback(map);
    let mut count = lfsr_visit_count(map);
    let mut tile = loop_state.cur_tileloop_tile;

    if tick.is_multiple_of(u64::from(MAP_TILE_LOOP_STRIDE)) {
        if let Some(coord) = tile_index_to_coord(0, map)
            && let Some(t) = map.get(coord)
        {
            visit(coord, t);
        }
        count = count.saturating_sub(1);
    }

    while count > 0 {
        let masked = mask_tile_index(tile, map);
        if masked != 0
            && let Some(coord) = tile_index_to_coord(masked, map)
            && let Some(t) = map.get(coord)
        {
            visit(coord, t);
        }
        tile = lfsr_next(tile, feedback);
        count -= 1;
    }

    loop_state.cur_tileloop_tile = if tile == 0 { 1 } else { tile };
}

/// Ejecuta `RunTileLoop` y devuelve las teselas visitadas (actualiza `cur_tileloop_tile`).
#[must_use]
pub fn collect_tile_loop_visits(
    map: &Map,
    tick: u64,
    cur_tileloop_tile: &mut u32,
) -> Vec<(TileCoord, Tile)> {
    let mut loop_state = TileLoopState {
        cur_tileloop_tile: *cur_tileloop_tile,
    };
    let mut visited = Vec::new();
    run_tile_loop(map, tick, &mut loop_state, |coord, tile| {
        visited.push((coord, tile));
    });
    *cur_tileloop_tile = loop_state.cur_tileloop_tile;
    visited
}

/// Visita teselas con el LFSR (una pasada por tick de sim).
pub fn for_each_map_tile_loop_stripe(
    map: &Map,
    tick: u64,
    loop_state: &mut TileLoopState,
    visit: impl FnMut(TileCoord, Tile),
) {
    run_tile_loop(map, tick, loop_state, visit);
}

/// Visita teselas del mapa: completo si es pequeño; si no, una pasada LFSR.
pub fn for_each_map_tile_loop(
    map: &Map,
    tick: u64,
    loop_state: &mut TileLoopState,
    mut visit: impl FnMut(TileCoord, Tile),
) {
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

    run_tile_loop(map, tick, loop_state, visit);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn small_maps_visit_every_tile() {
        let map = Map::new_flat(4, 4, 0);
        let mut loop_state = TileLoopState::default();
        let mut n = 0;
        for_each_map_tile_loop(&map, 0, &mut loop_state, |_, _| n += 1);
        assert_eq!(n, 16);
    }

    #[test]
    fn large_maps_visit_map_size_over_256_per_tick() {
        const _: () = assert!(512 * 512 > MAP_FULL_SCAN_TILE_LIMIT);
        let map = Map::new_flat(512, 512, 0);
        let mut loop_state = TileLoopState::default();
        let mut n0 = 0;
        for_each_map_tile_loop(&map, 0, &mut loop_state, |_, _| n0 += 1);
        let mut n1 = 0;
        for_each_map_tile_loop(&map, 1, &mut loop_state, |_, _| n1 += 1);
        assert_eq!(n0, (512 * 512) / MAP_TILE_LOOP_STRIDE);
        assert_eq!(n1, n0);
    }

    #[test]
    fn small_maps_visit_sixteen_tiles_per_tick() {
        let map = Map::new_flat(16, 16, 0);
        let mut loop_state = TileLoopState::default();
        let mut n = 0;
        run_tile_loop(&map, 1, &mut loop_state, |_, _| n += 1);
        assert_eq!(n, 16);
    }

    #[test]
    fn lfsr_never_visits_tile_zero_in_main_loop() {
        let map = Map::new_flat(64, 64, 0);
        let mut loop_state = TileLoopState::default();
        let mut main_loop_hits = HashSet::new();
        for tick in 1..256u64 {
            run_tile_loop(&map, tick, &mut loop_state, |c, _| {
                main_loop_hits.insert(c);
            });
        }
        assert!(
            !main_loop_hits.contains(&TileCoord::new(0, 0)),
            "el bucle LFSR principal nunca debe visitar la tesela 0"
        );
    }

    #[test]
    fn lfsr_covers_entire_map_after_256_ticks() {
        let map = Map::new_flat(64, 64, 0);
        let mut loop_state = TileLoopState::default();
        let mut visited = HashSet::new();
        for tick in 0..256u64 {
            run_tile_loop(&map, tick, &mut loop_state, |c, _| {
                visited.insert(c);
            });
        }
        assert_eq!(visited.len(), 64 * 64);
    }

    #[test]
    fn lfsr_is_deterministic_from_same_state() {
        let map = Map::new_flat(128, 128, 0);
        let mut a = TileLoopState::default();
        let mut b = TileLoopState::default();
        let mut seq_a = Vec::new();
        let mut seq_b = Vec::new();
        for tick in 0..32u64 {
            run_tile_loop(&map, tick, &mut a, |c, _| seq_a.push((tick, c)));
            run_tile_loop(&map, tick, &mut b, |c, _| seq_b.push((tick, c)));
        }
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn tile_zero_visited_only_on_stride_multiples() {
        let map = Map::new_flat(8, 8, 0);
        let mut loop_state = TileLoopState::default();
        let mut hits = 0;
        for tick in 0..512u64 {
            run_tile_loop(&map, tick, &mut loop_state, |c, _| {
                if c == TileCoord::new(0, 0) {
                    hits += 1;
                }
            });
        }
        assert_eq!(hits, 2, "tesela 0: una visita especial cada 256 ticks");
    }
}
