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
pub(crate) fn generate_clear_tiles(map: &mut Map, rng: &mut Randomizer, preserve: &[PreserveRect]) {
    let (map_w, map_h) = map.dimensions();
    if map_w == 0 || map_h == 0 {
        return;
    }
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
        let tile = random_tile(random, map_w, map_h);
        if !can_clear(map, tile, preserve) {
            continue;
        }
        place_rock_group(map, rng, preserve, tile, ((random >> 16) & 0x0F) + 5);
    }
}

/// Porta el bucle interno de rocas de `GenerateClearTile`.
///
/// Un salto inválido no termina el grupo: `OpenTTD` consume otro `Random()` e
/// intenta una dirección distinta, salvo que ya se agotó el largo del grupo.
/// Terminar en el primer borde/agua desalineaba el stream sólo en mapas que
/// encontraban ese caso (RMAP-021).
fn place_rock_group(
    map: &mut Map,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
    mut tile: TileCoord,
    mut steps: u32,
) {
    'group: loop {
        set_ground(map, tile, CLEAR_GROUND_ROCKY);
        loop {
            steps = steps.saturating_sub(1);
            if steps == 0 {
                break 'group;
            }
            let direction = rng.next() & 0x03;
            let (dx, dy) = match direction {
                0 => (-1, 0),
                1 => (0, 1),
                2 => (1, 0),
                _ => (0, -1),
            };
            let next = TileCoord::new(tile.x + dx, tile.y + dy);
            if can_clear(map, next, preserve) {
                tile = next;
                break;
            }
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
    use super::{generate_clear_tiles, place_rock_group};
    use crate::cargodist::parity::Randomizer;
    use crate::map::{Map, TileCoord, TileKind};
    use crate::world_gen::{CLEAR_GROUND_ROCKY, clear_ground_m5};

    #[test]
    fn clear_generation_is_deterministic_and_keeps_map_kind() {
        let mut a = Map::new_flat(64, 64, 0);
        let mut b = a.clone();
        let mut rng_a = Randomizer::new(42);
        let mut rng_b = Randomizer::new(42);
        generate_clear_tiles(&mut a, &mut rng_a, &[]);
        generate_clear_tiles(&mut b, &mut rng_b, &[]);
        assert_eq!(a.tiles(), b.tiles());
        assert!(a.tiles().iter().any(|tile| tile.m5 >> 2 == 1));
        assert!(a.tiles().iter().any(|tile| tile.m5 >> 2 == 2));
        assert!(a.tiles().iter().all(|tile| tile.kind == TileKind::Grass));
    }

    #[test]
    fn rocky_group_retries_an_invalid_direction_without_ending_the_group() {
        let mut map = Map::new_flat(3, 3, 0);
        // Con seed 42, los dos primeros sorteos de dirección son oeste (0)
        // y sur (1). El oeste no es clear; el segundo sí debe ser aceptado.
        map.set_kind(TileCoord::new(0, 1), TileKind::Water)
            .expect("water boundary");
        let mut actual = Randomizer::new(42);

        place_rock_group(&mut map, &mut actual, &[], TileCoord::new(1, 1), 3);

        assert_eq!(
            map.get(TileCoord::new(1, 1)).expect("center").m5,
            clear_ground_m5(CLEAR_GROUND_ROCKY, 3)
        );
        assert_eq!(
            map.get(TileCoord::new(1, 2)).expect("south").m5,
            clear_ground_m5(CLEAR_GROUND_ROCKY, 3)
        );
        let mut expected = Randomizer::new(42);
        let _ = expected.next();
        let _ = expected.next();
        assert_eq!(actual, expected);
    }
}
