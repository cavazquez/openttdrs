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
use crate::cargodist::parity::Randomizer;
use crate::company::OWNER_NONE_M1;
use crate::map::tile_loop::{MAP_TILE_LOOP_STRIDE, TileLoopState, collect_tile_loop_visits};
use crate::map::water_class::{
    WaterClass, is_coast_tile, set_water_class_m1, tile_has_water_class, water_class_from_m1,
};
use crate::map::water_flood::{DIR_OFFSETS, is_slope_one_corner_raised};
use crate::map::{Map, Tile, TileCoord, TileKind, coord_to_linear_index, tile_slope_and_z};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY,
    CLEAR_GROUND_ROUGH, CLEAR_GROUND_SNOW, Climate, DEF_SNOW_LINE_HEIGHT, clear_ground_m5,
    desert_patch,
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

/// Primer tick ≥ `after` en el que los campos avanzan en esa tesela (`cycle & 7 == 7`).
///
/// La hierba ya no depende de este ciclo global: lleva su contador en `m5`.
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

/// Visitas acumuladas del tile loop en `m5` bits 5–7 (`GetClearCounter`).
#[must_use]
pub const fn clear_counter(m5: u8) -> u8 {
    (m5 >> 5) & 0x07
}

/// Escribe el contador de visitas conservando suelo y densidad (`SetClearCounter`).
#[must_use]
pub const fn with_clear_counter(m5: u8, counter: u8) -> u8 {
    (m5 & !(0x07 << 5)) | ((counter & 0x07) << 5)
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

/// Avanza hierba / campos / árboles a partir de teselas ya visitadas por `RunTileLoop`.
pub fn process_tree_and_field_growth_from_visits(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    visits: &[(TileCoord, crate::map::Tile)],
) {
    let mut grass_updates = Vec::new();
    let mut field_updates = Vec::new();
    let mut forest_coords = Vec::new();
    let mut forest_ground_updates = Vec::new();

    for &(c, tile) in visits {
        let cycle = landscape_tile_cycle(c, tick);
        match tile.kind {
            TileKind::Forest => {
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
                    continue;
                }
                let stage = tree_or_field_stage(tile.m5);
                if stage < MAX_TREE_OR_FIELD_STAGE {
                    let new_m5 = with_tree_or_field_stage(tile.m5, stage + 1);
                    field_updates.push((c, tile.mapt, new_m5));
                }
            }
            TileKind::Grass => {
                // Releer: el desierto (P3.9) puede haber mutado la tesela antes.
                let tile = map.get(c).unwrap_or(tile);
                let ground = clear_ground_type(tile.m5);
                // `TileLoop_Clear` termina después de `TileLoopClearAlps`
                // cuando la capa de nieve está activa. No se debe avanzar la
                // densidad/counter subyacente en esa misma visita.
                if tile.m3 & 0x10 != 0 || ground == CLEAR_GROUND_SNOW {
                    continue;
                }
                let density = clear_density(tile.m5);
                if ground == CLEAR_GROUND_ROUGH && density == 0 {
                    grass_updates.push((c, tile.mapt, clear_ground_m5(CLEAR_GROUND_GRASS, 3)));
                    continue;
                }
                if ground != CLEAR_GROUND_GRASS || density >= 3 {
                    continue;
                }
                let counter = clear_counter(tile.m5);
                let new_m5 = if counter < 7 {
                    with_clear_counter(tile.m5, counter + 1)
                } else {
                    clear_ground_m5(CLEAR_GROUND_GRASS, density + 1)
                };
                grass_updates.push((c, tile.mapt, new_m5));
            }
            _ => {}
        }
    }

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

/// Avanza hierba / campos / árboles al ritmo de `RunTileLoop` + `TileLoop_Trees`.
pub fn step_tree_and_field_growth(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    loop_state: &mut TileLoopState,
) {
    let visits = collect_tile_loop_visits(map, tick, &mut loop_state.cur_tileloop_tile);
    process_tree_and_field_growth_from_visits(map, tick, world_seed, &visits);
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
    let Some(previous) = map.get(c) else {
        return;
    };
    // `MakeClear` resetea todos los planos auxiliares. Conservar `m1`/`m3`
    // de MP_TREES deja una WaterClass inválida y un tipo de árbol visibles en
    // el raw aunque la tesela ya sea `MP_CLEAR`.
    let clear = Tile {
        height: previous.height,
        kind: TileKind::Grass,
        // `SetTileType(MP_CLEAR)` sólo reemplaza el nibble alto: la zona
        // tropical de `MAPT` sigue disponible para los callbacks posteriores.
        mapt: previous.mapt & 0x0F,
        m5: clear_ground_m5(clear_ground, clear_density),
        m1: OWNER_NONE_M1,
        m6: 0,
        m8: 0,
        m3: 0,
        m2: 0,
        m2_hi: 0,
        m7: 0,
        m3hi: 0,
    };
    let _ = map.set_tile(c, clear);
}

/// Ejecuta el subconjunto de `TileLoop_Trees` que corre durante
/// `CreateRivers` en una partida temperate nueva.
///
/// La configuración inicial de OpenTTD usa `extra_tree_placement =
/// ETP_SPREAD_ALL`; por eso las ramas de crecimiento pueden propagarse y,
/// sobre todo, deben leer del `Random()` global. El tile loop normal conserva
/// por ahora su RNG determinista independiente, porque no comparte el stream
/// de construcción del mundo.
pub(crate) fn process_generation_tree_growth_at(
    map: &mut Map,
    climate: Climate,
    tick: u64,
    rng: &mut Randomizer,
    c: TileCoord,
) {
    debug_assert_eq!(climate, Climate::Temperate);
    let Some(tile) = map.get(c) else {
        return;
    };
    if tile.kind != TileKind::Forest {
        return;
    }

    let cycle = landscape_tile_cycle(c, tick);
    if cycle & 7 == 7 && tree_ground(tile.m2) == 0 {
        let density = tree_ground_density(tile.m2);
        if density < 3 {
            let _ = map.set_m2(c, make_tree_m2(0, density + 1));
        }
    }
    if cycle % TREE_UPDATE_FREQUENCY != TREE_UPDATE_FREQUENCY - 1 {
        return;
    }

    step_one_generation_forest_tile(map, rng, c);
}

fn step_one_generation_forest_tile(map: &mut Map, rng: &mut Randomizer, c: TileCoord) {
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
    let mapt = tile.mapt;

    match growth {
        TREE_GROWTH_GROWN => match rng.next() & 0x07 {
            0 => {
                let _ = map.set_mapt_m5(c, mapt, with_tree_or_field_stage(m5, growth + 1));
            }
            1 if count < 4 => {
                let m5 = with_tree_count(m5, count);
                let _ =
                    map.set_mapt_m5(c, mapt, with_tree_or_field_stage(m5, TREE_GROWTH_GROWING1));
            }
            1 | 2 => {
                let direction = (rng.next() & 0x07) as usize;
                try_spread_generation_neighbor(map, c, tree_type, direction);
            }
            _ => {}
        },
        TREE_GROWTH_DEAD => {
            if count > 1 {
                let m5 = with_tree_count(m5, count.saturating_sub(2));
                let _ = map.set_mapt_m5(c, mapt, with_tree_or_field_stage(m5, TREE_GROWTH_GROWN));
            } else {
                clear_dead_tree_tile(map, c, tile.m2);
            }
        }
        g if g < TREE_GROWTH_GROWN || (TREE_GROWTH_GROWN < g && g < TREE_GROWTH_DEAD) => {
            let _ = map.set_mapt_m5(c, mapt, with_tree_or_field_stage(m5, g + 1));
        }
        _ => {}
    }
}

fn try_spread_generation_neighbor(map: &mut Map, c: TileCoord, tree_type: u8, direction: usize) {
    let (dx, dy) = DIR_OFFSETS[direction];
    let neighbour = TileCoord::new(c.x + dx, c.y + dy);
    let Some(tile) = map.get(neighbour) else {
        return;
    };
    if !generation_tree_plantable(map, neighbour, tile) {
        return;
    }
    // OpenTTD evita replantar hierba recién despejada, pero deja que nieve y
    // costas usen el constructor normal de árbol.
    if tile.kind == TileKind::Grass
        && clear_ground_type(tile.m5) == CLEAR_GROUND_GRASS
        && tile.m3 & (1 << 4) == 0
        && clear_density(tile.m5) != 3
    {
        return;
    }
    plant_generation_tree(map, neighbour, tile, tree_type);
}

fn generation_tree_plantable(map: &Map, c: TileCoord, tile: Tile) -> bool {
    match tile.kind {
        TileKind::Water => tile_slope_and_z(map, c)
            .is_some_and(|(slope, _)| is_coast_tile(tile) && !is_slope_one_corner_raised(slope)),
        TileKind::Grass => !matches!(
            clear_ground_type(tile.m5),
            CLEAR_GROUND_FIELDS | CLEAR_GROUND_ROCKY | CLEAR_GROUND_DESERT
        ),
        _ => false,
    }
}

fn plant_generation_tree(map: &mut Map, c: TileCoord, previous: Tile, tree_type: u8) {
    let (ground, density) = match previous.kind {
        TileKind::Water => {
            crate::map::water_flood::clear_neighbour_non_flooding_states(map, c);
            (3, 3)
        }
        TileKind::Grass => {
            let clear_ground = clear_ground_type(previous.m5);
            let density = if clear_ground == CLEAR_GROUND_ROUGH {
                3
            } else {
                clear_density(previous.m5)
            };
            let ground = if previous.m3 & (1 << 4) != 0 {
                if clear_ground == CLEAR_GROUND_ROUGH {
                    4
                } else {
                    2
                }
            } else {
                match clear_ground {
                    CLEAR_GROUND_GRASS => 0,
                    CLEAR_GROUND_ROUGH => 1,
                    _ => 2,
                }
            };
            (ground, density)
        }
        _ => return,
    };
    let water_class = if ground == 3 {
        WaterClass::Sea
    } else {
        WaterClass::Invalid
    };
    let tree = Tile {
        height: previous.height,
        kind: TileKind::Forest,
        mapt: 0x40 | (previous.mapt & 0x0F),
        m5: TREE_GROWTH_GROWING1,
        m1: set_water_class_m1(OWNER_NONE_M1, water_class),
        m6: previous.m6 & 0x03,
        m8: 0,
        m3: tree_type,
        m2: make_tree_m2(ground, density),
        m2_hi: 0,
        m7: 0,
        m3hi: 0,
    };
    let _ = map.set_tile(c, tree);
}

/// Zona trópica desierto (`GetTropicZone == Desert`) para una coordenada
/// aislada. Se conserva como helper de compatibilidad para callers que no
/// tienen un mapa materializado; la generación de mundos usa los bits bajos
/// de `MAPT`, que son la fuente autoritativa de OpenTTD.
#[must_use]
pub fn is_tropic_desert_zone(c: TileCoord, climate: Climate, world_seed: u64) -> bool {
    climate.uses_desert_patches() && desert_patch(c.x, c.y, world_seed)
}

/// `NeighbourIsNormal`: algún vecino ortogonal no-desierto o mar.
#[must_use]
fn neighbour_is_normal(map: &Map, c: TileCoord) -> bool {
    for dir in 0..4u8 {
        let (dx, dy) = crate::map::diag_dir_offset(dir);
        let n = TileCoord::new(c.x + dx, c.y + dy);
        let Some(tile) = map.get(n) else {
            continue;
        };
        // `IsValidTile` excludes the freeform `MP_VOID` border. A void
        // neighbour therefore must not make a desert edge transition to
        // density one.
        if tile.kind == TileKind::Void {
            continue;
        }
        if tile.mapt & 0x03 != 1 {
            return true;
        }
        if tile_has_water_class(tile.kind) && water_class_from_m1(tile.m1) == WaterClass::Sea {
            return true;
        }
    }
    false
}

/// `TileLoopClearDesert`: ajusta densidad desierto según zona y vecinos.
pub fn tile_loop_clear_desert(
    map: &mut Map,
    c: TileCoord,
    climate: Climate,
    _world_seed: u64,
) -> bool {
    if !climate.uses_desert_patches() {
        return false;
    }
    let Some(tile) = map.get(c) else {
        return false;
    };
    if tile.kind != TileKind::Grass {
        return false;
    }
    let ground = clear_ground_type(tile.m5);
    let current = if ground == CLEAR_GROUND_DESERT {
        clear_density(tile.m5)
    } else {
        0
    };
    let expected = if tile.mapt & 0x03 == 1 {
        if neighbour_is_normal(map, c) { 1 } else { 3 }
    } else {
        0
    };
    if current == expected {
        return false;
    }
    let new_m5 = if expected == 0 {
        clear_ground_m5(CLEAR_GROUND_GRASS, 3)
    } else {
        clear_ground_m5(CLEAR_GROUND_DESERT, expected)
    };
    if new_m5 == tile.m5 {
        return false;
    }
    let _ = map.set_mapt_m5(c, tile.mapt, new_m5);
    true
}

/// Aplica `TileLoopClearDesert` sobre visitas del tile loop.
pub fn apply_desert_transition_from_visits(
    map: &mut Map,
    climate: Climate,
    world_seed: u64,
    visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    if !climate.uses_desert_patches() {
        return Vec::new();
    }
    let mut dirty = Vec::new();
    for &(c, _) in visits {
        if tile_loop_clear_desert(map, c, climate, world_seed) {
            dirty.push(c);
        }
    }
    dirty
}

/// Nieve ártica al estilo OpenTTD `TileLoopClearAlps`: altura frente a la
/// línea de nieve, en la franja del tile loop.
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
    loop_state: &mut TileLoopState,
) -> Vec<TileCoord> {
    let _ = world_seed;
    apply_seasonal_snow_with_line(map, climate, tick, DEF_SNOW_LINE_HEIGHT, loop_state)
}

/// Como [`apply_seasonal_snow`] con línea de nieve explícita (tests / settings futuros).
pub fn apply_seasonal_snow_with_line(
    map: &mut Map,
    climate: Climate,
    tick: u64,
    snow_line_height: u8,
    loop_state: &mut TileLoopState,
) -> Vec<TileCoord> {
    let visits = collect_tile_loop_visits(map, tick, &mut loop_state.cur_tileloop_tile);
    apply_seasonal_snow_from_visits(map, climate, snow_line_height, &visits)
}

/// Aplica nieve estacional sobre teselas ya visitadas por `RunTileLoop`.
pub fn apply_seasonal_snow_from_visits(
    map: &mut Map,
    climate: Climate,
    snow_line_height: u8,
    visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    if !climate.uses_snow_ground() {
        return Vec::new();
    }
    let mut dirty = Vec::new();
    for &(c, _) in visits {
        if tile_loop_clear_alps_at(map, c, snow_line_height) {
            dirty.push(c);
        }
    }
    dirty
}

/// Ejecuta `TileLoopClearAlps` sobre una sola tesela viva.
///
/// OpenTTD almacena la presencia de nieve en `MAP3` bit 4; los bits de suelo
/// de `MAP5` siguen describiendo el sustrato que queda debajo. El valor
/// `CLEAR_GROUND_SNOW` se acepta sólo como formato legado de mapas JSON y se
/// normaliza a la representación canónica en la primera visita.
pub(crate) fn tile_loop_clear_alps_at(map: &mut Map, c: TileCoord, snow_line_height: u8) -> bool {
    let Some(tile) = map.get(c) else {
        return false;
    };
    if tile.kind != TileKind::Grass {
        return false;
    }
    let Some((_, z)) = tile_slope_and_z(map, c) else {
        return false;
    };

    let raw_ground = clear_ground_type(tile.m5);
    let legacy_snow = raw_ground == CLEAR_GROUND_SNOW && tile.m3 & 0x10 == 0;
    let is_snow = tile.m3 & 0x10 != 0 || legacy_snow;
    // `CLEAR_SNOW` no es un tipo persistido: una tesela vieja que lo use en
    // MAP5 representa césped debajo de la capa de nieve.
    let underlying_ground = if legacy_snow {
        CLEAR_GROUND_GRASS
    } else {
        raw_ground
    };
    let density = clear_density(tile.m5);
    let k = i32::from(z) - i32::from(snow_line_height) + 1;

    let (new_m3, new_m5) = if is_snow {
        let required = if k < 0 {
            0
        } else {
            u8::try_from(k.clamp(0, 3)).unwrap_or(3)
        };
        if density == required {
            if k >= 0 {
                return false;
            }
            // `ClearSnow`: se conserva el suelo subyacente y se restaura la
            // densidad plena de césped/rough.
            (tile.m3 & !0x10, clear_ground_m5(underlying_ground, 3))
        } else {
            let next_density = if density < required {
                density.saturating_add(1)
            } else {
                density.saturating_sub(1)
            };
            (
                tile.m3 | 0x10,
                clear_ground_m5(underlying_ground, next_density),
            )
        }
    } else if k >= 0 {
        // `MakeSnow(tile, 0)`: los campos se convierten en césped, los demás
        // sustratos conservan su tipo y empiezan con densidad cero.
        let ground = if raw_ground == CLEAR_GROUND_FIELDS {
            CLEAR_GROUND_GRASS
        } else {
            raw_ground
        };
        (tile.m3 | 0x10, clear_ground_m5(ground, 0))
    } else {
        return false;
    };

    if new_m3 == tile.m3 && new_m5 == tile.m5 {
        return false;
    }
    let mut updated = tile;
    updated.m3 = new_m3;
    updated.m5 = new_m5;
    map.set_tile(c, updated).is_ok()
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

/// Hook combinado para `sim_step` (usa `runtime.tile_loop_visited` del tick).
pub fn tick_tree_tile_loop(state: &mut GameState) {
    let tick = state.tick.get();
    let visits = state.runtime.tile_loop_visited.clone();
    // OpenTTD `TileLoop_Clear`: desierto/alps antes del crecimiento de hierba.
    let desert_dirty = apply_desert_transition_from_visits(
        &mut state.map,
        state.climate,
        state.world_seed,
        &visits,
    );
    process_tree_and_field_growth_from_visits(&mut state.map, tick, state.world_seed, &visits);
    let snow_dirty = apply_seasonal_snow_from_visits(
        &mut state.map,
        state.climate,
        DEF_SNOW_LINE_HEIGHT,
        &visits,
    );
    state.runtime.landscape_tile_dirty.extend(desert_dirty);
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

    fn grow_trees_at(map: &mut Map, tick: u64, seed: u64, loop_state: &mut TileLoopState) {
        step_tree_and_field_growth(map, tick, seed, loop_state);
    }

    fn snow_at(
        map: &mut Map,
        climate: Climate,
        tick: u64,
        snow_line: u8,
        loop_state: &mut TileLoopState,
    ) -> Vec<TileCoord> {
        apply_seasonal_snow_with_line(map, climate, tick, snow_line, loop_state)
    }

    #[test]
    fn tree_grows_on_open_ttd_update_cycle() {
        let mut state = GameState::new(64, 64);
        let c = TileCoord::new(1, 1);
        apply_command(&mut state, &Command::PlantTree(c)).unwrap();
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 0);
        let mut loop_state = TileLoopState::default();
        let mut grew = false;
        for tick in 0..500_000u64 {
            grow_trees_at(&mut state.map, tick, 0, &mut loop_state);
            if tree_or_field_stage(state.map.get(c).unwrap().m5) > 0 {
                grew = true;
                break;
            }
        }
        assert!(grew, "el árbol debe avanzar tras visitas LFSR suficientes");
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
        let mut loop_state = TileLoopState::default();
        let mut found_stable = false;
        let mut after = 0u64;
        for _ in 0..64 {
            let tick = next_tree_update_tick(c, map_w(&map), after);
            after = tick;
            let before = map.get(c).unwrap().m5;
            grow_trees_at(&mut map, tick, 0xDEAD_BEEF, &mut loop_state);
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
        let mut loop_state = TileLoopState::default();
        let mut died = false;
        let mut after = 0u64;
        for _ in 0..256 {
            let tick = next_tree_update_tick(c, map_w(&map), after);
            after = tick;
            grow_trees_at(&mut map, tick, 42, &mut loop_state);
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
        let mut loop_state = TileLoopState::default();
        grow_trees_at(&mut map, tick, 0, &mut loop_state);
        assert_eq!(map.get_kind(c), Some(TileKind::Grass));
    }

    #[test]
    fn clearing_dead_tree_resets_make_clear_raw_planes() {
        let mut map = Map::new_flat(2, 2, 7);
        let c = TileCoord::new(0, 0);
        let dirty_tree = Tile {
            height: 7,
            kind: TileKind::Forest,
            mapt: 0x4F,
            m5: TREE_GROWTH_DEAD,
            m1: 0x70,
            m6: 0xFF,
            m8: 0xFFFF,
            m3: 0xAA,
            m2: make_tree_m2(0, 2),
            m2_hi: 0xFF,
            m7: 0xFF,
            m3hi: 0xFF,
        };
        map.set_tile(c, dirty_tree).unwrap();

        clear_dead_tree_tile(&mut map, c, dirty_tree.m2);

        assert_eq!(
            map.get(c),
            Some(Tile {
                height: 7,
                kind: TileKind::Grass,
                mapt: 0x0F,
                m5: clear_ground_m5(CLEAR_GROUND_GRASS, 2),
                m1: OWNER_NONE_M1,
                m6: 0,
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            })
        );
    }

    #[test]
    fn generation_tree_loop_consumes_shared_rng_for_grown_tree() {
        let mut map = Map::new_flat(32, 32, 0);
        // 11 * 13 + 9 * 16 = 287, de modo que el árbol entra en el ciclo 15
        // de `TileLoop_Trees` con `tick == 0`.
        let c = TileCoord::new(13, 16);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        // `state[0] = 8` produce `Random() == 0`: la primera rama inicia la
        // destrucción y consume exactamente una palabra del stream global.
        let mut rng = Randomizer { state: [8, 1] };
        let mut expected_rng = rng;
        assert_eq!(expected_rng.next() & 7, 0);

        process_generation_tree_growth_at(&mut map, Climate::Temperate, 0, &mut rng, c);

        assert_eq!(
            tree_or_field_stage(map.get(c).unwrap().m5),
            TREE_GROWTH_GROWN + 1
        );
        assert_eq!(rng, expected_rng, "debe avanzar el RNG compartido una vez");
    }

    #[test]
    fn generation_tree_spread_consumes_direction_from_shared_rng() {
        let mut map = Map::new_flat(32, 32, 0);
        let c = TileCoord::new(13, 16);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        let mut rng = Randomizer { state: [24, 1] };
        let mut expected_rng = rng;
        assert_eq!(expected_rng.next() & 7, 2);
        let direction = (expected_rng.next() & 7) as usize;
        let (dx, dy) = DIR_OFFSETS[direction];
        let neighbour = TileCoord::new(c.x + dx, c.y + dy);
        map.set_mapt_m5(neighbour, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();

        process_generation_tree_growth_at(&mut map, Climate::Temperate, 0, &mut rng, c);

        assert_eq!(rng, expected_rng, "spread debe consumir dos Random()");
        let planted = map.get(neighbour).unwrap();
        assert_eq!(planted.kind, TileKind::Forest);
        assert_eq!(planted.m3, 0);
        assert_eq!(planted.m5, TREE_GROWTH_GROWING1);
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
        let mut loop_state = TileLoopState::default();
        grow_trees_at(&mut state.map, tick, 0, &mut loop_state);
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
        let mut loop_state = TileLoopState::default();
        let mut after = 0u64;
        for _ in 0..8 {
            let tick = next_clear_update_tick(c, map_w(&map), after);
            after = tick;
            grow_trees_at(&mut map, tick, 0, &mut loop_state);
        }
        assert_eq!(
            map.get(c).unwrap().m5,
            0x03,
            "hierba completa no debe convertirse en Rough (0x04)"
        );
    }

    /// `TileLoop_Clear` gasta ocho visitas subiendo `GetClearCounter` y solo en la novena
    /// sube la densidad, dejando el contador otra vez a cero.
    #[test]
    fn grass_density_needs_eight_visits_per_step() {
        let mut map = Map::new_flat(1, 1, 0);
        let c = TileCoord::new(0, 0);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 0))
            .unwrap();
        let stride = u64::from(MAP_TILE_LOOP_STRIDE);
        let mut loop_state = TileLoopState::default();
        let mut tick = next_clear_update_tick(c, map_w(&map), 0);

        for expected in 1..=7u8 {
            grow_trees_at(&mut map, tick, 0, &mut loop_state);
            let m5 = map.get(c).unwrap().m5;
            assert_eq!(clear_counter(m5), expected);
            assert_eq!(clear_density(m5), 0, "la densidad espera al contador");
            tick += stride;
        }

        grow_trees_at(&mut map, tick, 0, &mut loop_state);
        let m5 = map.get(c).unwrap().m5;
        assert_eq!(clear_density(m5), 1);
        assert_eq!(clear_counter(m5), 0);
    }

    /// Dos parcelas con contador distinto maduran en visitas distintas: la franja
    /// entera ya no cambia de golpe.
    #[test]
    fn grass_tiles_with_different_counters_ripen_apart() {
        let mut map = Map::new_flat(64, 64, 0);
        let ready = TileCoord::new(10, 10);
        let fresh = TileCoord::new(50, 50);
        for (c, counter) in [(ready, 7u8), (fresh, 0u8)] {
            map.set_kind(c, TileKind::Grass).unwrap();
            map.set_mapt_m5(
                c,
                0,
                with_clear_counter(clear_ground_m5(CLEAR_GROUND_GRASS, 1), counter),
            )
            .unwrap();
        }
        let stride = u64::from(MAP_TILE_LOOP_STRIDE);
        let mut loop_state = TileLoopState::default();
        for tick in 1..=stride {
            grow_trees_at(&mut map, tick, 0, &mut loop_state);
        }
        assert!(
            clear_density(map.get(ready).unwrap().m5) > clear_density(map.get(fresh).unwrap().m5),
            "parcelas con contador distinto deben madurar a ritmos distintos"
        );
    }

    #[test]
    fn invalid_rough_density_zero_repairs_to_full_grass() {
        let mut map = Map::new_flat(1, 1, 0);
        let c = TileCoord::new(0, 0);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, 0x04).unwrap(); // Rough + density 0 (inválido)
        let tick = next_clear_update_tick(c, map_w(&map), 0);
        let mut loop_state = TileLoopState::default();
        grow_trees_at(&mut map, tick, 0, &mut loop_state);
        assert_eq!(map.get(c).unwrap().m5, 0x03);
    }

    #[test]
    fn grown_can_spread_to_neighbor_grass() {
        let mut map = Map::new_flat(64, 64, 0);
        let c = TileCoord::new(31, 31);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        for y in 30..=32 {
            for x in 30..=32 {
                let t = TileCoord::new(x, y);
                if t == c {
                    continue;
                }
                map.set_kind(t, TileKind::Grass).unwrap();
                map.set_mapt_m5(t, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
                    .unwrap();
            }
        }
        let mut loop_state = TileLoopState::default();
        let mut spread = false;
        for tick in 0..500_000u64 {
            grow_trees_at(&mut map, tick, 7, &mut loop_state);
            let forests = (30..=32)
                .flat_map(|y| (30..=32).map(move |x| TileCoord::new(x, y)))
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

    /// Paridad con OpenTTD #15133: la nieve parcial conserva una densidad de
    /// nieve, no de césped; no debe impedir que un árbol adulto se propague.
    #[test]
    fn grown_can_spread_to_partially_snowy_neighbor() {
        let mut map = Map::new_flat(64, 64, 0);
        let c = TileCoord::new(31, 31);
        force_forest(&mut map, c, with_tree_or_field_stage(0, TREE_GROWTH_GROWN));
        for y in 30..=32 {
            for x in 30..=32 {
                let t = TileCoord::new(x, y);
                if t == c {
                    continue;
                }
                map.set_kind(t, TileKind::Grass).unwrap();
                map.set_mapt_m5(t, 0, clear_ground_m5(CLEAR_GROUND_SNOW, 1))
                    .unwrap();
            }
        }

        let mut loop_state = TileLoopState::default();
        let mut spread = false;
        for tick in 0..500_000u64 {
            grow_trees_at(&mut map, tick, 7, &mut loop_state);
            let forests = (30..=32)
                .flat_map(|y| (30..=32).map(move |x| TileCoord::new(x, y)))
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
        assert!(spread, "debe poder propagarse a nieve parcial");
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

        let high_tile = map.get(high).unwrap();
        let dirty =
            apply_seasonal_snow_from_visits(&mut map, Climate::SubArctic, 10, &[(high, high_tile)]);
        assert!(dirty.contains(&high));
        assert_eq!(
            clear_ground_type(map.get(high).unwrap().m5),
            CLEAR_GROUND_GRASS
        );
        assert_ne!(map.get(high).unwrap().m3 & 0x10, 0);
        assert_eq!(clear_density(map.get(high).unwrap().m5), 0);

        map.set_mapt_m5(low, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 0))
            .unwrap();
        map.set_m3(low, 0x10).unwrap();
        let low_tile = map.get(low).unwrap();
        let dirty_thaw =
            apply_seasonal_snow_from_visits(&mut map, Climate::SubArctic, 10, &[(low, low_tile)]);
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
        map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 0))
            .unwrap();
        map.set_m3(c, 0x10).unwrap();
        let tile = map.get(c).unwrap();
        apply_seasonal_snow_from_visits(&mut map, Climate::SubArctic, 10, &[(c, tile)]);
        assert_eq!(clear_density(map.get(c).unwrap().m5), 1);
        let tile = map.get(c).unwrap();
        apply_seasonal_snow_from_visits(&mut map, Climate::SubArctic, 10, &[(c, tile)]);
        assert_eq!(clear_density(map.get(c).unwrap().m5), 2);
    }

    fn find_desert_and_normal_coords(seed: u64) -> (TileCoord, TileCoord) {
        let mut desert = None;
        let mut normal = None;
        for y in 2..30 {
            for x in 2..30 {
                let c = TileCoord::new(x, y);
                if desert_patch(x, y, seed) {
                    desert.get_or_insert(c);
                } else {
                    normal.get_or_insert(c);
                }
                if let (Some(d), Some(n)) = (desert, normal) {
                    return (d, n);
                }
            }
        }
        panic!("no se encontraron coords desierto/normal para seed {seed}");
    }

    #[test]
    fn clear_desert_interior_reaches_density_three() {
        // Esquina (0,0): vecinos fuera de mapa se ignoran → NeighbourIsNormal=false
        // si los diagonales válidos también son zona desierto. Seed 0: (0,0) es desierto.
        let seed = 0u64;
        assert!(desert_patch(0, 0, seed));
        let mut map = Map::new_flat(1, 1, 0);
        let c = TileCoord::new(0, 0);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 1, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        assert!(tile_loop_clear_desert(
            &mut map,
            c,
            Climate::SubTropical,
            seed
        ));
        assert_eq!(
            clear_ground_type(map.get(c).unwrap().m5),
            CLEAR_GROUND_DESERT
        );
        assert_eq!(clear_density(map.get(c).unwrap().m5), 3);
    }

    #[test]
    fn clear_desert_transition_replaces_rocky_ground() {
        let mut map = Map::new_flat(64, 64, 0);
        let c = TileCoord::new(32, 32);
        for y in 31..=33 {
            for x in 31..=33 {
                map.set_mapt_m5(TileCoord::new(x, y), 0x01, 0)
                    .expect("desert zone");
            }
        }
        map.set_mapt_m5(c, 0x01, clear_ground_m5(CLEAR_GROUND_ROCKY, 3))
            .expect("rocky ground");

        assert!(tile_loop_clear_desert(&mut map, c, Climate::SubTropical, 0));
        let tile = map.get(c).expect("transitioned tile");
        assert_eq!(clear_ground_type(tile.m5), CLEAR_GROUND_DESERT);
        assert_eq!(clear_density(tile.m5), 3);
    }

    #[test]
    fn clear_desert_edge_near_normal_is_density_one() {
        let seed = 7u64;
        let mut edge = None;
        'outer: for y in 3..28 {
            for x in 3..28 {
                if !desert_patch(x, y, seed) {
                    continue;
                }
                let has_normal = [(-1, 0), (0, 1), (1, 0), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| !desert_patch(x + dx, y + dy, seed));
                if has_normal {
                    edge = Some(TileCoord::new(x, y));
                    break 'outer;
                }
            }
        }
        let c = edge.expect("debe existir borde desierto/normal");
        let mut map = Map::new_flat(64, 64, 0);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 1, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        assert!(tile_loop_clear_desert(
            &mut map,
            c,
            Climate::SubTropical,
            seed
        ));
        assert_eq!(
            clear_ground_type(map.get(c).unwrap().m5),
            CLEAR_GROUND_DESERT
        );
        assert_eq!(clear_density(map.get(c).unwrap().m5), 1);
    }

    #[test]
    fn clear_desert_outside_zone_reverts_to_grass() {
        let seed = 3u64;
        let (_, normal) = find_desert_and_normal_coords(seed);
        let mut map = Map::new_flat(64, 64, 0);
        map.set_kind(normal, TileKind::Grass).unwrap();
        map.set_mapt_m5(normal, 0, clear_ground_m5(CLEAR_GROUND_DESERT, 3))
            .unwrap();
        assert!(tile_loop_clear_desert(
            &mut map,
            normal,
            Climate::SubTropical,
            seed
        ));
        assert_eq!(
            clear_ground_type(map.get(normal).unwrap().m5),
            CLEAR_GROUND_GRASS
        );
        assert_eq!(clear_density(map.get(normal).unwrap().m5), 3);
    }

    #[test]
    fn clear_desert_does_not_run_on_arctic() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        assert!(!tile_loop_clear_desert(&mut map, c, Climate::SubArctic, 1));
    }

    #[test]
    fn clear_alps_stripe_reaches_interior_high_tiles_within_256_ticks() {
        // Altura en el campo `Tile::height` de cada celda; GetTileZ usa 4 esquinas,
        // así que el borde E/S del mapa ve z=0 (fuera de mapa) — solo interior cuenta.
        let mut map = Map::new_flat(64, 64, 12);
        for y in 0..64 {
            for x in 0..64 {
                let c = TileCoord::new(x, y);
                map.set_kind(c, TileKind::Grass).unwrap();
                map.set_mapt_m5(c, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
                    .unwrap();
            }
        }
        let mut loop_state = TileLoopState::default();
        let mut snowed = 0_u32;
        for tick in 0..256_u64 {
            let dirty = snow_at(&mut map, Climate::SubArctic, tick, 10, &mut loop_state);
            snowed += u32::try_from(dirty.len()).unwrap_or(0);
        }
        assert!(
            snowed >= 62 * 62,
            "teselas interiores altas deben nevizarse en ≤256 ticks (got {snowed})"
        );
        assert_eq!(
            clear_ground_type(map.get(TileCoord::new(10, 10)).unwrap().m5),
            CLEAR_GROUND_GRASS
        );
        assert_ne!(map.get(TileCoord::new(10, 10)).unwrap().m3 & 0x10, 0);
    }
}
