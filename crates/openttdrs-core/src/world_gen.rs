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

use crate::map::{Map, MapError, TileCoord, TileKind};
use crate::tile_slope_and_z;

/// Clima del mundo (`LandscapeType` en `OpenTTD`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Climate {
    #[default]
    Temperate,
    SubArctic,
    SubTropical,
    Toyland,
}

impl Climate {
    /// Parsea nombres usados en `OPENTTDRS_CLIMATE` y saves JSON.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "temperate" | "temp" | "temperate_landscape" => Some(Self::Temperate),
            "arctic" | "sub_arctic" | "subarctic" | "snow" => Some(Self::SubArctic),
            "tropic" | "sub_tropical" | "subtropical" | "desert" => Some(Self::SubTropical),
            "toyland" | "toy" => Some(Self::Toyland),
            _ => None,
        }
    }

    #[must_use]
    pub const fn uses_snow_ground(self) -> bool {
        matches!(self, Self::SubArctic)
    }

    #[must_use]
    pub const fn uses_desert_patches(self) -> bool {
        matches!(self, Self::SubTropical)
    }
}

/// Subtipo de suelo en teselas `MP_CLEAR` (bits 2–4 de `m5`).
pub const CLEAR_GROUND_GRASS: u8 = 0;
pub const CLEAR_GROUND_ROUGH: u8 = 1;
pub const CLEAR_GROUND_ROCKY: u8 = 2;
pub const CLEAR_GROUND_SNOW: u8 = 4;
pub const CLEAR_GROUND_DESERT: u8 = 5;

/// Empaqueta `ClearGround` + densidad de hierba en `m5`.
#[must_use]
pub const fn clear_ground_m5(ground: u8, density: u8) -> u8 {
    ((ground & 7) << 2) | (density & 3)
}

/// Resuelve el suelo visible según clima y datos de tesela (para render / gen).
#[must_use]
pub fn effective_clear_ground(climate: Climate, tile_m5: u8, tx: i32, ty: i32, seed: u64) -> u8 {
    let explicit = (tile_m5 >> 2) & 0x7;
    if explicit != CLEAR_GROUND_GRASS {
        return explicit;
    }
    match climate {
        Climate::SubArctic => CLEAR_GROUND_SNOW,
        Climate::SubTropical if desert_patch(tx, ty, seed) => CLEAR_GROUND_DESERT,
        Climate::Temperate | Climate::SubTropical => CLEAR_GROUND_GRASS,
        Climate::Toyland => CLEAR_GROUND_ROUGH,
    }
}

/// Parámetros de generación procedural.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldGenConfig {
    pub climate: Climate,
    pub seed: u64,
    /// Altura máxima de agua (`GetTileZ` ≤ `sea_level` → `MP_WATER`).
    pub sea_level: u8,
    /// Bordes del mapa más bajos → costas / isla jugable.
    pub island: bool,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            climate: Climate::Temperate,
            seed: 0,
            sea_level: 1,
            island: false,
        }
    }
}

/// Rectángulo inclusivo `(x0, y0, x1, y1)` que no se modifica (zonas demo del cliente).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreserveRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl PreserveRect {
    #[must_use]
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}

/// Genera colinas, lagos y bosques sobre un mapa ya inicializado.
///
/// Las teselas dentro de `preserve` conservan tipo y altura actuales.
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

    let corners_w = map_w + 1;
    let corners_h = map_h + 1;
    let mut corners = vec![0f32; (corners_w * corners_h) as usize];

    for cy in 0..corners_h {
        for cx in 0..corners_w {
            let mut n = layered_noise(cx, cy, config.seed);
            if config.island {
                n *= island_falloff(cx, cy, map_w, map_h);
            }
            let lake = lake_depression(cx, cy, config.seed);
            if lake > 0.0 {
                n *= 1.0 - lake;
            }
            corners[(cy * corners_w + cx) as usize] = n.clamp(0.0, 1.0);
        }
    }

    for _ in 0..4 {
        smooth_corners(&mut corners, corners_w, corners_h);
    }

    for y in 0..map_h {
        for x in 0..map_w {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let h = corner_height_from_grid(&corners, corners_w, x, y, config.sea_level);
            let c = TileCoord::new(x, y);
            map.set_height(c, h)?;
        }
    }

    for y in 0..map_h {
        for x in 0..map_w {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            let Some((_, z)) = tile_slope_and_z(map, c) else {
                continue;
            };
            if z <= config.sea_level {
                map.set_kind(c, TileKind::Water)?;
                map.set_mapt_m5(c, 0, 0)?;
                continue;
            }
            let ground = effective_clear_ground(config.climate, 0, x, y, config.seed);
            let m5 = clear_ground_m5(ground, grass_density(x, y, config.seed));
            if forest_patch(x, y, config.seed, config.climate) {
                map.set_kind(c, TileKind::Forest)?;
            } else {
                map.set_kind(c, TileKind::Grass)?;
            }
            map.set_mapt_m5(c, 0, m5)?;
        }
    }

    mark_water_coasts(map, map_w, map_h, config.sea_level, preserve);
    Ok(())
}

fn mark_water_coasts(map: &mut Map, mw: i32, mh: i32, _sea_level: u8, preserve: &[PreserveRect]) {
    const WATER_COAST_M5: u8 = 0x10;
    for y in 0..mh {
        for x in 0..mw {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            if map.get_kind(c) != Some(TileKind::Water) {
                continue;
            }
            let adjacent_land = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| {
                    let nc = TileCoord::new(x + dx, y + dy);
                    map.get_kind(nc)
                        .is_some_and(|k| k != TileKind::Water && k != TileKind::Void)
                });
            if adjacent_land {
                let _ = map.set_mapt_m5(c, 0, WATER_COAST_M5);
            }
        }
    }
}

fn corner_height_from_grid(corners: &[f32], corners_w: i32, x: i32, y: i32, sea_level: u8) -> u8 {
    let idx = (y * corners_w + x) as usize;
    let n = corners.get(idx).copied().unwrap_or(0.5);
    // `n`≈0 → nivel del mar / lagos; `n`≈1 → colinas.
    let base = f32::from(sea_level) + n * 6.0;
    base.round().clamp(0.0, 15.0) as u8
}

/// Ruido grueso que marca cuencas de lagos interiores (0 = sin lago, 1 = depresión máxima).
fn lake_depression(cx: i32, cy: i32, seed: u64) -> f32 {
    const LAKE_SEED: u64 = 0xA11C_E000;
    const THRESHOLD: f32 = 0.52;
    let n = value_noise(cx / 6, cy / 6, seed.wrapping_add(LAKE_SEED));
    if n <= THRESHOLD {
        return 0.0;
    }
    ((n - THRESHOLD) / (1.0 - THRESHOLD)).min(1.0)
}

fn smooth_corners(corners: &mut [f32], w: i32, h: i32) {
    let mut next = corners.to_vec();
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            let mut count = 0.0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && ny >= 0 && nx < w && ny < h {
                        sum += corners[(ny * w + nx) as usize];
                        count += 1.0;
                    }
                }
            }
            next[(y * w + x) as usize] = sum / count;
        }
    }
    corners.copy_from_slice(&next);
}

fn layered_noise(x: i32, y: i32, seed: u64) -> f32 {
    let n0 = value_noise(x, y, seed);
    let n1 = value_noise(x / 2, y / 2, seed.wrapping_add(1));
    let n2 = value_noise(x / 4, y / 4, seed.wrapping_add(2));
    (n0 * 0.5 + n1 * 0.35 + n2 * 0.15).clamp(0.0, 1.0)
}

fn island_falloff(x: i32, y: i32, map_w: i32, map_h: i32) -> f32 {
    if map_w <= 1 || map_h <= 1 {
        return 1.0;
    }
    let fx = x as f32 / map_w as f32;
    let fy = y as f32 / map_h as f32;
    let edge = (fx - 0.5).abs().max((fy - 0.5).abs()) * 2.0;
    (1.0 - edge.powf(1.4)).clamp(0.0, 1.0)
}

fn value_noise(x: i32, y: i32, seed: u64) -> f32 {
    let h = hash_u64(seed.wrapping_add(i64_pair_hash(x, y)));
    (h % 10_000) as f32 / 10_000.0
}

fn hash_u64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn i64_pair_hash(x: i32, y: i32) -> u64 {
    u64::from(x as u32)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(y as u32).wrapping_mul(0x6C62_272E_07BB_0142))
}

fn grass_density(x: i32, y: i32, seed: u64) -> u8 {
    let n = hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(3), y.wrapping_mul(5))));
    // Variación suave: mayoría hierba completa; sin `bare` (m5==0) para no confundir con default.
    match n % 10 {
        0..=1 => 1,
        2..=4 => 2,
        _ => 3,
    }
}

fn forest_patch(x: i32, y: i32, seed: u64, climate: Climate) -> bool {
    if !matches!(climate, Climate::Temperate | Climate::SubArctic) {
        return false;
    }
    hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(7), y.wrapping_mul(11)))) % 9 == 0
}

fn desert_patch(x: i32, y: i32, seed: u64) -> bool {
    hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(13), y.wrapping_mul(17)))) % 5 == 0
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
    fn island_mode_increases_water_on_edges() {
        let mut flat = Map::new_flat(24, 18, 2);
        let mut island = Map::new_flat(24, 18, 2);
        let base_cfg = WorldGenConfig {
            seed: 7,
            island: false,
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
    fn arctic_effective_clear_ground_is_snow() {
        assert_eq!(
            effective_clear_ground(Climate::SubArctic, 0, 0, 0, 0),
            CLEAR_GROUND_SNOW
        );
    }
}
