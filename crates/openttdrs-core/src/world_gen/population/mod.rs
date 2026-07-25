//! Colocación procedural de pueblos e industrias (`GenerateTowns` / `GenerateIndustries`).
//!
//! Conteos base `OpenTTD` escalados con [`scale_by_size`] (`Map::ScaleBySize` / `CeilDiv`).
//! Orden de genworld: towns → industries, tras el terreno.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

mod industries;
mod towns;

use crate::game_state::GameState;
use crate::map::{Map, TileCoord, TileKind, tile_slope_and_z};
use crate::world_gen::PreserveRect;

/// Pueblos en mapa 256×256: very low, low, normal, high (`town_cmd.cpp`).
pub const NUM_INITIAL_TOWNS: [u32; 4] = [5, 11, 23, 46];

/// Industrias en mapa 256×256: none…high (`industry_cmd.cpp` `GetNumberOfIndustries`).
/// Índices: funded-only, minimal, very low, low, normal, high.
pub const NUM_INITIAL_INDUSTRIES: [u32; 6] = [0, 0, 10, 25, 55, 80];

/// Densidad de pueblos (`difficulty.number_towns` 0..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TownDensity {
    VeryLow = 0,
    Low = 1,
    #[default]
    Normal = 2,
    High = 3,
}

impl TownDensity {
    #[must_use]
    pub const fn base_count(self) -> u32 {
        NUM_INITIAL_TOWNS[self as usize]
    }
}

/// Densidad de industrias (`IndustryDensity` sin Custom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IndustryDensity {
    FundedOnly = 0,
    Minimal = 1,
    VeryLow = 2,
    Low = 3,
    #[default]
    Normal = 4,
    High = 5,
}

impl IndustryDensity {
    #[must_use]
    pub const fn base_count(self) -> u32 {
        NUM_INITIAL_INDUSTRIES[self as usize]
    }
}

/// Parámetros de población post-terreno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopulationGenConfig {
    pub town_density: TownDensity,
    pub industry_density: IndustryDensity,
    pub seed: u64,
}

impl Default for PopulationGenConfig {
    fn default() -> Self {
        Self {
            town_density: TownDensity::Normal,
            industry_density: IndustryDensity::Normal,
            seed: 0,
        }
    }
}

/// `CeilDiv(a, b)` de `OpenTTD` (`math_func.hpp`).
#[must_use]
pub const fn ceil_div(a: u32, b: u32) -> u32 {
    if b == 0 {
        return 0;
    }
    a.saturating_add(b - 1) / b
}

/// `Map::ScaleBySize`: valor pensado para 256×256, escalado por log2 del mapa.
///
/// `CeilDiv(n << (LogX + LogY - 12), 1 << 4)`.
#[must_use]
pub fn scale_by_size(n: u32, map_w: u32, map_h: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let log_x = map_w.max(1).ilog2();
    let log_y = map_h.max(1).ilog2();
    let shift = log_x.saturating_add(log_y).saturating_sub(12);
    ceil_div(n << shift, 1 << 4)
}

/// Objetivo de pueblos según densidad y tamaño de mapa.
#[must_use]
pub fn town_target_count(density: TownDensity, map_w: u32, map_h: u32) -> usize {
    let scaled = scale_by_size(density.base_count(), map_w, map_h).max(1);
    usize::try_from(scaled).unwrap_or(1)
}

/// Objetivo de industrias según densidad y tamaño de mapa.
#[must_use]
pub fn industry_target_count(density: IndustryDensity, map_w: u32, map_h: u32) -> usize {
    let scaled = scale_by_size(density.base_count(), map_w, map_h);
    usize::try_from(scaled).unwrap_or(0)
}

fn resolve_population_seed(state: &GameState, cfg_seed: u64) -> u64 {
    if cfg_seed != 0 {
        return cfg_seed;
    }
    if state.world_seed != 0 {
        return state.world_seed;
    }
    let (mw, mh) = state.map.dimensions();
    u64::from(mw)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(mh).wrapping_mul(0x6C62_272E_07BB_0142))
        .wrapping_add(0x5055_4C41_5449_4F4E)
}

/// `GenerateTowns`: coloca pueblos según densidad y `ScaleBySize`.
///
/// Devuelve el número de pueblos creados en esta llamada.
pub fn generate_towns(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
) -> usize {
    let (mw, mh) = state.map.dimensions();
    let mut rng = SeededRng::new(resolve_population_seed(state, cfg.seed));
    let mut town_centers = Vec::new();
    let mut ctx = PopCtx {
        state,
        preserve,
        rng: &mut rng,
        mw,
        mh,
    };
    towns::place_towns(
        &mut ctx,
        town_target_count(cfg.town_density, mw, mh),
        &mut town_centers,
    )
}

/// `GenerateIndustries`: coloca industrias según densidad y `ScaleBySize`.
///
/// Usa las posiciones de pueblos ya presentes en `state.towns` para separación.
pub fn generate_industries(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
) -> usize {
    let (mw, mh) = state.map.dimensions();
    // Desplazamos el RNG respecto a towns para no repetir la misma secuencia.
    let seed = resolve_population_seed(state, cfg.seed).wrapping_add(0x494E_4453_5452);
    let mut rng = SeededRng::new(seed);
    let town_centers: Vec<TileCoord> = state.towns.iter().map(|t| t.pos).collect();
    let mut ctx = PopCtx {
        state,
        preserve,
        rng: &mut rng,
        mw,
        mh,
    };
    industries::place_industries(
        &mut ctx,
        industry_target_count(cfg.industry_density, mw, mh),
        &town_centers,
    )
}

/// Genera pueblos e industrias sobre un mapa ya generado (orden genworld).
pub fn apply_population_gen(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
) {
    let (mw, mh) = state.map.dimensions();
    let seed = resolve_population_seed(state, cfg.seed);
    let mut rng = SeededRng::new(seed);
    let mut town_centers = Vec::new();
    let mut ctx = PopCtx {
        state,
        preserve,
        rng: &mut rng,
        mw,
        mh,
    };
    let _ = towns::place_towns(
        &mut ctx,
        town_target_count(cfg.town_density, mw, mh),
        &mut town_centers,
    );
    let _ = industries::place_industries(
        &mut ctx,
        industry_target_count(cfg.industry_density, mw, mh),
        &town_centers,
    );
}

/// Variación de estilo dentro de un mismo pueblo (índice en la tabla 1×1).
pub(crate) const PROCEDURAL_HOUSE_STYLE_SPREAD: u32 = 16;

/// IDs de casa 1×1 con población > 0 (evita piezas multi-tile y decoración vacía).
pub(crate) fn procedural_house_choices() -> &'static [u16] {
    use std::sync::OnceLock;
    static CHOICES: OnceLock<Vec<u16>> = OnceLock::new();
    CHOICES.get_or_init(|| {
        (0u16..110)
            .filter(|&id| {
                crate::sav::house_spec_is_size_1x1(id) && crate::sav::house_spec_population(id) > 0
            })
            .collect()
    })
}

/// Generador determinista para colocación (mismo seed → mismo mundo).
pub(crate) struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub(crate) fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (self.state >> 32) as u32
    }

    pub(crate) fn next_range(&mut self, max_exclusive: u32) -> u32 {
        if max_exclusive <= 1 {
            return 0;
        }
        self.next_u32() % max_exclusive
    }
}

pub(crate) fn in_preserve(preserve: &[PreserveRect], x: i32, y: i32) -> bool {
    preserve.iter().any(|r| r.contains(x, y))
}

pub(crate) fn tile_ok_for_house(
    state: &GameState,
    c: TileCoord,
    preserve: &[PreserveRect],
) -> bool {
    if in_preserve(preserve, c.x, c.y) {
        return false;
    }
    tile_is_flat_grass(&state.map, c)
}

pub(crate) fn tile_is_flat_grass(map: &Map, c: TileCoord) -> bool {
    if map.get_kind(c) != Some(TileKind::Grass) {
        return false;
    }
    tile_slope_and_z(map, c).is_some_and(|(tileh, _)| tileh == 0)
}

pub(crate) fn min_distance_sq(a: TileCoord, b: TileCoord) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

pub(crate) struct PopCtx<'a> {
    pub(crate) state: &'a mut GameState,
    pub(crate) preserve: &'a [PreserveRect],
    pub(crate) rng: &'a mut SeededRng,
    pub(crate) mw: u32,
    pub(crate) mh: u32,
}

/// ¿Todas las calles están en terreno plano? (tests / auditoría).
#[must_use]
pub fn road_tiles_are_flat(map: &Map, roads: &[TileCoord]) -> bool {
    roads.iter().all(|&c| {
        map.get_kind(c) == Some(TileKind::Road)
            && tile_slope_and_z(map, c).is_some_and(|(tileh, _)| tileh == 0)
    })
}

/// ¿Hay carretera ortogonal adyacente a la casa?
#[must_use]
pub fn house_beside_road(map: &Map, house: TileCoord) -> bool {
    for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
        let n = TileCoord::new(house.x + dx, house.y + dy);
        if map.get_kind(n) == Some(TileKind::Road) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_gen::{Climate, WorldGenConfig, apply_world_gen};

    #[test]
    fn ceil_div_matches_openttd() {
        assert_eq!(ceil_div(5, 16), 1);
        assert_eq!(ceil_div(16, 16), 1);
        assert_eq!(ceil_div(17, 16), 2);
        assert_eq!(ceil_div(0, 16), 0);
    }

    #[test]
    fn scale_by_size_identity_on_256() {
        assert_eq!(scale_by_size(5, 256, 256), 5);
        assert_eq!(scale_by_size(11, 256, 256), 11);
        assert_eq!(scale_by_size(23, 256, 256), 23);
        assert_eq!(scale_by_size(46, 256, 256), 46);
        assert_eq!(scale_by_size(55, 256, 256), 55);
        assert_eq!(scale_by_size(80, 256, 256), 80);
        assert_eq!(scale_by_size(0, 256, 256), 0);
    }

    #[test]
    fn scale_by_size_shrinks_on_64() {
        // LogX+LogY-12 = 0 → CeilDiv(n, 16)
        assert_eq!(scale_by_size(5, 64, 64), 1);
        assert_eq!(scale_by_size(46, 64, 64), 3);
        assert_eq!(scale_by_size(80, 64, 64), 5);
    }

    #[test]
    fn town_targets_follow_density_table() {
        assert_eq!(town_target_count(TownDensity::VeryLow, 256, 256), 5);
        assert_eq!(town_target_count(TownDensity::Low, 256, 256), 11);
        assert_eq!(town_target_count(TownDensity::Normal, 256, 256), 23);
        assert_eq!(town_target_count(TownDensity::High, 256, 256), 46);
        assert!(
            town_target_count(TownDensity::High, 64, 64)
                > town_target_count(TownDensity::VeryLow, 64, 64)
        );
    }

    #[test]
    fn industry_targets_follow_density_table() {
        assert_eq!(
            industry_target_count(IndustryDensity::FundedOnly, 256, 256),
            0
        );
        assert_eq!(industry_target_count(IndustryDensity::Minimal, 256, 256), 0);
        assert_eq!(
            industry_target_count(IndustryDensity::VeryLow, 256, 256),
            10
        );
        assert_eq!(industry_target_count(IndustryDensity::Low, 256, 256), 25);
        assert_eq!(industry_target_count(IndustryDensity::Normal, 256, 256), 55);
        assert_eq!(industry_target_count(IndustryDensity::High, 256, 256), 80);
    }

    fn gen_populated(seed: u64, towns: TownDensity, industries: IndustryDensity) -> GameState {
        let mut state = GameState::new(64, 64);
        state.climate = Climate::Temperate;
        state.world_seed = seed;
        apply_world_gen(
            &mut state.map,
            &WorldGenConfig {
                climate: Climate::Temperate,
                seed,
                island: true,
                ..WorldGenConfig::default()
            },
            &[],
        )
        .expect("terrain");
        apply_population_gen(
            &mut state,
            &PopulationGenConfig {
                town_density: towns,
                industry_density: industries,
                seed,
            },
            &[],
        );
        state
    }

    #[test]
    fn population_gen_is_deterministic_for_seed() {
        let a = gen_populated(4242, TownDensity::Normal, IndustryDensity::Normal);
        let b = gen_populated(4242, TownDensity::Normal, IndustryDensity::Normal);
        assert_eq!(a.towns.len(), b.towns.len());
        assert_eq!(a.industries.len(), b.industries.len());
        for (ta, tb) in a.towns.iter().zip(b.towns.iter()) {
            assert_eq!(ta.pos, tb.pos);
            assert_eq!(ta.name, tb.name);
        }
    }

    #[test]
    fn denser_settings_place_at_least_as_many() {
        let sparse = gen_populated(99, TownDensity::VeryLow, IndustryDensity::VeryLow);
        let dense = gen_populated(99, TownDensity::High, IndustryDensity::High);
        assert!(dense.towns.len() >= sparse.towns.len());
        assert!(dense.industries.len() >= sparse.industries.len());
    }

    #[test]
    fn population_gen_places_some_content() {
        let state = gen_populated(12345, TownDensity::High, IndustryDensity::High);
        assert!(!state.towns.is_empty(), "expected towns");
        assert!(!state.industries.is_empty(), "expected industries");
    }

    #[test]
    fn normal_density_places_content_on_typical_island_seed() {
        // Seed usado por el bootstrap del cliente (`procedural_island(..., 42)`).
        let state = gen_populated(42, TownDensity::Normal, IndustryDensity::Normal);
        assert!(!state.towns.is_empty(), "expected towns for seed 42");
        assert!(
            !state.industries.is_empty(),
            "expected industries for seed 42"
        );
    }
}
