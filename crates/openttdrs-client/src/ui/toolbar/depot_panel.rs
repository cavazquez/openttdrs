//! Ventana flotante de depósito (carretera y vía), estilo `OpenTTD`.
//!
//! Lista los vehículos estacionados con acciones por fila (Órdenes, Vender,
//! Iniciar/Detener) y el botón «Nuevos vehículos» que abre la ventana de
//! compra con el catálogo del tipo de depósito.

use bevy::prelude::*;
use openttdrs_core::{Command, TileCoord, TileKind, apply_command};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};

use super::{BuildMenuUi, OrderEditState};

const DEPOT_VEHICLE_ROWS: usize = 8;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct DepotPanelState {
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) selected_vehicle: Option<u32>,
}

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

/// Contenedor de una fila (vehículo + acciones) para mostrar/ocultar junta.
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotRowContainer {
    slot: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepotRowKind {
    Orders,
    ToggleRunning,
    Sell,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DepotRowAction {
    slot: usize,
    kind: DepotRowKind,
}

/// Texto del botón Iniciar/Detener de la fila (cambia según estado).
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotRowToggleText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum DepotPanelButton {
    NewVehicles,
    CloneFromFirst,
}

pub(crate) fn setup_depot_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Depot,
        "Depósito",
        TITLE_BROWN,
        Vec2::new(430.0, 160.0),
        430.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            DepotPanelText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|list| {
                for slot in 0..DEPOT_VEHICLE_ROWS {
                    spawn_depot_vehicle_row(list, asset_server, slot);
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
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::NewVehicles,
                    "Nuevos vehículos",
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::CloneFromFirst,
                    "Clonar órdenes",
                );
            });
    });
}

fn spawn_depot_vehicle_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    slot: usize,
) {
    parent
        .spawn((
            DepotRowContainer { slot },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(3.0),
                display: Display::None,
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                Button,
                DepotVehicleRow { slot },
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(22.0),
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
                BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
                Interaction::default(),
                BuildMenuUi,
                children![(
                    DepotVehicleRowText { slot },
                    Text::new(""),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(Color::srgb(0.92, 0.88, 0.72)),
                )],
            ));
            spawn_row_action(
                row,
                asset_server,
                slot,
                DepotRowKind::Orders,
                "Órdenes",
                58.0,
                None,
            );
            spawn_row_action(
                row,
                asset_server,
                slot,
                DepotRowKind::ToggleRunning,
                "",
                62.0,
                Some(DepotRowToggleText { slot }),
            );
            spawn_row_action(
                row,
                asset_server,
                slot,
                DepotRowKind::Sell,
                "Vender",
                52.0,
                None,
            );
        });
}

#[allow(clippy::too_many_arguments)]
fn spawn_row_action(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    slot: usize,
    kind: DepotRowKind,
    label: &'static str,
    width: f32,
    toggle_text: Option<DepotRowToggleText>,
) {
    parent
        .spawn((
            Button,
            DepotRowAction { slot, kind },
            Node {
                width: Val::Px(width),
                height: Val::Px(22.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            let mut text = btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            ));
            if let Some(marker) = toggle_text {
                text.insert(marker);
            }
        });
}

fn spawn_depot_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: DepotPanelButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(130.0),
            height: Val::Px(24.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn depot_title(sim: &SimWorld, depot_pos: TileCoord) -> String {
    let kind = sim.state.map.get_kind(depot_pos);
    let nombre = if kind == Some(TileKind::RailDepot) {
        "Depósito de Trenes"
    } else {
        "Depósito de Carretera"
    };
    format!("{nombre} ({}, {})", depot_pos.x, depot_pos.y)
}

fn vehicles_at_depot(sim: &SimWorld, depot_pos: TileCoord) -> Vec<&openttdrs_core::Vehicle> {
    let mut vehicles: Vec<_> = sim
        .state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.pos == depot_pos)
        .collect();
    vehicles.sort_by_key(|vehicle| vehicle.id);
    vehicles
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // sistema ECS Bevy
pub(crate) fn sync_depot_panel(
    depot_state: Res<DepotPanelState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut text_q: Query<&mut Text, (With<DepotPanelText>, Without<FloatingWindowTitleText>)>,
    mut container_q: Query<(&DepotRowContainer, &mut Node)>,
    mut row_q: Query<
        (
            &DepotVehicleRow,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut row_text_q: Query<
        (&DepotVehicleRowText, &mut Text),
        (Without<DepotPanelText>, Without<FloatingWindowTitleText>),
    >,
    mut toggle_text_q: Query<
        (&DepotRowToggleText, &mut Text),
        (
            Without<DepotPanelText>,
            Without<FloatingWindowTitleText>,
            Without<DepotVehicleRowText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Depot)
    else {
        return;
    };
    let Some(depot_pos) = depot_state.depot_pos else {
        *vis = Visibility::Hidden;
        for (_, mut node) in &mut container_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Depot)
    {
        **title = depot_title(&sim, depot_pos);
    }
    let vehicles_here = vehicles_at_depot(&sim, depot_pos);
    if let Ok(mut text) = text_q.single_mut() {
        **text = format!("Vehículos en depósito: {}", vehicles_here.len());
    }
    for (container, mut node) in &mut container_q {
        node.display = if vehicles_here.get(container.slot).is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (row, interaction, mut bg, mut border) in &mut row_q {
        let Some(vehicle) = vehicles_here.get(row.slot) else {
            continue;
        };
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
    for (toggle, mut text) in &mut toggle_text_q {
        if let Some(vehicle) = vehicles_here.get(toggle.slot) {
            **text = if vehicle.running {
                "Detener".to_string()
            } else {
                "Iniciar".to_string()
            };
        }
    }
}

fn depot_vehicle_row_label(vehicle: &openttdrs_core::Vehicle) -> String {
    let engine = vehicle.effective_engine();
    format!(
        "#{:<3} {:<26} carga {:>2}/{:<3}",
        vehicle.id, engine.name, vehicle.cargo, vehicle.capacity
    )
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn depot_panel_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut depot_state: ResMut<DepotPanelState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Depot {
            depot_state.depot_pos = None;
            depot_state.selected_vehicle = None;
        }
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
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
    mut action_q: Query<
        (&Interaction, &DepotRowAction),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
            Without<DepotVehicleRow>,
        ),
    >,
    mut depot_state: ResMut<DepotPanelState>,
    mut order_state: ResMut<OrderEditState>,
    mut buy_state: ResMut<BuyVehicleWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        let Some(vehicle_id) = vehicles_at_depot(&sim, depot_pos)
            .get(row.slot)
            .map(|v| v.id)
        else {
            continue;
        };
        let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
            continue;
        };
        depot_state.selected_vehicle = Some(vehicle_id);
        order_state.vehicle_id = Some(vehicle_id);
        order_state.orders = vehicle.orders.clone();
    }

    for (interaction, action) in &mut action_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        let Some(vehicle_id) = vehicles_at_depot(&sim, depot_pos)
            .get(action.slot)
            .map(|v| v.id)
        else {
            continue;
        };
        match action.kind {
            DepotRowKind::Orders => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    depot_state.selected_vehicle = Some(vehicle_id);
                    order_state.vehicle_id = Some(vehicle_id);
                    order_state.orders = vehicle.orders.clone();
                    order_state.picking_destination = false;
                }
            }
            DepotRowKind::ToggleRunning => {
                if apply_command(&mut sim.state, &Command::ToggleVehicleRunning(vehicle_id)).is_ok()
                {
                    pending.pending = true;
                }
            }
            DepotRowKind::Sell => {
                match apply_command(&mut sim.state, &Command::SellVehicle(vehicle_id)) {
                    Ok(()) => {
                        pending.pending = true;
                        if depot_state.selected_vehicle == Some(vehicle_id) {
                            depot_state.selected_vehicle = None;
                        }
                        if order_state.vehicle_id == Some(vehicle_id) {
                            order_state.vehicle_id = None;
                            order_state.orders.clear();
                            order_state.picking_destination = false;
                        }
                    }
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
        }
    }

    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        match button {
            DepotPanelButton::NewVehicles => {
                buy_state.depot_pos = Some(depot_pos);
                buy_state.selected_engine = None;
            }
            DepotPanelButton::CloneFromFirst => {
                let ids: Vec<u32> = vehicles_at_depot(&sim, depot_pos)
                    .iter()
                    .map(|v| v.id)
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
