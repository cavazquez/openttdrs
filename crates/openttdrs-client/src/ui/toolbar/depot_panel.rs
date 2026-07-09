//! Ventana flotante de depósito (carretera y vía), estilo `OpenTTD`.
//!
//! Muestra la lista de vehículos estacionados con su sprite lateral y un
//! botón de venta por fila; al hacer clic en un vehículo se abren sus órdenes.
//! La barra inferior replica el original: «Nuevos vehículos» y «Clonar», más
//! un botón de localización del depósito.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use openttdrs_core::{
    Command, TileCoord, TileKind, VehicleKind, apply_command, default_engine_id, engine_for_vehicle,
};

use crate::camera::tile_camera_world_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, TruckHandles};
use crate::state::SimWorld;
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::vehicle_window::VehicleWindowState;

use super::{BuildMenuUi, OrderEditState};

const DEPOT_VEHICLE_ROWS: usize = 8;
const ROW_HEIGHT: f32 = 30.0;
const SPRITE_W: f32 = 64.0;
const SPRITE_H: f32 = 24.0;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const LIST_BG: Color = Color::srgb(0.16, 0.13, 0.09);
const ROW_BG: Color = Color::srgb(0.22, 0.18, 0.12);
const ROW_BORDER: Color = Color::srgb(0.45, 0.39, 0.27);
const SELL_BG: Color = Color::srgb(0.58, 0.16, 0.13);
const SELL_BORDER: Color = Color::srgb(0.82, 0.34, 0.28);
const TEXT_COLOR: Color = Color::srgb(0.92, 0.88, 0.72);
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";

#[derive(Resource, Default)]
pub(crate) struct DepotPanelState {
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) selected_vehicle: Option<u32>,
    /// Origen de reordenación por clic (arrastre simplificado).
    pub(crate) reorder_from_slot: Option<usize>,
}

/// Contenedor de una fila (sprite + nombre + vender) para mostrar/ocultar junta.
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotRowContainer {
    slot: usize,
}

/// Zona clicable de la fila (sprite + texto) que abre las órdenes del vehículo.
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotVehicleRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DepotVehicleRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DepotVehicleSprite {
    slot: usize,
}

/// Botón de venta por fila (icono ✕ rojo, estilo original).
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotSellButton {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum DepotPanelButton {
    NewVehicles,
    /// Compra una copia del vehículo seleccionado (motor + órdenes).
    CloneVehicle,
    CenterDepot,
}

/// Texto del botón «Clonar» (cambia entre tren / vehículo según depósito).
#[derive(Component)]
pub(crate) struct DepotCloneLabel;

pub(crate) fn setup_depot_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Depot,
        "Depósito",
        TITLE_BROWN,
        Vec2::new(360.0, 300.0),
        360.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    padding: UiRect::all(Val::Px(2.0)),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(LIST_BG),
                BuildMenuUi,
            ))
            .with_children(|list| {
                for slot in 0..DEPOT_VEHICLE_ROWS {
                    spawn_depot_vehicle_row(list, asset_server, slot);
                }
            });
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::NewVehicles,
                    "Nuevos vehículos",
                    true,
                    false,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::CloneVehicle,
                    "Clonar tren",
                    true,
                    true,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::CenterDepot,
                    "Centrar",
                    false,
                    false,
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
                height: Val::Px(ROW_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
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
                    height: Val::Px(ROW_HEIGHT - 2.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::horizontal(Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(ROW_BG),
                BorderColor::all(ROW_BORDER),
                Interaction::default(),
                BuildMenuUi,
            ))
            .with_children(|inner| {
                inner.spawn((
                    DepotVehicleSprite { slot },
                    ImageNode::new(asset_server.load::<Image>(PLACEHOLDER_SPRITE)),
                    Node {
                        width: Val::Px(SPRITE_W),
                        height: Val::Px(SPRITE_H),
                        ..default()
                    },
                ));
                inner.spawn((
                    DepotVehicleRowText { slot },
                    Text::new(""),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(TEXT_COLOR),
                ));
            });
            row.spawn((
                Button,
                DepotSellButton { slot },
                Node {
                    width: Val::Px(26.0),
                    height: Val::Px(ROW_HEIGHT - 2.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(SELL_BG),
                BorderColor::all(SELL_BORDER),
                Interaction::default(),
                BuildMenuUi,
                children![(
                    Text::new("✕"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(Color::srgb(0.98, 0.92, 0.9)),
                )],
            ));
        });
}

fn spawn_depot_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: DepotPanelButton,
    label: &'static str,
    grow: bool,
    clone_label: bool,
) {
    let mut node = Node {
        height: Val::Px(24.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    if grow {
        node.flex_grow = 1.0;
    } else {
        node.width = Val::Px(70.0);
    }
    parent
        .spawn((
            Button,
            action,
            node,
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            let mut text = btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(TEXT_COLOR),
            ));
            if clone_label {
                text.insert(DepotCloneLabel);
            }
        });
}

fn depot_is_rail(sim: &SimWorld, depot_pos: TileCoord) -> bool {
    sim.state.map.get_kind(depot_pos) == Some(TileKind::RailDepot)
}

fn depot_title(sim: &SimWorld, depot_pos: TileCoord) -> String {
    let nombre = match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => "Depósito de Trenes",
        Some(TileKind::ShipDepot) => "Depósito de Barcos",
        Some(TileKind::Airport) => "Hangar de Aviones",
        _ => "Depósito de Carretera",
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
    format!(
        "{}  ({}a)  {}/{}",
        vehicle.display_name(),
        age,
        vehicle.cargo,
        vehicle.capacity
    )
}

fn vehicle_side_sprite(trucks: &TruckHandles, vehicle: &openttdrs_core::Vehicle) -> Handle<Image> {
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // sistema ECS Bevy
pub(crate) fn sync_depot_panel(
    depot_state: Res<DepotPanelState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
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
    mut row_text_q: Query<(&DepotVehicleRowText, &mut Text), Without<FloatingWindowTitleText>>,
    mut sprite_q: Query<(&DepotVehicleSprite, &mut ImageNode)>,
    mut clone_label_q: Query<
        &mut Text,
        (
            With<DepotCloneLabel>,
            Without<DepotVehicleRowText>,
            Without<FloatingWindowTitleText>,
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
    let is_rail = depot_is_rail(&sim, depot_pos);
    if let Ok(mut label) = clone_label_q.single_mut() {
        **label = if is_rail {
            "Clonar tren".to_string()
        } else {
            "Clonar vehículo".to_string()
        };
    }
    let vehicles_here = vehicles_at_depot(&sim, depot_pos);
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
            BackgroundColor(ROW_BG)
        };
        *border = if selected {
            BorderColor::all(Color::srgb(0.9, 0.78, 0.48))
        } else {
            BorderColor::all(ROW_BORDER)
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        if let Some(vehicle) = vehicles_here.get(row_text.slot) {
            **text = depot_vehicle_row_label(&sim, vehicle);
        } else {
            **text = String::new();
        }
    }
    if let Some(trucks) = trucks.as_ref() {
        for (sprite, mut image) in &mut sprite_q {
            if let Some(vehicle) = vehicles_here.get(sprite.slot) {
                image.image = vehicle_side_sprite(trucks, vehicle);
            }
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
    mut sell_q: Query<
        (&Interaction, &DepotSellButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
            Without<DepotVehicleRow>,
        ),
    >,
    mut depot_state: ResMut<DepotPanelState>,
    mut order_state: ResMut<OrderEditState>,
    mut vehicle_window: ResMut<VehicleWindowState>,
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
        depot_state.selected_vehicle = Some(vehicle_id);
        vehicle_window.vehicle_id = Some(vehicle_id);
        order_state.clear();
    }

    for (interaction, sell) in &mut sell_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        let Some(vehicle_id) = vehicles_at_depot(&sim, depot_pos)
            .get(sell.slot)
            .map(|v| v.id)
        else {
            continue;
        };
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
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
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
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::CenterDepot => {
                let world = tile_camera_world_pos(&sim.state.map, depot_pos);
                if let Ok(mut transform) = cam_q.single_mut() {
                    transform.translation.x = world.x;
                    transform.translation.y = world.y;
                }
            }
        }
    }
}
