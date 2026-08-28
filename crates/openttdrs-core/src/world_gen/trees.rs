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
/// Límite de altura original usado para normalizar la densidad de árboles.
const MAP_HEIGHT_LIMIT_ORIGINAL: u32 = 15;
/// `MAP_HEIGHT_LIMIT_AUTO_MINIMUM`: resolución de `map_height_limit = 0`.
const MAP_HEIGHT_LIMIT_AUTO_MINIMUM: u8 = 30;
const GROVE_RADIUS: i32 = 16;
const GROVE_SEGMENTS: usize = 16;
/// `WaterTileType::Coast` en los bits altos de `m5`.
const WATER_TYPE_COAST: u8 = 1;
/// `TreeGround` de `tree_map.h`.
const TREE_GROUND_GRASS: u8 = 0;
const TREE_GROUND_ROUGH: u8 = 1;
const TREE_GROUND_SNOW_DESERT: u8 = 2;
const TREE_GROUND_SHORE: u8 = 3;
/// `TreeGround::RoughSnow`; usa el bit 8 de MAP2.
const TREE_GROUND_ROUGH_SNOW: u8 = 4;
/// `IsSnowTile` de `clear_map.h`: bit 4 de MAP3 en una tesela `MP_CLEAR`.
const CLEAR_SNOW_M3_BIT: u8 = 1 << 4;
/// `TileType::Clear`; algunos `MP_OBJECT` legacy se representan como
/// `TileKind::Grass` hasta que se resuelve su pool, por lo que el tipo crudo
/// sigue siendo autoritativo para `CanPlantTreesOnTile`.
const OTTD_TILETYPE_CLEAR: u8 = 0;
/// `static_cast<float>(INT32_MAX / M_PI * 2)` de `CreateRandomStarShapedPolygon`.
///
/// El cálculo de C++ ocurre en doble precisión y sólo luego se reduce a
/// `float`; usar `TAU` directamente cambia la precedencia y deforma todos los
/// grupos de árboles para un mismo stream RNG.
const GROVE_PHASE_DIVISOR: f32 = ((i32::MAX as f64 / std::f64::consts::PI) * 2.0) as f32;
/// `(M_PI * 2) / GROVE_SEGMENTS`, redondeado una vez a `float` en C++.
const GROVE_ANGLE_STEP: f32 = ((std::f64::consts::PI * 2.0) / GROVE_SEGMENTS as f64) as f32;

#[derive(Clone, Copy, Debug, Default)]
struct Point {
    x: i32,
    y: i32,
}

/// Una colocación efectiva producida durante `GenerateTrees`.
///
/// El stream contiene sólo llamadas que llegaron a `PlaceTree`, no intentos
/// descartados por borde, forma del grupo, sustrato o altura. Es el mismo
/// límite que usa la traza del oráculo C++ para localizar la primera regla
/// divergente sin tener que comparar una captura raster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreePlacement {
    pub origin: TreePlacementOrigin,
    pub x: i32,
    pub y: i32,
    pub random: u32,
    pub parent: Option<TileCoord>,
}

/// Call-site de una colocación de árbol dentro de `GenerateTrees`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreePlacementOrigin {
    Group,
    Random,
    SameHeight,
}

impl TreePlacementOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::Random => "random",
            Self::SameHeight => "same_height",
        }
    }
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
    let mut ignore = |_placement: TreePlacement| {};
    generate_trees_with_rng_observer(map, climate, rng, preserve, &mut ignore);
}

/// Variante de [`generate_trees_with_rng`] que informa cada colocación efectiva.
///
/// La observación no toca el mapa ni el RNG; se usa por el oráculo diferencial
/// para alinear el primer `PlaceTree` de Rust con `OpenTTD`.
pub fn generate_trees_with_rng_observer<F>(
    map: &mut Map,
    climate: Climate,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
    observer: &mut F,
) where
    F: FnMut(TreePlacement),
{
    generate_trees_with_rng_observer_with_height_limit(
        map,
        climate,
        rng,
        preserve,
        MAP_HEIGHT_LIMIT_AUTO_MINIMUM,
        observer,
    );
}

/// Variante observada que respeta el límite efectivo de altura de `OpenTTD`.
///
/// `PlaceTreesRandomly` normaliza los refuerzos de igual altura por
/// `MAP_HEIGHT_LIMIT_ORIGINAL / construction.map_height_limit`. El parámetro
/// acepta `0` para el modo automático, que `OpenTTD` resuelve a 30 antes de
/// generar el mundo.
pub fn generate_trees_with_rng_observer_with_height_limit<F>(
    map: &mut Map,
    climate: Climate,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
    map_height_limit: u8,
    observer: &mut F,
) where
    F: FnMut(TreePlacement),
{
    generate_trees_with_rng_observer_with_map_settings(
        map,
        climate,
        rng,
        preserve,
        map_height_limit,
        super::DEF_SNOW_LINE_HEIGHT,
        observer,
    );
}

/// Variante observada que también usa la línea de nieve efectiva del save.
///
/// En ártico `PlaceTreesRandomly` triplica los refuerzos de igual altura
/// cuando el árbol base está por encima de `GetSnowLine()`. Esa línea queda
/// persistida en `PATS` como `game_creation.snow_line_height` al crear el
/// mundo, por lo que una reproducción desde un `.sav` no puede sustituirla
/// siempre por el default de una partida nueva.
pub fn generate_trees_with_rng_observer_with_map_settings<F>(
    map: &mut Map,
    climate: Climate,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
    map_height_limit: u8,
    snow_line_height: u8,
    observer: &mut F,
) where
    F: FnMut(TreePlacement),
{
    let (map_w, map_h) = map.dimensions();
    if map_w < 4 || map_h < 4 {
        return;
    }
    let attempts = scale_by_size(DEFAULT_TREE_STEPS, map_w, map_h);
    let groups = if matches!(climate, Climate::Toyland) {
        0
    } else {
        tree_group_count(rng.next(), map_w, map_h)
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
            if place_tree(map, tile, r, climate) {
                observer(TreePlacement {
                    origin: TreePlacementOrigin::Group,
                    x: tile.x,
                    y: tile.y,
                    random: r,
                    parent: Some(center),
                });
            }
        }
    }

    // `GenerateTrees` runs two passes on temperate maps in improved mode and
    // four on arctic maps. Each successful substrate also drives the
    // height-dependent same-level attempts from `PlaceTreesRandomly`.
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
                    observer(TreePlacement {
                        origin: TreePlacementOrigin::Random,
                        x: tile.x,
                        y: tile.y,
                        random: r,
                        parent: None,
                    });
                }
                // `PlaceTreesRandomly` in improved mode reinforces every
                // sustrato aceptado with `GetTileZ(tile) * 2` same-height
                // attempts. In temperate maps `PlaceTree` is always valid;
                // preserving the upstream order also keeps the tropical
                // invalid-tree path from changing RNG consumption.
                for _ in 0..same_height_attempt_count(
                    height,
                    climate,
                    snow_line_height,
                    map_height_limit,
                ) {
                    place_tree_at_same_height(map, tile, height, rng, climate, preserve, observer);
                }
            }
        }
    }
}

/// Cantidad de llamadas a `PlaceTreeAtSameHeight` de un árbol base.
///
/// Corresponde a `j = GetTileZ(tile) * 2`, el multiplicador ártico por encima
/// de `GetSnowLine()` y finalmente el escalado de `tree_cmd.cpp`, en ese
/// orden.
#[must_use]
const fn same_height_attempt_count(
    height: u8,
    climate: Climate,
    snow_line_height: u8,
    map_height_limit: u8,
) -> u32 {
    let mut attempts = (height as u32).saturating_mul(2);
    if matches!(climate, Climate::SubArctic) && height > snow_line_height {
        attempts = attempts.saturating_mul(3);
    }
    let effective_limit = if map_height_limit == 0 {
        MAP_HEIGHT_LIMIT_AUTO_MINIMUM as u32
    } else {
        map_height_limit as u32
    };
    if effective_limit > MAP_HEIGHT_LIMIT_ORIGINAL {
        attempts = attempts.saturating_mul(MAP_HEIGHT_LIMIT_ORIGINAL) / effective_limit;
    }
    attempts
}

/// `Map::ScaleBySize(GB(Random(), 0, 5) + 25)`: el `+ 25` es suma, no OR
/// bit a bit. En un mapa de 64×64 decide entre dos y cuatro grupos.
fn tree_group_count(random: u32, map_w: u32, map_h: u32) -> u32 {
    scale_by_size((random & 0x1F) + 0x19, map_w, map_h)
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
        TileKind::Water if tile.ottd_type_nibble() == 6 => {
            let is_coast = ((tile.m5 >> 4) & 0x0F) == WATER_TYPE_COAST;
            let slope = tile_slope_and_z(map, c).map_or(0, |(slope, _)| slope);
            is_coast && !is_slope_with_one_corner_raised(slope)
        }
        TileKind::Grass if tile.ottd_type_nibble() == OTTD_TILETYPE_CLEAR => {
            let ground = clear_ground_type(tile.m5);
            !matches!(ground, CLEAR_GROUND_FIELDS | CLEAR_GROUND_ROCKY)
                && (allow_desert || ground != CLEAR_GROUND_DESERT)
        }
        _ => false,
    }
}

/// `IsSlopeWithOneCornerRaised` compara el valor completo: un talud empinado
/// con un único bit de esquina no cuenta como una sola esquina elevada en
/// `OpenTTD` y por eso puede recibir un árbol de orilla.
#[must_use]
const fn is_slope_with_one_corner_raised(slope: u8) -> bool {
    matches!(slope, 1 | 2 | 4 | 8)
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
            // OpenTTD no codifica `CLEAR_SNOW` en `m5`: `IsSnowTile` mira
            // MAP3 bit 4. El generador Rust histórico también puede entregar
            // `CLEAR_GROUND_SNOW`, así que ambos formatos conservan el suelo
            // nevado al convertirlo en `MP_TREES`.
            let is_snow =
                previous.m3 & CLEAR_SNOW_M3_BIT != 0 || original_ground == CLEAR_GROUND_SNOW;
            let ground = if is_snow {
                if original_ground == CLEAR_GROUND_ROUGH {
                    TREE_GROUND_ROUGH_SNOW
                } else {
                    TREE_GROUND_SNOW_DESERT
                }
            } else {
                match original_ground {
                    CLEAR_GROUND_ROUGH => TREE_GROUND_ROUGH,
                    // El mapa procedural representa la cobertura de nieve como
                    // `CLEAR_GROUND_SNOW`; ambas variantes conservan densidad en
                    // `PlaceTree` en lugar de rerandomizarla.
                    CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => TREE_GROUND_SNOW_DESERT,
                    _ => TREE_GROUND_GRASS,
                }
            };
            (
                ground,
                density,
                matches!(ground, TREE_GROUND_SNOW_DESERT | TREE_GROUND_ROUGH_SNOW),
            )
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

    let tree_m2 = (u16::from(ground & 0x07) << 6) | (u16::from(density & 0x03) << 4);
    let mut tile = Tile {
        height: previous.height,
        kind: TileKind::Forest,
        mapt: 0x40,
        m5: (count_minus_one << 6) | growth,
        m1: set_water_class_m1(OWNER_NONE_M1, water_class),
        m6: 0,
        m8: 0,
        m3: tree_type,
        m2: tree_m2 as u8,
        m2_hi: (tree_m2 >> 8) as u8,
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

fn place_tree_at_same_height<F>(
    map: &mut Map,
    center: TileCoord,
    height: u8,
    rng: &mut Randomizer,
    climate: Climate,
    preserve: &[PreserveRect],
    observer: &mut F,
) where
    F: FnMut(TreePlacement),
{
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
        if place_tree(map, tile, r, climate) {
            observer(TreePlacement {
                origin: TreePlacementOrigin::SameHeight,
                x: tile.x,
                y: tile.y,
                random: r,
                parent: Some(center),
            });
        }
        break;
    }
}

fn random_grove(rng: &mut Randomizer) -> [Point; GROVE_SEGMENTS] {
    let harmonics = [
        (GROVE_RADIUS / 2, rng.next() as f32 / GROVE_PHASE_DIVISOR, 1),
        (GROVE_RADIUS / 4, rng.next() as f32 / GROVE_PHASE_DIVISOR, 2),
        (GROVE_RADIUS / 8, rng.next() as f32 / GROVE_PHASE_DIVISOR, 3),
        (
            GROVE_RADIUS / 16,
            rng.next() as f32 / GROVE_PHASE_DIVISOR,
            4,
        ),
    ];
    let mut grove = [Point::default(); GROVE_SEGMENTS];
    let mut theta = 0.0;
    for point in &mut grove {
        let deviation = harmonics
            .iter()
            .fold(0.0, |sum, (amplitude, phase, frequency)| {
                sum + ((theta + phase) * *frequency as f32).sin() * *amplitude as f32
            });
        let radius = GROVE_RADIUS as f32 / 2.0 + deviation / 2.0;
        point.x = (theta.cos() * radius) as i32;
        point.y = (theta.sin() * radius) as i32;
        // `CreateStarShapedPolygon` incrementa el `float` ya redondeado en
        // cada segmento; calcular `TAU * index` no conserva esos redondeos.
        theta += GROVE_ANGLE_STEP;
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
    use super::{
        GROVE_ANGLE_STEP, GROVE_PHASE_DIVISOR, generate_trees, generate_trees_with_rng_observer,
        is_plantable, is_slope_with_one_corner_raised, place_tree, random_tile,
        same_height_attempt_count, tree_group_count,
    };
    use crate::cargodist::parity::Randomizer;
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
    fn random_grove_phase_divisor_matches_upstream_double_then_float_contract() {
        assert_eq!(GROVE_PHASE_DIVISOR.to_bits(), 0x4EA2_F983);
        assert_eq!(GROVE_ANGLE_STEP.to_bits(), 0x3EC9_0FDB);
    }

    #[test]
    fn tree_group_count_uses_addition_after_the_low_five_bits() {
        assert_eq!(tree_group_count(0, 64, 64), 2);
        assert_eq!(tree_group_count(7, 64, 64), 2);
        assert_eq!(tree_group_count(31, 64, 64), 4);
        assert_eq!(tree_group_count(31, 256, 256), 56);
    }

    #[test]
    fn steep_single_corner_is_not_a_single_corner_slope() {
        assert!(is_slope_with_one_corner_raised(1));
        assert!(!is_slope_with_one_corner_raised(0x11));
    }

    #[test]
    fn same_height_reinforcement_scales_with_map_height_limit() {
        assert_eq!(same_height_attempt_count(3, Climate::Temperate, 2, 15), 6);
        assert_eq!(same_height_attempt_count(3, Climate::Temperate, 2, 30), 3);
        assert_eq!(same_height_attempt_count(3, Climate::Temperate, 2, 0), 3);
        assert_eq!(same_height_attempt_count(1, Climate::Temperate, 2, 30), 1);
    }

    #[test]
    fn arctic_reinforcement_triples_only_above_saved_snow_line() {
        // `tree_cmd.cpp`: j = z * 2; if (z > GetSnowLine()) j *= 3;
        // y recién después se normaliza al límite de altura del mapa.
        assert_eq!(same_height_attempt_count(3, Climate::SubArctic, 2, 30), 9);
        assert_eq!(same_height_attempt_count(2, Climate::SubArctic, 2, 30), 2);
        assert_eq!(same_height_attempt_count(3, Climate::Temperate, 2, 30), 3);
    }

    #[test]
    fn observer_reports_only_effective_tree_placements() {
        let mut map = Map::new_flat(64, 64, 2);
        let mut rng = Randomizer::new(0x1234_5678);
        let mut placements = Vec::new();
        generate_trees_with_rng_observer(
            &mut map,
            Climate::Temperate,
            &mut rng,
            &[],
            &mut |placement| placements.push(placement),
        );

        assert!(!placements.is_empty());
        assert!(placements.iter().all(|placement| {
            map.get(TileCoord::new(placement.x, placement.y))
                .is_some_and(|tile| tile.kind == TileKind::Forest)
        }));
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
    fn snowy_clear_tile_preserves_snow_ground_and_density() {
        let mut map = Map::new_flat(8, 8, 2);
        let snow = TileCoord::new(3, 3);
        let mut tile = map.get(snow).expect("snow fixture tile");
        tile.m3 = 0x10; // `IsSnowTile` / MAP3 bit 4.
        tile.m5 = clear_ground_m5(0, 0);
        map.set_tile(snow, tile).expect("snow fixture write");

        assert!(place_tree(&mut map, snow, 0xF123_4567, Climate::SubArctic));
        let tree = map.get(snow).expect("snow tree");
        assert_eq!(tree.m2, 0x80); // SnowOrDesert, density 0.
        assert_eq!(tree.m2_hi, 0);

        let rough_snow = TileCoord::new(4, 3);
        let mut rough = map.get(rough_snow).expect("rough snow fixture tile");
        rough.m3 = 0x10;
        rough.m5 = clear_ground_m5(1, 3);
        map.set_tile(rough_snow, rough)
            .expect("rough snow fixture write");
        assert!(place_tree(
            &mut map,
            rough_snow,
            0xF123_4567,
            Climate::SubArctic
        ));
        let rough_tree = map.get(rough_snow).expect("rough snow tree");
        assert_eq!(rough_tree.m2, 0x30); // low byte of RoughSnow + density 3.
        assert_eq!(rough_tree.m2_hi, 1);
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
    fn tree_planting_never_treats_raw_object_as_clear_grass() {
        let mut map = Map::new_flat(8, 8, 2);
        let object = TileCoord::new(3, 3);
        // `MP_OBJECT` aún puede usar `TileKind::Grass` para el fallback
        // visual; `CanPlantTreesOnTile` de OpenTTD mira MAPT y lo rechaza.
        let mut tile = map.get(object).expect("object fixture tile");
        tile.mapt = 0xA0;
        map.set_tile(object, tile).expect("object fixture write");
        assert!(!is_plantable(&map, object, &[], true));
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
