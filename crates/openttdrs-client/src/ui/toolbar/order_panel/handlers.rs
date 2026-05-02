use bevy::prelude::*;
use openttdrs_core::{Command, VehicleOrder, apply_command};

use crate::state::SimWorld;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::{
    BuildMenuAction, DragBuildState, OrderEditState, OrderPanelButton, UiToolState,
};

pub(crate) fn handle_order_panel_buttons(
    mut q: Query<(&Interaction, &OrderPanelButton), (Changed<Interaction>, With<Button>)>,
    mut order_state: ResMut<OrderEditState>,
    mut sim: ResMut<SimWorld>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            OrderPanelButton::Close => {
                order_state.vehicle_id = None;
                order_state.orders.clear();
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
                if order_state.vehicle_id.is_none() {
                    continue;
                }
                tool_state.active_tool = Some(BuildMenuAction::Orders);
                cancel_placement(&mut drag_state);
            }
        }
    }
}

pub(crate) fn apply_order_edit(
    state: &mut openttdrs_core::GameState,
    vehicle_id: u32,
    orders: &[VehicleOrder],
) -> Result<(), openttdrs_core::CommandError> {
    if orders
        .iter()
        .all(|order| matches!(order, VehicleOrder::Station { .. }))
    {
        let stations = orders.iter().map(|order| order.destination()).collect();
        apply_command(
            state,
            &Command::SetVehicleStationOrders(vehicle_id, stations),
        )
    } else {
        let tiles = orders.iter().map(|order| order.destination()).collect();
        apply_command(state, &Command::SetVehicleOrders(vehicle_id, tiles))
    }
}
