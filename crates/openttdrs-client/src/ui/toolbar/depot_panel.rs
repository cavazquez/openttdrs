//! Ventana flotante de depósito (carretera y vía), estilo `OpenTTD`.
//!
//! Lista con bandera start/stop, tira de sprites del consist y venta por fila.
//! Drag a zonas «Vender» / «Vender cadena»; en vía, Ctrl+drag mueve la cola
//! (`MoveRailVehicle.move_chain`). Barra: Nuevos / Clonar / Centrar (+ secundarios).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{consist_unit_ids, engine_by_id};

use crate::camera::tile_camera_world_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, TruckHandles};
use crate::state::SimWorld;
use crate::ui::autoreplace_window::AutoreplaceWindowState;
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_window::{
    CONSIST_STRIP_MAX_UNITS, CONSIST_UNIT_SPRITE_H, CONSIST_UNIT_SPRITE_W, VehicleWindowState,
    vehicle_side_sprite,
};

use super::{BuildMenuUi, OrderEditState};

const DEPOT_VEHICLE_ROWS: usize = 24;
const DEPOT_LIST_VISIBLE_ROWS: usize = 8;
const ROW_HEIGHT: f32 = 32.0;
const CONSIST_STRIP_W: f32 = CONSIST_UNIT_SPRITE_W * CONSIST_STRIP_MAX_UNITS as f32;

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
    /// Origen de enganche rail por clic A→B (`MoveRailVehicle`).
    pub(crate) reorder_from_slot: Option<usize>,
    /// Origen de drag (índice de fila en la lista del depósito).
    pub(crate) list_drag_from: Option<usize>,
    /// Si `Some`, el drag parte de un sprite de unidad del consist (vagón).
    pub(crate) list_drag_unit_idx: Option<usize>,
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

/// Mini-sprite de una unidad del consist en la fila del depósito.
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotConsistUnitSprite {
    slot: usize,
    unit_idx: usize,
}

/// Botón de venta por fila (icono ✕ rojo, estilo original).
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotSellButton {
    slot: usize,
}

/// Bandera start/stop por fila (`ToggleVehicleRunning`).
#[derive(Component, Clone, Copy)]
pub(crate) struct DepotRunningButton {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DepotRunningLabel {
    slot: usize,
}

/// Zona de drop lateral para vender al soltar el drag.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepotSellDrop {
    /// Vende la unidad arrastrada (`SellVehicle`; cabeza ⇒ cadena completa).
    Unit,
    /// Vende desde la unidad hasta el final del consist.
    Chain,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum DepotPanelButton {
    NewVehicles,
    /// Compra una copia del vehículo seleccionado (motor + órdenes).
    CloneVehicle,
    CenterDepot,
    /// Sube el vehículo seleccionado en la lista del depósito.
    MoveSlotUp,
    /// Baja el vehículo seleccionado en la lista del depósito.
    MoveSlotDown,
    /// Abre la ventana de autoreemplazo para este depósito.
    Autoreplace,
    /// Desengancha la última unidad del consist seleccionado (vía).
    DetachLastUnit,
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
        Vec2::new(480.0, 320.0),
        480.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            })
            .with_children(|body| {
                spawn_classic_scroll_area_with(
                    body,
                    asset_server,
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(1.0),
                        padding: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    LIST_BG,
                    Color::srgb(0.45, 0.39, 0.27),
                    (),
                    (),
                    |list| {
                        for slot in 0..DEPOT_VEHICLE_ROWS {
                            spawn_depot_vehicle_row(list, asset_server, slot);
                        }
                    },
                    ROW_HEIGHT * DEPOT_LIST_VISIBLE_ROWS as f32 + 4.0,
                );
                body.spawn(Node {
                    width: Val::Px(72.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    justify_content: JustifyContent::FlexStart,
                    ..default()
                })
                .with_children(|sell_col| {
                    spawn_depot_sell_drop(sell_col, asset_server, DepotSellDrop::Unit, "Vender");
                    spawn_depot_sell_drop(sell_col, asset_server, DepotSellDrop::Chain, "Cadena");
                });
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
                    "Nuevos",
                    true,
                    false,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::CloneVehicle,
                    "Clonar",
                    true,
                    true,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::CenterDepot,
                    "Loc",
                    false,
                    false,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::MoveSlotUp,
                    "↑",
                    false,
                    false,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::MoveSlotDown,
                    "↓",
                    false,
                    false,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::Autoreplace,
                    "Auto",
                    false,
                    false,
                );
                spawn_depot_button(
                    row,
                    asset_server,
                    DepotPanelButton::DetachLastUnit,
                    "Deseng.",
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
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::horizontal(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(ROW_BG),
            BorderColor::all(ROW_BORDER),
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                Button,
                DepotRunningButton { slot },
                Node {
                    width: Val::Px(22.0),
                    height: Val::Px(ROW_HEIGHT - 4.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.28, 0.32, 0.22)),
                BorderColor::all(BTN_BORDER),
                Interaction::default(),
                BuildMenuUi,
            ))
            .with_children(|btn| {
                btn.spawn((
                    DepotRunningLabel { slot },
                    Text::new("▶"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(TEXT_COLOR),
                ));
            });
            // Tira de unidades como botones hermanos (no anidados) para poder
            // arrastrar vagones a otra formación.
            row.spawn(Node {
                width: Val::Px(CONSIST_STRIP_W),
                height: Val::Px(CONSIST_UNIT_SPRITE_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|strip| {
                for unit_idx in 0..CONSIST_STRIP_MAX_UNITS {
                    strip.spawn((
                        Button,
                        DepotConsistUnitSprite { slot, unit_idx },
                        ImageNode::new(asset_server.load::<Image>(PLACEHOLDER_SPRITE)),
                        Node {
                            width: Val::Px(CONSIST_UNIT_SPRITE_W),
                            height: Val::Px(CONSIST_UNIT_SPRITE_H),
                            display: Display::None,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::NONE),
                        BackgroundColor(Color::NONE),
                        Interaction::default(),
                        BuildMenuUi,
                    ));
                }
            });
            row.spawn((
                Button,
                DepotVehicleRow { slot },
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(ROW_HEIGHT - 4.0),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                BuildMenuUi,
            ))
            .with_children(|inner| {
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
                    ImageNode::new(
                        asset_server.load::<Image>("assets/opengfx/tiles/window_close.png"),
                    ),
                    Node {
                        width: Val::Px(8.0),
                        height: Val::Px(9.0),
                        ..default()
                    },
                )],
            ));
        });
}

fn spawn_depot_sell_drop(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    kind: DepotSellDrop,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            kind,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(56.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(SELL_BG),
            BorderColor::all(SELL_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.98, 0.92, 0.9)),
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
        // Solo cabezas de consist / vehículos sueltos (vagones enganchados no tienen fila).
        .filter(|vehicle| vehicle.pos == depot_pos && vehicle.is_consist_head())
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
    let units = if vehicle.kind == VehicleKind::Train {
        let n = openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id).len();
        if n > 1 {
            format!("  [{n}u]")
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    format!(
        "{}{}  ({}a)  {}/{}",
        vehicle.display_name(),
        units,
        age,
        vehicle.cargo,
        vehicle.capacity
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // sistema ECS Bevy
pub(crate) fn sync_depot_panel(
    depot_state: Res<DepotPanelState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut container_q: Query<
        (
            &DepotRowContainer,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<DepotConsistUnitSprite>,
    >,
    row_interaction_q: Query<
        (&DepotVehicleRow, &Interaction),
        (With<Button>, Without<DepotConsistUnitSprite>),
    >,
    mut row_text_q: Query<(&DepotVehicleRowText, &mut Text), Without<FloatingWindowTitleText>>,
    mut consist_q: Query<
        (
            &DepotConsistUnitSprite,
            &mut ImageNode,
            &mut Node,
            &mut BorderColor,
            &Interaction,
        ),
        (Without<DepotRowContainer>, Without<DepotSellDrop>),
    >,
    mut clone_label_q: Query<
        &mut Text,
        (
            With<DepotCloneLabel>,
            Without<DepotVehicleRowText>,
            Without<FloatingWindowTitleText>,
            Without<DepotRunningLabel>,
        ),
    >,
    mut running_label_q: Query<
        (&DepotRunningLabel, &mut Text),
        (
            Without<DepotCloneLabel>,
            Without<DepotVehicleRowText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut sell_drop_q: Query<
        (
            &DepotSellDrop,
            &mut BackgroundColor,
            &mut BorderColor,
            &Interaction,
        ),
        (
            With<Button>,
            Without<DepotRowContainer>,
            Without<DepotConsistUnitSprite>,
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
        for (_, mut node, _, _) in &mut container_q {
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
    if let Ok(mut label) = clone_label_q.single_mut() {
        **label = "Clonar".to_string();
    }
    let vehicles_here = vehicles_at_depot(&sim, depot_pos);
    let drag_from = depot_state.list_drag_from;
    let drag_unit = depot_state.list_drag_unit_idx;
    let hovered_slot = row_interaction_q.iter().find_map(|(row, interaction)| {
        matches!(*interaction, Interaction::Hovered | Interaction::Pressed).then_some(row.slot)
    });
    for (container, mut node, mut bg, mut border) in &mut container_q {
        let Some(vehicle) = vehicles_here.get(container.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = depot_state.selected_vehicle == Some(vehicle.id);
        let is_drag_source = drag_from == Some(container.slot);
        let is_drop_target = drag_from
            .is_some_and(|from| from != container.slot && hovered_slot == Some(container.slot));
        *bg = if is_drop_target {
            BackgroundColor(Color::srgb(0.42, 0.48, 0.28))
        } else if is_drag_source {
            BackgroundColor(Color::srgb(0.62, 0.54, 0.34))
        } else if selected {
            BackgroundColor(Color::srgb(0.48, 0.41, 0.27))
        } else {
            BackgroundColor(ROW_BG)
        };
        *border = if is_drop_target {
            BorderColor::all(Color::srgb(0.72, 0.88, 0.42))
        } else if selected || is_drag_source {
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
    for (running, mut text) in &mut running_label_q {
        if let Some(vehicle) = vehicles_here.get(running.slot) {
            **text = if vehicle.running {
                "■".to_string()
            } else {
                "▶".to_string()
            };
        } else {
            **text = String::new();
        }
    }
    let dragging = drag_from.is_some();
    for (_kind, mut bg, mut border, interaction) in &mut sell_drop_q {
        let hot = dragging && matches!(*interaction, Interaction::Hovered | Interaction::Pressed);
        *bg = BackgroundColor(if hot {
            Color::srgb(0.78, 0.28, 0.22)
        } else {
            SELL_BG
        });
        *border = BorderColor::all(if hot {
            Color::srgb(0.95, 0.55, 0.4)
        } else {
            SELL_BORDER
        });
    }
    if let Some(trucks) = trucks.as_ref() {
        for (sprite, mut image, mut node, mut border, interaction) in &mut consist_q {
            let Some(head) = vehicles_here.get(sprite.slot) else {
                node.display = Display::None;
                continue;
            };
            let unit_ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, head.id);
            if let Some(&unit_id) = unit_ids.get(sprite.unit_idx)
                && let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id)
            {
                node.display = Display::Flex;
                image.image = vehicle_side_sprite(trucks, unit);
                let dragging_this =
                    drag_from == Some(sprite.slot) && drag_unit == Some(sprite.unit_idx);
                *border = if dragging_this || *interaction == Interaction::Hovered {
                    BorderColor::all(Color::srgb(0.9, 0.78, 0.48))
                } else {
                    BorderColor::all(Color::NONE)
                };
            } else {
                node.display = Display::None;
            }
        }
    } else {
        for (_, _, mut node, _, _) in &mut consist_q {
            node.display = Display::None;
        }
    }
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn depot_panel_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut depot_state: ResMut<DepotPanelState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::Depot {
            depot_state.depot_pos = None;
            depot_state.selected_vehicle = None;
            depot_state.reorder_from_slot = None;
            depot_state.list_drag_from = None;
            depot_state.list_drag_unit_idx = None;
        }
    }
}

/// Inicia drag al pulsar una fila o un sprite de unidad del consist.
pub(crate) fn begin_depot_list_drag(
    mut row_q: Query<
        (&Interaction, &DepotVehicleRow),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
            Without<DepotConsistUnitSprite>,
        ),
    >,
    mut unit_q: Query<
        (&Interaction, &DepotConsistUnitSprite, &Node),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
            Without<DepotVehicleRow>,
        ),
    >,
    mut depot_state: ResMut<DepotPanelState>,
    sim: Res<SimWorld>,
) {
    let Some(depot_pos) = depot_state.depot_pos else {
        return;
    };
    for (interaction, unit, node) in &mut unit_q {
        if *interaction != Interaction::Pressed || node.display == Display::None {
            continue;
        }
        let vehicles = vehicles_at_depot(&sim, depot_pos);
        let Some(head_id) = vehicles.get(unit.slot).map(|v| v.id) else {
            continue;
        };
        depot_state.list_drag_from = Some(unit.slot);
        depot_state.list_drag_unit_idx = Some(unit.unit_idx);
        depot_state.selected_vehicle = Some(head_id);
        return;
    }
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(vehicle_id) = vehicles_at_depot(&sim, depot_pos)
            .get(row.slot)
            .map(|v| v.id)
        else {
            continue;
        };
        depot_state.list_drag_from = Some(row.slot);
        depot_state.list_drag_unit_idx = None;
        depot_state.selected_vehicle = Some(vehicle_id);
    }
}

/// Al soltar: zonas vender, en vía engancha vagones (`MoveRailVehicle`); si no, reordena o clic.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn finish_depot_list_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut depot_state: ResMut<DepotPanelState>,
    row_q: Query<(&DepotVehicleRow, &Interaction), With<Button>>,
    unit_q: Query<(&DepotConsistUnitSprite, &Interaction, &Node), With<Button>>,
    sell_drop_q: Query<(&DepotSellDrop, &Interaction), With<Button>>,
    mut vehicle_window: ResMut<VehicleWindowState>,
    mut vehicle_chain: ResMut<VehicleChainRegistry>,
    mut order_state: ResMut<OrderEditState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    let Some(from_slot) = depot_state.list_drag_from else {
        return;
    };
    if mouse.pressed(MouseButton::Left) {
        return;
    }
    let unit_idx = depot_state.list_drag_unit_idx.take();
    depot_state.list_drag_from = None;
    let Some(depot_pos) = depot_state.depot_pos else {
        return;
    };
    let move_chain = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let now = time.elapsed_secs();

    let sell_drop = sell_drop_q.iter().find_map(|(kind, interaction)| {
        matches!(*interaction, Interaction::Hovered | Interaction::Pressed).then_some(*kind)
    });
    if let Some(kind) = sell_drop
        && let Some(vehicle_id) =
            resolve_depot_drag_vehicle_id(&sim, depot_pos, from_slot, unit_idx)
    {
        let sell_chain = kind == DepotSellDrop::Chain || move_chain;
        let _ = apply_depot_sell_drop(
            &mut sim,
            vehicle_id,
            sell_chain,
            &mut depot_state,
            &mut order_state,
            &mut pending,
            &mut hud_feedback,
            now,
        );
        return;
    }

    let drop_slot = row_q
        .iter()
        .find_map(|(row, interaction)| {
            matches!(*interaction, Interaction::Hovered | Interaction::Pressed).then_some(row.slot)
        })
        .or_else(|| {
            unit_q.iter().find_map(|(unit, interaction, node)| {
                (node.display != Display::None
                    && matches!(*interaction, Interaction::Hovered | Interaction::Pressed))
                .then_some(unit.slot)
            })
        });

    if let Some(to_slot) = drop_slot
        && to_slot != from_slot
    {
        if depot_is_rail(&sim, depot_pos)
            && let Some((head_id, unit_id)) =
                resolve_rail_depot_drag_move(&sim, depot_pos, from_slot, to_slot, unit_idx)
        {
            match crate::network::apply_player_command(
                &mut sim.state,
                &Command::MoveRailVehicle {
                    head_id,
                    unit_id,
                    after_id: None,
                    move_chain,
                },
            ) {
                Ok(()) => {
                    pending.pending = true;
                    depot_state.selected_vehicle = Some(head_id);
                    depot_state.reorder_from_slot = None;
                }
                Err(e) => {
                    push_build_command_error(&mut hud_feedback, e, now);
                }
            }
            return;
        }
        match crate::network::apply_player_command(
            &mut sim.state,
            &Command::DepotReorderVehicleSlot {
                depot_pos,
                from_slot,
                to_slot,
            },
        ) {
            Ok(()) => {
                pending.pending = true;
                if let Some(vehicle) = vehicles_at_depot(&sim, depot_pos).get(to_slot) {
                    depot_state.selected_vehicle = Some(vehicle.id);
                }
            }
            Err(e) => push_build_command_error(&mut hud_feedback, e, now),
        }
        depot_state.reorder_from_slot = None;
        return;
    }

    activate_depot_row_click(
        from_slot,
        &mut depot_state,
        &mut vehicle_window,
        &mut vehicle_chain,
        &mut sim,
        &mut pending,
        &mut hud_feedback,
        now,
    );
}

fn resolve_depot_drag_vehicle_id(
    sim: &SimWorld,
    depot_pos: TileCoord,
    from_slot: usize,
    unit_idx: Option<usize>,
) -> Option<u32> {
    let vehicles = vehicles_at_depot(sim, depot_pos);
    let head = vehicles.get(from_slot)?;
    if let Some(idx) = unit_idx {
        let ids = consist_unit_ids(&sim.state.vehicles, head.id);
        ids.get(idx).copied()
    } else {
        Some(head.id)
    }
}

/// Vende unidad o cola (`sell_chain`: desde la unidad hasta el final).
#[allow(clippy::too_many_arguments)]
fn apply_depot_sell_drop(
    sim: &mut SimWorld,
    vehicle_id: u32,
    sell_chain: bool,
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    now: f32,
) -> Result<(), ()> {
    let ids = if sell_chain {
        let mut chain = Vec::new();
        let mut cur = Some(vehicle_id);
        while let Some(id) = cur {
            chain.push(id);
            cur = sim
                .state
                .vehicles
                .iter()
                .find(|v| v.id == id)
                .and_then(|v| v.next_unit);
        }
        chain
    } else {
        vec![vehicle_id]
    };
    if ids.is_empty() {
        return Err(());
    }
    // Cola → cabeza para no romper enlaces al vender del medio.
    for id in ids.iter().rev().copied() {
        match crate::network::apply_player_command(&mut sim.state, &Command::SellVehicle(id)) {
            Ok(()) => {
                pending.pending = true;
                if depot_state.selected_vehicle == Some(id) {
                    depot_state.selected_vehicle = None;
                }
                if order_state.vehicle_id == Some(id) {
                    order_state.clear();
                }
            }
            Err(e) => {
                push_build_command_error(hud_feedback, e, now);
                return Err(());
            }
        }
    }
    Ok(())
}

fn vehicle_is_wagon(vehicle: &openttdrs_core::Vehicle) -> bool {
    vehicle
        .engine_id
        .and_then(engine_by_id)
        .is_some_and(openttdrs_core::EngineDef::is_wagon)
}

/// Destino de enganche tras drag en depósito de vía.
///
/// - Sprite de unidad (`unit_idx`): mueve esa unidad (salvo loco en índice 0).
/// - Fila de vagón suelto: engancha la cabeza-vagón al tren destino.
/// - Fila de locomotora: `None` → el caller reordena la lista.
fn resolve_rail_depot_drag_move(
    sim: &SimWorld,
    depot_pos: TileCoord,
    from_slot: usize,
    to_slot: usize,
    unit_idx: Option<usize>,
) -> Option<(u32, u32)> {
    let vehicles = vehicles_at_depot(sim, depot_pos);
    let from = vehicles.get(from_slot)?;
    let to = vehicles.get(to_slot)?;
    if from.id == to.id {
        return None;
    }
    if let Some(idx) = unit_idx {
        let ids = consist_unit_ids(&sim.state.vehicles, from.id);
        let &unit_id = ids.get(idx)?;
        if idx == 0 && !vehicle_is_wagon(from) {
            // Arrastrar la loco completa: no fusionar trenes por drag de fila.
            return None;
        }
        return Some((to.id, unit_id));
    }
    if vehicle_is_wagon(from) {
        return Some((to.id, from.id));
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn activate_depot_row_click(
    slot: usize,
    depot_state: &mut DepotPanelState,
    vehicle_window: &mut VehicleWindowState,
    vehicle_chain: &mut VehicleChainRegistry,
    sim: &mut SimWorld,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    now: f32,
) {
    let Some(depot_pos) = depot_state.depot_pos else {
        return;
    };
    let Some(vehicle_id) = vehicles_at_depot(sim, depot_pos).get(slot).map(|v| v.id) else {
        return;
    };
    // Rail: segundo clic en otra fila engancha/reordena (MoveRailVehicle).
    if depot_is_rail(sim, depot_pos) {
        if let Some(from_slot) = depot_state.reorder_from_slot
            && from_slot != slot
        {
            let vehicles = vehicles_at_depot(sim, depot_pos);
            if let (Some(from), Some(to)) = (vehicles.get(from_slot), vehicles.get(slot)) {
                let (head_id, unit_id) = if from.next_unit.is_none()
                    && from
                        .engine_id
                        .and_then(engine_by_id)
                        .is_some_and(openttdrs_core::EngineDef::is_wagon)
                {
                    (to.id, from.id)
                } else if to.next_unit.is_none()
                    && to
                        .engine_id
                        .and_then(engine_by_id)
                        .is_some_and(openttdrs_core::EngineDef::is_wagon)
                {
                    (from.id, to.id)
                } else {
                    (to.id, from.id)
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::MoveRailVehicle {
                        head_id,
                        unit_id,
                        after_id: None,
                        move_chain: false,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(hud_feedback, e, now);
                    }
                }
            }
            depot_state.reorder_from_slot = None;
            depot_state.selected_vehicle = Some(vehicle_id);
            return;
        }
        depot_state.reorder_from_slot = Some(slot);
    }
    depot_state.selected_vehicle = Some(vehicle_id);
    // Solo View; órdenes/detalles se abren desde botones de la ventana (#173).
    vehicle_window.open_or_focus(vehicle_chain, vehicle_id);
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_depot_panel_buttons(
    mut q: Query<(&Interaction, &DepotPanelButton), (Changed<Interaction>, With<Button>)>,
    mut sell_q: Query<
        (&Interaction, &DepotSellButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
            Without<DepotVehicleRow>,
            Without<DepotRunningButton>,
        ),
    >,
    mut running_q: Query<
        (&Interaction, &DepotRunningButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DepotPanelButton>,
            Without<DepotVehicleRow>,
            Without<DepotSellButton>,
        ),
    >,
    mut depot_state: ResMut<DepotPanelState>,
    mut order_state: ResMut<OrderEditState>,
    mut buy_state: ResMut<BuyVehicleWindowState>,
    mut autoreplace: ResMut<AutoreplaceWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    time: Res<Time>,
) {
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
        match crate::network::apply_player_command(
            &mut sim.state,
            &Command::SellVehicle(vehicle_id),
        ) {
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

    for (interaction, running) in &mut running_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(depot_pos) = depot_state.depot_pos else {
            continue;
        };
        let Some(vehicle_id) = vehicles_at_depot(&sim, depot_pos)
            .get(running.slot)
            .map(|v| v.id)
        else {
            continue;
        };
        match crate::network::apply_player_command(
            &mut sim.state,
            &Command::ToggleVehicleRunning(vehicle_id),
        ) {
            Ok(()) => {
                pending.pending = true;
                depot_state.selected_vehicle = Some(vehicle_id);
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
            DepotPanelButton::Autoreplace => {
                autoreplace.open_for_depot(depot_pos);
            }
            DepotPanelButton::DetachLastUnit => {
                if !depot_is_rail(&sim, depot_pos) {
                    continue;
                }
                let Some(head_id) = depot_state.selected_vehicle else {
                    continue;
                };
                let units = consist_unit_ids(&sim.state.vehicles, head_id);
                let Some(&tail_id) = units.last() else {
                    continue;
                };
                if units.len() < 2 {
                    continue;
                }
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::DetachConsistUnit(tail_id),
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        depot_state.selected_vehicle = Some(head_id);
                    }
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::MoveSlotUp | DepotPanelButton::MoveSlotDown => {
                let vehicles = vehicles_at_depot(&sim, depot_pos);
                let Some(selected) = depot_state.selected_vehicle else {
                    continue;
                };
                let Some(from_slot) = vehicles.iter().position(|v| v.id == selected) else {
                    continue;
                };
                let to_slot = match button {
                    DepotPanelButton::MoveSlotUp => from_slot.checked_sub(1),
                    DepotPanelButton::MoveSlotDown => {
                        let next = from_slot + 1;
                        (next < vehicles.len()).then_some(next)
                    }
                    _ => None,
                };
                let Some(to_slot) = to_slot else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::DepotReorderVehicleSlot {
                        depot_pos,
                        from_slot,
                        to_slot,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            DepotPanelButton::CloneVehicle => {
                let source_id = depot_state
                    .selected_vehicle
                    .or_else(|| vehicles_at_depot(&sim, depot_pos).first().map(|v| v.id));
                let Some(source_id) = source_id else {
                    continue;
                };
                match crate::network::apply_player_command(
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
