//! Arranque del mapa procedural con opciones de nueva partida.

use openttdrs_core::{Climate, GameState, WorldGenConfig, apply_world_gen};

use super::demo_layout::{
    apply_optional_world_gen, demo_preserve_rects, fill_flat_grass, place_bridge_demo_gap,
    place_clean_demo_transport, place_demo_economy_loop,
};
use super::gameplay_showcase::place_gameplay_showcase;
use super::terrain::place_tunnel_demo_ridge;
use crate::state::{MAP_H, MAP_W};

/// Opciones de «Nueva partida» (menú principal o tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGameSettings {
    pub climate: Climate,
    pub world_gen: bool,
    pub island: bool,
    /// Conserva carretera/vía/puente demo al generar terreno.
    pub preserve_demo: bool,
    pub seed: u64,
}

impl Default for NewGameSettings {
    fn default() -> Self {
        Self {
            climate: Climate::Temperate,
            world_gen: false,
            island: false,
            preserve_demo: true,
            seed: 0,
        }
    }
}

impl NewGameSettings {
    /// Partida con isla procedural completa (sin reservar zonas demo).
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn procedural_island(climate: Climate, seed: u64) -> Self {
        Self {
            climate,
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed,
        }
    }
}

/// Mapa demo jugable: hierba plana o terreno procedural + transporte/industrias.
pub(crate) fn build_procedural_demo_world(settings: &NewGameSettings) -> GameState {
    let mut state = GameState::new(MAP_W, MAP_H);
    state.climate = settings.climate;
    fill_flat_grass(&mut state);
    if settings.world_gen {
        let seed = if settings.seed == 0 {
            0xDEAD_BEEF_u64
        } else {
            settings.seed
        };
        state.world_seed = seed;
        let preserve = if settings.preserve_demo {
            demo_preserve_rects()
        } else {
            Vec::new()
        };
        apply_optional_world_gen(
            &mut state,
            WorldGenConfig {
                climate: settings.climate,
                seed,
                sea_level: 1,
                island: settings.island,
            },
            &preserve,
        );
    }
    if settings.preserve_demo {
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);
        place_gameplay_showcase(&mut state);
        place_tunnel_demo_ridge(&mut state);
        place_bridge_demo_gap(&mut state);
    }
    state
}

/// Genera un mapa vacío con solo terreno (sin demo de transporte).
#[allow(dead_code)]
pub(crate) fn build_empty_procedural_world(
    width: u32,
    height: u32,
    settings: &NewGameSettings,
) -> GameState {
    let mut state = GameState::new(width, height);
    state.climate = settings.climate;
    fill_flat_grass(&mut state);
    if settings.world_gen {
        let seed = if settings.seed == 0 {
            0xCAFE_BABE_u64
        } else {
            settings.seed
        };
        state.world_seed = seed;
        let _ = apply_world_gen(
            &mut state.map,
            &WorldGenConfig {
                climate: settings.climate,
                seed,
                sea_level: 1,
                island: settings.island,
            },
            &[],
        );
    }
    state
}
