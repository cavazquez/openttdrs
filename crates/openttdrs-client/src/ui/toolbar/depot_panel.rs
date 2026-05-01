use bevy::prelude::*;
use openttdrs_core::{Command, TileCoord, VehicleKind, apply_command};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;

use super::{BuildMenuUi, OrderEditState};

const DEPOT_VEHICLE_ROWS: usize = 8;

#[derive(Resource, Default)]
pub(crate) struct DepotPanelState {
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) selected_vehicle: Option<u32>,
}

#[derive(Component)]
pub(crate) struct DepotPanelRoot;

#[derive(Component)]
pub(crate) struct DepotPanelText;

#[derive(Component, Clone, Copy)]
pub(crate) struct DepotVehicleRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DepotVehicleRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum DepotPanelButton {
    BuyBus,
    BuyTruck,
    Orders,
    ToggleRunning,
    Sell,
    CloneFromFirst,
    Close,
}

pub(crate) fn setup_depot_panel(mut commands: Commands) {
    commands
        .spawn((
            DepotPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(160.0),
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.08, 0.06, 0.95)),
            BorderColor::all(Color::srgb(0.75, 0.67, 0.45)),
            Visibility::Hidden,
            BuildMenuUi,
        ))
        .with_children(|panel| {
            panel.spawn((
                DepotPanelText,
                Text::new("Depósito"),
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
                    for slot in 0..DEPOT_VEHICLE_ROWS {
                        spawn_depot_vehicle_row(list, slot);
                    }
                });
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    spawn_depot_button(row, DepotPanelButton::BuyBus, "Comprar bus");
                    spawn_depot_button(row, DepotPanelButton::BuyTruck, "Comprar camión");
                    spawn_depot_button(row, DepotPanelButton::Orders, "Órdenes");
                    spawn_depot_button(row, DepotPanelButton::ToggleRunning, "Iniciar/Detener");
                    spawn_depot_button(row, DepotPanelButton::Sell, "Vender");
                    spawn_depot_button(row, DepotPanelButton::CloneFromFirst, "Clonar órdenes");
                    spawn_depot_button(row, DepotPanelButton::Close, "Cerrar");
                });
        });
}

fn spawn_depot_vehicle_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent.spawn((
        Button,
        DepotVehicleRow { slot },
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
        Interaction::default(),
        BuildMenuUi,
        children![(
            DepotVehicleRowText { slot },
            Text::new(""),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_depot_button(
    parent: &mut ChildSpawnerCommands,
    action: DepotPanelButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(110.0),
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

pub(crate) fn sync_depot_panel(
    depot_state: Res<DepotPanelState>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<DepotPanelRoot>>,
    mut text_q: Query<&mut Text, With<DepotPanelText>>,
    mut row_q: Query<
        (
            &DepotVehicleRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut row_text_q: Query<(&DepotVehicleRowText, &mut Text), Without<DepotPanelText>>,
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    let Some(depot_pos) = depot_state.depot_pos else {
        *vis = Visibility::Hidden;
        for (_, _, mut node, _, _) in &mut row_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;
    let mut out = format!("Depósito en ({}, {})", depot_pos.x, depot_pos.y);
    let mut vehicles_here: Vec<_> = sim
        .state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.pos == depot_pos)
        .collect();
    vehicles_here.sort_by_key(|vehicle| vehicle.id);
    out.push_str(&format!("\nVehículos en depósito: {}", vehicles_here.len()));
    if let Ok(mut text) = text_q.single_mut() {
        **text = out;
    }
    for (row, interaction, mut node, mut bg, mut border) in &mut row_q {
        let Some(vehicle) = vehicles_here.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = depot_state.selected_vehicle == Some(vehicle.id);
        *bg = if selected && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.62, 0.54, 0.34))
        } else if selected {
            BackgroundColor(Color::srgb(0.48, 0.41, 0.27))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.34, 0.29, 0.2))
        } else {
            BackgroundColor(Color::srgb(0.22, 0.18, 0.12))
        };
        *border = if selected {
            BorderColor::all(Color::srgb(0.9, 0.78, 0.48))
        } else {
            BorderColor::all(Color::srgb(0.45, 0.39, 0.27))
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        if let Some(vehicle) = vehicles_here.get(row_text.slot) {
            **text = depot_vehicle_row_label(vehicle);
        } else {
            **text = String::new();
        }
    }
}

fn depot_vehicle_row_label(vehicle: &openttdrs_core::Vehicle) -> String {
    format!(
        "#{:<3} {:<5} {:<4} carga {:>2}/{:<2} órdenes {}",
        vehicle.id,
        match vehicle.kind {
            VehicleKind::Bus => "Bus",
            VehicleKind::Truck => "Cam.",
            VehicleKind::Train => "Tren",
        },
        if vehicle.running { "RUN" } else { "STOP" },
        vehicle.cargo,
        vehicle.capacity,
        vehicle.orders.len()
    )
}

pub(crate) fn handle_depot_panel_buttons(
    mut q: Query<(&Interaction, &DepotPanelButton), (Changed<Interaction>, With<Button>)>,
    mut row_q: Query<
        (&Interaction, &DepotVehicleRow),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
        ),
    >,
    mut depot_state: ResMut<DepotPanelState>,
    mut order_state: ResMut<OrderEditState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
) {
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        let mut ids: Vec<u32> = sim
            .state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.pos == depot_pos)
            .map(|vehicle| vehicle.id)
            .collect();
        ids.sort_unstable();
        let Some(vehicle_id) = ids.get(row.slot).copied() else {
            continue;
        };
        let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
            continue;
        };
        depot_state.selected_vehicle = Some(vehicle_id);
        order_state.vehicle_id = Some(vehicle_id);
        order_state.orders = vehicle.orders.clone();
    }

    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        match button {
            DepotPanelButton::Close => {
                depot_state.depot_pos = None;
                depot_state.selected_vehicle = None;
            }
            DepotPanelButton::BuyBus => {
                if apply_command(
                    &mut sim.state,
                    &Command::BuildRoadVehicleAtDepot(depot_pos, VehicleKind::Bus),
                )
                .is_ok()
                {
                    pending.pending = true;
                }
            }
            DepotPanelButton::BuyTruck => {
                if apply_command(
                    &mut sim.state,
                    &Command::BuildRoadVehicleAtDepot(depot_pos, VehicleKind::Truck),
                )
                .is_ok()
                {
                    pending.pending = true;
                }
            }
            DepotPanelButton::Orders => {
                let target_id = depot_state.selected_vehicle.or_else(|| {
                    sim.state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.pos == depot_pos)
                        .map(|vehicle| vehicle.id)
                });
                if let Some(vehicle_id) = target_id
                    && let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
                {
                    depot_state.selected_vehicle = Some(vehicle_id);
                    order_state.vehicle_id = Some(vehicle_id);
                    order_state.orders = vehicle.orders.clone();
                }
            }
            DepotPanelButton::ToggleRunning => {
                let target_id = depot_state.selected_vehicle.or_else(|| {
                    sim.state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.pos == depot_pos)
                        .map(|vehicle| vehicle.id)
                });
                if let Some(vehicle_id) = target_id
                    && apply_command(&mut sim.state, &Command::ToggleVehicleRunning(vehicle_id))
                        .is_ok()
                {
                    depot_state.selected_vehicle = Some(vehicle_id);
                }
            }
            DepotPanelButton::Sell => {
                let target_id = depot_state.selected_vehicle.or_else(|| {
                    sim.state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.pos == depot_pos)
                        .map(|vehicle| vehicle.id)
                });
                if let Some(vehicle_id) = target_id
                    && apply_command(&mut sim.state, &Command::SellVehicle(vehicle_id)).is_ok()
                {
                    pending.pending = true;
                    depot_state.selected_vehicle = None;
                }
            }
            DepotPanelButton::CloneFromFirst => {
                let ids: Vec<u32> = sim
                    .state
                    .vehicles
                    .iter()
                    .filter(|vehicle| vehicle.pos == depot_pos)
                    .map(|vehicle| vehicle.id)
                    .collect();
                if ids.len() >= 2
                    && apply_command(
                        &mut sim.state,
                        &Command::CloneVehicleOrders {
                            from_vehicle_id: ids[0],
                            to_vehicle_id: ids[1],
                        },
                    )
                    .is_ok()
                {
                    depot_state.selected_vehicle = Some(ids[1]);
                }
            }
        }
    }
}
