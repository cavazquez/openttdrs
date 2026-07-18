//! Helpers compartidos entre benches headless (#116).
#![allow(dead_code)] // cada [[bench]] incluye el módulo completo
#![allow(clippy::expect_used)] // fixtures de bench: fallo = setup inválido

use openttdrs_core::parity::build_scenario;
use openttdrs_core::{Climate, GameState, WorldGenConfig, apply_world_gen};

/// Escenario parity o panic con mensaje claro (fixture de bench, no runtime).
#[must_use]
pub fn scenario(name: &str) -> GameState {
    build_scenario(name).unwrap_or_else(|| panic!("escenario parity desconocido: {name}"))
}

/// Mapa grande procedural (256×256) sin flota — mide coste de tick sobre terreno.
#[must_use]
pub fn large_world_gen_map() -> GameState {
    large_world_gen_map_sized(256)
}

/// Mapa procedural cuadrado `side×side` sin flota (seed 116, temperate).
#[must_use]
pub fn large_world_gen_map_sized(side: u32) -> GameState {
    let mut state = GameState::new(side, side);
    let cfg = WorldGenConfig {
        climate: Climate::Temperate,
        seed: 116,
        sea_level: 1,
        island: false,
        height_span: 6,
    };
    apply_world_gen(&mut state.map, &cfg, &[]).unwrap_or_else(|e| {
        panic!("world_gen bench {side}×{side}: {e:?}");
    });
    state.world_seed = cfg.seed;
    state.climate = cfg.climate;
    state
}

/// Avanza exactamente `n` ticks.
pub fn step_n(state: &mut GameState, n: u32) {
    for _ in 0..n {
        state.step();
    }
}
