use bevy::prelude::*;
use openttdrs_core::{Command, CommandError, TileCoord, Vehicle, VehicleOrder, apply_command};

use crate::render::RemapMapVisualsPending;
use crate::state::{OrderPickState, SimWorld};
use crate::ui::destination_window::DestinationPickerState;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::timetable_window::{TimetableWindowState, open_timetable_for_vehicle};
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::build_input::orders::{order_for_clicked_tile, order_pick_valid};
use crate::ui::toolbar::{DragBuildState, OrderEditState, OrderPanelButton};

use super::OrderPanelRow;

/// Carga el vehículo en el panel de órdenes (lista + selección en la orden actual).
pub(crate) fn open_order_edit_for_vehicle(
    order_state: &mut OrderEditState,
    vehicle: &Vehicle,
    next_pick: &mut NextState<OrderPickState>,
) {
    order_state.vehicle_id = Some(vehicle.id);
    order_state.orders = vehicle.orders.clone();
    order_state.selected_slot = selected_slot_for_vehicle(vehicle);
    next_pick.set(OrderPickState::Idle);
}

fn selected_slot_for_vehicle(vehicle: &Vehicle) -> Option<usize> {
    if vehicle.orders.is_empty() {
        None
    } else {
        Some(vehicle.current_order.min(vehicle.orders.len() - 1))
    }
}

fn refresh_orders_from_sim(order_state: &mut OrderEditState, sim: &SimWorld) {
    let Some(vehicle_id) = order_state.vehicle_id else {
        return;
    };
    let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        order_state.vehicle_id = None;
        order_state.orders.clear();
        order_state.selected_slot = None;
        return;
    };
    order_state.orders = vehicle.orders.clone();
    if let Some(sel) = order_state.selected_slot
        && sel >= order_state.orders.len()
    {
        order_state.selected_slot =
            order_state
                .orders
                .len()
                .checked_sub(1)
                .or(if order_state.orders.is_empty() {
                    None
                } else {
                    Some(0)
                });
    }
}

fn clamp_selected_after_remove(order_state: &mut OrderEditState, removed_index: usize) {
    let Some(sel) = order_state.selected_slot else {
        return;
    };
    let len = order_state.orders.len();
    if len == 0 {
        order_state.selected_slot = None;
    } else if sel == removed_index {
        order_state.selected_slot = Some(removed_index.min(len - 1));
    } else if sel > removed_index {
        order_state.selected_slot = Some(sel - 1);
    }
}

pub(crate) fn start_order_destination_pick(
    order_state: &OrderEditState,
    next_pick: &mut NextState<OrderPickState>,
) {
    if order_state.vehicle_id.is_some() {
        next_pick.set(OrderPickState::Picking);
    }
}

pub(crate) fn cancel_order_destination_pick(next_pick: &mut NextState<OrderPickState>) {
    next_pick.set(OrderPickState::Idle);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_order_panel_buttons(
    mut row_q: Query<(&Interaction, &OrderPanelRow), (Changed<Interaction>, With<Button>)>,
    mut btn_q: Query<
        (&Interaction, &OrderPanelButton),
        (Changed<Interaction>, With<Button>, Without<OrderPanelRow>),
    >,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut sim: ResMut<SimWorld>,
    mut drag_state: ResMut<DragBuildState>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut tt_state: ResMut<TimetableWindowState>,
    mut destination_picker: Option<ResMut<DestinationPickerState>>,
    time: Res<Time>,
) {
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if row.slot < order_state.orders.len() {
            order_state.selected_slot = Some(row.slot);
        }
    }

    for (interaction, button) in &mut btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            OrderPanelButton::Close => {
                order_state.clear();
            }
            OrderPanelButton::DeleteSelected => {
                let Some(vehicle_id) = order_state.vehicle_id else {
                    continue;
                };
                let Some(index) = order_state.selected_slot else {
                    continue;
                };
                match apply_command(
                    &mut sim.state,
                    &Command::RemoveVehicleOrderAt { vehicle_id, index },
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        refresh_orders_from_sim(&mut order_state, &sim);
                        clamp_selected_after_remove(&mut order_state, index);
                    }
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            OrderPanelButton::SkipOrder => {
                let Some(vehicle_id) = order_state.vehicle_id else {
                    continue;
                };
                match apply_command(&mut sim.state, &Command::SkipVehicleOrder(vehicle_id)) {
                    Ok(()) => {
                        pending.pending = true;
                        refresh_orders_from_sim(&mut order_state, &sim);
                        if let Some(vehicle) =
                            sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
                        {
                            order_state.selected_slot = selected_slot_for_vehicle(vehicle);
                        }
                    }
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            OrderPanelButton::ToggleFullLoad => {
                toggle_order_flag(
                    &mut order_state,
                    &mut sim,
                    &mut pending,
                    &mut hud_feedback,
                    time.elapsed_secs(),
                    |vehicle_id, index| Command::ToggleVehicleOrderFullLoad { vehicle_id, index },
                );
            }
            OrderPanelButton::ToggleNoUnload => {
                toggle_order_flag(
                    &mut order_state,
                    &mut sim,
                    &mut pending,
                    &mut hud_feedback,
                    time.elapsed_secs(),
                    |vehicle_id, index| Command::ToggleVehicleOrderNoUnload { vehicle_id, index },
                );
            }
            OrderPanelButton::OpenTimetableWindow => {
                let Some(vehicle_id) = order_state.vehicle_id else {
                    continue;
                };
                open_timetable_for_vehicle(&mut tt_state, vehicle_id);
            }
            OrderPanelButton::PickDestOnMap => {
                // Primero abre la lista global de destinos. Desde esa ventana
                // se puede elegir una fila o pasar al picker sobre el mapa.
                if let Some(picker) = destination_picker.as_deref_mut() {
                    picker.open = order_state.vehicle_id.is_some();
                    next_pick.set(OrderPickState::Idle);
                } else {
                    start_order_destination_pick(&order_state, &mut next_pick);
                }
                cancel_placement(&mut drag_state);
            }
        }
    }
}

fn toggle_order_flag(
    order_state: &mut OrderEditState,
    sim: &mut SimWorld,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    elapsed_secs: f32,
    make_cmd: impl FnOnce(u32, usize) -> Command,
) {
    let Some(vehicle_id) = order_state.vehicle_id else {
        return;
    };
    let Some(index) = order_state.selected_slot else {
        push_build_command_error(
            hud_feedback,
            CommandError::OrderIndexOutOfRange,
            elapsed_secs,
        );
        return;
    };
    match apply_command(&mut sim.state, &make_cmd(vehicle_id, index)) {
        Ok(()) => {
            pending.pending = true;
            refresh_orders_from_sim(order_state, sim);
        }
        Err(e) => push_build_command_error(hud_feedback, e, elapsed_secs),
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
    next_pick: &mut NextState<OrderPickState>,
    sim: &mut SimWorld,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    elapsed_secs: f32,
) -> bool {
    if mouse.just_pressed(MouseButton::Right) {
        cancel_order_destination_pick(next_pick);
        return true;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return false;
    }
    let Some(vehicle_id) = order_state.vehicle_id else {
        return false;
    };
    // Prioridad 1: añadir la parada válida (estación/waypoint/depósito) de la
    // tesela clicada, aunque haya un vehículo parado ahí (p.ej. el propio tren
    // dentro del depósito).
    if order_pick_valid(sim, vehicle_id, pos) {
        match try_append_order_at_tile(sim, vehicle_id, pos, &mut order_state.orders) {
            Ok(()) => {
                pending.pending = true;
                order_state.selected_slot = order_state.orders.len().checked_sub(1);
            }
            Err(e) => {
                order_state.orders.pop();
                push_build_command_error(hud_feedback, e, elapsed_secs);
            }
        }
        return true;
    }
    // Prioridad 2: clic sobre otro vehículo (fuera de un destino) → editar sus
    // órdenes.
    if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.pos == pos) {
        open_order_edit_for_vehicle(order_state, vehicle, next_pick);
        return true;
    }
    // Estación presente pero incompatible con este vehículo → feedback.
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
    true
}
