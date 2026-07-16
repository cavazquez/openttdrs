//! Ventana flotante de tren/vehículo estilo `OpenTTD`.
//!
//! Se abre al hacer clic en un vehículo del mapa: vista previa en vivo
//! (cámara a render-target sobre el vehículo), tira horizontal del consist,
//! modelo, velocidad actual y máxima, carga, estado («Detenido» en rojo /
//! «En marcha» en verde) y acciones Iniciar/Detener, Órdenes, Enviar al
//! depósito y Centrar vista. La venta es una acción del depósito, no de esta
//! ventana.
//!
//! **Single-instance:** solo hay una [`FloatingWindowId::Vehicle`]; al elegir
//! otro vehículo se reemplaza `VehicleWindowState.vehicle_id` (y el contexto
//! de Órdenes/Refit/Timetable vinculados).

mod actions;
mod details;
mod rename;
mod setup;
mod sync;

use bevy::prelude::*;
use openttdrs_core::{VehicleKind, default_engine_id, engine_for_vehicle};

use crate::render::TruckHandles;
use crate::ui::floating_window::{FloatingWindowClosed, FloatingWindowId};

pub(crate) use actions::handle_vehicle_window_buttons;
pub(crate) use rename::{
    handle_vehicle_rename_buttons, vehicle_window_rename_editable_keyboard,
    vehicle_window_rename_keyboard,
};
pub(crate) use setup::setup_vehicle_window;
pub(crate) use sync::sync_vehicle_window;

const PREVIEW_TEX_W: u32 = 280;
const PREVIEW_TEX_H: u32 = 120;
const PREVIEW_SCALE: f32 = 0.5;
pub(crate) const CONSIST_STRIP_MAX_UNITS: usize = 8;
pub(crate) const CONSIST_UNIT_SPRITE_W: f32 = 28.0;
pub(crate) const CONSIST_UNIT_SPRITE_H: f32 = 14.0;
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const STATUS_STOPPED: Color = Color::srgb(0.92, 0.35, 0.3);
const STATUS_RUNNING: Color = Color::srgb(0.45, 0.85, 0.4);
const STATUS_NO_ROUTE: Color = Color::srgb(0.95, 0.75, 0.25);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum VehicleDetailsTab {
    #[default]
    Info,
    Cargo,
    Capacity,
    Totals,
}

#[derive(Resource, Default)]
pub(crate) struct VehicleWindowState {
    pub(crate) vehicle_id: Option<u32>,
    pub(crate) rename_editing: bool,
    pub(crate) details_tab: VehicleDetailsTab,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleDetailsTabButton(VehicleDetailsTab);

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
pub(crate) struct VehicleWindowBodyText;

#[derive(Component)]
pub(crate) struct VehicleWindowStatusText;

#[derive(Component, Clone, Copy)]
pub(crate) enum VehicleWindowButton {
    ToggleRunning,
    Orders,
    GotoDepot,
    CenterOrder,
    CenterCamera,
    Rename,
    TurnAround,
    ForceProceed,
    Refit,
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
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Vehicle {
            window_state.vehicle_id = None;
            window_state.rename_editing = false;
            window_state.details_tab = VehicleDetailsTab::Info;
        }
    }
}
