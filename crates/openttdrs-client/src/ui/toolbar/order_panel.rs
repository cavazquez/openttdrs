use bevy::prelude::*;
use openttdrs_core::{
    Command, TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder, apply_command,
};

use crate::state::SimWorld;

use super::{BuildMenuUi, OrderEditState, OrderPanelButton, OrderPanelRoot, OrderPanelText};

const ORDER_PANEL_ROWS: usize = 10;

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRowText {
    slot: usize,
}

pub(crate) fn setup_order_panel(mut commands: Commands) {
    commands
        .spawn((
            OrderPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(320.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.13, 0.1, 0.07, 0.95)),
            BorderColor::all(Color::srgb(0.75, 0.67, 0.45)),
            Visibility::Hidden,
            BuildMenuUi,
        ))
        .with_children(|panel| {
            panel.spawn((
                OrderPanelText,
                Text::new("Ordenes"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|list| {
                    for slot in 0..ORDER_PANEL_ROWS {
                        spawn_order_panel_row(list, slot);
                    }
                });
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_order_button(row, OrderPanelButton::ClearLast, "Ultima");
                    spawn_order_button(row, OrderPanelButton::ClearAll, "Borrar");
                    spawn_order_button(row, OrderPanelButton::Close, "Cerrar");
                });
        });
}

fn spawn_order_panel_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent.spawn((
        OrderPanelRow { slot },
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
        BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
        BuildMenuUi,
        children![(
            OrderPanelRowText { slot },
            Text::new(""),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_order_button(
    parent: &mut ChildSpawnerCommands,
    action: OrderPanelButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(74.0),
            height: Val::Px(24.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

pub(crate) fn sync_order_panel(
    order_state: Res<OrderEditState>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<OrderPanelRoot>>,
    mut text_q: Query<&mut Text, With<OrderPanelText>>,
    mut row_q: Query<(
        &OrderPanelRow,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut row_text_q: Query<(&OrderPanelRowText, &mut Text), Without<OrderPanelText>>,
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    let Some(vehicle_id) = order_state.vehicle_id else {
        *vis = Visibility::Hidden;
        for (_, mut node, _, _) in &mut row_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;
    let Some(vehicle) = sim
        .state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == vehicle_id)
    else {
        return;
    };
    let out = format!(
        "Vehículo #{} {} | carga {}/{} | dest ({},{})",
        vehicle.id,
        vehicle_kind_label(vehicle.kind),
        vehicle.cargo,
        vehicle.capacity,
        vehicle.dest.x,
        vehicle.dest.y
    );
    if let Ok(mut text) = text_q.single_mut() {
        **text = out;
    }
    for (row, mut node, mut bg, mut border) in &mut row_q {
        let has_content = row.slot == 0 && order_state.orders.is_empty()
            || row.slot < order_state.orders.len().min(ORDER_PANEL_ROWS);
        node.display = if has_content {
            Display::Flex
        } else {
            Display::None
        };
        let is_current = !order_state.orders.is_empty()
            && row.slot
                == vehicle
                    .current_order
                    .min(order_state.orders.len().saturating_sub(1));
        *bg = if is_current {
            BackgroundColor(Color::srgb(0.42, 0.35, 0.22))
        } else {
            BackgroundColor(Color::srgb(0.22, 0.18, 0.12))
        };
        *border = if is_current {
            BorderColor::all(Color::srgb(0.88, 0.74, 0.46))
        } else {
            BorderColor::all(Color::srgb(0.45, 0.39, 0.27))
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        **text = if order_state.orders.is_empty() && row_text.slot == 0 {
            "Sin órdenes cargadas".to_string()
        } else if let Some(order) = order_state.orders.get(row_text.slot) {
            order_row_label(row_text.slot, *order, vehicle, &sim)
        } else {
            String::new()
        };
    }
}

fn vehicle_kind_label(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::Bus => "Bus",
        VehicleKind::Truck => "Camión",
        VehicleKind::Train => "Tren",
    }
}

fn order_row_label(index: usize, order: VehicleOrder, vehicle: &Vehicle, sim: &SimWorld) -> String {
    let pos = order.destination();
    let current = if !vehicle.orders.is_empty() && vehicle.current_order == index {
        ">"
    } else {
        " "
    };
    let label = match order {
        VehicleOrder::Station { .. } => "Estación",
        VehicleOrder::Tile(tile) if sim.state.map.get_kind(tile) == Some(TileKind::RoadDepot) => {
            "Depósito"
        }
        VehicleOrder::Tile(_) => "Tile",
    };
    format!("{current} {:>2}. {label} ({}, {})", index + 1, pos.x, pos.y)
}

pub(crate) fn handle_order_panel_buttons(
    mut q: Query<(&Interaction, &OrderPanelButton), (Changed<Interaction>, With<Button>)>,
    mut order_state: ResMut<OrderEditState>,
    mut sim: ResMut<SimWorld>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_row_labels_depots() {
        let mut sim = SimWorld::default();
        let depot = TileCoord::new(1, 2);
        sim.state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        let vehicle = Vehicle::new(1, VehicleKind::Bus, depot, depot);

        assert!(order_row_label(0, VehicleOrder::tile(depot), &vehicle, &sim).contains("Depósito"));
    }
}
