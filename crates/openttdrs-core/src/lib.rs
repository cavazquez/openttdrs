//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod map;
pub mod tick;

pub use map::{Map, MapError, Tile, TileCoord};
pub use tick::GameTick;

/// Estado global mínimo del mundo simulado.
#[derive(Debug, Clone)]
pub struct GameState {
    pub map: Map,
    pub tick: GameTick,
}

impl GameState {
    #[must_use]
    pub fn new(map_width: u32, map_height: u32) -> Self {
        Self {
            map: Map::new_flat(map_width, map_height, 1),
            tick: GameTick::default(),
        }
    }

    /// Avanza un tick de simulación (equivalente conceptual a un frame lógico del juego).
    pub fn step(&mut self) {
        self.tick.advance();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_map_has_expected_dimensions() {
        let s = GameState::new(8, 8);
        assert_eq!(s.map.dimensions(), (8, 8));
    }

    #[test]
    fn step_increments_tick() {
        let mut s = GameState::new(4, 4);
        assert_eq!(s.tick.get(), 0);
        s.step();
        assert_eq!(s.tick.get(), 1);
        s.step();
        assert_eq!(s.tick.get(), 2);
    }

    #[test]
    fn tile_height_roundtrip() {
        let mut s = GameState::new(3, 3);
        let c = TileCoord::new(1, 1);
        s.map.set_height(c, 5).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 5);
    }
}
