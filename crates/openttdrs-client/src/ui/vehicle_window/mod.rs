//! Ventana de vista del vehículo (`VehicleViewWindow` OpenTTD / #173).
//!
//! Se abre al hacer clic en un vehículo del mapa o fila del depósito: vista
//! previa en vivo, tira del consist, estado y acciones (Iniciar/Detener,
//! Órdenes, Depósito, Detalles, Centrar…). Los stats/tabs viven en
//! [`crate::ui::vehicle_details_window`]. La venta es del depósito.
//!
//! Multi-instancia (#242): hasta 2 Views concurrentes vía [`VehicleChainRegistry`].
//! `vehicle_id` es el enfocado; `open` lista todas las abiertas.

mod actions;
mod rename;
mod setup;
mod status;
mod sync;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{default_engine_id, engine_for_vehicle};

use crate::render::TruckHandles;
use crate::ui::floating_window::FloatingWindowClosed;
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_details_window::VehicleDetailsWindowState;

pub(crate) use actions::handle_vehicle_window_buttons;
pub(crate) use rename::{
    handle_vehicle_rename_buttons, vehicle_window_rename_editable_keyboard,
    vehicle_window_rename_keyboard,
};
pub(crate) use setup::setup_vehicle_window;
pub(crate) use sync::sync_vehicle_window;

const PREVIEW_TEX_W: u32 = 260;
const PREVIEW_TEX_H: u32 = 100;
const PREVIEW_SCALE: f32 = 0.5;
pub(crate) const CONSIST_STRIP_MAX_UNITS: usize = 8;
pub(crate) const CONSIST_UNIT_SPRITE_W: f32 = 28.0;
pub(crate) const CONSIST_UNIT_SPRITE_H: f32 = 14.0;
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";
/// Botón icono de la toolbar de vista (#174).
const ICON_BTN: f32 = 28.0;
const ICON_IMG: f32 = 20.0;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const STATUS_STOPPED: Color = Color::srgb(0.92, 0.35, 0.3);
const STATUS_RUNNING: Color = Color::srgb(0.45, 0.85, 0.4);
const STATUS_NO_ROUTE: Color = Color::srgb(0.95, 0.75, 0.25);

#[derive(Resource, Default)]
pub(crate) struct VehicleWindowState {
    /// Vehículo enfocado (acciones/rename/preview RT).
    pub(crate) vehicle_id: Option<u32>,
    /// Todas las vistas abiertas (máx. 2); espejo de [`VehicleChainRegistry`].
    pub(crate) open: Vec<u32>,
    pub(crate) rename_editing: bool,
}

impl VehicleWindowState {
    /// Abre o trae al frente la vista de `vehicle_id` (#242).
    pub(crate) fn open_or_focus(
        &mut self,
        chain: &mut VehicleChainRegistry,
        vehicle_id: u32,
    ) -> u8 {
        let slot = chain.open_or_focus(vehicle_id);
        self.vehicle_id = chain.focused;
        self.open = chain.open_ids();
        self.rename_editing = false;
        slot
    }

    pub(crate) fn sync_from_chain(&mut self, chain: &VehicleChainRegistry) {
        self.vehicle_id = chain.focused;
        self.open = chain.open_ids();
        if self.vehicle_id.is_none() {
            self.rename_editing = false;
        }
    }

    pub(crate) fn clear_with_chain(&mut self, chain: &mut VehicleChainRegistry) {
        chain.clear();
        self.vehicle_id = None;
        self.open.clear();
        self.rename_editing = false;
    }
}

#[derive(Component)]
pub(crate) struct VehicleWindowRenameRow;

#[derive(Component)]
pub(crate) struct VehicleWindowRenameInput;

#[derive(Component, Clone, Copy)]
pub(crate) enum VehicleWindowRenameButton {
    Apply,
    Cancel,
}

#[derive(Component)]
pub(crate) struct VehicleWindowPreviewCamera;

#[derive(Component)]
pub(crate) struct VehicleWindowStatusText;

#[derive(Component, Clone, Copy)]
pub(crate) enum VehicleWindowButton {
    ToggleRunning,
    Orders,
    Timetable,
    GotoDepot,
    CenterOrder,
    CenterCamera,
    Rename,
    TurnAround,
    ForceProceed,
    Refit,
    Details,
}

#[derive(Component)]
pub(crate) struct VehicleWindowTrainOnly;

#[derive(Component)]
pub(crate) struct VehicleWindowRefitOnly;

#[derive(Component)]
pub(crate) struct VehicleWindowToggleText;

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleConsistUnitSprite {
    unit_idx: usize,
}

pub(crate) fn vehicle_side_sprite(
    trucks: &TruckHandles,
    vehicle: &openttdrs_core::Vehicle,
) -> Handle<Image> {
    let engine_id = vehicle
        .engine_id
        .unwrap_or_else(|| default_engine_id(vehicle.kind));
    if vehicle.kind == VehicleKind::Train {
        let engine = engine_for_vehicle(vehicle.kind, engine_id);
        trucks.train_preview(engine.train_image_index, 2)
    } else {
        trucks.intro_sprite(vehicle.kind, 2)
    }
}

pub(crate) fn vehicle_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut window_state: ResMut<VehicleWindowState>,
    mut chain: ResMut<VehicleChainRegistry>,
    mut details_state: ResMut<VehicleDetailsWindowState>,
) {
    use crate::ui::floating_window::FloatingWindowId;
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::Vehicle {
            continue;
        }
        let vehicle_id = msg.0.instance;
        if vehicle_id == 0 {
            // Slot sin bind (instance 0): limpiar todo el estado legacy.
            window_state.clear_with_chain(&mut chain);
            *details_state = Default::default();
            continue;
        }
        chain.close_vehicle(vehicle_id);
        window_state.sync_from_chain(&chain);
        // No pisar Details de otra instancia (#244).
        details_state.close_vehicle(vehicle_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::vehicle_chain::VehicleChainRegistry;

    #[test]
    fn open_or_focus_keeps_two_ids() {
        let mut state = VehicleWindowState::default();
        let mut chain = VehicleChainRegistry::default();
        state.open_or_focus(&mut chain, 10);
        state.open_or_focus(&mut chain, 20);
        assert_eq!(state.open, vec![10, 20]);
        assert_eq!(state.vehicle_id, Some(20));
        state.open_or_focus(&mut chain, 10);
        assert_eq!(state.open, vec![10, 20]);
        assert_eq!(state.vehicle_id, Some(10));
    }

    #[test]
    fn vehicle_window_sync_follows_chain_after_close() {
        let mut state = VehicleWindowState::default();
        let mut chain = VehicleChainRegistry::default();
        state.open_or_focus(&mut chain, 42);
        state.open_or_focus(&mut chain, 99);

        chain.close_vehicle(99);
        state.sync_from_chain(&chain);

        assert_eq!(state.vehicle_id, Some(42));
        assert_eq!(state.open, vec![42]);
        assert!(!state.rename_editing);
    }

    #[test]
    fn clearing_vehicle_window_resets_all_view_state() {
        let mut state = VehicleWindowState::default();
        let mut chain = VehicleChainRegistry::default();
        state.open_or_focus(&mut chain, 42);
        state.rename_editing = true;

        state.clear_with_chain(&mut chain);

        assert_eq!(state.vehicle_id, None);
        assert!(state.open.is_empty());
        assert!(!state.rename_editing);
    }
}
