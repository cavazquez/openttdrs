//! Ventana flotante de depósito (carretera y vía), estilo `OpenTTD`.
//!
//! Lista con bandera start/stop, tira de sprites del consist y venta por fila.
//! Drag a zonas «Vender» / «Vender cadena»; en vía, Ctrl+drag mueve la cola
//! (`MoveRailVehicle.move_chain`). Barra: Nuevos / Clonar / Centrar (+ secundarios).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::widget::ImageNode;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{MAX_DEPOT_NAME_CHARS, consist_unit_ids, engine_by_id};

use crate::camera::tile_camera_world_pos;
use crate::i18n::{Locale, localized_text};
use crate::render::{
    MapPreviewCamera, NewGrfTrainSpriteCache, PrimaryGameCamera, RemapMapVisualsPending,
    TruckHandles,
};
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::autoreplace_window::AutoreplaceWindowState;
use crate::ui::buy_window::{BuyVehicleWindowState, prepare_buy_window_for_depot};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error, push_vehicle_start_stop_error};
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_window::{
    CONSIST_STRIP_MAX_UNITS, CONSIST_UNIT_SPRITE_H, CONSIST_UNIT_SPRITE_W, VehicleWindowState,
    vehicle_side_sprite_for_sim,
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
    /// El campo de nombre está visible y recibe entrada de teclado.
    pub(crate) rename_editing: bool,
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

#[derive(Component)]
pub(crate) struct DepotRenameRow;

#[derive(Component)]
pub(crate) struct DepotRenameInput;

#[derive(Component, Clone, Copy)]
pub(crate) enum DepotRenameButton {
    Apply,
    Cancel,
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
    /// Abre el campo de nombre del depósito.
    Rename,
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
            .spawn((
                DepotRenameRow,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    display: Display::None,
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                row.spawn((
                    DepotRenameInput,
                    EditableText::new(""),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(TEXT_COLOR),
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(22.0),
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(BTN_BORDER),
                ));
                spawn_depot_rename_action(row, asset_server, DepotRenameButton::Apply, "OK");
                spawn_depot_rename_action(row, asset_server, DepotRenameButton::Cancel, "No");
            });
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
                    DepotPanelButton::Rename,
                    "Nom.",
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

fn spawn_depot_rename_action(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: DepotRenameButton,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                min_width: Val::Px(36.0),
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
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(TEXT_COLOR),
            ));
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

fn depot_metadata(sim: &SimWorld, depot_pos: TileCoord) -> Option<&openttdrs_core::SavDepot> {
    let depot_id = sim
        .state
        .map
        .get(depot_pos)
        .and_then(openttdrs_core::depot_id_from_tile)?;
    sim.state
        .depots
        .iter()
        .find(|depot| depot.depot_id == depot_id)
}

fn generated_depot_title(locale: Locale, sim: &SimWorld, depot_pos: TileCoord) -> Option<String> {
    let depot = depot_metadata(sim, depot_pos)?;
    if !depot.name.is_empty() {
        return Some(depot.name.clone());
    }
    let town_name = depot
        .town_id
        .and_then(|town_id| sim.state.towns.iter().find(|town| town.id == town_id))
        .map(|town| town.name.as_str())
        .filter(|name| !name.is_empty())?;
    let kind = match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => "Depósito de Trenes",
        Some(TileKind::ShipDepot) => "Depósito de Barcos",
        Some(TileKind::RoadDepot) => "Depósito de Carretera",
        _ => return None,
    };
    let serial = (depot.town_cn != 0).then(|| format!(" #{}", u32::from(depot.town_cn) + 1));
    Some(format!(
        "{} {}{}",
        town_name,
        localized_text(locale, kind),
        serial.unwrap_or_default()
    ))
}

fn depot_title(locale: Locale, sim: &SimWorld, depot_pos: TileCoord) -> String {
    if let Some(title) = generated_depot_title(locale, sim, depot_pos) {
        return title;
    }
    let nombre = match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => "Depósito de Trenes",
        Some(TileKind::ShipDepot) => "Depósito de Barcos",
        Some(TileKind::Airport) => "Hangar de Aviones",
        _ => "Depósito de Carretera",
    };
    format!(
        "{} ({}, {})",
        localized_text(locale, nombre),
        depot_pos.x,
        depot_pos.y
    )
}

fn vehicle_is_in_depot_panel(
    sim: &SimWorld,
    depot_pos: TileCoord,
    vehicle: &openttdrs_core::Vehicle,
) -> bool {
    vehicle.pos == depot_pos && openttdrs_core::vehicle_is_in_depot(&sim.state.map, vehicle)
}

fn vehicles_at_depot(sim: &SimWorld, depot_pos: TileCoord) -> Vec<&openttdrs_core::Vehicle> {
    let mut vehicles: Vec<_> = sim
        .state
        .vehicles
        .iter()
        // Solo cabezas de consist / vehículos sueltos (vagones enganchados no tienen fila).
        .filter(|vehicle| {
            vehicle.is_consist_head() && vehicle_is_in_depot_panel(sim, depot_pos, vehicle)
        })
        .collect();
    vehicles.sort_by(|a, b| match (a.depot_display_slot, b.depot_display_slot) {
        (Some(sa), Some(sb)) => sa.cmp(&sb).then_with(|| a.id.cmp(&b.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.id.cmp(&b.id),
    });
    vehicles
}

fn depot_vehicle_row_label(
    locale: Locale,
    sim: &SimWorld,
    vehicle: &openttdrs_core::Vehicle,
) -> String {
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
    let age_suffix = match locale {
        Locale::Es => "a",
        Locale::En => "y",
    };
    format!(
        "{}{}  ({}{age_suffix})  {}/{}",
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
    prefs: Res<ClientPreferences>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut rename_row_q: Query<&mut Node, With<DepotRenameRow>>,
    mut container_q: Query<
        (
            &DepotRowContainer,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Without<DepotConsistUnitSprite>, Without<DepotRenameRow>),
    >,
    row_interaction_q: Query<
        (&DepotVehicleRow, &Interaction),
        (With<Button>, Without<DepotConsistUnitSprite>),
    >,
    mut row_text_q: Query<(&DepotVehicleRowText, &mut Text), Without<FloatingWindowTitleText>>,
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
    let locale = prefs.locale();
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Depot)
    else {
        return;
    };
    let Some(depot_pos) = depot_state.depot_pos else {
        *vis = Visibility::Hidden;
        if let Ok(mut row) = rename_row_q.single_mut() {
            row.display = Display::None;
        }
        for (_, mut node, _, _) in &mut container_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;
    if let Ok(mut row) = rename_row_q.single_mut() {
        row.display = if depot_state.rename_editing {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Depot)
    {
        **title = depot_title(locale, &sim, depot_pos);
    }
    if let Ok(mut label) = clone_label_q.single_mut() {
        **label = localized_text(locale, "Clonar");
    }
    let vehicles_here = vehicles_at_depot(&sim, depot_pos);
    let drag_from = depot_state.list_drag_from;
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
            **text = depot_vehicle_row_label(locale, &sim, vehicle);
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
}

/// Sincroniza los mini-sprites del depósito en un sistema separado para poder
/// resolver capas NewGRF con el cache de imágenes sin sobrepasar el límite de
/// parámetros ECS del panel principal.
pub(crate) fn sync_depot_panel_consist(
    depot_state: Res<DepotPanelState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut cache: ResMut<NewGrfTrainSpriteCache>,
    mut images: ResMut<Assets<Image>>,
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
) {
    let Some(depot_pos) = depot_state.depot_pos else {
        for (_, _, mut node, _, _) in &mut consist_q {
            node.display = Display::None;
        }
        return;
    };
    let Some(trucks) = trucks.as_ref() else {
        for (_, _, mut node, _, _) in &mut consist_q {
            node.display = Display::None;
        }
        return;
    };
    let vehicles_here = vehicles_at_depot(&sim, depot_pos);
    let drag_from = depot_state.list_drag_from;
    let drag_unit = depot_state.list_drag_unit_idx;
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
            image.image = vehicle_side_sprite_for_sim(trucks, &sim, unit, &mut cache, &mut images);
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
            depot_state.rename_editing = false;
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
                    pending.request_full();
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
                pending.request_full();
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
                pending.request_full();
                if depot_state.selected_vehicle == Some(id) {
                    depot_state.selected_vehicle = None;
                }
                if order_state.is_open_for(id) {
                    order_state.close_vehicle(id);
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
                    Ok(()) => pending.request_full(),
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
    mut rename_input_q: Query<&mut EditableText, With<DepotRenameInput>>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    prefs: Option<Res<ClientPreferences>>,
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
                pending.request_full();
                if depot_state.selected_vehicle == Some(vehicle_id) {
                    depot_state.selected_vehicle = None;
                }
                if order_state.is_open_for(vehicle_id) {
                    order_state.close_vehicle(vehicle_id);
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
                pending.request_full();
                depot_state.selected_vehicle = Some(vehicle_id);
            }
            Err(e) => push_vehicle_start_stop_error(
                &mut hud_feedback,
                &mut sim,
                e,
                vehicle_id,
                prefs.as_ref().map_or(Locale::Es, |prefs| prefs.locale()),
                time.elapsed_secs(),
            ),
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
                prepare_buy_window_for_depot(&mut buy_state, &sim, depot_pos);
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
                        pending.request_full();
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
                    Ok(()) => pending.request_full(),
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
                        pending.request_full();
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
            DepotPanelButton::Rename => {
                depot_state.rename_editing = true;
                if let Some(depot) = depot_metadata(&sim, depot_pos)
                    && let Ok(mut editable) = rename_input_q.single_mut()
                {
                    editable.editor_mut().set_text(&depot.name);
                }
            }
        }
    }
}

fn apply_depot_rename(
    depot_state: &mut DepotPanelState,
    sim: &mut SimWorld,
    hud_feedback: &mut HudBuildFeedback,
    rename_input_q: &Query<&EditableText, With<DepotRenameInput>>,
    elapsed_secs: f32,
) {
    let Some(depot_pos) = depot_state.depot_pos else {
        return;
    };
    let name = rename_input_q
        .single()
        .ok()
        .map(|editable| editable.value().to_string())
        .filter(|name| !name.trim().is_empty());
    match crate::network::apply_player_command(
        &mut sim.state,
        &Command::RenameDepot { depot_pos, name },
    ) {
        Ok(()) => depot_state.rename_editing = false,
        Err(error) => push_build_command_error(hud_feedback, error, elapsed_secs),
    }
}

/// Aplica o cancela el nombre desde los botones del editor del depósito.
pub(crate) fn handle_depot_rename_buttons(
    mut buttons: Query<(&Interaction, &DepotRenameButton), (Changed<Interaction>, With<Button>)>,
    mut depot_state: ResMut<DepotPanelState>,
    rename_input_q: Query<&EditableText, With<DepotRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, action) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            DepotRenameButton::Cancel => depot_state.rename_editing = false,
            DepotRenameButton::Apply => apply_depot_rename(
                &mut depot_state,
                &mut sim,
                &mut hud_feedback,
                &rename_input_q,
                time.elapsed_secs(),
            ),
        }
    }
}

/// Enter aplica el nombre; Escape cancela la edición del depósito.
pub(crate) fn depot_rename_keyboard(
    mut depot_state: ResMut<DepotPanelState>,
    keys: Res<ButtonInput<KeyCode>>,
    rename_input_q: Query<&EditableText, With<DepotRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !depot_state.rename_editing {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        depot_state.rename_editing = false;
    } else if keys.just_pressed(KeyCode::Enter) {
        apply_depot_rename(
            &mut depot_state,
            &mut sim,
            &mut hud_feedback,
            &rename_input_q,
            time.elapsed_secs(),
        );
    }
}

/// Teclas alfanuméricas en el campo de nombre del depósito.
pub(crate) fn depot_rename_editable_keyboard(
    depot_state: Res<DepotPanelState>,
    mut key_events: MessageReader<KeyboardInput>,
    mut rename_input_q: Query<&mut EditableText, With<DepotRenameInput>>,
) {
    if !depot_state.rename_editing {
        return;
    }
    let Ok(mut editable) = rename_input_q.single_mut() else {
        return;
    };
    for event in key_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if matches!(event.logical_key, Key::Backspace) {
            editable.queue_edit(bevy::text::TextEdit::Backspace);
            continue;
        }
        if matches!(event.logical_key, Key::Delete) {
            editable.queue_edit(bevy::text::TextEdit::Delete);
            continue;
        }
        let Some(text) = &event.text else {
            continue;
        };
        for character in text.chars() {
            if !character.is_control()
                && editable.value().chars().count() < MAX_DEPOT_NAME_CHARS - 1
            {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(character.to_string()),
                ));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn world_with_bus() -> (SimWorld, TileCoord, u32) {
        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        apply_command(&mut state, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
        apply_command(&mut state, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
        apply_command(
            &mut state,
            &Command::BuildVehicleAtDepot(depot, openttdrs_core::ENGINE_BUS_MPS),
        )
        .unwrap();
        let vehicle_id = state.vehicles[0].id;
        (
            SimWorld {
                state,
                ..SimWorld::default()
            },
            depot,
            vehicle_id,
        )
    }

    fn insert_depot_resources(world: &mut World, sim: SimWorld, depot: TileCoord) {
        world.insert_resource(sim);
        world.init_resource::<DepotPanelState>();
        world.init_resource::<OrderEditState>();
        world.init_resource::<BuyVehicleWindowState>();
        world.init_resource::<AutoreplaceWindowState>();
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.resource_mut::<DepotPanelState>().depot_pos = Some(depot);
    }

    #[test]
    fn clone_button_duplicates_selected_depot_vehicle() {
        let (sim, depot, source_id) = world_with_bus();
        let mut world = World::new();
        insert_depot_resources(&mut world, sim, depot);
        world.resource_mut::<DepotPanelState>().selected_vehicle = Some(source_id);
        world.spawn((Button, DepotPanelButton::CloneVehicle, Interaction::Pressed));

        world.run_system_once(handle_depot_panel_buttons).unwrap();

        let sim = world.resource::<SimWorld>();
        assert_eq!(sim.state.vehicles.len(), 2);
        assert_ne!(
            world.resource::<DepotPanelState>().selected_vehicle,
            Some(source_id)
        );
        assert!(world.resource::<RemapMapVisualsPending>().is_pending());
    }

    #[test]
    fn sell_button_removes_vehicle_from_depot() {
        let (sim, depot, _) = world_with_bus();
        let mut world = World::new();
        insert_depot_resources(&mut world, sim, depot);
        world.spawn((Button, DepotSellButton { slot: 0 }, Interaction::Pressed));

        world.run_system_once(handle_depot_panel_buttons).unwrap();

        assert!(world.resource::<SimWorld>().state.vehicles.is_empty());
        assert!(world.resource::<RemapMapVisualsPending>().is_pending());
    }

    #[test]
    fn new_vehicles_button_opens_buy_window_for_depot() {
        let (sim, depot, _) = world_with_bus();
        let mut world = World::new();
        insert_depot_resources(&mut world, sim, depot);
        world.spawn((Button, DepotPanelButton::NewVehicles, Interaction::Pressed));

        world.run_system_once(handle_depot_panel_buttons).unwrap();

        assert_eq!(
            world.resource::<BuyVehicleWindowState>().depot_pos,
            Some(depot)
        );
    }

    #[test]
    fn rename_button_stores_custom_depot_name() {
        let (sim, depot, _) = world_with_bus();
        let mut world = World::new();
        insert_depot_resources(&mut world, sim, depot);
        world.spawn((DepotRenameInput, EditableText::new("Terminal Central")));
        world.spawn((Button, DepotRenameButton::Apply, Interaction::Pressed));

        world.run_system_once(handle_depot_rename_buttons).unwrap();

        let sim = world.resource::<SimWorld>();
        assert_eq!(sim.state.depots[0].name, "Terminal Central");
        assert!(!world.resource::<DepotPanelState>().rename_editing);
    }

    #[test]
    fn running_button_toggles_vehicle_state() {
        let (sim, depot, vehicle_id) = world_with_bus();
        let mut world = World::new();
        insert_depot_resources(&mut world, sim, depot);
        world.spawn((Button, DepotRunningButton { slot: 0 }, Interaction::Pressed));

        world.run_system_once(handle_depot_panel_buttons).unwrap();

        assert!(
            world
                .resource::<SimWorld>()
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == vehicle_id)
                .unwrap()
                .running
        );
    }

    #[test]
    fn depot_panel_lists_only_ships_with_native_depot_state() {
        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(3, 3);
        state.map.set_kind(depot, TileKind::ShipDepot).unwrap();

        let mut inside = Vehicle::new(1, VehicleKind::Ship, depot, depot);
        inside.ship_state = openttdrs_core::ship_movement::SHIP_STATE_DEPOT;
        let mut leaving = Vehicle::new(2, VehicleKind::Ship, depot, depot);
        leaving.ship_state = openttdrs_core::ship_movement::SHIP_STATE_TRACK_X;
        state.vehicles = vec![leaving, inside];
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };

        let listed = vehicles_at_depot(&sim, depot);
        assert_eq!(
            listed.iter().map(|vehicle| vehicle.id).collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn depot_chrome_uses_locale_without_translating_vehicle_data() {
        let (sim, depot, _) = world_with_bus();
        let vehicle = &sim.state.vehicles[0];

        assert_eq!(depot_title(Locale::En, &sim, depot), "Road depot (2, 2)");
        let english_row = depot_vehicle_row_label(Locale::En, &sim, vehicle);
        assert!(english_row.contains(&vehicle.display_name()));
        assert!(english_row.contains("(0y)"));
        assert!(english_row.ends_with(&format!("{}/{}", vehicle.cargo, vehicle.capacity)));

        assert_eq!(
            depot_title(Locale::Es, &sim, depot),
            "Depósito de Carretera (2, 2)"
        );
        assert!(depot_vehicle_row_label(Locale::Es, &sim, vehicle).contains("(0a)"));
    }

    #[test]
    fn depot_title_resolves_custom_and_generated_native_names() {
        let (mut sim, depot, _) = world_with_bus();
        sim.state.towns.push(openttdrs_core::Town {
            id: 7,
            pos: TileCoord::new(2, 2),
            name: "Villa Central".into(),
            ..Default::default()
        });
        sim.state.depots[0].town_id = Some(7);
        sim.state.depots[0].town_cn = 2;

        assert_eq!(
            depot_title(Locale::En, &sim, depot),
            "Villa Central Road depot #3"
        );
        assert_eq!(
            depot_title(Locale::Es, &sim, depot),
            "Villa Central Depósito de Carretera #3"
        );

        sim.state.depots[0].name = "Taller Norte".into();
        assert_eq!(depot_title(Locale::En, &sim, depot), "Taller Norte");
    }
}
