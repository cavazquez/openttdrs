//! Crecimiento de árboles y campos (`tree_cmd.cpp` / `tree_map.h`, `clear_cmd.cpp`).
//!
//! Árboles: etapas OpenTTD `TreeGrowthStage` (0…6). Adulto puede morir, densificar
//! la tesela o propagarse a un vecino. Campos (`CoalField`) siguen etapa 0…7 lineal.

use crate::GameState;
use crate::economy::TICKS_PER_TRANSIT_DAY;
use crate::map::{Map, TileCoord, TileKind};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, clear_ground_m5,
};

/// Intervalo entre actualizaciones de crecimiento (8 ticks lógicos).
pub const TREE_GROWTH_TICK_INTERVAL: u64 = 8;
/// Etapa máxima de cultivo en `CoalField` (0–7).
pub const MAX_TREE_OR_FIELD_STAGE: u8 = 7;

/// `TreeGrowthStage::Growing1` … `Dead` (`tree_map.h`).
pub const TREE_GROWTH_GROWING1: u8 = 0;
pub const TREE_GROWTH_GROWN: u8 = 3;
pub const TREE_GROWTH_DEAD: u8 = 6;

const GROWTH_MASK: u8 = 0x07;
const TREE_COUNT_SHIFT: u8 = 6;

/// Frecuencia relativa OpenTTD (`TREE_UPDATE_FREQUENCY = 16` vs grass cada 8).
const TREE_UPDATE_EVERY_N_GENERATIONS: u64 = 2;

#[must_use]
pub const fn tree_or_field_stage(m5: u8) -> u8 {
    m5 & GROWTH_MASK
}

#[must_use]
pub const fn with_tree_or_field_stage(m5: u8, stage: u8) -> u8 {
    (m5 & !GROWTH_MASK) | (stage & GROWTH_MASK)
}

#[must_use]
pub const fn tree_count(m5: u8) -> u8 {
    ((m5 >> TREE_COUNT_SHIFT) & 0x03) + 1
}

#[must_use]
pub const fn with_tree_count(m5: u8, count_minus_one: u8) -> u8 {
    (m5 & !(0x03 << TREE_COUNT_SHIFT)) | ((count_minus_one & 0x03) << TREE_COUNT_SHIFT)
}

#[must_use]
pub const fn clear_ground_type(m5: u8) -> u8 {
    (m5 >> 2) & 0x07
}

#[must_use]
pub const fn clear_density(m5: u8) -> u8 {
    m5 & 0x03
}

#[must_use]
const fn tree_ground(m2: u8) -> u8 {
    (m2 >> 6) & 0x07
}

#[must_use]
const fn tree_ground_density(m2: u8) -> u8 {
    (m2 >> 4) & 0x03
}

#[must_use]
const fn make_tree_m2(ground: u8, density: u8) -> u8 {
    ((ground & 0x07) << 6) | ((density & 0x03) << 4)
}

/// Normaliza etapas inválidas (> `Dead`) dejadas por el crecimiento lineal antiguo.
#[must_use]
pub const fn normalize_tree_growth(m5: u8) -> u8 {
    let g = tree_or_field_stage(m5);
    if g > TREE_GROWTH_DEAD {
        with_tree_or_field_stage(m5, TREE_GROWTH_GROWN)
    } else {
        m5
    }
}

fn tree_rng(world_seed: u64, tick: u64, c: TileCoord, salt: u64) -> u32 {
    let mut x = world_seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick)
        .wrapping_add(u64::from(c.x.cast_unsigned()).wrapping_mul(0xC2B2_AE3D))
        .wrapping_add(u64::from(c.y.cast_unsigned()).wrapping_mul(0x1656_67B1))
        .wrapping_add(salt);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    u32::try_from(x & 0xFFFF_FFFF).unwrap_or(0)
}

const NEIGHBOR_OFFSETS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
];

fn can_plant_trees_on_tile(map: &Map, c: TileCoord) -> bool {
    let Some(tile) = map.get(c) else {
        return false;
    };
    match tile.kind {
        TileKind::Grass => {
            let ground = clear_ground_type(tile.m5);
            !matches!(ground, CLEAR_GROUND_ROCKY | CLEAR_GROUND_DESERT)
        }
        _ => false,
    }
}

fn plant_trees_on_clear(map: &mut Map, c: TileCoord, growth: u8, tree_type: u8) {
    let Some(tile) = map.get(c) else {
        return;
    };
    if tile.kind != TileKind::Grass {
        return;
    }
    let ground = clear_ground_type(tile.m5);
    let density = if ground == CLEAR_GROUND_ROUGH {
        3
    } else {
        clear_density(tile.m5)
    };
    let tree_ground = match ground {
        CLEAR_GROUND_ROUGH => 1,
        CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => 2,
        _ => 0,
    };
    let m5 = with_tree_or_field_stage(with_tree_count(0, 0), growth);
    let m2 = make_tree_m2(tree_ground, density);
    let _ = map.set_kind(c, TileKind::Forest);
    let _ = map.set_mapt_m5(c, 0x40, m5);
    let _ = map.set_m2(c, m2);
    let _ = map.set_m3(c, tree_type);
}

/// Avanza hierba / campos cada [`TREE_GROWTH_TICK_INTERVAL`]; árboles con lógica OpenTTD.
pub fn step_tree_and_field_growth(map: &mut Map, tick: u64, world_seed: u64) {
    if tick == 0 || !tick.is_multiple_of(TREE_GROWTH_TICK_INTERVAL) {
        return;
    }
    let generation = tick / TREE_GROWTH_TICK_INTERVAL;
    let update_trees = generation.is_multiple_of(TREE_UPDATE_EVERY_N_GENERATIONS);

    let mut grass_updates = Vec::new();
    let mut field_updates = Vec::new();
    let mut forest_coords = Vec::new();

    super::tile_loop::for_each_map_tile_loop(map, generation, |c, tile| match tile.kind {
        TileKind::Forest if update_trees => forest_coords.push(c),
        TileKind::CoalField => {
            let stage = tree_or_field_stage(tile.m5);
            if stage < MAX_TREE_OR_FIELD_STAGE {
                let new_m5 = with_tree_or_field_stage(tile.m5, stage + 1);
                field_updates.push((c, tile.mapt, new_m5));
            }
        }
        TileKind::Grass => {
            let ground = clear_ground_type(tile.m5);
            let density = clear_density(tile.m5);
            if ground == CLEAR_GROUND_ROUGH && density == 0 {
                grass_updates.push((c, tile.mapt, clear_ground_m5(CLEAR_GROUND_GRASS, 3)));
                return;
            }
            if ground == CLEAR_GROUND_GRASS && density < 3 {
                grass_updates.push((
                    c,
                    tile.mapt,
                    clear_ground_m5(CLEAR_GROUND_GRASS, density + 1),
                ));
            }
        }
        _ => {}
    });

    for (c, mapt, new_m5) in grass_updates {
        let _ = map.set_mapt_m5(c, mapt, new_m5);
    }
    for (c, mapt, new_m5) in field_updates {
        let _ = map.set_mapt_m5(c, mapt, new_m5);
    }

    for c in forest_coords {
        step_one_forest_tile(map, tick, world_seed, c);
    }
}

fn step_one_forest_tile(map: &mut Map, tick: u64, world_seed: u64, c: TileCoord) {
    let Some(tile) = map.get(c) else {
        return;
    };
    if tile.kind != TileKind::Forest {
        return;
    }

    let m5 = normalize_tree_growth(tile.m5);
    if m5 != tile.m5 {
        let _ = map.set_mapt_m5(c, tile.mapt, m5);
    }
    let growth = tree_or_field_stage(m5);
    let count = tree_count(m5);
    let tree_type = tile.m3;
    let m2 = tile.m2;
    let mapt = tile.mapt;

    match growth {
        TREE_GROWTH_GROWN => {
            // GB(Random(), 0, 3): 0 die, 1 densify, 2 spread, 3–7 nada.
            match tree_rng(world_seed, tick, c, 1) & 0x07 {
                0 => {
                    let new_m5 = with_tree_or_field_stage(m5, growth + 1);
                    let _ = map.set_mapt_m5(c, mapt, new_m5);
                }
                1 => {
                    if count < 4 {
                        let new_m5 = with_tree_or_field_stage(
                            with_tree_count(m5, count),
                            TREE_GROWTH_GROWING1,
                        );
                        let _ = map.set_mapt_m5(c, mapt, new_m5);
                    } else {
                        try_spread_neighbor(map, tick, world_seed, c, tree_type);
                    }
                }
                2 => {
                    try_spread_neighbor(map, tick, world_seed, c, tree_type);
                }
                _ => {}
            }
        }
        TREE_GROWTH_DEAD => {
            if count > 1 {
                let new_m5 = with_tree_or_field_stage(
                    with_tree_count(m5, count.saturating_sub(2)),
                    TREE_GROWTH_GROWN,
                );
                let _ = map.set_mapt_m5(c, mapt, new_m5);
            } else {
                clear_dead_tree_tile(map, c, m2);
            }
        }
        g if g < TREE_GROWTH_GROWN || (TREE_GROWTH_GROWN < g && g < TREE_GROWTH_DEAD) => {
            let new_m5 = with_tree_or_field_stage(m5, g + 1);
            let _ = map.set_mapt_m5(c, mapt, new_m5);
        }
        _ => {}
    }
}

fn try_spread_neighbor(map: &mut Map, tick: u64, world_seed: u64, c: TileCoord, tree_type: u8) {
    let dir = (tree_rng(world_seed, tick, c, 2) as usize) % NEIGHBOR_OFFSETS.len();
    let (dx, dy) = NEIGHBOR_OFFSETS[dir];
    let n = TileCoord::new(c.x + dx, c.y + dy);
    if !can_plant_trees_on_tile(map, n) {
        return;
    }
    let Some(tile) = map.get(n) else {
        return;
    };
    // No plantar sobre hierba recién despejada (densidad ≠ 3).
    if clear_ground_type(tile.m5) == CLEAR_GROUND_GRASS && clear_density(tile.m5) != 3 {
        return;
    }
    plant_trees_on_clear(map, n, TREE_GROWTH_GROWING1, tree_type);
}

fn clear_dead_tree_tile(map: &mut Map, c: TileCoord, m2: u8) {
    let ground = tree_ground(m2);
    let density = tree_ground_density(m2);
    let (clear_ground, clear_density) = match ground {
        1 => (CLEAR_GROUND_ROUGH, 3), // Rough
        2 | 4 => {
            // SnowOrDesert / RoughSnow → hierba+nieve o rough según clima simplificado.
            if ground == 4 {
                (CLEAR_GROUND_ROUGH, 3)
            } else {
                (CLEAR_GROUND_GRASS, 3)
            }
        }
        _ => (CLEAR_GROUND_GRASS, density), // Grass / Shore → hierba
    };
    let _ = map.set_kind(c, TileKind::Grass);
    let _ = map.set_mapt_m5(c, 0x00, clear_ground_m5(clear_ground, clear_density));
    let _ = map.set_m2(c, 0);
}

/// Nieve estacional simplificada para clima ártico (línea de nieve por latitud + estación).
pub fn apply_seasonal_snow(map: &mut Map, climate: Climate, tick: u64, world_seed: u64) {
    if !climate.uses_snow_ground() {
        return;
    }
    if tick == 0 || !tick.is_multiple_of(u64::from(TICKS_PER_TRANSIT_DAY)) {
        return;
    }
    let day = tick / u64::from(TICKS_PER_TRANSIT_DAY);
    let day_of_year = day % 365;
    let winter = (300..=364).contains(&day_of_year) || day_of_year < 75;
    let (_, h) = map.dimensions();
    let snow_line = i32::try_from(h).unwrap_or(0) * 2 / 5;

    let mut updates = Vec::new();
    super::tile_loop::for_each_map_tile_loop(map, day, |c, tile| {
        if tile.kind != TileKind::Grass {
            return;
        }
        let ground = if winter && c.y < snow_line {
            CLEAR_GROUND_SNOW
        } else {
            CLEAR_GROUND_GRASS
        };
        let density = tile.m5 & 0x03;
        let new_m5 = clear_ground_m5(ground, density);
        if new_m5 != tile.m5 {
            updates.push((c, tile.mapt, new_m5));
        }
    });
    for (c, mapt, new_m5) in updates {
        let _ = map.set_mapt_m5(c, mapt, new_m5);
    }
    let _ = world_seed;
}

/// Coloca un árbol (hierba → bosque etapa 0; bosque → +1 árbol si hay sitio).
pub fn plant_tree(
    game_state: &mut GameState,
    c: TileCoord,
) -> Result<(), crate::command::CommandError> {
    use crate::command::{CommandError, in_bounds};
    in_bounds(&game_state.map, c)?;
    let tile = game_state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    match tile.kind {
        TileKind::Grass => {
            let density = if clear_ground_type(tile.m5) == CLEAR_GROUND_ROUGH {
                3
            } else {
                clear_density(tile.m5)
            };
            let tree_ground = match clear_ground_type(tile.m5) {
                CLEAR_GROUND_ROUGH => 1,
                CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => 2,
                _ => 0,
            };
            let m5 = with_tree_or_field_stage(with_tree_count(0, 0), TREE_GROWTH_GROWING1);
            game_state
                .map
                .set_kind(c, TileKind::Forest)
                .map_err(|_| CommandError::OutOfBounds)?;
            game_state
                .map
                .set_mapt_m5(c, 0x40, m5)
                .map_err(|_| CommandError::OutOfBounds)?;
            game_state
                .map
                .set_m2(c, make_tree_m2(tree_ground, density))
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        TileKind::Forest => {
            let count = tree_count(tile.m5);
            if count >= 4 {
                return Err(CommandError::CannotPlantTreeHere);
            }
            let new_m5 = with_tree_count(tile.m5, count);
            game_state
                .map
                .set_mapt_m5(c, tile.mapt, new_m5)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        _ => return Err(CommandError::CannotPlantTreeHere),
    }
    Ok(())
}

/// Quita un árbol de la tesela; sin árboles → hierba.
pub fn clear_tree(
    game_state: &mut GameState,
    c: TileCoord,
) -> Result<(), crate::command::CommandError> {
    use crate::command::{CommandError, in_bounds};
    in_bounds(&game_state.map, c)?;
    let kind = game_state
        .map
        .get_kind(c)
        .ok_or(CommandError::OutOfBounds)?;
    match kind {
        TileKind::Forest => {
            let tile = game_state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            let count = tree_count(tile.m5);
            if count <= 1 {
                clear_dead_tree_tile(&mut game_state.map, c, tile.m2);
            } else {
                let new_m5 = with_tree_count(tile.m5, count.saturating_sub(2));
                game_state
                    .map
                    .set_mapt_m5(c, tile.mapt, new_m5)
                    .map_err(|_| CommandError::OutOfBounds)?;
            }
        }
        TileKind::CoalField => {
            let tile = game_state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            let growth = tree_or_field_stage(tile.m5);
            if growth == 0 {
                return Err(CommandError::NoTreeHere);
            }
            let new_m5 = with_tree_or_field_stage(tile.m5, growth - 1);
            game_state
                .map
                .set_mapt_m5(c, tile.mapt, new_m5)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        _ => return Err(CommandError::NoTreeHere),
    }
    Ok(())
}

/// Hook combinado para `sim_step`.
pub fn tick_tree_tile_loop(state: &mut GameState) {
    let tick = state.tick.get();
    step_tree_and_field_growth(&mut state.map, tick, state.world_seed);
    apply_seasonal_snow(&mut state.map, state.climate, tick, state.world_seed);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Command, GameState, apply_command};

    fn force_forest(map: &mut Map, c: TileCoord, m5: u8) {
        map.set_kind(c, TileKind::Forest).unwrap();
        map.set_mapt_m5(c, 0x40, m5).unwrap();
        map.set_m2(c, make_tree_m2(0, 3)).unwrap();
        map.set_m3(c, 0).unwrap();
    }

    #[test]
    fn tree_grows_every_update_generation() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(1, 1);
        apply_command(&mut state, &Command::PlantTree(c)).unwrap();
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 0);
        // generation 2 (tick 16) es múltiplo de TREE_UPDATE_EVERY_N_GENERATIONS.
        let tick = TREE_GROWTH_TICK_INTERVAL * TREE_UPDATE_EVERY_N_GENERATIONS;
        step_tree_and_field_growth(&mut state.map, tick, 0);
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 1);
    }

    #[test]
    fn growing_stops_advancing_past_grown_without_rng_death() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(0, 0);
        // Grown: con seed/tick que den GB&7 >= 3 no cambia.
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        let mut found_stable = false;
        for salt_tick in
            (1u64..=64).map(|g| g * TREE_GROWTH_TICK_INTERVAL * TREE_UPDATE_EVERY_N_GENERATIONS)
        {
            let before = map.get(c).unwrap().m5;
            step_tree_and_field_growth(&mut map, salt_tick, 0xDEAD_BEEF);
            let after = map.get(c).unwrap().m5;
            if after == before && tree_or_field_stage(after) == TREE_GROWTH_GROWN {
                found_stable = true;
                break;
            }
            // Si murió o densificó, volver a Grown 1 árbol.
            force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        }
        assert!(found_stable, "adulto debe poder quedarse estable");
    }

    #[test]
    fn grown_can_start_dying() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(0, 0);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        let mut died = false;
        for g in 1u64..=256 {
            let tick = g * TREE_GROWTH_TICK_INTERVAL * TREE_UPDATE_EVERY_N_GENERATIONS;
            step_tree_and_field_growth(&mut map, tick, 42);
            let stage = tree_or_field_stage(map.get(c).unwrap().m5);
            if stage == TREE_GROWTH_GROWN + 1 {
                died = true;
                break;
            }
            if map.get_kind(c) != Some(TileKind::Forest) {
                // se propagó o limpió; reinstalar adulto
                force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
            } else if stage != TREE_GROWTH_GROWN {
                force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
            }
        }
        assert!(died, "adulto debe poder iniciar Dying1");
    }

    #[test]
    fn dead_single_tree_becomes_grass() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(0, 0);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_DEAD));
        let tick = TREE_GROWTH_TICK_INTERVAL * TREE_UPDATE_EVERY_N_GENERATIONS;
        step_tree_and_field_growth(&mut map, tick, 0);
        assert_eq!(map.get_kind(c), Some(TileKind::Grass));
    }

    #[test]
    fn invalid_stage_seven_normalizes_to_grown() {
        assert_eq!(
            tree_or_field_stage(normalize_tree_growth(0x07)),
            TREE_GROWTH_GROWN
        );
    }

    #[test]
    fn plant_and_clear_tree_on_grass() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(2, 2);
        // Hierba completa para no confundir clear_density.
        state
            .map
            .set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        plant_tree(&mut state, c).unwrap();
        assert_eq!(state.map.get_kind(c), Some(TileKind::Forest));
        assert_eq!(tree_count(state.map.get(c).unwrap().m5), 1);
        clear_tree(&mut state, c).unwrap();
        assert_eq!(state.map.get_kind(c), Some(TileKind::Grass));
    }

    #[test]
    fn plant_on_forest_adds_tree_count() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(1, 1);
        force_forest(
            &mut state.map,
            c,
            with_tree_or_field_stage(0, TREE_GROWTH_GROWN),
        );
        plant_tree(&mut state, c).unwrap();
        assert_eq!(tree_count(state.map.get(c).unwrap().m5), 2);
    }

    #[test]
    fn field_stage_caps_at_seven() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(0, 0);
        state.map.set_kind(c, TileKind::CoalField).unwrap();
        state
            .map
            .set_mapt_m5(c, 0x50, MAX_TREE_OR_FIELD_STAGE)
            .unwrap();
        step_tree_and_field_growth(&mut state.map, TREE_GROWTH_TICK_INTERVAL, 0);
        assert_eq!(
            tree_or_field_stage(state.map.get(c).unwrap().m5),
            MAX_TREE_OR_FIELD_STAGE
        );
    }

    #[test]
    fn full_grass_m5_is_not_corrupted_into_rough() {
        let mut map = Map::new_flat(1, 1, 0);
        let c = TileCoord::new(0, 0);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        assert_eq!(map.get(c).unwrap().m5, 0x03);
        for generation in 1..=8 {
            step_tree_and_field_growth(&mut map, generation * TREE_GROWTH_TICK_INTERVAL, 0);
        }
        assert_eq!(
            map.get(c).unwrap().m5,
            0x03,
            "hierba completa no debe convertirse en Rough (0x04)"
        );
    }

    #[test]
    fn invalid_rough_density_zero_repairs_to_full_grass() {
        let mut map = Map::new_flat(1, 1, 0);
        let c = TileCoord::new(0, 0);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, 0x04).unwrap(); // Rough + density 0 (inválido)
        step_tree_and_field_growth(&mut map, TREE_GROWTH_TICK_INTERVAL, 0);
        assert_eq!(map.get(c).unwrap().m5, 0x03);
    }

    #[test]
    fn grown_can_spread_to_neighbor_grass() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        for y in 0..3 {
            for x in 0..3 {
                let t = TileCoord::new(x, y);
                if t == c {
                    continue;
                }
                map.set_kind(t, TileKind::Grass).unwrap();
                map.set_mapt_m5(t, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
                    .unwrap();
            }
        }
        let mut spread = false;
        for g in 1u64..=512 {
            let tick = g * TREE_GROWTH_TICK_INTERVAL * TREE_UPDATE_EVERY_N_GENERATIONS;
            step_tree_and_field_growth(&mut map, tick, 7);
            let forests = (0..3)
                .flat_map(|y| (0..3).map(move |x| TileCoord::new(x, y)))
                .filter(|&t| map.get_kind(t) == Some(TileKind::Forest))
                .count();
            if forests >= 2 {
                spread = true;
                break;
            }
            // Mantener el origen adulto si murió.
            if map.get_kind(c) != Some(TileKind::Forest)
                || tree_or_field_stage(map.get(c).unwrap().m5) != TREE_GROWTH_GROWN
            {
                force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
            }
        }
        assert!(spread, "debe poder propagarse a hierba vecina");
    }
}
