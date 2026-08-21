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

mod config;
mod height;
mod heightmap;
mod landcover;
pub(crate) use landcover::desert_patch;
mod population;
mod rivers;
mod tgp;

pub use config::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, DEF_DESERT_COVERAGE, DEF_SNOW_COVERAGE, DEF_SNOW_LINE_HEIGHT,
    PreserveRect, QuantitySeaLakes, TerrainType, TgenSmoothness, WorldGenConfig, clear_ground_m5,
    effective_clear_ground, initial_clear_ground, initial_clear_ground_with_lines,
};
pub use heightmap::{HeightmapData, apply_heightmap, parse_hmap, serialize_heightmap};
pub use population::{
    IndustryDensity, NUM_INITIAL_INDUSTRIES, NUM_INITIAL_TOWNS, PopulationGenConfig, TownDensity,
    apply_population_gen, ceil_div, generate_industries, generate_towns, house_beside_road,
    industry_target_count, road_tiles_are_flat, scale_by_size, town_target_count,
};

use crate::map::{
    Map, MapError, TileCoord, TileKind, WaterClass, set_water_class_m1, tile_slope_and_z,
};

use landcover::{forest_patch, grass_density};
use rivers::{carve_rivers, mark_water_coasts};
use tgp::{calculate_coverage_line, generate_tgp_heights};

/// Genera colinas, lagos y bosques sobre un mapa ya inicializado (backend TGP / Perlin).
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
    let (mw, mh) = map.dimensions();
    let map_w = i32::try_from(mw).expect("map width fits i32");
    let map_h = i32::try_from(mh).expect("map height fits i32");

    let heights = generate_tgp_heights(map_w, map_h, config);

    for y in 0..map_h {
        for x in 0..map_w {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let h = heights[(y * map_w + x) as usize];
            map.set_height(TileCoord::new(x, y), h)?;
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
                map.set_m1(c, set_water_class_m1(0, WaterClass::Sea))?;
                continue;
            }
            let ground = initial_clear_ground_with_lines(
                config.climate,
                x,
                y,
                z,
                config.seed,
                snow_line,
                desert_line,
            );
            let m5 = clear_ground_m5(ground, grass_density(x, y, config.seed));
            if forest_patch(x, y, config.seed, config.climate) && ground != CLEAR_GROUND_DESERT {
                // MP_TREES: m5 = (count-1)<<6 | growth; adulto por defecto (OpenTTD Grown).
                let count_m1 = ((config
                    .seed
                    .wrapping_mul(x as u64 + 1)
                    .wrapping_add(y as u64 * 17))
                    & 3) as u8;
                let tree_m5 = (count_m1 << 6) | 3; // TreeGrowthStage::Grown
                let density = grass_density(x, y, config.seed) & 3;
                let tree_m2 = density << 4; // TreeGround::Grass
                map.set_kind(c, TileKind::Forest)?;
                map.set_mapt_m5(c, 0x40, tree_m5)?;
                map.set_m2(c, tree_m2)?;
            } else {
                map.set_kind(c, TileKind::Grass)?;
                map.set_mapt_m5(c, 0, m5)?;
            }
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
                map.set_height(c, 0)?;
                map.set_kind(c, TileKind::Void)?;
                map.set_mapt_m5(c, 0x70, 0)?;
                map.set_m1(c, 0)?;
                map.set_m2(c, 0)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
