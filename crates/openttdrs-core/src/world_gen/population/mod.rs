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

use crate::cargodist::parity::Randomizer;
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
    /// Margen de tierra nivelada alrededor de industrias (`construction.industry_platform`).
    ///
    /// `OpenTTD` usa `1` en partidas nuevas. El generador lo conserva de forma
    /// explícita porque afecta qué intentos force-one se admiten y cómo queda
    /// el mapa después de terraformar la plataforma.
    pub industry_platform: u8,
    pub seed: u64,
}

impl Default for PopulationGenConfig {
    fn default() -> Self {
        Self {
            town_density: TownDensity::Normal,
            industry_density: IndustryDensity::Normal,
            industry_platform: 1,
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

/// `Map::CountLandTiles` de `OpenTTD`, retenido al iniciar la partida.
///
/// Las costas (`WaterTileType::Coast`) cuentan como tierra: `IsWaterTile()`
/// sólo reconoce agua plana. Este detalle alimenta
/// `Map::ScaleByLandProportion` en `GenerateTowns`.
fn initial_land_count(map: &Map) -> u32 {
    let size = u32::try_from(map.tiles().len()).unwrap_or(u32::MAX);
    let land = map
        .tiles()
        .iter()
        .filter(|tile| !(tile.kind == TileKind::Water && (tile.m5 >> 4) == 0))
        .count();
    let land = u32::try_from(land).unwrap_or(size);
    land.saturating_add(land / 12).min(size)
}

/// Cantidad sugerida de `GenerateTowns` para una partida nueva.
///
/// Primero escala la dificultad a tamaño de mapa, suma los tres bits bajos de
/// un `Random()` y sólo entonces ajusta por el terreno inicial. El valor no es
/// un mínimo garantizado: `CreateRandomTown` puede fallar sus 20 intentos.
fn town_generation_target_count(density: TownDensity, map: &Map, rng: &mut Randomizer) -> usize {
    let (map_w, map_h) = map.dimensions();
    let suggested = scale_by_size(density.base_count(), map_w, map_h)
        .max(1)
        .saturating_add(rng.next() & 7);
    let size = u32::try_from(map.tiles().len()).unwrap_or(1).max(1);
    let scaled = u32::try_from(
        u64::from(suggested).saturating_mul(u64::from(initial_land_count(map))) / u64::from(size),
    )
    .unwrap_or(u32::MAX);
    usize::try_from(scaled.clamp(1, 64_000)).unwrap_or(64_000)
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
    let mut rng = Randomizer::new(resolve_population_seed(state, cfg.seed) as u32);
    generate_towns_with_rng(state, cfg, preserve, &mut rng)
}

/// `GenerateTowns` continuando el stream global de generación.
///
/// Esta frontera pública permite que el oráculo de mapas aleatorios exporte
/// exactamente el estado posterior a pueblos, sin volver a sembrar el RNG.
pub fn generate_towns_with_rng(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
    rng: &mut Randomizer,
) -> usize {
    let (mw, mh) = state.map.dimensions();
    let target = town_generation_target_count(cfg.town_density, &state.map, rng);
    let mut town_centers: Vec<TileCoord> = state.towns.iter().map(|town| town.pos).collect();
    let mut ctx = PopCtx {
        state,
        preserve,
        rng,
        mw,
        mh,
        industry_platform: cfg.industry_platform,
    };
    towns::place_towns(&mut ctx, target, &mut town_centers)
}

/// `GenerateIndustries`: coloca industrias según densidad y `ScaleBySize`.
///
/// Usa las posiciones de pueblos ya presentes en `state.towns` para separación.
pub fn generate_industries(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
) -> usize {
    // Desplazamos el RNG respecto a towns para no repetir la misma secuencia.
    let seed = resolve_population_seed(state, cfg.seed).wrapping_add(0x494E_4453_5452);
    let mut rng = Randomizer::new(seed as u32);
    generate_industries_with_rng(state, cfg, preserve, &mut rng)
}

/// `GenerateIndustries` continuando el stream global de generación.
///
/// Usa los pueblos que ya viven en `state`; no deriva un RNG independiente,
/// porque `OpenTTD` comparte el stream desde la creación del terreno.
pub fn generate_industries_with_rng(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
    rng: &mut Randomizer,
) -> usize {
    let (mw, mh) = state.map.dimensions();
    let town_centers: Vec<TileCoord> = state.towns.iter().map(|t| t.pos).collect();
    let mut ctx = PopCtx {
        state,
        preserve,
        rng,
        mw,
        mh,
        industry_platform: cfg.industry_platform,
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
    let seed = resolve_population_seed(state, cfg.seed);
    let mut rng = Randomizer::new(seed as u32);
    apply_population_gen_with_rng(state, cfg, preserve, &mut rng);
}

/// Genera pueblos, industrias y árboles continuando el stream global de
/// `OpenTTD` que dejó [`super::apply_world_gen_with_rng`].
pub fn apply_population_gen_with_rng(
    state: &mut GameState,
    cfg: &PopulationGenConfig,
    preserve: &[PreserveRect],
    rng: &mut Randomizer,
) {
    let _ = generate_towns_with_rng(state, cfg, preserve, rng);
    let _ = generate_industries_with_rng(state, cfg, preserve, rng);
    // OpenTTD's new-game order is terrain → towns → industries → objects →
    // trees. Objects must run before `GenerateTrees`, because the object
    // footprint replaces the clear tile and consumes the shared RNG stream.
    let climate = state.climate;
    super::objects::generate_objects_with_rng(state, climate, rng, preserve);
    super::trees::generate_trees_with_rng(&mut state.map, state.climate, rng, preserve);
}

pub(crate) fn in_preserve(preserve: &[PreserveRect], x: i32, y: i32) -> bool {
    preserve.iter().any(|r| r.contains(x, y))
}

pub(crate) struct PopCtx<'a> {
    pub(crate) state: &'a mut GameState,
    pub(crate) preserve: &'a [PreserveRect],
    pub(crate) rng: &'a mut Randomizer,
    pub(crate) mw: u32,
    pub(crate) mh: u32,
    pub(crate) industry_platform: u8,
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
    use crate::cargodist::parity::Randomizer;
    use crate::map::water_flood::make_shore_tile;
    use crate::map::{TileCoord, WaterClass, make_water_tile};
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
    fn initial_land_count_keeps_coasts_but_excludes_plain_water() {
        let mut map = Map::new_flat(64, 64, 0);
        for index in 0..1_000 {
            let c = TileCoord::new(index % 64, index / 64);
            make_water_tile(&mut map, c, WaterClass::Sea).expect("plain water");
        }
        make_shore_tile(&mut map, TileCoord::new(1, 0)).expect("shore");

        // 4096 - 999 water tiles, más la compensación de 1/12 de OpenTTD.
        assert_eq!(initial_land_count(&map), 3_097 + 3_097 / 12);
    }

    #[test]
    fn town_generation_target_replays_land_proportion_and_random_low_bits() {
        let mut map = Map::new_flat(64, 64, 0);
        // Fixture de la frontera `clear` de la seed 1330935378: 1121
        // teselas de agua plana y 278 costas, que `CountLandTiles` conserva.
        for index in 0..1_121 {
            let c = TileCoord::new(index % 64, index / 64);
            make_water_tile(&mut map, c, WaterClass::Sea).expect("plain water");
        }
        for index in 1_121..1_399 {
            let c = TileCoord::new(index % 64, index / 64);
            make_shore_tile(&mut map, c).expect("shore");
        }
        let mut rng = Randomizer {
            state: [1_168_016_413, 2_955_223_551],
        };

        assert_eq!(
            town_generation_target_count(TownDensity::Normal, &map, &mut rng),
            3
        );
        assert_eq!(rng.state, [1_189_259_021, 2_830_356_610]);
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
                ..PopulationGenConfig::default()
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
    fn denser_town_settings_place_at_least_as_many_towns() {
        let sparse = gen_populated(99, TownDensity::VeryLow, IndustryDensity::VeryLow);
        let dense = gen_populated(99, TownDensity::High, IndustryDensity::High);
        assert!(dense.towns.len() >= sparse.towns.len());
        // `GenerateIndustries` no garantiza que una densidad más alta termine
        // con más industrias: primero prueba las especies `force_one`, y un
        // mapa con más pueblos puede bloquear una plataforma o una huella que
        // el mapa disperso aceptaba. La monotonía verificable es el objetivo
        // de `industry_target_count`, cubierto por la tabla anterior; exigir
        // una cantidad colocada alteraría el contrato nativo de reintentos.
    }

    #[test]
    fn population_gen_places_some_content() {
        let state = gen_populated(12345, TownDensity::High, IndustryDensity::High);
        assert!(!state.towns.is_empty(), "expected towns");
        assert!(!state.industries.is_empty(), "expected industries");
        let (width, height) = state.map.dimensions();
        for y in 0..height {
            for x in 0..width {
                let coord = TileCoord::new(x as i32, y as i32);
                let Some(tile) = state.map.get(coord) else {
                    continue;
                };
                if tile.kind != TileKind::House {
                    continue;
                }
                let town_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
                assert!(
                    state.towns.iter().any(|town| town.id == town_id),
                    "house at {coord:?} must retain its town id"
                );
            }
        }
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
