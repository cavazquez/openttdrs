//! `GenerateClearTile` de `clear_cmd.cpp` para mapas nuevos.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expect_used
)]

use crate::cargodist::parity::Randomizer;
use crate::map::tree_tile_loop::clear_ground_type;
use crate::map::{Map, TileCoord, TileKind};

use super::PreserveRect;
use super::config::{CLEAR_GROUND_DESERT, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH, clear_ground_m5};
use super::population::scale_by_size;

/// Materializa suelo áspero y grupos de roca antes de pueblos, industrias y
/// árboles, como `GenerateClearTile` en `genworld.cpp`.
pub(crate) fn generate_clear_tiles(map: &mut Map, seed: u64, preserve: &[PreserveRect]) {
    let (map_w, map_h) = map.dimensions();
    if map_w == 0 || map_h == 0 {
        return;
    }
    let mut rng = Randomizer::new(seed as u32);
    let rough_steps = scale_by_size((rng.next() & 0x3FF) + 0x400, map_w, map_h);
    let rock_groups = scale_by_size((rng.next() & 0x7F) + 0x80, map_w, map_h);

    for _ in 0..rough_steps {
        let tile = random_tile(rng.next(), map_w, map_h);
        if can_clear(map, tile, preserve) {
            set_ground(map, tile, CLEAR_GROUND_ROUGH);
        }
    }

    for _ in 0..rock_groups {
        let random = rng.next();
        let mut tile = random_tile(random, map_w, map_h);
        if !can_clear(map, tile, preserve) {
            continue;
        }
        let mut steps = ((random >> 16) & 0x0F) + 5;
        loop {
            set_ground(map, tile, CLEAR_GROUND_ROCKY);
            steps = steps.saturating_sub(1);
            if steps == 0 {
                break;
            }
            let direction = rng.next() & 0x03;
            let (dx, dy) = match direction {
                0 => (-1, 0),
                1 => (0, 1),
                2 => (1, 0),
                _ => (0, -1),
            };
            let next = TileCoord::new(tile.x + dx, tile.y + dy);
            if !can_clear(map, next, preserve) {
                break;
            }
            tile = next;
        }
    }
}

fn random_tile(seed: u32, map_w: u32, map_h: u32) -> TileCoord {
    let count = map_w.saturating_mul(map_h).max(1);
    let index = if map_w.is_power_of_two() && map_h.is_power_of_two() {
        seed & count.saturating_sub(1)
    } else {
        seed % count
    };
    TileCoord::new(
        i32::try_from(index % map_w.max(1)).unwrap_or(0),
        i32::try_from(index / map_w.max(1)).unwrap_or(0),
    )
}

fn can_clear(map: &Map, c: TileCoord, preserve: &[PreserveRect]) -> bool {
    if preserve.iter().any(|rect| rect.contains(c.x, c.y)) {
        return false;
    }
    map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Grass && clear_ground_type(tile.m5) != CLEAR_GROUND_DESERT
    })
}

fn set_ground(map: &mut Map, c: TileCoord, ground: u8) {
    let Some(tile) = map.get(c) else {
        return;
    };
    let _ = map.set_mapt_m5(c, tile.mapt, clear_ground_m5(ground, 3));
}

#[cfg(test)]
mod tests {
    use super::generate_clear_tiles;
    use crate::map::{Map, TileKind};

    #[test]
    fn clear_generation_is_deterministic_and_keeps_map_kind() {
        let mut a = Map::new_flat(64, 64, 0);
        let mut b = a.clone();
        generate_clear_tiles(&mut a, 42, &[]);
        generate_clear_tiles(&mut b, 42, &[]);
        assert_eq!(a.tiles(), b.tiles());
        assert!(a.tiles().iter().any(|tile| tile.m5 >> 2 == 1));
        assert!(a.tiles().iter().any(|tile| tile.m5 >> 2 == 2));
        assert!(a.tiles().iter().all(|tile| tile.kind == TileKind::Grass));
    }
}
