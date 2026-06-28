//! Pueblos e industrias en mapas procedurales (≥64²), fuera del demo 24×18.

mod industries;
mod towns;

#[cfg(test)]
mod tests;

use openttdrs_core::{GameState, PreserveRect, TileCoord, TileKind, tile_slope_and_z};

use super::world::{MapSizePreset, NewGameSettings, PopulationDensity};

/// HouseID originales OpenTTD (0..=109); evitamos NewGRF.
pub(super) const PROCEDURAL_HOUSE_ID_MAX: u32 = 110;
/// Variación de estilo dentro de un mismo pueblo.
pub(super) const PROCEDURAL_HOUSE_STYLE_SPREAD: u32 = 16;

/// Generador determinista para colocación (mismo seed → mismo mundo).
pub(super) struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub(super) fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub(super) fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    pub(super) fn next_range(&mut self, max_exclusive: u32) -> u32 {
        if max_exclusive <= 1 {
            return 0;
        }
        self.next_u32() % max_exclusive
    }
}

/// El demo compacto ya coloca pueblo e industrias en `gameplay_showcase`.
#[must_use]
pub(crate) fn should_populate_procedurally(settings: &NewGameSettings) -> bool {
    let s = settings.sanitized();
    !(s.preserve_demo && s.map_size == MapSizePreset::Compact)
}

pub(crate) fn populate_procedural_world(
    state: &mut GameState,
    settings: &NewGameSettings,
    preserve: &[PreserveRect],
) {
    let settings = settings.sanitized();
    let (mw, mh) = state.map.dimensions();
    let area = u64::from(mw).saturating_mul(u64::from(mh));
    let seed = procedural_seed(state.world_seed, settings.seed, mw, mh);
    let mut rng = SeededRng::new(seed);

    let town_target = scaled_population_count((area / 512).clamp(4, 40), settings.town_density, 2);
    let industry_target =
        scaled_population_count((area / 768).clamp(6, 48), settings.industry_density, 3);

    let mut town_centers: Vec<TileCoord> = Vec::with_capacity(town_target);
    let mut ctx = PopCtx {
        state,
        preserve,
        rng: &mut rng,
        mw,
        mh,
    };
    towns::place_towns(&mut ctx, town_target, &mut town_centers);
    industries::place_industries(&mut ctx, settings.climate, industry_target, &town_centers);
}

fn scaled_population_count(base: u64, density: PopulationDensity, min_count: u64) -> usize {
    let scaled = base
        .saturating_mul(u64::from(density.multiplier_bps()))
        .saturating_div(100);
    usize::try_from(scaled.max(min_count)).unwrap_or(min_count as usize)
}

fn procedural_seed(world_seed: u64, settings_seed: u64, mw: u32, mh: u32) -> u64 {
    if world_seed != 0 {
        return world_seed;
    }
    if settings_seed != 0 {
        return settings_seed;
    }
    u64::from(mw)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(mh).wrapping_mul(0x6C62_272E_07BB_0142))
        .wrapping_add(0x5055_4C41_5449_4F4E)
}

pub(super) fn in_preserve(preserve: &[PreserveRect], x: i32, y: i32) -> bool {
    preserve.iter().any(|r| r.contains(x, y))
}

pub(super) fn tile_ok_for_house(
    state: &GameState,
    c: TileCoord,
    preserve: &[PreserveRect],
) -> bool {
    if in_preserve(preserve, c.x, c.y) {
        return false;
    }
    tile_is_flat_grass(&state.map, c)
}

pub(super) fn tile_is_flat_grass(map: &openttdrs_core::Map, c: TileCoord) -> bool {
    if map.get_kind(c) != Some(TileKind::Grass) {
        return false;
    }
    tile_slope_and_z(map, c).is_some_and(|(tileh, _)| tileh == 0)
}

pub(super) fn min_distance_sq(a: TileCoord, b: TileCoord) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

pub(super) struct PopCtx<'a> {
    pub(super) state: &'a mut GameState,
    pub(super) preserve: &'a [PreserveRect],
    pub(super) rng: &'a mut SeededRng,
    pub(super) mw: u32,
    pub(super) mh: u32,
}
