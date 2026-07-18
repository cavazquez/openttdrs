//! Crecimiento de árboles y campos (`tree_cmd.cpp` / `tree_map.h`, `clear_cmd.cpp`).
//!
//! Ritmo OpenTTD (`landscape.cpp` + `TileLoop_Trees`):
//! - cada tesela se visita cada [`TILE_LOOP_FREQUENCY`] ticks (`RunTileLoop`);
//! - hierba/campos avanzan cada 8 visitas (`cycle & 7 == 7`);
//! - árboles avanzan cada [`TREE_UPDATE_FREQUENCY`] visitas (`cycle % 16 == 15`).
//!
//! Árboles: etapas `TreeGrowthStage` (0…6). Adulto puede morir, densificar o propagarse.
//! Campos (`CoalField`) siguen etapa 0…7 lineal.

use crate::GameState;
use crate::map::tile_loop::{MAP_TILE_LOOP_STRIDE, for_each_map_tile_loop_stripe};
use crate::map::{Map, TileCoord, TileKind, coord_to_linear_index, tile_slope_and_z};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, DEF_SNOW_LINE_HEIGHT, clear_ground_m5,
};

/// OpenTTD `TILE_UPDATE_FREQUENCY`: ticks entre visitas a la misma tesela.
pub const TILE_LOOP_FREQUENCY: u64 = 256;
/// Alias histórico (= [`TILE_LOOP_FREQUENCY`]).
pub const TREE_GROWTH_TICK_INTERVAL: u64 = TILE_LOOP_FREQUENCY;
/// OpenTTD `TREE_UPDATE_FREQUENCY`: visitas al tile loop por un avance de árbol.
pub const TREE_UPDATE_FREQUENCY: u32 = 16;
/// Etapa máxima de cultivo en `CoalField` (0–7).
pub const MAX_TREE_OR_FIELD_STAGE: u8 = 7;

/// `TreeGrowthStage::Growing1` … `Dead` (`tree_map.h`).
pub const TREE_GROWTH_GROWING1: u8 = 0;
pub const TREE_GROWTH_GROWN: u8 = 3;
pub const TREE_GROWTH_DEAD: u8 = 6;

const GROWTH_MASK: u8 = 0x07;
const TREE_COUNT_SHIFT: u8 = 6;

/// Ciclo de paisaje OpenTTD: `11*x + 9*y + (tick >> 8)`.
#[must_use]
pub fn landscape_tile_cycle(c: TileCoord, tick: u64) -> u32 {
    let x = c.x.cast_unsigned();
    let y = c.y.cast_unsigned();
    let epoch = u32::try_from(tick >> 8).unwrap_or(u32::MAX);
    11u32
        .wrapping_mul(x)
        .wrapping_add(9u32.wrapping_mul(y))
        .wrapping_add(epoch)
}

/// Índice lineal para franjas del tile loop.
///
/// Precondición habitual: coords de mapa no negativas. Si son negativas, se
/// preserva el wrap `cast_unsigned` histórico del landscape cycle.
#[must_use]
fn tile_index(c: TileCoord, map_w: u32) -> u32 {
    coord_to_linear_index(c, map_w).unwrap_or_else(|| {
        c.y.cast_unsigned()
            .saturating_mul(map_w)
            .saturating_add(c.x.cast_unsigned())
    })
}

/// Primer tick ≥ `after` en el que la tesela recibe una actualización de árbol.
#[must_use]
pub fn next_tree_update_tick(c: TileCoord, map_w: u32, after: u64) -> u64 {
    let stripe = u64::from(tile_index(c, map_w) % MAP_TILE_LOOP_STRIDE);
    let mut tick = after.saturating_add(1);
    // Alinear a la franja de esta tesela.
    let rem = tick % u64::from(MAP_TILE_LOOP_STRIDE);
    if rem != stripe {
        tick += (stripe + u64::from(MAP_TILE_LOOP_STRIDE) - rem) % u64::from(MAP_TILE_LOOP_STRIDE);
    }
    for _ in 0..(u64::from(TREE_UPDATE_FREQUENCY) * 2) {
        if landscape_tile_cycle(c, tick) % TREE_UPDATE_FREQUENCY == TREE_UPDATE_FREQUENCY - 1 {
            return tick;
        }
        tick = tick.saturating_add(u64::from(MAP_TILE_LOOP_STRIDE));
    }
    tick
}

/// Primer tick ≥ `after` en el que hierba/campos avanzan en esa tesela (`cycle & 7 == 7`).
#[must_use]
pub fn next_clear_update_tick(c: TileCoord, map_w: u32, after: u64) -> u64 {
    let stripe = u64::from(tile_index(c, map_w) % MAP_TILE_LOOP_STRIDE);
    let mut tick = after.saturating_add(1);
    let rem = tick % u64::from(MAP_TILE_LOOP_STRIDE);
    if rem != stripe {
        tick += (stripe + u64::from(MAP_TILE_LOOP_STRIDE) - rem) % u64::from(MAP_TILE_LOOP_STRIDE);
    }
    for _ in 0..16u64 {
        if landscape_tile_cycle(c, tick) & 7 == 7 {
            return tick;
        }
        tick = tick.saturating_add(u64::from(MAP_TILE_LOOP_STRIDE));
    }
    tick
}

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

/// Avanza hierba / campos / árboles al ritmo de `RunTileLoop` + `TileLoop_Trees`.
pub fn step_tree_and_field_growth(map: &mut Map, tick: u64, world_seed: u64) {
    let mut grass_updates = Vec::new();
    let mut field_updates = Vec::new();
    let mut forest_coords = Vec::new();
    let mut forest_ground_updates = Vec::new();

    for_each_map_tile_loop_stripe(map, tick, |c, tile| {
        let cycle = landscape_tile_cycle(c, tick);
        match tile.kind {
            TileKind::Forest => {
                // Hierba bajo árboles: cada 8 visitas, como Clear grass.
                if cycle & 7 == 7 && tree_ground(tile.m2) == 0 {
                    let density = tree_ground_density(tile.m2);
                    if density < 3 {
                        forest_ground_updates.push((c, make_tree_m2(0, density + 1)));
                    }
                }
                if cycle % TREE_UPDATE_FREQUENCY == TREE_UPDATE_FREQUENCY - 1 {
                    forest_coords.push(c);
                }
            }
            TileKind::CoalField => {
                if cycle & 7 != 7 {
                    return;
                }
                let stage = tree_or_field_stage(tile.m5);
                if stage < MAX_TREE_OR_FIELD_STAGE {
                    let new_m5 = with_tree_or_field_stage(tile.m5, stage + 1);
                    field_updates.push((c, tile.mapt, new_m5));
                }
            }
            TileKind::Grass => {
                if cycle & 7 != 7 {
                    return;
                }
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
        }
    });

    for (c, m2) in forest_ground_updates {
        let _ = map.set_m2(c, m2);
    }
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

/// Nieve ártico al estilo OpenTTD `TileLoopClearAlps`: altura vs snow line, franja tile-loop.
///
/// Cada tick procesa `MapSize/256` teselas (misma franja que el landscape). La densidad
/// sube/baja de a 1 hasta el nivel requerido; no hay barrido O(map) diario.
///
/// Devuelve las teselas cuyo `m5` cambió (para remap del cliente).
pub fn apply_seasonal_snow(
    map: &mut Map,
    climate: Climate,
    tick: u64,
    world_seed: u64,
) -> Vec<TileCoord> {
    let _ = world_seed;
    apply_seasonal_snow_with_line(map, climate, tick, DEF_SNOW_LINE_HEIGHT)
}

/// Como [`apply_seasonal_snow`] con línea de nieve explícita (tests / settings futuros).
pub fn apply_seasonal_snow_with_line(
    map: &mut Map,
    climate: Climate,
    tick: u64,
    snow_line_height: u8,
) -> Vec<TileCoord> {
    if !climate.uses_snow_ground() {
        return Vec::new();
    }
    let snow_line = i32::from(snow_line_height);
    let mut updates = Vec::new();
    for_each_map_tile_loop_stripe(map, tick, |c, tile| {
        if tile.kind != TileKind::Grass {
            return;
        }
        let ground = clear_ground_type(tile.m5);
        if matches!(ground, CLEAR_GROUND_ROCKY | CLEAR_GROUND_DESERT) {
            return;
        }
        let Some((_, z)) = tile_slope_and_z(map, c) else {
            return;
        };
        let k = i32::from(z) - snow_line + 1;
        let is_snow = ground == CLEAR_GROUND_SNOW;
        let density = clear_density(tile.m5);
        let new_m5 = if is_snow {
            let req = if k < 0 {
                0_u8
            } else {
                u8::try_from(k.clamp(0, 3)).unwrap_or(3)
            };
            match density.cmp(&req) {
                std::cmp::Ordering::Equal => {
                    if k < 0 {
                        // ClearSnow → hierba densa.
                        Some(clear_ground_m5(CLEAR_GROUND_GRASS, 3))
                    } else {
                        None
                    }
                }
                std::cmp::Ordering::Less => Some(clear_ground_m5(
                    CLEAR_GROUND_SNOW,
                    density.saturating_add(1),
                )),
                std::cmp::Ordering::Greater => Some(clear_ground_m5(
                    CLEAR_GROUND_SNOW,
                    density.saturating_sub(1),
                )),
            }
        } else if k >= 0 {
            // MakeSnow(density=0): transición gradual hacia la densidad requerida.
            Some(clear_ground_m5(CLEAR_GROUND_SNOW, 0))
        } else {
            None
        };
        if let Some(new_m5) = new_m5
            && new_m5 != tile.m5
        {
            updates.push((c, tile.mapt, new_m5));
        }
    });
    let mut dirty = Vec::with_capacity(updates.len());
    for (c, mapt, new_m5) in updates {
        let _ = map.set_mapt_m5(c, mapt, new_m5);
        dirty.push(c);
    }
    dirty
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
    let snow_dirty = apply_seasonal_snow(&mut state.map, state.climate, tick, state.world_seed);
    state.runtime.landscape_tile_dirty.extend(snow_dirty);
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

    fn map_w(map: &Map) -> u32 {
        map.dimensions().0
    }

    #[test]
    fn tree_grows_on_open_ttd_update_cycle() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(1, 1);
        apply_command(&mut state, &Command::PlantTree(c)).unwrap();
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 0);
        let tick = next_tree_update_tick(c, map_w(&state.map), 0);
        step_tree_and_field_growth(&mut state.map, tick, 0);
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 1);
        // Un tick de franja sin ciclo de árbol no debe avanzar otra etapa.
        let between = tick + u64::from(MAP_TILE_LOOP_STRIDE);
        if landscape_tile_cycle(c, between) % TREE_UPDATE_FREQUENCY != TREE_UPDATE_FREQUENCY - 1 {
            step_tree_and_field_growth(&mut state.map, between, 0);
            assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 1);
        }
    }

    #[test]
    fn tree_stage_needs_about_4096_ticks_per_step() {
        // Visita cada 256 ticks × TREE_UPDATE_FREQUENCY 16 ≈ 4096 ticks/etapa.
        let c = TileCoord::new(0, 0);
        let t0 = next_tree_update_tick(c, 4, 0);
        let t1 = next_tree_update_tick(c, 4, t0);
        assert!(
            t1 - t0 >= 256 * 15,
            "intervalo entre avances debe ser ~4096 ticks, got {}",
            t1 - t0
        );
        assert!(t1 - t0 <= 256 * 17);
    }

    #[test]
    fn growing_stops_advancing_past_grown_without_rng_death() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(0, 0);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        let mut found_stable = false;
        let mut after = 0u64;
        for _ in 0..64 {
            let tick = next_tree_update_tick(c, map_w(&map), after);
            after = tick;
            let before = map.get(c).unwrap().m5;
            step_tree_and_field_growth(&mut map, tick, 0xDEAD_BEEF);
            let after_m5 = map.get(c).unwrap().m5;
            if after_m5 == before && tree_or_field_stage(after_m5) == TREE_GROWTH_GROWN {
                found_stable = true;
                break;
            }
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
        let mut after = 0u64;
        for _ in 0..256 {
            let tick = next_tree_update_tick(c, map_w(&map), after);
            after = tick;
            step_tree_and_field_growth(&mut map, tick, 42);
            let stage = tree_or_field_stage(map.get(c).unwrap().m5);
            if stage == TREE_GROWTH_GROWN + 1 {
                died = true;
                break;
            }
            if map.get_kind(c) != Some(TileKind::Forest) || stage != TREE_GROWTH_GROWN {
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
        let tick = next_tree_update_tick(c, map_w(&map), 0);
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
        let tick = next_clear_update_tick(c, map_w(&state.map), 0);
        step_tree_and_field_growth(&mut state.map, tick, 0);
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
        let mut after = 0u64;
        for _ in 0..8 {
            let tick = next_clear_update_tick(c, map_w(&map), after);
            after = tick;
            step_tree_and_field_growth(&mut map, tick, 0);
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
        let tick = next_clear_update_tick(c, map_w(&map), 0);
        step_tree_and_field_growth(&mut map, tick, 0);
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
        let mut after = 0u64;
        for _ in 0..512 {
            let tick = next_tree_update_tick(c, map_w(&map), after);
            after = tick;
            step_tree_and_field_growth(&mut map, tick, 7);
            let forests = (0..3)
                .flat_map(|y| (0..3).map(move |x| TileCoord::new(x, y)))
                .filter(|&t| map.get_kind(t) == Some(TileKind::Forest))
                .count();
            if forests >= 2 {
                spread = true;
                break;
            }
            if map.get_kind(c) != Some(TileKind::Forest)
                || tree_or_field_stage(map.get(c).unwrap().m5) != TREE_GROWTH_GROWN
            {
                force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
            }
        }
        assert!(spread, "debe poder propagarse a hierba vecina");
    }

    #[test]
    fn clear_alps_makes_snow_above_line_and_thaws_below() {
        let mut map = Map::new_flat(8, 8, 0);
        let high = TileCoord::new(2, 2);
        let low = TileCoord::new(5, 5);
        for c in [high, low] {
            map.set_kind(c, TileKind::Grass).unwrap();
            map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
                .unwrap();
        }
        // Esquinas altas → GetTileZ ≈ 12; plano → 0.
        map.set_height(high, 12).unwrap();
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            map.set_height(TileCoord::new(high.x + dx, high.y + dy), 12)
                .unwrap();
        }

        let stripe = u64::from(coord_to_linear_index(high, 8).unwrap() % MAP_TILE_LOOP_STRIDE);
        let dirty = apply_seasonal_snow_with_line(&mut map, Climate::SubArctic, stripe, 10);
        assert!(dirty.contains(&high));
        assert_eq!(
            clear_ground_type(map.get(high).unwrap().m5),
            CLEAR_GROUND_SNOW
        );
        assert_eq!(clear_density(map.get(high).unwrap().m5), 0);

        // Bajo la línea: nieve existente se descongela en visitas sucesivas.
        map.set_mapt_m5(low, 0, clear_ground_m5(CLEAR_GROUND_SNOW, 0))
            .unwrap();
        let low_stripe = u64::from(coord_to_linear_index(low, 8).unwrap() % MAP_TILE_LOOP_STRIDE);
        let dirty_thaw =
            apply_seasonal_snow_with_line(&mut map, Climate::SubArctic, low_stripe, 10);
        assert!(dirty_thaw.contains(&low));
        assert_eq!(
            clear_ground_type(map.get(low).unwrap().m5),
            CLEAR_GROUND_GRASS
        );
    }

    #[test]
    fn clear_alps_raises_snow_density_gradually() {
        let mut map = Map::new_flat(4, 4, 12);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_SNOW, 0))
            .unwrap();
        let stripe = u64::from(coord_to_linear_index(c, 4).unwrap() % MAP_TILE_LOOP_STRIDE);
        // z≈12, snow_line=10 → k=3 → req density 3.
        apply_seasonal_snow_with_line(&mut map, Climate::SubArctic, stripe, 10);
        assert_eq!(clear_density(map.get(c).unwrap().m5), 1);
        apply_seasonal_snow_with_line(
            &mut map,
            Climate::SubArctic,
            stripe + u64::from(MAP_TILE_LOOP_STRIDE),
            10,
        );
        assert_eq!(clear_density(map.get(c).unwrap().m5), 2);
    }

    #[test]
    fn clear_alps_stripe_reaches_interior_high_tiles_within_256_ticks() {
        // Altura en el campo `Tile::height` de cada celda; GetTileZ usa 4 esquinas,
        // así que el borde E/S del mapa ve z=0 (fuera de mapa) — solo interior cuenta.
        let mut map = Map::new_flat(32, 32, 12);
        for y in 0..32 {
            for x in 0..32 {
                let c = TileCoord::new(x, y);
                map.set_kind(c, TileKind::Grass).unwrap();
                map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
                    .unwrap();
            }
        }
        let mut snowed = 0_u32;
        for tick in 0..256_u64 {
            let dirty = apply_seasonal_snow_with_line(&mut map, Climate::SubArctic, tick, 10);
            snowed += u32::try_from(dirty.len()).unwrap_or(0);
        }
        assert!(
            snowed >= 31 * 31,
            "teselas interiores altas deben nevizarse en ≤256 ticks (got {snowed})"
        );
        assert_eq!(
            clear_ground_type(map.get(TileCoord::new(10, 10)).unwrap().m5),
            CLEAR_GROUND_SNOW
        );
    }
}
