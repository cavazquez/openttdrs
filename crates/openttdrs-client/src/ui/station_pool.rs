//! Multi-instancia mínima de Station View (#269 / #242).
//!
//! Hasta [`MAX_STATION_POOL_SLOTS`] estaciones pueden estar «abiertas» a la vez
//! (pool stub). La entidad Bevy Station sigue siendo singleton hoy: el slot
//! enfocado rebinda [`WindowKey::instance`] al índice de estación. Dual-entity
//! completo queda residual (#242).

use bevy::prelude::*;
use openttdrs_core::TileCoord;

use crate::ui::floating_window::{FloatingWindowId, WindowKey};

/// Máximo de Station View concurrentes en el pool (aceptación ≥2).
pub(crate) const MAX_STATION_POOL_SLOTS: usize = 2;

/// Registro de posiciones de estación por slot.
#[derive(Resource, Debug)]
pub(crate) struct StationPoolRegistry {
    pub(crate) slots: [Option<TileCoord>; MAX_STATION_POOL_SLOTS],
    pub(crate) focused: Option<TileCoord>,
}

impl Default for StationPoolRegistry {
    fn default() -> Self {
        Self {
            slots: [None; MAX_STATION_POOL_SLOTS],
            focused: None,
        }
    }
}

impl StationPoolRegistry {
    /// Abre o enfoca `station_pos`. Devuelve el slot asignado.
    pub(crate) fn open_or_focus(&mut self, station_pos: TileCoord) -> u8 {
        if let Some(slot) = self.slot_of(station_pos) {
            self.focused = Some(station_pos);
            return slot;
        }
        let slot = self
            .slots
            .iter()
            .position(Option::is_none)
            .or_else(|| {
                self.slots
                    .iter()
                    .position(|&p| p.is_some() && p != self.focused)
                    .or(Some(0))
            })
            .unwrap_or(0);
        self.slots[slot] = Some(station_pos);
        self.focused = Some(station_pos);
        slot as u8
    }

    #[must_use]
    pub(crate) fn slot_of(&self, station_pos: TileCoord) -> Option<u8> {
        self.slots
            .iter()
            .position(|&p| p == Some(station_pos))
            .map(|i| i as u8)
    }

    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn open_positions(&self) -> Vec<TileCoord> {
        self.slots.iter().flatten().copied().collect()
    }
}

/// Clave Station con `instance` = índice de estación en el save (o hash estable).
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn station_window_key(station_index: u32) -> WindowKey {
    WindowKey {
        class: FloatingWindowId::Station,
        instance: station_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_or_focus_keeps_two_stations() {
        let mut reg = StationPoolRegistry::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        assert_eq!(reg.open_or_focus(a), 0);
        assert_eq!(reg.open_or_focus(b), 1);
        assert_eq!(reg.open_positions(), vec![a, b]);
        assert_eq!(reg.focused, Some(b));
        assert_eq!(reg.open_or_focus(a), 0);
        assert_eq!(reg.focused, Some(a));
    }

    #[test]
    fn third_station_evicts_unfocused() {
        let mut reg = StationPoolRegistry::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        let c = TileCoord::new(3, 3);
        reg.open_or_focus(a);
        reg.open_or_focus(b);
        reg.open_or_focus(a);
        let slot = reg.open_or_focus(c);
        assert_eq!(slot, 1);
        assert!(reg.open_positions().contains(&a));
        assert!(reg.open_positions().contains(&c));
        assert!(!reg.open_positions().contains(&b));
    }

    #[test]
    fn station_window_key_uses_index_as_instance() {
        let key = station_window_key(7);
        assert_eq!(key.class, FloatingWindowId::Station);
        assert_eq!(key.instance, 7);
    }
}
