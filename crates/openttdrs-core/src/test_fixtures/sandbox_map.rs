//! Mapas planos de sandbox para tests (#151).

use crate::{GameState, Map};

/// Dinero típico de sandbox en tests de comandos caros.
pub const SANDBOX_MONEY: i64 = 1_000_000;

/// Builder concreto: mapa plano (+ opcionalmente dinero de sandbox).
///
/// Sin infra, vehículos ni estaciones: solo setup común de terreno/economía.
pub struct SandboxMap;

impl SandboxMap {
    /// `GameState` vacío sobre mapa plano a `level` (Grass).
    #[must_use]
    pub fn flat(width: u32, height: u32, level: u8) -> GameState {
        GameState::from_map(Map::new_flat(width, height, level))
    }

    /// Como [`Self::flat`] con [`SANDBOX_MONEY`].
    #[must_use]
    pub fn flat_rich(width: u32, height: u32, level: u8) -> GameState {
        let mut state = Self::flat(width, height, level);
        state.economy.money = SANDBOX_MONEY;
        state
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::TileCoord;

    #[test]
    fn flat_sets_uniform_height_and_default_money() {
        let state = SandboxMap::flat(4, 3, 7);
        assert_eq!(state.map.tiles().len(), 12);
        assert_eq!(state.map.get(TileCoord::new(0, 0)).unwrap().height, 7);
        assert_eq!(state.map.get(TileCoord::new(3, 2)).unwrap().height, 7);
        assert!(state.map.get(TileCoord::new(4, 0)).is_none());
        assert!(state.map.get(TileCoord::new(0, 3)).is_none());
        assert_eq!(state.economy.money, 100_000);
    }

    #[test]
    fn flat_rich_sets_sandbox_money() {
        let state = SandboxMap::flat_rich(2, 2, 1);
        assert_eq!(state.economy.money, SANDBOX_MONEY);
        assert_eq!(state.map.get(TileCoord::new(1, 1)).unwrap().height, 1);
    }
}
