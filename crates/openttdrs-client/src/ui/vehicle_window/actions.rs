//! Manejo de acciones de botones de la ventana de vehículo.

use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::{Command, apply_command, station::resolve_order_destination};

use crate::camera::tile_camera_world_pos;
use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, vehicle_world_position,
};
use crate::state::{OrderPickState, SimWorld};
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::refit_window::RefitWindowState;
use crate::ui::toolbar::{OrderEditState, open_order_edit_for_vehicle};

use super::{
    VehicleDetailsTabButton, VehicleWindowButton, VehicleWindowRenameInput, VehicleWindowState,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_vehicle_window_buttons(
    mut buttons: Query<(&Interaction, &VehicleWindowButton), (Changed<Interaction>, With<Button>)>,
    mut tab_buttons: Query<
        (&Interaction, &VehicleDetailsTabButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<VehicleWindowButton>,
        ),
    >,
    mut window_state: ResMut<VehicleWindowState>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut rename_input_q: Query<&mut EditableText, With<VehicleWindowRenameInput>>,
    mut refit_window: ResMut<RefitWindowState>,
    time: Res<Time>,
) {
    for (interaction, tab) in &mut tab_buttons {
        if *interaction == Interaction::Pressed {
            window_state.details_tab = tab.0;
        }
    }
    for (interaction, button) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(vehicle_id) = window_state.vehicle_id else {
            continue;
        };
        match button {
            VehicleWindowButton::ToggleRunning => {
                match apply_command(&mut sim.state, &Command::ToggleVehicleRunning(vehicle_id)) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::Orders => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    open_order_edit_for_vehicle(&mut order_state, vehicle, &mut next_pick);
                }
            }
            VehicleWindowButton::GotoDepot => {
                match apply_command(&mut sim.state, &Command::AppendGotoNearestDepot(vehicle_id)) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::CenterOrder => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
                    && !vehicle.orders.is_empty()
                {
                    let order = vehicle.orders[vehicle.current_order.min(vehicle.orders.len() - 1)];
                    let dest = resolve_order_destination(&sim.state.map, vehicle.kind, order);
                    let world = tile_camera_world_pos(&sim.state.map, dest);
                    if let Ok(mut transform) = cam_q.single_mut() {
                        transform.translation.x = world.x;
                        transform.translation.y = world.y;
                    }
                }
            }
            VehicleWindowButton::Rename => {
                window_state.rename_editing = true;
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
                    && let Ok(mut editable) = rename_input_q.single_mut()
                {
                    let seed = vehicle
                        .name
                        .as_deref()
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or(vehicle.effective_engine().name.as_str());
                    editable.editor_mut().set_text(seed);
                }
            }
            VehicleWindowButton::CenterCamera => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    let world_pos = vehicle_world_position(vehicle, &sim.state.map);
                    if let Ok(mut transform) = cam_q.single_mut() {
                        transform.translation.x = world_pos.x;
                        transform.translation.y = world_pos.y;
                    }
                }
            }
            VehicleWindowButton::TurnAround => {
                match apply_command(&mut sim.state, &Command::TurnAroundVehicle(vehicle_id)) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::ForceProceed => {
                match apply_command(&mut sim.state, &Command::ForceVehicleProceed(vehicle_id)) {
                    Ok(()) => {}
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::Refit => {
                refit_window.open_for(vehicle_id);
            }
        }
    }
}
