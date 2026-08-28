//! Colocación de árboles de `tree_cmd.cpp` durante la generación de un mundo.
//!
//! El árbol no es sólo una etiqueta visual: `OpenTTD` persiste tipo, cantidad,
//! crecimiento, suelo y densidad en `m1..m5`. Mantener ese contrato permite que
//! un mapa procedural se compare con el raw de `OpenTTD` aunque la topografía
//! todavía esté en una etapa distinta.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::expect_used,
    clippy::many_single_char_names
)]

use std::f32::consts::TAU;

use crate::cargodist::parity::Randomizer;
use crate::company::OWNER_NONE_M1;
use crate::map::tree_tile_loop::{clear_density, clear_ground_type};
use crate::map::{
    Map, Tile, TileCoord, TileKind, WaterClass, set_water_class_m1, tile_slope_and_z,
};

use super::Climate;
use super::PreserveRect;
use super::config::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW,
};
use super::population::scale_by_size;

const DEFAULT_TREE_STEPS: u32 = 1_000;
const GROVE_RADIUS: i32 = 16;
const GROVE_SEGMENTS: usize = 16;
/// `WaterTileType::Coast` en los bits altos de `m5`.
const WATER_TYPE_COAST: u8 = 1;
/// `TreeGround` de `tree_map.h`.
const TREE_GROUND_GRASS: u8 = 0;
const TREE_GROUND_ROUGH: u8 = 1;
const TREE_GROUND_SNOW_DESERT: u8 = 2;
const TREE_GROUND_SHORE: u8 = 3;

#[derive(Clone, Copy, Debug, Default)]
struct Point {
    x: i32,
    y: i32,
}

/// Ejecuta la variante `TP_IMPROVED`, que es el valor predeterminado de
/// `OpenTTD` (`game_creation.tree_placer = 2`).
/// Genera árboles con el algoritmo mejorado predeterminado de `OpenTTD`.
///
/// La función es pública para que las herramientas de comparación puedan
/// aislar la etapa `GenerateTrees` sin ejecutar pueblos o industrias.
pub fn generate_trees(map: &mut Map, climate: Climate, seed: u64, preserve: &[PreserveRect]) {
    let mut rng = Randomizer::new(seed as u32);
    generate_trees_with_rng(map, climate, &mut rng, preserve);
}

/// Variante de [`generate_trees`] que continúa el stream global de generación
/// de `OpenTTD` después de terreno, suelo, pueblos e industrias.
pub fn generate_trees_with_rng(
    map: &mut Map,
    climate: Climate,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
) {
    let (map_w, map_h) = map.dimensions();
    if map_w < 4 || map_h < 4 {
        return;
    }
    let attempts = scale_by_size(DEFAULT_TREE_STEPS, map_w, map_h);
    let groups = if matches!(climate, Climate::Toyland) {
        0
    } else {
        scale_by_size(rng.next() & 0x1F | 0x19, map_w, map_h)
    };

    for _ in 0..groups {
        let center = random_tile(rng.next(), map_w, map_h);
        let grove = random_grove(rng);
        for _ in 0..DEFAULT_TREE_STEPS {
            let r = rng.next();
            let x = ((r & 0x1F) as i32) - GROVE_RADIUS;
            let y = (((r >> 8) & 0x1F) as i32) - GROVE_RADIUS;
            let Some(tile) = tile_add_wrap(center, x, y, map_w, map_h) else {
                continue;
            };
            if !is_plantable(map, tile, preserve, true) || !point_in_grove(x, y, &grove) {
                continue;
            }
            let _ = place_tree(map, tile, r, climate);
        }
    }

    // `GenerateTrees` runs two passes on temperate maps in improved mode and
    // four on arctic maps. The extra height-dependent spreading from C++ is
    // intentionally omitted until the heightmap and snowline generators share
    // the same random stream; the base placement and tile contract are exact.
    let passes = if matches!(climate, Climate::SubArctic) {
        4
    } else {
        2
    };
    for _ in 0..passes {
        for _ in 0..attempts {
            let r = rng.next();
            let tile = random_tile(r, map_w, map_h);
            if is_plantable(map, tile, preserve, true) {
                let height = tile_slope_and_z(map, tile).map_or(0, |(_, z)| z);
                if place_tree(map, tile, r, climate) {
                    // `PlaceTreesRandomly` in improved mode reinforces a
                    // successful placement with `GetTileZ(tile) * 2`
                    // same-height attempts. This is the part that turns the
                    // otherwise sparse random pass into visible groves on
                    // hilly maps.
                    for _ in 0..u32::from(height).saturating_mul(2) {
                        place_tree_at_same_height(map, tile, height, rng, climate, preserve);
                    }
                }
            }
        }
    }
}

fn random_tile(seed: u32, map_w: u32, map_h: u32) -> TileCoord {
    let tile_count = map_w.saturating_mul(map_h).max(1);
    let index = if map_w.is_power_of_two() && map_h.is_power_of_two() {
        seed & tile_count.saturating_sub(1)
    } else {
        seed % tile_count
    };
    TileCoord::new(
        i32::try_from(index % map_w.max(1)).unwrap_or(0),
        i32::try_from(index / map_w.max(1)).unwrap_or(0),
    )
}

/// `TileAddWrap` no envuelve en realidad cuando `freeform_edges` está activo:
/// descarta los bordes y cualquier desplazamiento fuera del mapa.
fn tile_add_wrap(center: TileCoord, dx: i32, dy: i32, map_w: u32, map_h: u32) -> Option<TileCoord> {
    let x = center.x.saturating_add(dx);
    let y = center.y.saturating_add(dy);
    let max_x = i32::try_from(map_w).ok()?.saturating_sub(1);
    let max_y = i32::try_from(map_h).ok()?.saturating_sub(1);
    if x <= 0 || y <= 0 || x >= max_x || y >= max_y {
        None
    } else {
        Some(TileCoord::new(x, y))
    }
}

/// Equivalente a `CanPlantTreesOnTile`. Las teselas de costa se pueden
/// convertir en árboles de orilla, mientras que campos y rocas nunca son
/// sustrato válido. `allow_desert` sólo es falso en el pase tropical extra.
fn is_plantable(map: &Map, c: TileCoord, preserve: &[PreserveRect], allow_desert: bool) -> bool {
    if preserve.iter().any(|rect| rect.contains(c.x, c.y)) {
        return false;
    }
    let Some(tile) = map.get(c) else {
        return false;
    };
    match tile.kind {
        TileKind::Water => {
            let is_coast = ((tile.m5 >> 4) & 0x0F) == WATER_TYPE_COAST;
            let slope = tile_slope_and_z(map, c).map_or(0, |(slope, _)| slope);
            is_coast && !is_slope_with_one_corner_raised(slope)
        }
        TileKind::Grass => {
            let ground = clear_ground_type(tile.m5);
            !matches!(ground, CLEAR_GROUND_FIELDS | CLEAR_GROUND_ROCKY)
                && (allow_desert || ground != CLEAR_GROUND_DESERT)
        }
        _ => false,
    }
}

/// `IsSlopeWithOneCornerRaised` ignora el bit `SLOPE_STEEP`, igual que el
/// predicado de `OpenTTD` para impedir árboles de orilla sobre un talud simple.
#[must_use]
const fn is_slope_with_one_corner_raised(slope: u8) -> bool {
    matches!(slope & 0x0F, 1 | 2 | 4 | 8)
}

fn tree_range(climate: Climate) -> (u8, u8) {
    match climate {
        Climate::Temperate => (0, 12),
        Climate::SubArctic => (12, 8),
        // Tropic zones are not represented in the procedural map yet. Use
        // the normal subtropical range until the zone generator is ported.
        Climate::SubTropical => (28, 4),
        Climate::Toyland => (32, 9),
    }
}

fn place_tree(map: &mut Map, c: TileCoord, random: u32, climate: Climate) -> bool {
    let Some(previous) = map.get(c) else {
        return false;
    };
    let (base, count) = tree_range(climate);
    let tree_type = base.saturating_add((((random >> 24) & 0xFF) * u32::from(count) / 256) as u8);
    let count_minus_one = ((random >> 22) & 0x03) as u8;
    let growth = (((random >> 16) & 0x07) as u8).min(6);

    let (mut ground, mut density, preserve_ground) = match previous.kind {
        TileKind::Water => {
            // `PlantTreesOnTile` transforma sólo costas válidas en orilla y
            // borra el estado non-flooding de sus ocho vecinas.
            clear_neighbour_non_flooding_states(map, c);
            (TREE_GROUND_SHORE, 3, true)
        }
        TileKind::Grass => {
            let original_ground = clear_ground_type(previous.m5);
            let density = if original_ground == CLEAR_GROUND_ROUGH {
                3
            } else {
                clear_density(previous.m5)
            };
            let ground = match original_ground {
                CLEAR_GROUND_ROUGH => TREE_GROUND_ROUGH,
                // El mapa procedural representa la cobertura de nieve como
                // `CLEAR_GROUND_SNOW`; ambas variantes conservan densidad en
                // `PlaceTree` en lugar de rerandomizarla.
                CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => TREE_GROUND_SNOW_DESERT,
                _ => TREE_GROUND_GRASS,
            };
            (ground, density, matches!(ground, TREE_GROUND_SNOW_DESERT))
        }
        _ => return false,
    };
    // `PlaceTree(..., false)` rerandomiza suelo normal, pero conserva nieve,
    // desierto y orilla. Esta ruta de generación nunca solicita keep_density.
    if !preserve_ground {
        ground = ((random >> 28) & 1) as u8;
        density = 3;
    }

    let water_class = if ground == TREE_GROUND_SHORE {
        WaterClass::Sea
    } else {
        WaterClass::Invalid
    };

    let mut tile = Tile {
        height: previous.height,
        kind: TileKind::Forest,
        mapt: 0x40,
        m5: (count_minus_one << 6) | growth,
        m1: set_water_class_m1(OWNER_NONE_M1, water_class),
        m6: 0,
        m8: 0,
        m3: tree_type,
        m2: (ground << 6) | (density << 4),
        m2_hi: 0,
        m7: 0,
        m3hi: 0,
    };
    // Keep the assignment explicit: this is the byte-for-byte `MakeTree`
    // contract, including zeroed auxiliary bytes.
    tile.m1 = set_water_class_m1(OWNER_NONE_M1, water_class);
    map.set_tile(c, tile).is_ok()
}

/// `ClearNeighbourNonFloodingStates` de `water_cmd.cpp`: una costa convertida
/// a árbol deja de actuar como soporte de estados non-flooding en las ocho
/// teselas de agua adyacentes.
fn clear_neighbour_non_flooding_states(map: &mut Map, c: TileCoord) {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let neighbour = TileCoord::new(c.x.saturating_add(dx), c.y.saturating_add(dy));
            let Some(mut tile) = map.get(neighbour) else {
                continue;
            };
            if tile.kind == TileKind::Water {
                tile.m3 &= !1;
                let _ = map.set_tile(neighbour, tile);
            }
        }
    }
}

fn place_tree_at_same_height(
    map: &mut Map,
    center: TileCoord,
    height: u8,
    rng: &mut Randomizer,
    climate: Climate,
    preserve: &[PreserveRect],
) {
    for _ in 0..DEFAULT_TREE_STEPS {
        let r = rng.next();
        let x = ((r & 0x1F) as i32) - GROVE_RADIUS;
        let y = (((r >> 8) & 0x1F) as i32) - GROVE_RADIUS;
        if x.abs().saturating_add(y.abs()) > GROVE_RADIUS {
            continue;
        }
        let Some(tile) = tile_add_wrap(center, x, y, map.dimensions().0, map.dimensions().1) else {
            continue;
        };
        if !is_plantable(map, tile, preserve, true)
            || tile_slope_and_z(map, tile).is_none_or(|(_, z)| u8::abs_diff(z, height) > 2)
        {
            continue;
        }
        let _ = place_tree(map, tile, r, climate);
        break;
    }
}

fn random_grove(rng: &mut Randomizer) -> [Point; GROVE_SEGMENTS] {
    const PHASE_DIVISOR: f32 = (i32::MAX as f32) / TAU;
    let harmonics = [
        (GROVE_RADIUS / 2, rng.next() as f32 / PHASE_DIVISOR, 1),
        (GROVE_RADIUS / 4, rng.next() as f32 / PHASE_DIVISOR, 2),
        (GROVE_RADIUS / 8, rng.next() as f32 / PHASE_DIVISOR, 3),
        (GROVE_RADIUS / 16, rng.next() as f32 / PHASE_DIVISOR, 4),
    ];
    let mut grove = [Point::default(); GROVE_SEGMENTS];
    for (index, point) in grove.iter_mut().enumerate() {
        let theta = TAU * index as f32 / GROVE_SEGMENTS as f32;
        let deviation = harmonics
            .iter()
            .fold(0.0, |sum, (amplitude, phase, frequency)| {
                sum + ((theta + phase) * *frequency as f32).sin() * *amplitude as f32
            });
        let radius = GROVE_RADIUS as f32 / 2.0 + deviation / 2.0;
        point.x = (theta.cos() * radius) as i32;
        point.y = (theta.sin() * radius) as i32;
    }
    grove
}

fn point_in_grove(x: i32, y: i32, shape: &[Point; GROVE_SEGMENTS]) -> bool {
    shape.iter().enumerate().any(|(index, &v1)| {
        let v2 = shape[(index + 1) % shape.len()];
        point_in_triangle(x, y, v1, v2, Point::default())
    })
}

fn point_in_triangle(x: i32, y: i32, v1: Point, v2: Point, v3: Point) -> bool {
    let s = (v1.x - v3.x) * (y - v3.y) - (v1.y - v3.y) * (x - v3.x);
    let t = (v2.x - v1.x) * (y - v1.y) - (v2.y - v1.y) * (x - v1.x);
    if (s < 0) != (t < 0) && s != 0 && t != 0 {
        return false;
    }
    let d = (v3.x - v2.x) * (y - v2.y) - (v3.y - v2.y) * (x - v2.x);
    (d < 0) == (s + t <= 0)
}

#[cfg(test)]
mod tests {
    use super::{generate_trees, is_plantable, place_tree, random_tile};
    use crate::map::{
        Map, TileCoord, TileKind, WaterClass, set_water_class_m1, water_class_from_m1,
    };
    use crate::world_gen::{CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, Climate, clear_ground_m5};

    #[test]
    fn random_tile_matches_power_of_two_tile_index_layout() {
        assert_eq!(random_tile(0, 64, 64), TileCoord::new(0, 0));
        assert_eq!(random_tile(65, 64, 64), TileCoord::new(1, 1));
        assert_eq!(random_tile(u32::MAX, 64, 64), TileCoord::new(63, 63));
    }

    #[test]
    fn place_tree_writes_make_tree_contract() {
        let mut map = Map::new_flat(8, 8, 2);
        place_tree(
            &mut map,
            TileCoord::new(3, 3),
            0xF123_4567,
            Climate::Temperate,
        );
        let tile = map.get(TileCoord::new(3, 3)).expect("tree tile");
        assert_eq!(tile.kind, TileKind::Forest);
        assert_eq!(tile.mapt, 0x40);
        assert_eq!(tile.m3, 11);
        assert_eq!(tile.m5, 0x03);
        assert_eq!(tile.m2, 0x70);
        assert_eq!(water_class_from_m1(tile.m1), WaterClass::Invalid);
        assert_eq!(
            tile.m6 | tile.m7 | tile.m8 as u8 | tile.m2_hi | tile.m3hi,
            0
        );
    }

    #[test]
    fn tree_planting_rejects_fields_and_honours_desert_policy() {
        let mut map = Map::new_flat(8, 8, 2);
        let field = TileCoord::new(3, 3);
        map.set_mapt_m5(field, 0, clear_ground_m5(CLEAR_GROUND_FIELDS, 3))
            .expect("field tile");
        assert!(!is_plantable(&map, field, &[], true));

        let desert = TileCoord::new(4, 3);
        map.set_mapt_m5(desert, 0, clear_ground_m5(CLEAR_GROUND_DESERT, 2))
            .expect("desert tile");
        assert!(is_plantable(&map, desert, &[], true));
        assert!(!is_plantable(&map, desert, &[], false));
    }

    #[test]
    fn coast_tree_keeps_shore_contract_and_clears_neighbour_flood_state() {
        let mut map = Map::new_flat(8, 8, 2);
        let coast = TileCoord::new(3, 3);
        let neighbour = TileCoord::new(4, 3);
        for c in [coast, neighbour] {
            let mut tile = map.get(c).expect("water fixture tile");
            tile.kind = TileKind::Water;
            tile.mapt = 0x60;
            tile.m5 = 0x10;
            tile.m1 = set_water_class_m1(tile.m1, WaterClass::Sea);
            tile.m3 = 1;
            map.set_tile(c, tile).expect("water fixture write");
        }

        assert!(is_plantable(&map, coast, &[], true));
        assert!(place_tree(&mut map, coast, 0xF123_4567, Climate::Temperate));

        let tree = map.get(coast).expect("shore tree");
        assert_eq!(tree.kind, TileKind::Forest);
        assert_eq!(tree.m2, 0xF0);
        assert_eq!(water_class_from_m1(tree.m1), WaterClass::Sea);
        assert_eq!(map.get(neighbour).expect("water neighbour").m3 & 1, 0);
    }

    #[test]
    fn generated_trees_are_deterministic() {
        let mut a = Map::new_flat(64, 64, 2);
        let mut b = a.clone();
        generate_trees(&mut a, Climate::Temperate, 42, &[]);
        generate_trees(&mut b, Climate::Temperate, 42, &[]);
        assert_eq!(a.tiles(), b.tiles());
        assert!(a.tiles().iter().any(|tile| tile.kind == TileKind::Forest));
    }
}
