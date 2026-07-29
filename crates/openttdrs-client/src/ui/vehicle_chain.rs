//! Multi-instancia de la cadena Vehicle → hijas (#242 / #244).
//!
//! Hasta [`MAX_VEHICLE_CHAIN_SLOTS`] vehículos pueden tener View + subventanas
//! abiertas a la vez. Cada slot reutiliza entidades Bevy pre-spawneadas y
//! rebinda [`WindowKey::instance`] = `vehicle_id`.

use bevy::prelude::*;

use crate::ui::floating_window::{FloatingWindow, FloatingWindowId, WindowKey, WindowZCounter};

/// Máximo de cadenas Vehicle concurrentes (aceptación #242: ≥2).
pub(crate) const MAX_VEHICLE_CHAIN_SLOTS: usize = 2;

/// Marca la raíz (y widgets) de un slot de la cadena vehículo.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VehicleChainSlot(pub u8);

/// Registro de qué `vehicle_id` ocupa cada slot.
#[derive(Resource, Debug)]
pub(crate) struct VehicleChainRegistry {
    pub(crate) slots: [Option<u32>; MAX_VEHICLE_CHAIN_SLOTS],
    pub(crate) focused: Option<u32>,
}

impl Default for VehicleChainRegistry {
    fn default() -> Self {
        Self {
            slots: [None; MAX_VEHICLE_CHAIN_SLOTS],
            focused: None,
        }
    }
}

impl VehicleChainRegistry {
    /// Abre o enfoca `vehicle_id`. Devuelve el slot asignado.
    pub(crate) fn open_or_focus(&mut self, vehicle_id: u32) -> u8 {
        if let Some(slot) = self.slot_of(vehicle_id) {
            self.focused = Some(vehicle_id);
            return slot;
        }
        let slot = self
            .slots
            .iter()
            .position(Option::is_none)
            .or_else(|| {
                // Evictar el no enfocado; si ambos ocupados, el más antiguo (0).
                self.slots
                    .iter()
                    .position(|&id| id.is_some() && id != self.focused)
                    .or(Some(0))
            })
            .unwrap_or(0);
        self.slots[slot] = Some(vehicle_id);
        self.focused = Some(vehicle_id);
        slot as u8
    }

    #[must_use]
    pub(crate) fn slot_of(&self, vehicle_id: u32) -> Option<u8> {
        self.slots
            .iter()
            .position(|&id| id == Some(vehicle_id))
            .map(|i| i as u8)
    }

    #[must_use]
    pub(crate) fn vehicle_at(&self, slot: u8) -> Option<u32> {
        self.slots.get(slot as usize).copied().flatten()
    }

    pub(crate) fn close_vehicle(&mut self, vehicle_id: u32) {
        for slot in &mut self.slots {
            if *slot == Some(vehicle_id) {
                *slot = None;
            }
        }
        if self.focused == Some(vehicle_id) {
            self.focused = self.slots.iter().flatten().next().copied();
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub(crate) fn open_ids(&self) -> Vec<u32> {
        self.slots.iter().flatten().copied().collect()
    }
}

/// Clave de una clase de la cadena para un vehículo.
#[must_use]
pub(crate) fn vehicle_window_key(class: FloatingWindowId, vehicle_id: u32) -> WindowKey {
    WindowKey {
        class,
        instance: vehicle_id,
    }
}

/// Rebinda `key.instance` de las raíces del `slot` a `vehicle_id` (o 0 si None).
#[allow(dead_code)]
pub(crate) fn rebind_slot_keys(
    windows: &mut Query<(&VehicleChainSlot, &mut FloatingWindow)>,
    slot: u8,
    vehicle_id: Option<u32>,
) {
    let instance = vehicle_id.unwrap_or(0);
    for (chain_slot, mut win) in windows.iter_mut() {
        if chain_slot.0 != slot {
            continue;
        }
        if matches!(
            win.id,
            FloatingWindowId::Vehicle
                | FloatingWindowId::VehicleDetails
                | FloatingWindowId::Orders
                | FloatingWindowId::Timetable
                | FloatingWindowId::Refit
                | FloatingWindowId::DestinationPicker
        ) {
            win.key.instance = instance;
        }
    }
}

/// Muestra y eleva la ventana `class` del vehículo (si el slot está vivo).
#[allow(dead_code)]
pub(crate) fn raise_vehicle_class(
    windows: &mut Query<(
        &VehicleChainSlot,
        &FloatingWindow,
        &mut Visibility,
        &mut GlobalZIndex,
    )>,
    z_counter: &mut WindowZCounter,
    registry: &VehicleChainRegistry,
    class: FloatingWindowId,
    vehicle_id: u32,
) -> bool {
    let Some(slot) = registry.slot_of(vehicle_id) else {
        return false;
    };
    let key = vehicle_window_key(class, vehicle_id);
    for (chain_slot, win, mut vis, mut z) in windows.iter_mut() {
        if chain_slot.0 != slot || win.id != class {
            continue;
        }
        // key puede aún no estar rebindeada; aceptar class+slot.
        let _ = key;
        *vis = Visibility::Visible;
        z.0 = z_counter.bump();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_or_focus_keeps_two_vehicles() {
        let mut reg = VehicleChainRegistry::default();
        assert_eq!(reg.open_or_focus(10), 0);
        assert_eq!(reg.open_or_focus(20), 1);
        assert_eq!(reg.open_ids(), vec![10, 20]);
        assert_eq!(reg.focused, Some(20));
        // Reopen first → focus, no duplicate
        assert_eq!(reg.open_or_focus(10), 0);
        assert_eq!(reg.open_ids(), vec![10, 20]);
        assert_eq!(reg.focused, Some(10));
    }

    #[test]
    fn close_one_vehicle_keeps_the_other() {
        let mut reg = VehicleChainRegistry::default();
        reg.open_or_focus(1);
        reg.open_or_focus(2);
        reg.close_vehicle(1);
        assert_eq!(reg.open_ids(), vec![2]);
        assert_eq!(reg.focused, Some(2));
    }

    #[test]
    fn third_vehicle_evicts_unfocused_slot() {
        let mut reg = VehicleChainRegistry::default();
        reg.open_or_focus(1);
        reg.open_or_focus(2);
        reg.open_or_focus(1); // focus 1
        let slot = reg.open_or_focus(3);
        assert_eq!(slot, 1); // evicts 2
        assert!(reg.open_ids().contains(&1));
        assert!(reg.open_ids().contains(&3));
        assert!(!reg.open_ids().contains(&2));
    }

    #[test]
    fn vehicle_window_key_uses_vehicle_id_as_instance() {
        let key = vehicle_window_key(FloatingWindowId::Orders, 42);
        assert_eq!(key.class, FloatingWindowId::Orders);
        assert_eq!(key.instance, 42);
    }

    #[test]
    fn two_parent_chains_have_distinct_keys() {
        let a_view = vehicle_window_key(FloatingWindowId::Vehicle, 1);
        let a_orders = vehicle_window_key(FloatingWindowId::Orders, 1);
        let b_view = vehicle_window_key(FloatingWindowId::Vehicle, 2);
        let b_details = vehicle_window_key(FloatingWindowId::VehicleDetails, 2);
        assert_ne!(a_view, b_view);
        assert_ne!(a_orders.instance, b_details.instance);
        assert_eq!(a_view.instance, a_orders.instance);
    }
}
