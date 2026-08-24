//! Thin wrapper: densidad del menú → `apply_population_gen` en core (P3.1).

#[cfg(test)]
mod tests;

use openttdrs_core::prelude::*;
use openttdrs_core::{
    PopulationGenConfig, PreserveRect, WorldGenRng, apply_population_gen,
    apply_population_gen_with_rng,
};

use super::world::NewGameSettings;

/// El demo compacto ya coloca pueblo e industrias en `gameplay_showcase`.
#[must_use]
pub(crate) fn should_populate_procedurally(settings: &NewGameSettings) -> bool {
    let s = settings.sanitized();
    !(s.preserve_demo && s.map_size.is_compact())
}

pub(crate) fn populate_procedural_world(
    state: &mut GameState,
    settings: &NewGameSettings,
    preserve: &[PreserveRect],
    mut generation_rng: Option<&mut WorldGenRng>,
) {
    let settings = settings.sanitized();
    let (mw, mh) = state.map.dimensions();
    let seed = procedural_seed(state.world_seed, settings.seed, mw, mh);
    let config = PopulationGenConfig {
        town_density: settings.town_density.to_town_density(),
        industry_density: settings.industry_density.to_industry_density(),
        seed,
    };
    if let Some(rng) = generation_rng.as_mut() {
        apply_population_gen_with_rng(state, &config, preserve, rng);
    } else {
        apply_population_gen(state, &config, preserve);
    }
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
