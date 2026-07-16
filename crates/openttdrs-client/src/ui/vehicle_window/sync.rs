//! Sincronización de la ventana de vista del vehículo.

use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::VehicleKind;

use crate::render::{PrimaryGameCamera, TruckHandles, vehicle_world_position};
use crate::state::SimWorld;
use crate::ui::floating_window::{FloatingWindow, FloatingWindowId, FloatingWindowTitleText};
use crate::ui::vehicle_details_window::VehicleDetailsWindowState;

use super::status::format_vehicle_status;
use super::{
    VehicleConsistUnitSprite, VehicleWindowPreviewCamera, VehicleWindowRefitOnly,
    VehicleWindowRenameInput, VehicleWindowRenameRow, VehicleWindowState, VehicleWindowStatusText,
    VehicleWindowToggleText, VehicleWindowTrainOnly, vehicle_side_sprite,
};

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync_vehicle_window(
    window_state: Res<VehicleWindowState>,
    mut details_state: ResMut<VehicleDetailsWindowState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut status_q: Query<
        (&mut Text, &mut TextColor),
        (
            With<VehicleWindowStatusText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut toggle_q: Query<
        &mut Text,
        (
            With<VehicleWindowToggleText>,
            Without<VehicleWindowStatusText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut rename_row_q: Query<&mut Node, With<VehicleWindowRenameRow>>,
    mut train_row_q: Query<
        &mut Node,
        (
            With<VehicleWindowTrainOnly>,
            Without<VehicleWindowRenameRow>,
        ),
    >,
    mut refit_row_q: Query<
        &mut Node,
        (
            With<VehicleWindowRefitOnly>,
            Without<VehicleWindowRenameRow>,
            Without<VehicleWindowTrainOnly>,
        ),
    >,
    _rename_input_q: Query<&mut EditableText, With<VehicleWindowRenameInput>>,
    mut consist_q: Query<
        (&VehicleConsistUnitSprite, &mut ImageNode, &mut Node),
        (
            Without<VehicleWindowRenameRow>,
            Without<VehicleWindowTrainOnly>,
            Without<VehicleWindowRefitOnly>,
        ),
    >,
    mut preview: Query<
        (&mut Transform, &mut Camera),
        (With<VehicleWindowPreviewCamera>, Without<PrimaryGameCamera>),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Vehicle)
    else {
        return;
    };
    let vehicle = window_state
        .vehicle_id
        .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
    let Some(vehicle) = vehicle else {
        *vis = Visibility::Hidden;
        if let Ok((_, mut cam)) = preview.single_mut() {
            cam.is_active = false;
        }
        for (_, _, mut node) in &mut consist_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;

    // Si Details está abierta, seguir al vehículo de View (#173).
    if details_state.vehicle_id.is_some() {
        details_state.vehicle_id = Some(vehicle.id);
    }

    let unit_ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id);
    if let Some(trucks) = trucks.as_ref() {
        for (sprite, mut image, mut node) in &mut consist_q {
            if let Some(&unit_id) = unit_ids.get(sprite.unit_idx)
                && let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id)
            {
                node.display = Display::Flex;
                image.image = vehicle_side_sprite(trucks, unit);
            } else {
                node.display = Display::None;
            }
        }
    } else {
        for (_, _, mut node) in &mut consist_q {
            node.display = Display::None;
        }
    }

    if window_state.rename_editing
        && let Ok(mut row) = rename_row_q.single_mut()
    {
        row.display = Display::Flex;
    } else if let Ok(mut row) = rename_row_q.single_mut() {
        row.display = Display::None;
    }

    let train_display = if vehicle.kind == VehicleKind::Train {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut row) = train_row_q.single_mut() {
        row.display = train_display;
    }

    let refit_display = if openttdrs_core::refit_allowed(vehicle, &sim.state.map) {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut row) = refit_row_q.single_mut() {
        row.display = refit_display;
    }

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Vehicle)
    {
        **title = vehicle.display_name();
    }
    if let Ok((mut status, mut color)) = status_q.single_mut() {
        let (text, status_color) = format_vehicle_status(vehicle, &sim);
        **status = text;
        *color = TextColor(status_color);
    }
    if let Ok(mut toggle) = toggle_q.single_mut() {
        // Iconos ▶ / ■ en la toolbar (#174).
        **toggle = if vehicle.running {
            "■".to_string()
        } else {
            "▶".to_string()
        };
    }
    if let Ok((mut tf, mut cam)) = preview.single_mut() {
        cam.is_active = true;
        let world_pos = vehicle_world_position(vehicle, &sim.state.map);
        tf.translation = Vec3::new(world_pos.x, world_pos.y, 999.0);
    }
}
