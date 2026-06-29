//! Ventana flotante de depósito (carretera y vía), estilo `OpenTTD`.
//!
//! Lista los vehículos estacionados con acciones por fila (Órdenes, Vender,
//! Iniciar/Detener) y el botón «Nuevos vehículos» que abre la ventana de
//! compra con el catálogo del tipo de depósito.

use bevy::prelude::*;
use openttdrs_core::{
    Command, EngineCatalogSort, RoadEngineFilter, TileCoord, TileKind, apply_command,
    calendar_year_at_tick, default_engine_id, engine_by_id, engines_for_depot_purchase,
};

use crate::camera::tile_camera_world_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending};
use crate::state::SimWorld;
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};

use super::{BuildMenuUi, OrderEditState, open_order_edit_for_vehicle};

const DEPOT_VEHICLE_ROWS: usize = 8;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct DepotPanelState {
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) selected_vehicle: Option<u32>,
    /// Origen de reordenación por clic (arrastre simplificado).
    pub(crate) reorder_from_slot: Option<usize>,
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
    CopyOrders,
    Sell,
    MoveUp,
    MoveDown,
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
    /// Compra una copia del vehículo seleccionado (motor + órdenes).
    CloneVehicle,
    SellAll,
    CenterDepot,
    StopAll,
    StartAll,
    /// Regla de autoreemplazo para el motor del vehículo seleccionado.
    Autoreplace,
    /// Reemplaza todos los vehículos del depósito según reglas activas.
    MassAutoreplace,
    /// Crea/enlaza pool de órdenes compartidas.
    ShareOrders,
    /// Cicla asignación de grupo del vehículo seleccionado.
    CycleGroup,
    /// Alterna «solo viejos» en la regla del motor seleccionado.
    ToggleOnlyWhenOld,
}

pub(crate) fn setup_depot_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Depot,
        "Depósito",
        TITLE_BROWN,
        Vec2::new(460.0, 188.0),
        460.0,
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
                spawn_depot_button(row, asset_server, DepotPanelButton::CloneVehicle, "Clonar");
                spawn_depot_button(row, asset_server, DepotPanelButton::SellAll, "Vender todo");
                spawn_depot_button(row, asset_server, DepotPanelButton::CenterDepot, "Centrar");
                spawn_depot_button(row, asset_server, DepotPanelButton::StopAll, "Parar todos");
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::StartAll,
                    "Arrancar todos",
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::Autoreplace,
                    "Auto-reempl.",
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::MassAutoreplace,
                    "Auto-reempl. todo",
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::ShareOrders,
                    "Compartir órdenes",
                );
                spawn_depot_button(row, asset_server, DepotPanelButton::CycleGroup, "Grupo");
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::ToggleOnlyWhenOld,
                    "Solo viejos",
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
                DepotRowKind::CopyOrders,
                "Copiar ord.",
                72.0,
                None,
            );
            spawn_row_action(
                row,
                asset_server,
                slot,
                DepotRowKind::MoveUp,
                "↑",
                28.0,
                None,
            );
            spawn_row_action(
                row,
                asset_server,
                slot,
                DepotRowKind::MoveDown,
                "↓",
                28.0,
                None,
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
    vehicles.sort_by(|a, b| match (a.depot_display_slot, b.depot_display_slot) {
        (Some(sa), Some(sb)) => sa.cmp(&sb).then_with(|| a.id.cmp(&b.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.id.cmp(&b.id),
    });
    vehicles
}

fn depot_vehicle_row_label(sim: &SimWorld, vehicle: &openttdrs_core::Vehicle) -> String {
    let age = vehicle.vehicle_age_years(sim.state.tick.get());
    let group = vehicle
        .group_id
        .and_then(|gid| sim.state.vehicle_groups.iter().find(|g| g.id == gid))
        .map(|g| format!(" [{}]", g.name))
        .unwrap_or_default();
    format!(
        "{:<24}{group} ed.{age}a carga {:>2}/{}",
        vehicle.display_name(),
        vehicle.cargo,
        vehicle.capacity
    )
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
            **text = depot_vehicle_row_label(&sim, vehicle);
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

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn depot_panel_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut depot_state: ResMut<DepotPanelState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Depot {
            depot_state.depot_pos = None;
            depot_state.selected_vehicle = None;
            depot_state.reorder_from_slot = None;
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
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
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
        open_order_edit_for_vehicle(&mut order_state, vehicle);
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
                    open_order_edit_for_vehicle(&mut order_state, vehicle);
                }
            }
            DepotRowKind::ToggleRunning => {
                if apply_command(&mut sim.state, &Command::ToggleVehicleRunning(vehicle_id)).is_ok()
                {
                    pending.pending = true;
                }
            }
            DepotRowKind::CopyOrders => {
                let Some(from_id) = depot_state.selected_vehicle else {
                    continue;
                };
                if from_id == vehicle_id {
                    continue;
                }
                match apply_command(
                    &mut sim.state,
                    &Command::CloneVehicleOrders {
                        from_vehicle_id: from_id,
                        to_vehicle_id: vehicle_id,
                    },
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        depot_state.selected_vehicle = Some(vehicle_id);
                    }
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
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
                            order_state.clear();
                        }
                    }
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            DepotRowKind::MoveUp | DepotRowKind::MoveDown => {
                let to_slot = match action.kind {
                    DepotRowKind::MoveUp if action.slot > 0 => action.slot - 1,
                    DepotRowKind::MoveDown => action.slot + 1,
                    _ => continue,
                };
                if apply_command(
                    &mut sim.state,
                    &Command::DepotReorderVehicleSlot {
                        depot_pos,
                        from_slot: action.slot,
                        to_slot,
                    },
                )
                .is_ok()
                {
                    pending.pending = true;
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
            DepotPanelButton::CloneVehicle => {
                let source_id = depot_state
                    .selected_vehicle
                    .or_else(|| vehicles_at_depot(&sim, depot_pos).first().map(|v| v.id));
                let Some(source_id) = source_id else {
                    continue;
                };
                match apply_command(
                    &mut sim.state,
                    &Command::CloneVehicleAtDepot {
                        source_vehicle_id: source_id,
                        depot_pos,
                    },
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        if let Some(new_id) = sim.state.vehicles.last().map(|v| v.id) {
                            depot_state.selected_vehicle = Some(new_id);
                        }
                    }
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            DepotPanelButton::SellAll => {
                let selected = depot_state.selected_vehicle;
                match apply_command(&mut sim.state, &Command::SellAllVehiclesAtDepot(depot_pos)) {
                    Ok(()) => {
                        pending.pending = true;
                        depot_state.selected_vehicle = None;
                        if selected.is_some_and(|id| order_state.vehicle_id == Some(id)) {
                            order_state.clear();
                        }
                    }
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            DepotPanelButton::CenterDepot => {
                let world = tile_camera_world_pos(&sim.state.map, depot_pos);
                if let Ok(mut transform) = cam_q.single_mut() {
                    transform.translation.x = world.x;
                    transform.translation.y = world.y;
                }
            }
            DepotPanelButton::StopAll => {
                if apply_command(
                    &mut sim.state,
                    &Command::SetDepotVehiclesRunning {
                        depot_pos,
                        running: false,
                    },
                )
                .is_ok()
                {
                    pending.pending = true;
                }
            }
            DepotPanelButton::StartAll => {
                if apply_command(
                    &mut sim.state,
                    &Command::SetDepotVehiclesRunning {
                        depot_pos,
                        running: true,
                    },
                )
                .is_ok()
                {
                    pending.pending = true;
                }
            }
            DepotPanelButton::Autoreplace => {
                let Some(from_vehicle_id) = depot_state.selected_vehicle else {
                    continue;
                };
                let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == from_vehicle_id)
                else {
                    continue;
                };
                let from_engine = vehicle
                    .engine_id
                    .unwrap_or_else(|| default_engine_id(vehicle.kind));
                let Some(to_engine) =
                    next_autoreplace_target(&sim, depot_pos, from_engine, vehicle.kind)
                else {
                    continue;
                };
                let cmd = if sim
                    .state
                    .autoreplace_rules
                    .iter()
                    .any(|r| r.from_engine_id == from_engine)
                {
                    Command::ToggleAutoReplaceRule {
                        from_engine_id: from_engine,
                    }
                } else {
                    Command::SetAutoReplaceRule {
                        from_engine_id: from_engine,
                        to_engine_id: to_engine,
                    }
                };
                match apply_command(&mut sim.state, &cmd) {
                    Ok(()) => {
                        pending.pending = true;
                        let group_id = sim
                            .state
                            .vehicles
                            .iter()
                            .find(|v| v.id == from_vehicle_id)
                            .and_then(|v| v.group_id);
                        if sim
                            .state
                            .autoreplace_rules
                            .iter()
                            .any(|r| r.from_engine_id == from_engine)
                        {
                            let _ = apply_command(
                                &mut sim.state,
                                &Command::SetAutoReplaceRuleGroup {
                                    from_engine_id: from_engine,
                                    group_id,
                                },
                            );
                        }
                    }
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::MassAutoreplace => {
                match apply_command(&mut sim.state, &Command::DepotMassAutoreplace { depot_pos }) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::ShareOrders => {
                let Some(from_vehicle_id) = depot_state.selected_vehicle else {
                    continue;
                };
                let cmd = if sim
                    .state
                    .vehicles
                    .iter()
                    .any(|v| v.id == from_vehicle_id && v.shared_order_id.is_some())
                {
                    Command::UnlinkVehicleSharedOrders(from_vehicle_id)
                } else {
                    Command::CreateSharedOrdersFromVehicle(from_vehicle_id)
                };
                match apply_command(&mut sim.state, &cmd) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::CycleGroup => {
                let Some(vehicle_id) = depot_state.selected_vehicle else {
                    continue;
                };
                let current_group = sim
                    .state
                    .vehicles
                    .iter()
                    .find(|v| v.id == vehicle_id)
                    .and_then(|v| v.group_id);
                if sim.state.vehicle_groups.is_empty()
                    && apply_command(
                        &mut sim.state,
                        &Command::CreateVehicleGroup {
                            name: "Grupo 1".into(),
                        },
                    )
                    .is_err()
                {
                    continue;
                }
                let next_group = if sim.state.vehicle_groups.is_empty() {
                    None
                } else if current_group.is_none() {
                    Some(sim.state.vehicle_groups[0].id)
                } else {
                    let Some(current) = current_group else {
                        continue;
                    };
                    match sim
                        .state
                        .vehicle_groups
                        .iter()
                        .position(|g| g.id == current)
                    {
                        Some(i) if i + 1 < sim.state.vehicle_groups.len() => {
                            Some(sim.state.vehicle_groups[i + 1].id)
                        }
                        Some(_) => None,
                        None => Some(sim.state.vehicle_groups[0].id),
                    }
                };
                match apply_command(
                    &mut sim.state,
                    &Command::AssignVehicleToGroup {
                        vehicle_id,
                        group_id: next_group,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::ToggleOnlyWhenOld => {
                let Some(from_vehicle_id) = depot_state.selected_vehicle else {
                    continue;
                };
                let Some(from_engine) = sim
                    .state
                    .vehicles
                    .iter()
                    .find(|v| v.id == from_vehicle_id)
                    .and_then(|v| v.engine_id)
                else {
                    continue;
                };
                match apply_command(
                    &mut sim.state,
                    &Command::ToggleAutoReplaceOnlyWhenOld {
                        from_engine_id: from_engine,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
        }
    }
}

fn next_autoreplace_target(
    sim: &SimWorld,
    depot_pos: TileCoord,
    from_engine: u16,
    kind: openttdrs_core::VehicleKind,
) -> Option<u16> {
    let is_rail = sim.state.map.get_kind(depot_pos) == Some(TileKind::RailDepot);
    let road_filter = match kind {
        openttdrs_core::VehicleKind::Bus => RoadEngineFilter::BusOnly,
        openttdrs_core::VehicleKind::Truck => RoadEngineFilter::TruckOnly,
        openttdrs_core::VehicleKind::Train => RoadEngineFilter::All,
    };
    let year = calendar_year_at_tick(sim.state.tick);
    let list = engines_for_depot_purchase(is_rail, year, EngineCatalogSort::Catalog, road_filter);
    if list.is_empty() {
        return None;
    }
    let idx = list.iter().position(|e| e.id == from_engine)?;
    if idx + 1 < list.len() {
        return Some(list[idx + 1].id);
    }
    list.iter()
        .find(|e| e.id != from_engine)
        .map(|e| e.id)
        .or_else(|| engine_by_id(from_engine).map(|e| e.id))
}
