use bevy::prelude::*;
use openttdrs_core::{Command, CommandError, TileCoord, VehicleOrder, apply_command};

use crate::render::{RemapMapVisualsPending, VehiclePreviewCamera};
use crate::state::SimWorld;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::build_input::orders::{order_for_clicked_tile, order_pick_valid};
use crate::ui::toolbar::{DragBuildState, OrderEditState, OrderPanelButton};

pub(crate) fn start_order_destination_pick(order_state: &mut OrderEditState) {
    if order_state.vehicle_id.is_some() {
        order_state.picking_destination = true;
    }
}

pub(crate) fn cancel_order_destination_pick(order_state: &mut OrderEditState) {
    order_state.picking_destination = false;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_order_panel_buttons(
    mut q: Query<(&Interaction, &OrderPanelButton), (Changed<Interaction>, With<Button>)>,
    mut order_state: ResMut<OrderEditState>,
    mut sim: ResMut<SimWorld>,
    mut drag_state: ResMut<DragBuildState>,
    _pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut preview_cam: Query<
        &mut Camera,
        (
            With<VehiclePreviewCamera>,
            Without<crate::render::PrimaryGameCamera>,
        ),
    >,
    time: Res<Time>,
) {
    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            OrderPanelButton::Close => {
                order_state.vehicle_id = None;
                order_state.orders.clear();
                order_state.picking_destination = false;
                if let Ok(mut cam) = preview_cam.single_mut() {
                    cam.is_active = false;
                }
            }
            OrderPanelButton::ClearLast => {
                let Some(vehicle_id) = order_state.vehicle_id else {
                    continue;
                };
                order_state.orders.pop();
                let _ = apply_order_edit(&mut sim.state, vehicle_id, &order_state.orders);
            }
            OrderPanelButton::ClearAll => {
                let Some(vehicle_id) = order_state.vehicle_id else {
                    continue;
                };
                order_state.orders.clear();
                let _ = apply_command(
                    &mut sim.state,
                    &Command::SetVehicleOrders(vehicle_id, Vec::new()),
                );
            }
            OrderPanelButton::PickDestOnMap => {
                start_order_destination_pick(&mut order_state);
                cancel_placement(&mut drag_state);
            }
            OrderPanelButton::ToggleRunning => {
                let Some(vehicle_id) = order_state.vehicle_id else {
                    continue;
                };
                match apply_command(&mut sim.state, &Command::ToggleVehicleRunning(vehicle_id)) {
                    Ok(()) => {}
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
        }
    }
}

pub(crate) fn apply_order_edit(
    state: &mut openttdrs_core::GameState,
    vehicle_id: u32,
    orders: &[VehicleOrder],
) -> Result<(), openttdrs_core::CommandError> {
    apply_command(
        state,
        &Command::SetVehicleOrderList(vehicle_id, orders.to_vec()),
    )
}

pub(crate) fn try_append_order_at_tile(
    sim: &mut SimWorld,
    vehicle_id: u32,
    pos: TileCoord,
    orders: &mut Vec<VehicleOrder>,
) -> Result<(), CommandError> {
    let Some(order) = order_for_clicked_tile(sim, vehicle_id, pos) else {
        if sim.state.stations.iter().any(|s| s.pos == pos) {
            let vehicle = sim
                .state
                .vehicles
                .iter()
                .find(|v| v.id == vehicle_id)
                .ok_or(CommandError::VehicleNotFound)?;
            let station = sim
                .state
                .stations
                .iter()
                .find(|s| s.pos == pos)
                .ok_or(CommandError::StationNotFound)?;
            if !station.can_service_vehicle(vehicle.kind) {
                return Err(CommandError::IncompatibleStopForVehicle);
            }
        }
        return Err(CommandError::StationNotFound);
    };
    orders.push(order);
    apply_order_edit(&mut sim.state, vehicle_id, orders)
}

/// Clic en mapa mientras se eligen destinos (modo «Agregar destino» o herramienta Órdenes).
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_order_destination_click(
    mouse: &ButtonInput<MouseButton>,
    pos: TileCoord,
    order_state: &mut OrderEditState,
    sim: &mut SimWorld,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    elapsed_secs: f32,
) -> bool {
    if mouse.just_pressed(MouseButton::Right) {
        cancel_order_destination_pick(order_state);
        return true;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return false;
    }
    if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.pos == pos) {
        order_state.vehicle_id = Some(vehicle.id);
        order_state.orders = vehicle.orders.clone();
        order_state.picking_destination = false;
        return true;
    }
    let Some(vehicle_id) = order_state.vehicle_id else {
        return false;
    };
    if !order_pick_valid(sim, vehicle_id, pos) {
        if sim.state.stations.iter().any(|s| s.pos == pos) {
            let err = sim
                .state
                .vehicles
                .iter()
                .find(|v| v.id == vehicle_id)
                .and_then(|v| {
                    sim.state
                        .stations
                        .iter()
                        .find(|s| s.pos == pos)
                        .filter(|s| !s.can_service_vehicle(v.kind))
                        .map(|_| CommandError::IncompatibleStopForVehicle)
                })
                .unwrap_or(CommandError::StationNotFound);
            push_build_command_error(hud_feedback, err, elapsed_secs);
        }
        return true;
    }
    match try_append_order_at_tile(sim, vehicle_id, pos, &mut order_state.orders) {
        Ok(()) => pending.pending = true,
        Err(e) => {
            order_state.orders.pop();
            push_build_command_error(hud_feedback, e, elapsed_secs);
        }
    }
    true
}
