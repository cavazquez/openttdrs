use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::save;
use openttdrs_core::{
    Command, Map, TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder, apply_command,
};
#[cfg(not(test))]
use std::path::Path;

use crate::iso::{ISO_HW, ISO_QH, tile_pos, world_pos_to_tile_coord};
use crate::render::{
    IndustryPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, VehicleIndex,
};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;
use crate::ui::industry_panel::IndustryPanelState;

use super::super::hud::SelectedTileInfo;
use super::{
    BuildMenuAction, BuildMenuUi, DragBuildState, MinimapCell, MinimapRoot, MinimapViewport,
    OrderEditState, OrderPanelButton, OrderPanelRoot, OrderPanelText, SaveMenuAction,
    StationBuildState, ToolButtonGroup, ToolSelectButton, ToolbarCloseButton, ToolbarGroup,
    ToolbarGroupButton, ToolbarState, ToolbarTooltipTarget, TooltipBox, TooltipText, UiToolState,
};

const MINIMAP_COLS: u32 = 64;
const MINIMAP_ROWS: u32 = 40;
const MINIMAP_CELL: f32 = 3.0;
const MINIMAP_PAD: f32 = 6.0;
const MINIMAP_RIGHT: f32 = 10.0;
const MINIMAP_BOTTOM: f32 = 10.0;

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
pub(crate) struct OrderPanelRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct OrderPanelRowText {
    slot: usize,
}

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

const DEPOT_VEHICLE_ROWS: usize = 8;
const ORDER_PANEL_ROWS: usize = 10;

#[derive(Resource, Default)]
pub(crate) struct StationCargoPanelState {
    pub(crate) station_pos: Option<TileCoord>,
}

#[derive(Component)]
pub(crate) struct StationCargoPanelRoot;

#[derive(Component)]
pub(crate) struct StationCargoPanelText;

#[derive(Component, Clone, Copy)]
pub(crate) enum StationCargoPanelButton {
    Close,
}

fn cancel_placement(drag_state: &mut DragBuildState) {
    drag_state.armed = false;
    drag_state.start_tile = None;
    drag_state.last_tile = None;
    drag_state.last_action = None;
    drag_state.pending_tiles.clear();
}

pub(crate) fn toolbar_group_interaction(
    mut q: Query<(&Interaction, &ToolbarGroup), (Changed<Interaction>, With<ToolbarGroupButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for (interaction, group) in &mut q {
        if *interaction == Interaction::Pressed {
            if toolbar_state.active_group == Some(*group) {
                toolbar_state.active_group = None;
                tool_state.active_tool = None;
                cancel_placement(&mut drag_state);
            } else {
                toolbar_state.active_group = Some(*group);
            }
        }
    }
}

pub(crate) fn update_toolbar_group_visuals(
    toolbar_state: Res<ToolbarState>,
    mut q: Query<
        (
            &ToolbarGroup,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<ToolbarGroupButton>,
    >,
) {
    for (group, interaction, mut bg, mut border) in &mut q {
        let is_active = Some(*group) == toolbar_state.active_group;
        *bg = if is_active && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.78, 0.68, 0.43))
        } else if is_active && *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.7, 0.61, 0.38))
        } else if Some(*group) == toolbar_state.active_group {
            BackgroundColor(Color::srgb(0.62, 0.54, 0.34))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.42, 0.36, 0.24))
        } else {
            BackgroundColor(Color::srgb(0.33, 0.28, 0.19))
        };
        *border = if is_active {
            BorderColor::all(Color::srgb(0.86, 0.76, 0.5))
        } else {
            BorderColor::all(Color::srgb(0.64, 0.57, 0.39))
        };
    }
}

pub(crate) fn update_toolbar_tool_visibility(
    toolbar_state: Res<ToolbarState>,
    mut q: Query<(&ToolButtonGroup, &mut Node)>,
) {
    if !toolbar_state.is_changed() {
        return;
    }
    for (tool_group, mut node) in &mut q {
        node.display = if Some(tool_group.0) == toolbar_state.active_group {
            Display::Flex
        } else {
            Display::None
        };
        let offset = match toolbar_state.active_group {
            Some(ToolbarGroup::Rail) => -112.0,
            Some(ToolbarGroup::Road) => -56.0,
            Some(ToolbarGroup::Economy) => 0.0,
            Some(ToolbarGroup::Info) => 56.0,
            Some(ToolbarGroup::Settings) => 112.0,
            None => 0.0,
        };
        node.margin.left = Val::Px(offset);
    }
}

pub(crate) fn close_toolbar_panel_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

pub(crate) fn close_toolbar_button_interaction(
    q: Query<&Interaction, (Changed<Interaction>, With<ToolbarCloseButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

fn toolbar_group_for_action(action: BuildMenuAction) -> ToolbarGroup {
    match action {
        BuildMenuAction::Rail
        | BuildMenuAction::RailDepot
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel => ToolbarGroup::Rail,
        BuildMenuAction::Road
        | BuildMenuAction::RoadX
        | BuildMenuAction::RoadY
        | BuildMenuAction::RoadDepot
        | BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::BusStop
        | BuildMenuAction::Station
        | BuildMenuAction::Clear => ToolbarGroup::Road,
        BuildMenuAction::Orders => ToolbarGroup::Info,
        BuildMenuAction::BuildHouse
        | BuildMenuAction::BuildCoalMine
        | BuildMenuAction::BuildIronOreMine
        | BuildMenuAction::BuildGoldMine
        | BuildMenuAction::BuildOilWell
        | BuildMenuAction::BuildOilRefinery
        | BuildMenuAction::BuildFactory
        | BuildMenuAction::BuildSawmill
        | BuildMenuAction::BuildForest
        | BuildMenuAction::BuildFarm => ToolbarGroup::Economy,
    }
}

pub(crate) fn hide_tool_when_panel_closed(
    toolbar_state: Res<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    let Some(action) = tool_state.active_tool else {
        return;
    };
    if toolbar_state.active_group != Some(toolbar_group_for_action(action)) {
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

/// El boton del menu selecciona la herramienta activa para aplicar en el mapa.
#[allow(clippy::type_complexity)]
pub(crate) fn build_menu_interaction(
    mut q: Query<(&Interaction, &BuildMenuAction), (Changed<Interaction>, With<Button>)>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        tool_state.active_tool = Some(*action);
        cancel_placement(&mut drag_state);
    }
}

/// Resalta el boton de herramienta actualmente activo.
pub(crate) fn update_tool_button_visuals(
    tool_state: Res<UiToolState>,
    mut q: Query<
        (
            &BuildMenuAction,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<ToolSelectButton>,
    >,
) {
    for (action, interaction, mut bg, mut border) in &mut q {
        let is_active = tool_state
            .active_tool
            .is_some_and(|active| active == *action);
        *bg = if is_active && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.76, 0.67, 0.42))
        } else if is_active && *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.68, 0.59, 0.37))
        } else if is_active {
            BackgroundColor(Color::srgb(0.6, 0.52, 0.33))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.4, 0.34, 0.23))
        } else {
            BackgroundColor(Color::srgb(0.28, 0.24, 0.16))
        };
        *border = if is_active {
            BorderColor::all(Color::srgb(0.84, 0.74, 0.5))
        } else {
            BorderColor::all(Color::srgb(0.64, 0.57, 0.39))
        };
    }
}

pub(crate) fn update_toolbar_tooltip(
    mut tooltip_q: Query<&mut Node, With<TooltipBox>>,
    mut text_q: Query<&mut Text, With<TooltipText>>,
    target_q: Query<(&Interaction, &ToolbarTooltipTarget)>,
) {
    let mut hovered: Option<&'static str> = None;
    for (interaction, tip) in &target_q {
        if *interaction == Interaction::Hovered {
            hovered = Some(tip.text);
            break;
        }
    }

    let Ok(mut tooltip_text) = text_q.single_mut() else {
        return;
    };
    let Ok(mut node) = tooltip_q.single_mut() else {
        return;
    };

    if let Some(text) = hovered {
        **tooltip_text = text.to_string();
        node.display = Display::Flex;
    } else {
        node.display = Display::None;
    }
}

pub(crate) fn setup_minimap(mut commands: Commands) {
    let root = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(MINIMAP_RIGHT),
                bottom: Val::Px(MINIMAP_BOTTOM),
                width: Val::Px(MINIMAP_COLS as f32 * MINIMAP_CELL + 12.0),
                height: Val::Px(MINIMAP_ROWS as f32 * MINIMAP_CELL + 12.0),
                padding: UiRect::all(Val::Px(MINIMAP_PAD)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.04, 0.82)),
            BorderColor::all(Color::srgb(0.55, 0.5, 0.34)),
            BuildMenuUi,
            MinimapRoot,
            Interaction::default(),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        for row in 0..MINIMAP_ROWS {
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(0.0),
                ..default()
            })
            .with_children(|line| {
                for col in 0..MINIMAP_COLS {
                    line.spawn((
                        MinimapCell { col, row },
                        Node {
                            width: Val::Px(MINIMAP_CELL),
                            height: Val::Px(MINIMAP_CELL),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.2, 0.09)),
                    ));
                }
            });
        }
        root.spawn((
            MinimapViewport,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(MINIMAP_PAD),
                top: Val::Px(MINIMAP_PAD),
                width: Val::Px(12.0),
                height: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgb(1.0, 1.0, 0.9)),
        ));
    });
}

pub(crate) fn sync_minimap(
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    mut root_q: Query<&mut Visibility, With<MinimapRoot>>,
    mut cells: Query<(&MinimapCell, &mut BackgroundColor)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    mut viewport_q: Query<&mut Node, With<MinimapViewport>>,
) {
    if let Ok(mut vis) = root_q.single_mut() {
        *vis = if hud.minimap_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !hud.minimap_visible {
        return;
    }
    let (mw, mh) = sim.state.map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    for (cell, mut bg) in &mut cells {
        let x = (MINIMAP_COLS.saturating_sub(1).saturating_sub(cell.col)) * mw / MINIMAP_COLS;
        let y = cell.row * mh / MINIMAP_ROWS;
        let c = TileCoord::new(x as i32, y as i32);
        *bg = BackgroundColor(minimap_color(
            sim.state.map.get_kind(c).unwrap_or(TileKind::Void),
        ));
    }

    update_minimap_viewport(&sim.state.map, &windows, &cam_q, &mut viewport_q);
}

pub(crate) fn handle_minimap_click(
    mouse: Res<ButtonInput<MouseButton>>,
    hud: Res<SimHudControls>,
    windows: Query<&Window, With<PrimaryWindow>>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
) {
    if !hud.minimap_visible || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if menu_pointer.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some((tile_x, tile_y)) = cursor_to_minimap_tile(cursor, window, sim.state.map.dimensions())
    else {
        return;
    };
    let coord = TileCoord::new(tile_x, tile_y);
    let height = sim.state.map.get(coord).map_or(0, |tile| tile.height);
    let pos = tile_pos(tile_x, tile_y, height, 0.0);
    let Ok(mut tf) = cam_q.single_mut() else {
        return;
    };
    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
}

fn cursor_to_minimap_tile(
    cursor: Vec2,
    window: &Window,
    dimensions: (u32, u32),
) -> Option<(i32, i32)> {
    let (mw, mh) = dimensions;
    if mw == 0 || mh == 0 {
        return None;
    }
    let total_w = MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
    let left = window.width() - MINIMAP_RIGHT - total_w;
    let total_h = MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
    let top = window.height() - MINIMAP_BOTTOM - total_h;
    let local_x = cursor.x - left - MINIMAP_PAD;
    let local_y_from_top = cursor.y - top - MINIMAP_PAD;
    if local_x < 0.0
        || local_y_from_top < 0.0
        || local_x >= MINIMAP_COLS as f32 * MINIMAP_CELL
        || local_y_from_top >= MINIMAP_ROWS as f32 * MINIMAP_CELL
    {
        return None;
    }
    let col = (local_x / MINIMAP_CELL).floor() as u32;
    let row = (local_y_from_top / MINIMAP_CELL).floor() as u32;
    let x = ((MINIMAP_COLS.saturating_sub(1).saturating_sub(col)) * mw / MINIMAP_COLS) as i32;
    let y = (row * mh / MINIMAP_ROWS) as i32;
    Some((x, y))
}

fn update_minimap_viewport(
    map: &Map,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_q: &Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    viewport_q: &mut Query<&mut Node, With<MinimapViewport>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((cam_tf, projection)) = cam_q.single() else {
        return;
    };
    let Projection::Orthographic(proj) = projection else {
        return;
    };
    let (mw, mh) = map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    let center_world = Vec2::new(cam_tf.translation.x, cam_tf.translation.y);
    let Some((cx, cy)) = world_pos_to_tile_coord(center_world, map) else {
        return;
    };

    // Estimación estable del tamaño de viewport en tiles (evita saltos por esquinas en iso).
    // Inversa aproximada de:
    //   sx = (ty - tx) * ISO_HW
    //   sy = (tx + ty) * -ISO_QH
    // Para un rectángulo de pantalla, la cota en tiles queda:
    //   dtx,dty ~= 0.5 * (|dx|/ISO_HW + |dy|/ISO_QH)
    let half_w = window.width() * proj.scale * 0.5;
    let half_h = window.height() * proj.scale * 0.5;
    let half_tiles = (0.5 * (half_w / ISO_HW + half_h / ISO_QH)).max(2.0);
    let half_tiles_x = half_tiles.clamp(2.0, mw as f32);
    let half_tiles_y = half_tiles.clamp(2.0, mh as f32);

    let min_x = (cx as f32 - half_tiles_x).clamp(0.0, mw.saturating_sub(1) as f32);
    let max_x = (cx as f32 + half_tiles_x).clamp(0.0, mw.saturating_sub(1) as f32);
    let min_y = (cy as f32 - half_tiles_y).clamp(0.0, mh.saturating_sub(1) as f32);
    let max_y = (cy as f32 + half_tiles_y).clamp(0.0, mh.saturating_sub(1) as f32);
    let left_min = MINIMAP_PAD
        + ((mw as f32 - 1.0 - max_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL);
    let left_max = MINIMAP_PAD
        + ((mw as f32 - 1.0 - min_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL);
    let top_min = MINIMAP_PAD + (min_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL);
    let top_max = MINIMAP_PAD + (max_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL);
    let width = (left_max - left_min).max(3.0);
    let height = (top_max - top_min).max(3.0);
    let Ok(mut node) = viewport_q.single_mut() else {
        return;
    };
    node.left = Val::Px(left_min);
    node.top = Val::Px(top_min);
    node.width = Val::Px(width);
    node.height = Val::Px(height);
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

pub(crate) fn setup_station_cargo_panel(mut commands: Commands) {
    commands
        .spawn((
            StationCargoPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(300.0),
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
                StationCargoPanelText,
                Text::new("Estación"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Button,
                        StationCargoPanelButton::Close,
                        Node {
                            width: Val::Px(90.0),
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
                            Text::new("Cerrar"),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        )],
                    ));
                });
        });
}

pub(crate) fn sync_station_cargo_panel(
    station_panel: Res<StationCargoPanelState>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<StationCargoPanelRoot>>,
    mut text_q: Query<&mut Text, With<StationCargoPanelText>>,
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    let Some(station_pos) = station_panel.station_pos else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    let Some(station) = sim.state.stations.iter().find(|st| st.pos == station_pos) else {
        return;
    };
    let mut out = format!(
        "Estación ({}, {}) {:?}\nColas cargo: pax:{} mail:{} goods:{} coal:{} wood:{} oil:{}",
        station_pos.x,
        station_pos.y,
        station.stop_kind,
        station.cargo_stock.passengers,
        station.cargo_stock.mail,
        station.cargo_stock.goods,
        station.cargo_stock.coal,
        station.cargo_stock.wood,
        station.cargo_stock.oil
    );
    let en_route = sim
        .state
        .vehicles
        .iter()
        .filter(|vehicle| {
            vehicle
                .orders
                .iter()
                .any(|order| matches!(order, VehicleOrder::Station { station } if *station == station_pos))
        })
        .count();
    out.push_str(&format!("\nVehículos en ruta a esta estación: {en_route}"));
    if let Ok(mut text) = text_q.single_mut() {
        **text = out;
    }
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

pub(crate) fn handle_station_cargo_panel_buttons(
    mut q: Query<(&Interaction, &StationCargoPanelButton), (Changed<Interaction>, With<Button>)>,
    mut station_panel: ResMut<StationCargoPanelState>,
) {
    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(button, StationCargoPanelButton::Close) {
            station_panel.station_pos = None;
        }
    }
}

fn apply_order_edit(
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

fn order_for_clicked_tile(sim: &SimWorld, vehicle_id: u32, pos: TileCoord) -> Option<VehicleOrder> {
    let vehicle = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)?;
    if let Some(station) = sim.state.stations.iter().find(|station| station.pos == pos) {
        return station
            .can_service_vehicle(vehicle.kind)
            .then_some(VehicleOrder::station(pos));
    }
    if sim.state.map.get_kind(pos) == Some(TileKind::RoadDepot)
        && !matches!(vehicle.kind, VehicleKind::Train)
    {
        return Some(VehicleOrder::tile(pos));
    }
    Some(VehicleOrder::tile(pos))
}

pub(crate) fn handle_settings_menu_buttons(
    mut q: Query<(&Interaction, &SaveMenuAction), (Changed<Interaction>, With<Button>)>,
    mut hud: ResMut<SimHudControls>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SaveMenuAction::SaveAs => {
                let Some(save_path) = choose_save_path(&hud.json_save_path) else {
                    continue;
                };
                hud.json_save_path = save_path.clone();
                match save::save(&sim.state, std::path::Path::new(&save_path)) {
                    Ok(()) => info!("Guardado en {save_path}"),
                    Err(e) => error!("No se pudo guardar en {save_path}: {e}"),
                }
            }
            SaveMenuAction::LoadFrom => {
                let Some(save_path) = choose_load_path(&hud.json_save_path) else {
                    continue;
                };
                hud.json_save_path = save_path.clone();
                match std::fs::read_to_string(&save_path) {
                    Ok(text) => match save::load_from_str(&text) {
                        Ok(loaded) => {
                            let prev = sim.state.map.dimensions();
                            let nw = loaded.map.dimensions();
                            sim.state = loaded;
                            sim.ottdmap_extras = None;
                            sim.loaded_file = true;
                            vehicle_index.rebuild(&sim.state.vehicles);
                            remap.pending = true;
                            remap.sync_camera = true;
                            if prev != nw {
                                info!("Mapa {prev:?} -> {nw:?}; recarga visual y camara.");
                            } else {
                                info!("Estado cargado desde {save_path}; recarga visual.");
                            }
                        }
                        Err(e) => error!("Carga: JSON invalido ({save_path}): {e}"),
                    },
                    Err(e) => error!("Carga: no se pudo leer {save_path}: {e}"),
                }
            }
            SaveMenuAction::PauseResume => {
                hud.paused = !hud.paused;
                info!("Pausa: {}", if hud.paused { "ON" } else { "OFF" });
            }
            SaveMenuAction::SpeedUp => {
                hud.sim_speed = if hud.sim_speed < 1.5 {
                    2.0
                } else if hud.sim_speed < 3.5 {
                    4.0
                } else {
                    1.0
                };
                info!("Velocidad simulacion: {:.0}x", hud.sim_speed);
            }
            SaveMenuAction::Normalize => {
                hud.sim_speed = 1.0;
                if let Ok((mut cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    let keep_pos = cam_tf.translation;
                    o.scale = 1.0;
                    // Normalizar NO debe recentrar: mantenemos posicion exacta.
                    cam_tf.translation = keep_pos;
                }
                info!("Normalizado: velocidad 1x y zoom 1.0x");
            }
            SaveMenuAction::ZoomIn => {
                if let Ok((_cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 0.85).max(0.25);
                }
            }
            SaveMenuAction::ZoomOut => {
                if let Ok((_cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 1.15).min(20.0);
                }
            }
        }
    }
}

#[cfg(test)]
fn choose_save_path(current: &str) -> Option<String> {
    Some(current.to_string())
}

#[cfg(not(test))]
fn choose_save_path(current: &str) -> Option<String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let mut dialog = rfd::FileDialog::new().add_filter("JSON", &["json"]);
        if let Some(parent) = Path::new(current).parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(name) = Path::new(current).file_name().and_then(|n| n.to_str()) {
            dialog = dialog.set_file_name(name);
        }
        return dialog.save_file().map(|p| p.to_string_lossy().to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut cmd = std::process::Command::new("zenity");
        cmd.arg("--file-selection")
            .arg("--save")
            .arg("--confirm-overwrite")
            .arg("--title=Guardar simulacion JSON")
            .arg("--file-filter=*.json");
        if Path::new(current).exists() || Path::new(current).parent().is_some() {
            cmd.arg("--filename").arg(current);
        }
        match cmd.output() {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if path.is_empty() { None } else { Some(path) }
            }
            Ok(_) => None,
            Err(e) => {
                error!("No se pudo abrir selector de archivo (zenity): {e}");
                None
            }
        }
    }
}

#[cfg(test)]
fn choose_load_path(current: &str) -> Option<String> {
    Some(current.to_string())
}

#[cfg(not(test))]
fn choose_load_path(current: &str) -> Option<String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let mut dialog = rfd::FileDialog::new().add_filter("JSON", &["json"]);
        if let Some(parent) = Path::new(current).parent() {
            dialog = dialog.set_directory(parent);
        }
        return dialog.pick_file().map(|p| p.to_string_lossy().to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut cmd = std::process::Command::new("zenity");
        cmd.arg("--file-selection")
            .arg("--title=Cargar simulacion JSON")
            .arg("--file-filter=*.json");
        if Path::new(current).exists() || Path::new(current).parent().is_some() {
            cmd.arg("--filename").arg(current);
        }
        match cmd.output() {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if path.is_empty() { None } else { Some(path) }
            }
            Ok(_) => None,
            Err(e) => {
                error!("No se pudo abrir selector de archivo (zenity): {e}");
                None
            }
        }
    }
}

fn minimap_color(kind: TileKind) -> Color {
    match kind {
        TileKind::Water => Color::srgb(0.08, 0.25, 0.55),
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadBridge | TileKind::RoadTunnel => {
            Color::srgb(0.48, 0.42, 0.32)
        }
        TileKind::Rail | TileKind::RailDepot | TileKind::RailBridge | TileKind::RailTunnel => {
            Color::srgb(0.68, 0.68, 0.62)
        }
        TileKind::House => Color::srgb(0.72, 0.28, 0.2),
        TileKind::Industry | TileKind::CoalField => Color::srgb(0.78, 0.64, 0.2),
        TileKind::Station => Color::srgb(0.95, 0.95, 0.86),
        TileKind::Forest => Color::srgb(0.05, 0.34, 0.1),
        TileKind::Grass => Color::srgb(0.16, 0.48, 0.12),
        TileKind::Void => Color::srgb(0.02, 0.02, 0.02),
        TileKind::Unknown(_) => Color::srgb(0.38, 0.12, 0.45),
    }
}

fn action_supports_drag(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::Road
            | BuildMenuAction::RoadX
            | BuildMenuAction::RoadY
            | BuildMenuAction::RoadBridge
            | BuildMenuAction::RoadTunnel
            | BuildMenuAction::Rail
            | BuildMenuAction::RailBridge
            | BuildMenuAction::RailTunnel
            | BuildMenuAction::Clear
    )
}

fn action_is_tunnel(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel | BuildMenuAction::RailTunnel
    )
}

fn tunnel_placement_is_valid(map: &Map, action: BuildMenuAction, tiles: &[(i32, i32)]) -> bool {
    if !action_is_tunnel(action) || tiles.len() < 3 {
        return false;
    }
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    let Some(start) = map.get(TileCoord::new(sx, sy)) else {
        return false;
    };
    let Some(end) = map.get(TileCoord::new(ex, ey)) else {
        return false;
    };
    !matches!(start.kind, TileKind::Water | TileKind::Void)
        && !matches!(end.kind, TileKind::Water | TileKind::Void)
        && start.height == end.height
}

fn command_for_action(
    action: BuildMenuAction,
    pos: TileCoord,
    station_state: &StationBuildState,
) -> Option<Command> {
    match action {
        BuildMenuAction::Road => Some(Command::PlaceRoadBits(pos, 0x0F)),
        BuildMenuAction::RoadX => Some(Command::PlaceRoadBits(pos, 0x0A)),
        BuildMenuAction::RoadY => Some(Command::PlaceRoadBits(pos, 0x05)),
        BuildMenuAction::Rail => Some(Command::PlaceRail(pos)),
        BuildMenuAction::Station => Some(Command::PlaceStationDir(pos, station_state.orientation)),
        BuildMenuAction::BusStop => Some(Command::PlaceBusStop(pos, station_state.orientation)),
        BuildMenuAction::Clear => Some(Command::ClearTile(pos)),
        BuildMenuAction::RoadDepot => {
            Some(Command::PlaceRoadDepotDir(pos, station_state.orientation))
        }
        BuildMenuAction::RailDepot => Some(Command::PlaceRailDepot(pos)),
        BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::Orders => None,
        BuildMenuAction::BuildHouse => Some(Command::PlaceHouse(pos)),
        BuildMenuAction::BuildCoalMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::CoalMine,
        )),
        BuildMenuAction::BuildIronOreMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::IronOreMine,
        )),
        BuildMenuAction::BuildGoldMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::GoldMine,
        )),
        BuildMenuAction::BuildOilWell => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::OilWells,
        )),
        BuildMenuAction::BuildOilRefinery => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::OilRefinery,
        )),
        BuildMenuAction::BuildFactory => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Factory,
        )),
        BuildMenuAction::BuildSawmill => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Sawmill,
        )),
        BuildMenuAction::BuildForest => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Forest,
        )),
        BuildMenuAction::BuildFarm => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Farm,
        )),
    }
}

fn command_for_line_action(action: BuildMenuAction, tiles: &[(i32, i32)]) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let &(ex, ey) = tiles.last()?;
    let a = TileCoord::new(sx, sy);
    let b = TileCoord::new(ex, ey);
    match action {
        BuildMenuAction::RoadTunnel => Some(Command::PlaceRoadTunnel(a, b)),
        BuildMenuAction::RailTunnel => Some(Command::PlaceRailTunnel(a, b)),
        BuildMenuAction::RoadBridge => Some(Command::PlaceRoadBridge(a, b)),
        BuildMenuAction::RailBridge => Some(Command::PlaceRailBridge(a, b)),
        _ => None,
    }
}

fn road_bits_for_drag_action(action: BuildMenuAction, tiles: &[(i32, i32)]) -> Option<u8> {
    match action {
        BuildMenuAction::RoadX => Some(0x0A),
        BuildMenuAction::RoadY => Some(0x05),
        BuildMenuAction::Road => {
            let &(sx, sy) = tiles.first()?;
            let &(ex, ey) = tiles.last().unwrap_or(&(sx, sy));
            Some(if (ex - sx).abs() >= (ey - sy).abs() {
                0x0A
            } else {
                0x05
            })
        }
        _ => None,
    }
}

fn apply_drag_action(
    sim: &mut SimWorld,
    action: BuildMenuAction,
    tiles: Vec<(i32, i32)>,
    station_state: &StationBuildState,
) -> bool {
    if let Some(cmd) = command_for_line_action(action, &tiles) {
        return apply_command(&mut sim.state, &cmd).is_ok();
    }

    if let Some(road_bits) = road_bits_for_drag_action(action, &tiles) {
        let mut changed = false;
        for (x, y) in tiles {
            changed |= apply_command(
                &mut sim.state,
                &Command::SetRoadBits(TileCoord::new(x, y), road_bits),
            )
            .is_ok();
        }
        return changed;
    }

    let mut changed = false;
    for (x, y) in tiles {
        if let Some(cmd) = command_for_action(action, TileCoord::new(x, y), station_state) {
            changed |= apply_command(&mut sim.state, &cmd).is_ok();
        }
    }
    changed
}

fn drag_line_tiles(action: BuildMenuAction, from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let use_x_axis = match action {
        BuildMenuAction::RoadX => true,
        BuildMenuAction::RoadY => false,
        _ => (to.0 - from.0).abs() >= (to.1 - from.1).abs(),
    };
    let mut out = Vec::new();

    if use_x_axis {
        let step = if to.0 >= from.0 { 1 } else { -1 };
        let mut x = from.0;
        loop {
            out.push((x, from.1));
            if x == to.0 {
                break;
            }
            x += step;
        }
    } else {
        let step = if to.1 >= from.1 { 1 } else { -1 };
        let mut y = from.1;
        loop {
            out.push((from.0, y));
            if y == to.1 {
                break;
            }
            y += step;
        }
    }

    out
}

/// Dos clicks: el primero ancla el ghost, el segundo confirma. Click derecho cancela.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &Transform), (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
    mut selected: ResMut<SelectedTileInfo>,
    mut sim: ResMut<SimWorld>,
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
    mut depot_state: ResMut<DepotPanelState>,
    mut station_panel: ResMut<StationCargoPanelState>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut industry_panel: ResMut<IndustryPanelState>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
) {
    if mouse.just_pressed(MouseButton::Right) && drag_state.armed {
        cancel_placement(&mut drag_state);
        return;
    }

    if menu_pointer.iter().any(|i| *i != Interaction::None) && mouse.just_pressed(MouseButton::Left)
    {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.single() else {
        return;
    };
    let cam_global = GlobalTransform::from(*cam_tf);
    let Ok(world_pos) = camera.viewport_to_world_2d(&cam_global, cursor_pos) else {
        return;
    };

    let Some((tx, ty)) = world_pos_to_tile_coord(world_pos, &sim.state.map) else {
        selected.pos = None;
        return;
    };
    let pos = TileCoord::new(tx, ty);
    selected.pos = Some(pos);

    let Some(action) = tool_state.active_tool else {
        cancel_placement(&mut drag_state);
        if mouse.just_pressed(MouseButton::Left) {
            let tile_kind = sim.state.map.get_kind(pos);
            match tile_kind {
                Some(TileKind::Industry) => {
                    industry_panel.open = true;
                    industry_panel.focus_tile = Some(pos);
                    depot_state.depot_pos = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    station_panel.station_pos = None;
                    return;
                }
                Some(TileKind::RoadDepot) => {
                    depot_state.depot_pos = Some(pos);
                    depot_state.selected_vehicle = sim
                        .state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.pos == pos)
                        .map(|vehicle| vehicle.id);
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    station_panel.station_pos = None;
                    industry_panel.open = false;
                    return;
                }
                Some(TileKind::Station) => {
                    station_panel.station_pos = Some(pos);
                    depot_state.depot_pos = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    industry_panel.open = false;
                    return;
                }
                _ => {}
            }
            if let Some(vehicle) = sim.state.vehicles.iter().find(|vehicle| vehicle.pos == pos) {
                order_state.vehicle_id = Some(vehicle.id);
                order_state.orders = vehicle.orders.clone();
                depot_state.depot_pos = None;
                station_panel.station_pos = None;
                industry_panel.open = false;
                return;
            }
            depot_state.depot_pos = None;
            station_panel.station_pos = None;
            industry_panel.open = false;
        }
        return;
    };

    let current = (tx, ty);

    if action == BuildMenuAction::Orders {
        if mouse.just_pressed(MouseButton::Right) {
            order_state.vehicle_id = None;
            order_state.orders.clear();
            return;
        }
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.pos == pos) {
            order_state.vehicle_id = Some(vehicle.id);
            order_state.orders = vehicle.orders.clone();
            return;
        }
        let Some(vehicle_id) = order_state.vehicle_id else {
            return;
        };
        let Some(order) = order_for_clicked_tile(&sim, vehicle_id, pos) else {
            return;
        };
        order_state.orders.push(order);
        if apply_order_edit(&mut sim.state, vehicle_id, &order_state.orders).is_ok() {
            pending.pending = true;
        }
        return;
    }

    if action_supports_drag(action) {
        if !drag_state.armed || drag_state.last_action != Some(action) {
            if mouse.just_pressed(MouseButton::Left) {
                drag_state.armed = true;
                drag_state.start_tile = Some(current);
                drag_state.last_tile = Some(current);
                drag_state.last_action = Some(action);
                drag_state.pending_tiles = vec![current];
            }
            return;
        }

        let start = drag_state.start_tile.unwrap_or(current);
        drag_state.pending_tiles = drag_line_tiles(action, start, current);
        drag_state.last_tile = Some(current);

        if mouse.just_pressed(MouseButton::Left) {
            if action_is_tunnel(action)
                && !tunnel_placement_is_valid(&sim.state.map, action, &drag_state.pending_tiles)
            {
                return;
            }
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let changed = apply_drag_action(&mut sim, action, tiles, &station_state);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            }
        } else if mouse.just_released(MouseButton::Left) && drag_state.pending_tiles.len() == 1 {
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let changed = apply_drag_action(&mut sim, action, tiles, &station_state);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            }
        }
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    if let Some(cmd) = command_for_action(action, TileCoord::new(tx, ty), &station_state)
        && apply_command(&mut sim.state, &cmd).is_ok()
    {
        pending.pending = true;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;

    use crate::render::RemapMapVisualsPending;

    #[test]
    fn close_toolbar_escape_clears_state() {
        let mut world = World::new();
        let mut kb = ButtonInput::<KeyCode>::default();
        kb.press(KeyCode::Escape);
        world.insert_resource(kb);
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::RoadY),
        });
        world.insert_resource(DragBuildState {
            armed: true,
            ..default()
        });
        world
            .run_system_once(close_toolbar_panel_on_escape)
            .unwrap();
    }

    #[test]
    fn hide_tool_mismatch_group_clears_tool() {
        let mut world = World::new();
        world.insert_resource(ToolbarState {
            active_group: Some(ToolbarGroup::Road),
        });
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::Rail),
        });
        world.insert_resource(DragBuildState::default());
        world.run_system_once(hide_tool_when_panel_closed).unwrap();
    }

    #[test]
    fn setup_minimap_then_sync_minimap() {
        let mut world = World::new();
        world.run_system_once(setup_minimap).unwrap();
        world.insert_resource(SimWorld::default());
        world.insert_resource(SimHudControls::default());
        world.run_system_once(sync_minimap).unwrap();
    }

    #[test]
    fn handle_minimap_click_ignored_when_ui_is_interacting() {
        let mut world = World::new();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SimWorld::default());
        world.spawn((BuildMenuUi, Interaction::Pressed));
        world.run_system_once(handle_minimap_click).unwrap();
    }

    #[test]
    fn cursor_to_minimap_tile_top_left_maps_to_small_coords() {
        let window = Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        };
        let total_w = MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
        let total_h = MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
        let left = window.width() - MINIMAP_RIGHT - total_w;
        let top = window.height() - MINIMAP_BOTTOM - total_h;
        let cursor = Vec2::new(left + MINIMAP_PAD + 1.0, top + MINIMAP_PAD + 1.0);
        let (x, y) = cursor_to_minimap_tile(cursor, &window, (256, 256)).unwrap();
        assert!(x >= 249);
        assert_eq!(y, 0);
    }

    #[test]
    fn setup_order_panel_then_sync_order_panel() {
        let mut world = World::new();
        world.run_system_once(setup_order_panel).unwrap();
        world.insert_resource(OrderEditState::default());
        world.insert_resource(SimWorld::default());
        world.run_system_once(sync_order_panel).unwrap();
    }

    #[test]
    fn toolbar_interaction_systems_run_with_empty_queries() {
        let mut world = World::new();
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(DragBuildState::default());
        world.run_system_once(toolbar_group_interaction).unwrap();
        world.run_system_once(update_toolbar_group_visuals).unwrap();
        world
            .run_system_once(update_toolbar_tool_visibility)
            .unwrap();
        world
            .run_system_once(close_toolbar_button_interaction)
            .unwrap();
        world.run_system_once(build_menu_interaction).unwrap();
    }

    #[test]
    fn update_tool_button_visuals_empty() {
        let mut world = World::new();
        world.insert_resource(UiToolState::default());
        world.run_system_once(update_tool_button_visuals).unwrap();
    }

    #[test]
    fn update_toolbar_tooltip_no_ui_returns_early() {
        let mut world = World::new();
        world.run_system_once(update_toolbar_tooltip).unwrap();
    }

    #[test]
    fn handle_order_panel_buttons_empty() {
        let mut world = World::new();
        world.insert_resource(OrderEditState::default());
        world.insert_resource(SimWorld::default());
        world.run_system_once(handle_order_panel_buttons).unwrap();
    }

    #[test]
    fn handle_settings_menu_buttons_save_load_with_file_dialog_abstraction() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("sim.json");

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            paused: false,
            sim_speed: 1.0,
            json_save_path: save_path.to_string_lossy().to_string(),
            minimap_visible: true,
        });

        world.spawn((Button, SaveMenuAction::SaveAs, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();

        world.spawn((Button, SaveMenuAction::LoadFrom, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        let remap = world.resource::<RemapMapVisualsPending>();
        assert!(remap.pending);
        assert!(remap.sync_camera);
    }

    #[test]
    fn handle_settings_menu_buttons_pause_speed_and_zoom() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls::default());
        world.spawn((
            PrimaryGameCamera,
            Transform::from_xyz(123.0, -45.0, 0.0),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.spawn((Button, SaveMenuAction::PauseResume, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert!(world.resource::<SimHudControls>().paused);

        world.spawn((Button, SaveMenuAction::SpeedUp, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert_eq!(world.resource::<SimHudControls>().sim_speed, 2.0);

        world.spawn((Button, SaveMenuAction::Normalize, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert_eq!(world.resource::<SimHudControls>().sim_speed, 1.0);
        let mut q_norm =
            world.query_filtered::<(&Transform, &Projection), With<PrimaryGameCamera>>();
        let (tf_norm, proj_norm) = q_norm.single(&world).unwrap();
        let Projection::Orthographic(o_norm) = proj_norm else {
            panic!("expected orthographic projection");
        };
        assert_eq!(o_norm.scale, 1.0);
        assert_eq!(tf_norm.translation.x, 123.0);
        assert_eq!(tf_norm.translation.y, -45.0);

        let mut world_zoom_in = World::new();
        world_zoom_in.insert_resource(SimWorld::default());
        world_zoom_in.insert_resource(VehicleIndex::default());
        world_zoom_in.insert_resource(RemapMapVisualsPending::default());
        world_zoom_in.insert_resource(SimHudControls::default());
        world_zoom_in.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world_zoom_in.spawn((Button, SaveMenuAction::ZoomIn, Interaction::Pressed));
        world_zoom_in
            .run_system_once(handle_settings_menu_buttons)
            .unwrap();
        let mut q_in = world_zoom_in.query_filtered::<&Projection, With<PrimaryGameCamera>>();
        let Projection::Orthographic(o_in) = q_in.single(&world_zoom_in).unwrap() else {
            panic!("expected orthographic projection");
        };
        assert!(o_in.scale < 1.0);

        let mut world_zoom_out = World::new();
        world_zoom_out.insert_resource(SimWorld::default());
        world_zoom_out.insert_resource(VehicleIndex::default());
        world_zoom_out.insert_resource(RemapMapVisualsPending::default());
        world_zoom_out.insert_resource(SimHudControls::default());
        world_zoom_out.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world_zoom_out.spawn((Button, SaveMenuAction::ZoomOut, Interaction::Pressed));
        world_zoom_out
            .run_system_once(handle_settings_menu_buttons)
            .unwrap();
        let mut q_out = world_zoom_out.query_filtered::<&Projection, With<PrimaryGameCamera>>();
        let Projection::Orthographic(o_out) = q_out.single(&world_zoom_out).unwrap() else {
            panic!("expected orthographic projection");
        };
        assert!(o_out.scale > 1.0);
    }

    #[test]
    fn handle_tile_click_minimal_returns_early() {
        let mut world = World::new();
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(SelectedTileInfo::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(StationBuildState::default());
        world.insert_resource(DragBuildState::default());
        world.insert_resource(OrderEditState::default());
        world.insert_resource(DepotPanelState::default());
        world.insert_resource(StationCargoPanelState::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(IndustryPanelState::default());
        world.run_system_once(handle_tile_click).unwrap();
    }

    #[test]
    fn pure_toolbar_helpers_cover_branches() {
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::RailTunnel),
            ToolbarGroup::Rail
        ));
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::BuildFactory),
            ToolbarGroup::Economy
        ));
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::Orders),
            ToolbarGroup::Info
        ));

        assert!(action_supports_drag(BuildMenuAction::RoadBridge));
        assert!(action_supports_drag(BuildMenuAction::RailTunnel));
        assert!(action_supports_drag(BuildMenuAction::Clear));
        assert!(!action_supports_drag(BuildMenuAction::BuildHouse));
        assert!(!action_supports_drag(BuildMenuAction::Station));

        assert!(action_is_tunnel(BuildMenuAction::RoadTunnel));
        assert!(action_is_tunnel(BuildMenuAction::RailTunnel));
        assert!(!action_is_tunnel(BuildMenuAction::RoadBridge));

        assert!(matches!(
            command_for_action(
                BuildMenuAction::Station,
                TileCoord::new(1, 2),
                &StationBuildState { orientation: 3 }
            ),
            Some(Command::PlaceStationDir(_, 3))
        ));
        assert!(matches!(
            command_for_action(
                BuildMenuAction::RoadDepot,
                TileCoord::new(1, 2),
                &StationBuildState { orientation: 2 }
            ),
            Some(Command::PlaceRoadDepotDir(_, 2))
        ));
        assert!(matches!(
            command_for_action(
                BuildMenuAction::BuildCoalMine,
                TileCoord::new(1, 2),
                &StationBuildState::default()
            ),
            Some(Command::PlaceIndustrySpec(
                _,
                openttdrs_core::IndustrySpec::CoalMine
            ))
        ));
        assert!(
            command_for_action(
                BuildMenuAction::RoadTunnel,
                TileCoord::new(1, 2),
                &StationBuildState::default()
            )
            .is_none()
        );

        assert!(matches!(
            command_for_line_action(BuildMenuAction::RoadTunnel, &[(1, 1), (3, 1)]),
            Some(Command::PlaceRoadTunnel(_, _))
        ));
        assert!(matches!(
            command_for_line_action(BuildMenuAction::RailBridge, &[(1, 1), (3, 1)]),
            Some(Command::PlaceRailBridge(_, _))
        ));
        assert!(command_for_line_action(BuildMenuAction::RoadX, &[(1, 1), (3, 1)]).is_none());
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::Road, &[(1, 1), (4, 1)]),
            Some(0x0A)
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::Road, &[(1, 1), (1, 4)]),
            Some(0x05)
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::RoadX, &[(1, 1), (1, 4)]),
            Some(0x0A)
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::Clear, &[(1, 1), (1, 4)]),
            None
        );

        assert_eq!(
            drag_line_tiles(BuildMenuAction::RoadX, (1, 2), (4, 9)),
            vec![(1, 2), (2, 2), (3, 2), (4, 2)]
        );
        assert_eq!(
            drag_line_tiles(BuildMenuAction::RoadY, (3, 1), (0, 4)),
            vec![(3, 1), (3, 2), (3, 3), (3, 4)]
        );
        assert_eq!(
            drag_line_tiles(BuildMenuAction::Road, (5, 2), (2, 2)),
            vec![(5, 2), (4, 2), (3, 2), (2, 2)]
        );
        assert_eq!(
            drag_line_tiles(BuildMenuAction::Road, (2, 2), (3, 6)),
            vec![(2, 2), (2, 3), (2, 4), (2, 5), (2, 6)]
        );
    }

    #[test]
    fn map_related_helpers_cover_color_and_tunnels() {
        assert_eq!(
            minimap_color(TileKind::Water),
            Color::srgb(0.08, 0.25, 0.55)
        );
        assert_eq!(minimap_color(TileKind::Road), Color::srgb(0.48, 0.42, 0.32));
        assert_eq!(minimap_color(TileKind::Rail), Color::srgb(0.68, 0.68, 0.62));
        assert_eq!(
            minimap_color(TileKind::Station),
            Color::srgb(0.95, 0.95, 0.86)
        );
        assert_eq!(minimap_color(TileKind::Void), Color::srgb(0.02, 0.02, 0.02));
        assert_eq!(
            minimap_color(TileKind::Unknown(9)),
            Color::srgb(0.38, 0.12, 0.45)
        );

        let mut map = Map::new_flat(6, 6, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        map.set_height(c(1, 1), 2).unwrap();
        map.set_height(c(3, 1), 2).unwrap();
        map.set_kind(c(1, 1), TileKind::Road).unwrap();
        map.set_kind(c(2, 1), TileKind::Road).unwrap();
        map.set_kind(c(3, 1), TileKind::Road).unwrap();
        map.set_kind(c(4, 1), TileKind::Water).unwrap();

        assert!(!tunnel_placement_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1)]
        ));
        assert!(tunnel_placement_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (3, 1)]
        ));
        assert!(!tunnel_placement_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (4, 1)]
        ));
        assert!(!tunnel_placement_is_valid(
            &map,
            BuildMenuAction::Road,
            &[(1, 1), (2, 1)]
        ));
    }

    #[test]
    fn order_for_clicked_tile_accepts_depot_and_rejects_incompatible_station() {
        let mut sim = SimWorld::default();
        sim.state.vehicles.clear();
        sim.state.stations.clear();
        let depot = TileCoord::new(2, 2);
        let truck_stop = TileCoord::new(3, 2);
        sim.state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        sim.state
            .stations
            .push(openttdrs_core::Station::new_with_kind(
                truck_stop,
                openttdrs_core::StopKind::TruckStop,
            ));
        sim.state.vehicles.push(openttdrs_core::Vehicle::new(
            42,
            VehicleKind::Bus,
            depot,
            depot,
        ));

        assert!(matches!(
            order_for_clicked_tile(&sim, 42, depot),
            Some(VehicleOrder::Tile(_))
        ));
        assert!(order_for_clicked_tile(&sim, 42, truck_stop).is_none());
    }
}
