//! Generación procedural de terreno y climas (T4), al estilo `genworld` / `LandscapeType` de `OpenTTD`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::manual_is_multiple_of
)]

mod clear_tiles;
mod config;
mod height;
mod heightmap;
mod landcover;
pub(crate) use landcover::desert_patch;
mod objects;
mod population;
mod rivers;
mod tgp;
mod tile_loop;
mod trees;

pub(crate) use clear_tiles::generate_clear_tiles;
pub use config::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY,
    CLEAR_GROUND_ROUGH, CLEAR_GROUND_SNOW, Climate, DEF_DESERT_COVERAGE, DEF_SNOW_COVERAGE,
    DEF_SNOW_LINE_HEIGHT, PreserveRect, QuantitySeaLakes, TerrainType, TgenSmoothness,
    WorldGenConfig, clear_ground_m5, effective_clear_ground, initial_clear_ground,
    initial_clear_ground_with_lines,
};
pub use heightmap::{HeightmapData, apply_heightmap, parse_hmap, serialize_heightmap};
pub use objects::generate_objects_with_rng;
pub use population::{
    IndustryDensity, NUM_INITIAL_INDUSTRIES, NUM_INITIAL_TOWNS, PopulationGenConfig, TownDensity,
    apply_population_gen, apply_population_gen_with_rng, ceil_div, generate_industries,
    generate_industries_with_rng, generate_towns, generate_towns_with_rng, house_beside_road,
    industry_target_count, road_tiles_are_flat, scale_by_size, town_target_count,
};
pub use tile_loop::run_generation_tile_loop;
pub use trees::{
    TreePlacement, TreePlacementOrigin, generate_trees, generate_trees_with_rng,
    generate_trees_with_rng_observer, generate_trees_with_rng_observer_with_height_limit,
    generate_trees_with_rng_observer_with_map_settings,
};

use crate::cargodist::parity::Randomizer;
use crate::company::{OWNER_NONE_M1, OWNER_WATER_M1};
use crate::map::{
    Map, MapError, TileCoord, TileKind, WaterClass, set_water_class_m1, tile_slope_and_z,
};

use rivers::{carve_rivers, mark_water_coasts};
use tgp::{calculate_coverage_line, generate_tgp_heights};

/// Stream de RNG que `OpenTTD` comparte entre terreno, suelo, población y
/// árboles durante la creación de una partida nueva.
pub type WorldGenRng = Randomizer;

/// Limite mínimo que `OpenTTD` elige cuando `construction.map_height_limit` está
/// en automático (`MAP_HEIGHT_LIMIT_AUTO_MINIMUM`). La generación procedural
/// de la matriz usa el ajuste automático del juego original.
const MAP_HEIGHT_LIMIT_AUTO_MINIMUM: u8 = 30;

/// Aplica `FixSlopes()` de `heightmap.cpp` después de TGP.
///
/// `OpenTTD` limita a una unidad la diferencia de altura entre teselas
/// ortogonales. La pasada modifica el stream global y, cuando corrige una
/// tesela interior, puede convertirla en roca usando `RandomRange`; por eso
/// debe ejecutarse antes de `ConvertGroundTilesIntoWaterTiles` y
/// `GenerateClearTile`.
fn fix_slopes(
    map: &mut Map,
    rng: &mut WorldGenRng,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    let (width, height) = map.dimensions();
    let max_height = MAP_HEIGHT_LIMIT_AUTO_MINIMUM;

    // Primera pasada: borde superior e izquierdo.
    for y in 0..height {
        for x in 0..width {
            let c = TileCoord::new(x as i32, y as i32);
            if preserve.iter().any(|rect| rect.contains(c.x, c.y)) {
                continue;
            }
            let Some(tile) = map.get(c) else {
                continue;
            };
            let mut neighbour_height = u8::MAX;
            if x != 0 {
                neighbour_height = map
                    .get(TileCoord::new((x - 1) as i32, y as i32))
                    .map_or(neighbour_height, |neighbour| neighbour.height);
            }
            if y != 0
                && let Some(neighbour) = map.get(TileCoord::new(x as i32, (y - 1) as i32))
                && neighbour.height < neighbour_height
            {
                neighbour_height = neighbour.height;
            }
            if u16::from(tile.height) < u16::from(neighbour_height) + 2 {
                continue;
            }
            let corrected = neighbour_height.saturating_add(1);
            map.set_height(c, corrected)?;
            if is_inner_tile(x, y, width, height)
                && rng.random_range(u32::from(max_height)) <= u32::from(neighbour_height)
            {
                let mut corrected_tile = map.get(c).ok_or(MapError::OutOfBounds)?;
                corrected_tile.m5 = clear_ground_m5(CLEAR_GROUND_ROCKY, 3);
                map.set_tile(c, corrected_tile)?;
            }
        }
    }

    // Segunda pasada: borde inferior y derecho.
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let c = TileCoord::new(x as i32, y as i32);
            if preserve.iter().any(|rect| rect.contains(c.x, c.y)) {
                continue;
            }
            let Some(tile) = map.get(c) else {
                continue;
            };
            let mut neighbour_height = u8::MAX;
            if x + 1 < width {
                neighbour_height = map
                    .get(TileCoord::new((x + 1) as i32, y as i32))
                    .map_or(neighbour_height, |neighbour| neighbour.height);
            }
            if y + 1 < height
                && let Some(neighbour) = map.get(TileCoord::new(x as i32, (y + 1) as i32))
                && neighbour.height < neighbour_height
            {
                neighbour_height = neighbour.height;
            }
            if u16::from(tile.height) < u16::from(neighbour_height) + 2 {
                continue;
            }
            let corrected = neighbour_height.saturating_add(1);
            map.set_height(c, corrected)?;
            if is_inner_tile(x, y, width, height)
                && rng.random_range(u32::from(max_height)) <= u32::from(neighbour_height)
            {
                let mut corrected_tile = map.get(c).ok_or(MapError::OutOfBounds)?;
                corrected_tile.m5 = clear_ground_m5(CLEAR_GROUND_ROCKY, 3);
                map.set_tile(c, corrected_tile)?;
            }
        }
    }
    Ok(())
}

#[inline]
const fn is_inner_tile(x: u32, y: u32, width: u32, height: u32) -> bool {
    x > 0 && y > 0 && x + 1 < width && y + 1 < height
}

/// Genera colinas y lagos sobre un mapa ya inicializado (backend TGP / Perlin).
///
/// Las teselas dentro de `preserve` conservan tipo y altura actuales.
/// El heightmap externo sigue disponible vía [`apply_heightmap`].
///
/// # Errors
///
/// Fallos de `Map::set_height` / `set_kind` / `set_mapt_m5`.
pub fn apply_world_gen(
    map: &mut Map,
    config: &WorldGenConfig,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    apply_world_gen_with_rng(map, config, preserve).map(|_| ())
}

/// Ejecuta `GenerateLandscape` hasta justo antes de `GenerateClearTile` y
/// devuelve el RNG compartido para continuar la partida nueva.
///
/// Esta frontera conserva TGP, `FixSlopes`, conversión de agua, coberturas,
/// ríos y bordes `MP_VOID`, pero no materializa rough/rocks de clear. Permite
/// comparar el paisaje crudo con `OpenTTD` sin mezclar ambas fases.
///
/// # Errors
///
/// Fallos de `Map::set_height` / `set_kind` / `set_mapt_m5`.
pub fn apply_landscape_with_rng(
    map: &mut Map,
    config: &WorldGenConfig,
    preserve: &[PreserveRect],
) -> Result<WorldGenRng, MapError> {
    let (mw, mh) = map.dimensions();
    let map_w = i32::try_from(mw).expect("map width fits i32");
    let map_h = i32::try_from(mh).expect("map height fits i32");

    let mut rng = Randomizer::new(config.seed as u32);
    for _ in 0..config.startup_rng_draws {
        // `_GenerateWorld` llama a `StartupEconomy` tras sembrar `_random`;
        // esa rutina consume exactamente un `Random()` antes de entrar en
        // `GenerateTerrainPerlin`.
        let _ = rng.next();
    }
    let heights = generate_tgp_heights(map_w, map_h, config, &mut rng);

    for y in 0..map_h {
        for x in 0..map_w {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let h = heights[(y * map_w + x) as usize];
            map.set_height(TileCoord::new(x, y), h)?;
        }
    }

    fix_slopes(map, &mut rng, preserve)?;

    // `FixSlopes` puede cambiar alturas y marcar rocas; las coberturas se
    // calculan sobre el mapa ya corregido, igual que `GenerateLandscape`.
    let mut heights = Vec::with_capacity((map_w * map_h) as usize);
    for y in 0..map_h {
        for x in 0..map_w {
            heights.push(map.get(TileCoord::new(x, y)).map_or(0, |tile| tile.height));
        }
    }

    // Coberturas post-relieve (`CalculateSnowLine` / `CalculateDesertLine`).
    let snow_line = if config.climate == Climate::SubArctic {
        calculate_coverage_line(&heights, map_w, map_h, config.snow_coverage, 0).max(2)
    } else {
        DEF_SNOW_LINE_HEIGHT
    };
    let desert_line = if config.climate == Climate::SubTropical {
        Some(calculate_coverage_line(
            &heights,
            map_w,
            map_h,
            100u8.saturating_sub(config.desert_coverage),
            4,
        ))
    } else {
        None
    };

    // TGP normaliza el mar a altura 0 (`ConvertGroundTilesIntoWaterTiles`).
    for y in 0..map_h {
        for x in 0..map_w {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            let Some((_, z)) = tile_slope_and_z(map, c) else {
                continue;
            };
            if z == 0 {
                map.set_kind(c, TileKind::Water)?;
                map.set_mapt_m5(c, 0x60, 0)?;
                map.set_m1(c, set_water_class_m1(OWNER_WATER_M1, WaterClass::Sea))?;
                continue;
            }
            let ground =
                if (map.get(c).map_or(0, |tile| (tile.m5 >> 2) & 0x07)) == CLEAR_GROUND_ROCKY {
                    CLEAR_GROUND_ROCKY
                } else {
                    initial_clear_ground_with_lines(
                        config.climate,
                        x,
                        y,
                        z,
                        config.seed,
                        snow_line,
                        desert_line,
                    )
                };
            // `InitializeLandscape`/`MakeClear` arranca todas las teselas de
            // suelo con densidad 3. La densidad variable que usamos aquí
            // antes era un atajo visual, pero diverge del mapa nuevo de
            // OpenTTD incluso antes de que corra `GenerateClearTile`.
            let m5 = clear_ground_m5(ground, 3);
            map.set_kind(c, TileKind::Grass)?;
            map.set_mapt_m5(c, 0, m5)?;
            map.set_m1(c, OWNER_NONE_M1)?;
        }
    }

    mark_water_coasts(map, map_w, map_h, 0, preserve);
    carve_rivers(map, config, map_w, map_h, preserve)?;

    // OpenTTD materializa MP_VOID en los cuatro bordes cuando
    // `freeform_edges` está habilitado (el valor predeterminado de una
    // partida nueva). Antes de este paso el generador Rust dejaba agua/suelo
    // válido en esos índices, haciendo divergir incluso los mapas planos.
    let last_x = map_w.saturating_sub(1);
    let last_y = map_h.saturating_sub(1);
    for y in 0..map_h {
        for x in 0..map_w {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            if x == 0 || y == 0 || x == last_x || y == last_y {
                let c = TileCoord::new(x, y);
                map.set_kind(c, TileKind::Void)?;
                map.set_mapt_m5(c, 0x70, 0)?;
                map.set_m1(c, 0)?;
                map.set_m2(c, 0)?;
            }
        }
    }

    Ok(rng)
}

/// Ejecuta exclusivamente `GenerateClearTile` sobre un paisaje ya generado.
///
/// No consume un RNG propio: debe recibir el stream que devolvió
/// [`apply_landscape_with_rng`].
pub fn apply_clear_generation_with_rng(
    map: &mut Map,
    rng: &mut WorldGenRng,
    preserve: &[PreserveRect],
) {
    generate_clear_tiles(map, rng, preserve);
}

/// Genera el paisaje y devuelve el estado de RNG para continuar con
/// `GenerateTowns`/`GenerateIndustries`/`GenerateTrees` sin reiniciar la
/// secuencia global de `OpenTTD`.
pub fn apply_world_gen_with_rng(
    map: &mut Map,
    config: &WorldGenConfig,
    preserve: &[PreserveRect],
) -> Result<WorldGenRng, MapError> {
    let mut rng = apply_landscape_with_rng(map, config, preserve)?;
    // `GenerateClearTile` runs after the landscape converter and before
    // towns/industries. Its rough/rocky bits are part of the raw map contract,
    // not a renderer-only decoration.
    apply_clear_generation_with_rng(map, &mut rng, preserve);

    Ok(rng)
}

#[cfg(test)]
mod tests {
    use crate::company::OWNER_NONE_M1;

    use super::*;

    #[test]
    fn climate_parse_accepts_aliases() {
        assert_eq!(Climate::parse("arctic"), Some(Climate::SubArctic));
        assert_eq!(Climate::parse("tropic"), Some(Climate::SubTropical));
        assert_eq!(Climate::parse("toyland"), Some(Climate::Toyland));
        assert!(Climate::parse("mars").is_none());
    }

    #[test]
    fn world_gen_is_deterministic_for_seed() {
        let mut map_a = Map::new_flat(16, 12, 2);
        let mut map_b = Map::new_flat(16, 12, 2);
        let cfg = WorldGenConfig {
            seed: 42,
            ..WorldGenConfig::default()
        };
        apply_world_gen(&mut map_a, &cfg, &[]).expect("gen a");
        apply_world_gen(&mut map_b, &cfg, &[]).expect("gen b");
        assert_eq!(map_a.dimensions(), map_b.dimensions());
        let (width, height) = map_a.dimensions();
        for ty in 0..height {
            for tx in 0..width {
                let coord = TileCoord::new(tx as i32, ty as i32);
                assert_eq!(
                    map_a.get_kind(coord),
                    map_b.get_kind(coord),
                    "kind at {tx},{ty}"
                );
                assert_eq!(
                    map_a.get(coord).map(|t| t.height),
                    map_b.get(coord).map(|t| t.height),
                    "height at {tx},{ty}"
                );
            }
        }
    }

    #[test]
    fn split_landscape_and_clear_preserve_combined_map_and_rng() {
        let config = WorldGenConfig {
            seed: 1_330_935_378,
            ..WorldGenConfig::default()
        };
        let mut combined = Map::new_flat(64, 64, 0);
        let combined_rng =
            apply_world_gen_with_rng(&mut combined, &config, &[]).expect("combined generation");

        let mut split = Map::new_flat(64, 64, 0);
        let mut split_rng =
            apply_landscape_with_rng(&mut split, &config, &[]).expect("landscape generation");
        apply_clear_generation_with_rng(&mut split, &mut split_rng, &[]);

        assert_eq!(combined_rng.state, split_rng.state);
        for y in 0..64 {
            for x in 0..64 {
                let c = TileCoord::new(x, y);
                assert_eq!(combined.get(c), split.get(c), "tile {x},{y}");
            }
        }
    }

    #[test]
    fn fix_slopes_caps_orthogonal_height_jumps() {
        let mut map = Map::new_flat(4, 4, 0);
        map.set_height(TileCoord::new(2, 2), 8)
            .expect("set high tile");
        let mut rng = Randomizer::new(123);
        fix_slopes(&mut map, &mut rng, &[]).expect("fix slopes");
        assert_eq!(
            map.get(TileCoord::new(2, 2)).map(|tile| tile.height),
            Some(1)
        );
        for y in 0..4i32 {
            for x in 0..4i32 {
                let Some(tile) = map.get(TileCoord::new(x, y)) else {
                    continue;
                };
                for (dx, dy) in [(1, 0), (0, 1)] {
                    let Some(neighbour) = map.get(TileCoord::new(x + dx, y + dy)) else {
                        continue;
                    };
                    assert!(
                        (i16::from(tile.height) - i16::from(neighbour.height)).abs() <= 1,
                        "jump at ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn world_gen_creates_water_and_land() {
        let mut map = Map::new_flat(20, 16, 2);
        apply_world_gen(
            &mut map,
            &WorldGenConfig {
                seed: 99,
                quantity_sea_lakes: QuantitySeaLakes::Medium,
                ..Default::default()
            },
            &[],
        )
        .expect("gen");
        let (w, h) = map.dimensions();
        let mut water = 0;
        let mut land = 0;
        for y in 0..h {
            for x in 0..w {
                match map.get_kind(TileCoord::new(x as i32, y as i32)) {
                    Some(TileKind::Water) => water += 1,
                    Some(TileKind::Grass | TileKind::Forest) => land += 1,
                    _ => {}
                }
            }
        }
        assert!(water > 0, "expected some water tiles");
        assert!(land > water, "expected mostly land");
    }

    #[test]
    fn preserve_rect_skips_terrain_changes() {
        let mut map = Map::new_flat(8, 8, 3);
        let center = TileCoord::new(4, 4);
        map.set_kind(center, TileKind::Grass).expect("kind");
        let before_h = map.get(center).expect("tile").height;
        apply_world_gen(
            &mut map,
            &WorldGenConfig {
                seed: 1,
                ..Default::default()
            },
            &[PreserveRect {
                x0: 3,
                y0: 3,
                x1: 5,
                y1: 5,
            }],
        )
        .expect("gen");
        assert_eq!(map.get(center).expect("tile").height, before_h);
    }

    #[test]
    fn world_gen_creates_interior_lakes() {
        let mut map = Map::new_flat(64, 64, 2);
        apply_world_gen(
            &mut map,
            &WorldGenConfig {
                seed: 0xDEAD_BEEF,
                island: true,
                quantity_sea_lakes: QuantitySeaLakes::Medium,
                ..Default::default()
            },
            &[],
        )
        .expect("gen");
        let interior_water = (8..56i32)
            .flat_map(|y| (8..56).map(move |x| (x, y)))
            .filter(|&(x, y)| map.get_kind(TileCoord::new(x, y)) == Some(TileKind::Water))
            .count();
        assert!(
            interior_water >= 24,
            "expected interior lakes, got {interior_water} water tiles away from borders"
        );
    }

    #[test]
    fn world_gen_carves_some_rivers() {
        use crate::map::{WaterClass, is_river_tile, water_class_from_m1};

        let mut map = Map::new_flat(48, 48, 2);
        apply_world_gen(
            &mut map,
            &WorldGenConfig {
                seed: 0x00C0_FFEE,
                island: true,
                quantity_sea_lakes: QuantitySeaLakes::Low,
                ..Default::default()
            },
            &[],
        )
        .expect("gen");
        let rivers = (0..48i32)
            .flat_map(|y| (0..48).map(move |x| TileCoord::new(x, y)))
            .filter(|&c| map.get(c).is_some_and(is_river_tile))
            .count();
        assert!(rivers >= 4, "expected carved rivers, got {rivers}");
        let sea = (0..48i32)
            .flat_map(|y| (0..48).map(move |x| TileCoord::new(x, y)))
            .filter(|&c| {
                map.get(c).is_some_and(|t| {
                    t.kind == TileKind::Water && water_class_from_m1(t.m1) == WaterClass::Sea
                })
            })
            .count();
        assert!(sea > rivers, "sea should dominate rivers");
    }

    #[test]
    fn world_gen_uses_openttd_reserved_tile_owners() {
        let mut map = Map::new_flat(64, 64, 0);
        let cfg = WorldGenConfig {
            seed: 1_330_928_978,
            amount_of_rivers: 0,
            water_borders: Some(0),
            ..WorldGenConfig::default()
        };
        apply_world_gen(&mut map, &cfg, &[]).expect("generate map");
        generate_trees(&mut map, cfg.climate, cfg.seed, &[]);

        let mut clear = 0;
        let mut water = 0;
        for y in 1..63 {
            for x in 1..63 {
                let tile = map.get(TileCoord::new(x, y)).expect("tile");
                match tile.kind {
                    TileKind::Grass => {
                        clear += 1;
                        assert_eq!(tile.m1, OWNER_NONE_M1);
                    }
                    TileKind::Forest => {
                        clear += 1;
                        let water_class = if ((tile.m2 >> 6) & 0x07) == 3 {
                            WaterClass::Sea
                        } else {
                            WaterClass::Invalid
                        };
                        assert_eq!(tile.m1, set_water_class_m1(OWNER_NONE_M1, water_class));
                    }
                    TileKind::Water => {
                        water += 1;
                        assert_eq!(tile.m1, OWNER_WATER_M1);
                    }
                    _ => {}
                }
            }
        }
        assert!(clear > 0);
        assert!(water > 0);
    }

    #[test]
    fn generated_clear_tiles_start_at_full_density() {
        let mut map = Map::new_flat(64, 64, 0);
        apply_world_gen(
            &mut map,
            &WorldGenConfig {
                seed: 1_330_928_978,
                amount_of_rivers: 0,
                ..WorldGenConfig::default()
            },
            &[],
        )
        .expect("generate map");
        let grass = map
            .tiles()
            .iter()
            .filter(|tile| tile.kind == TileKind::Grass)
            .collect::<Vec<_>>();
        assert!(!grass.is_empty());
        assert!(grass.iter().all(|tile| tile.m5 & 0x03 == 3));
    }

    #[test]
    fn generated_tree_tiles_preserve_shore_water_class() {
        let mut map = Map::new_flat(64, 64, 0);
        apply_world_gen(
            &mut map,
            &WorldGenConfig {
                seed: 1_330_928_978,
                amount_of_rivers: 0,
                ..WorldGenConfig::default()
            },
            &[],
        )
        .expect("generate map");
        generate_trees(&mut map, Climate::Temperate, 1_330_928_978, &[]);
        let trees = map
            .tiles()
            .iter()
            .filter(|tile| tile.kind == TileKind::Forest)
            .collect::<Vec<_>>();
        assert!(!trees.is_empty());
        assert!(trees.iter().all(|tile| {
            let water_class = if ((tile.m2 >> 6) & 0x07) == 3 {
                WaterClass::Sea
            } else {
                WaterClass::Invalid
            };
            tile.m1 == set_water_class_m1(OWNER_NONE_M1, water_class)
        }));
        assert!(trees.iter().any(|tile| ((tile.m2 >> 6) & 0x07) == 3));
    }

    #[test]
    fn island_mode_increases_water_on_edges() {
        let mut flat = Map::new_flat(24, 18, 2);
        let mut island = Map::new_flat(24, 18, 2);
        let base_cfg = WorldGenConfig {
            seed: 7,
            island: false,
            quantity_sea_lakes: QuantitySeaLakes::VeryLow,
            ..Default::default()
        };
        let island_cfg = WorldGenConfig {
            island: true,
            ..base_cfg
        };
        apply_world_gen(&mut flat, &base_cfg, &[]).expect("flat gen");
        apply_world_gen(&mut island, &island_cfg, &[]).expect("island gen");
        let edge_water = |map: &Map| {
            let mut n = 0;
            for x in 0..24 {
                if map.get_kind(TileCoord::new(x, 0)) == Some(TileKind::Water) {
                    n += 1;
                }
                if map.get_kind(TileCoord::new(x, 17)) == Some(TileKind::Water) {
                    n += 1;
                }
            }
            n
        };
        assert!(
            edge_water(&island) >= edge_water(&flat),
            "island mode should wet map borders"
        );
    }

    #[test]
    fn arctic_effective_clear_ground_respects_m5_snow() {
        assert_eq!(
            effective_clear_ground(Climate::SubArctic, 0, 0, 0, 0),
            CLEAR_GROUND_GRASS
        );
        assert_eq!(
            effective_clear_ground(
                Climate::SubArctic,
                clear_ground_m5(CLEAR_GROUND_SNOW, 0),
                0,
                0,
                0
            ),
            CLEAR_GROUND_SNOW
        );
        assert_eq!(
            initial_clear_ground(Climate::SubArctic, 0, 0, 12, 0),
            CLEAR_GROUND_SNOW
        );
        assert_eq!(
            initial_clear_ground(Climate::SubArctic, 0, 50, 0, 0),
            CLEAR_GROUND_GRASS
        );
    }

    #[test]
    fn parse_and_apply_hmap_roundtrip() {
        let text = "OTDRHMAP1\n4 3\n\
            0 0 1 1\n\
            0 2 3 1\n\
            1 2 2 0\n";
        let data = parse_hmap(text).expect("parse");
        assert_eq!(data.width, 4);
        assert_eq!(data.height, 3);
        assert_eq!(data.heights.len(), 12);
        let mut map = Map::new_flat(4, 3, 2);
        apply_heightmap(&mut map, &data, 0, Climate::Temperate, 1).expect("apply");
        assert_eq!(map.get(TileCoord::new(0, 0)).map(|t| t.height), Some(0));
        assert_eq!(map.get_kind(TileCoord::new(0, 0)), Some(TileKind::Water));
        assert_eq!(map.get(TileCoord::new(1, 1)).map(|t| t.height), Some(2));
        assert_eq!(map.get_kind(TileCoord::new(1, 1)), Some(TileKind::Grass));
    }

    #[test]
    fn hilly_terrain_type_raises_peaks() {
        let mut flat = Map::new_flat(64, 64, 2);
        let mut hilly = Map::new_flat(64, 64, 2);
        let base = WorldGenConfig {
            seed: 123,
            island: false,
            ..WorldGenConfig::default().with_terrain_type(TerrainType::VeryFlat)
        };
        let tall = WorldGenConfig {
            ..base.with_terrain_type(TerrainType::Mountainous)
        };
        apply_world_gen(&mut flat, &base, &[]).expect("flat");
        apply_world_gen(&mut hilly, &tall, &[]).expect("hilly");
        let max_h = |map: &Map| {
            let (w, h) = map.dimensions();
            (0..h)
                .flat_map(|y| (0..w).map(move |x| TileCoord::new(x as i32, y as i32)))
                .filter_map(|c| map.get(c).map(|t| t.height))
                .max()
                .unwrap_or(0)
        };
        assert!(max_h(&hilly) >= max_h(&flat));
    }

    #[test]
    fn distinct_params_diverge_stably() {
        let a = WorldGenConfig {
            seed: 1,
            ..WorldGenConfig::default().with_terrain_type(TerrainType::Flat)
        };
        let b = WorldGenConfig {
            seed: 1,
            quantity_sea_lakes: QuantitySeaLakes::High,
            ..WorldGenConfig::default().with_terrain_type(TerrainType::Mountainous)
        };
        let mut ma = Map::new_flat(32, 32, 2);
        let mut mb = Map::new_flat(32, 32, 2);
        apply_world_gen(&mut ma, &a, &[]).expect("a");
        apply_world_gen(&mut mb, &b, &[]).expect("b");
        let mut ha = Vec::with_capacity(32 * 32);
        let mut hb = Vec::with_capacity(32 * 32);
        for y in 0..32i32 {
            for x in 0..32 {
                ha.push(ma.get(TileCoord::new(x, y)).map(|t| t.height));
                hb.push(mb.get(TileCoord::new(x, y)).map(|t| t.height));
            }
        }
        assert_ne!(
            ha, hb,
            "terrain_type / sea params should change the height field"
        );
    }
}
