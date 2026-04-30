use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Command, Map, TileCoord, TileKind, apply_command};

use crate::iso::{tile_pos, world_pos_to_tile_coord};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;
use crate::ui::industry_panel::IndustryPanelState;

use super::super::hud::SelectedTileInfo;
use super::{
    BuildMenuAction, BuildMenuUi, DragBuildState, MinimapCell, MinimapRoot, MinimapViewport,
    OrderEditState, OrderPanelButton, OrderPanelRoot, OrderPanelText, StationBuildState,
    ToolButtonGroup, ToolSelectButton, ToolbarCloseButton, ToolbarGroup, ToolbarGroupButton,
    ToolbarState, ToolbarTooltipTarget, TooltipBox, TooltipText, UiToolState,
};

const MINIMAP_COLS: u32 = 64;
const MINIMAP_ROWS: u32 = 40;
const MINIMAP_CELL: f32 = 3.0;
const MINIMAP_PAD: f32 = 6.0;
const MINIMAP_RIGHT: f32 = 10.0;
const MINIMAP_BOTTOM: f32 = 10.0;

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
        | BuildMenuAction::Station
        | BuildMenuAction::Clear => ToolbarGroup::Road,
        BuildMenuAction::Orders => ToolbarGroup::Info,
        BuildMenuAction::BuildHouse
        | BuildMenuAction::BuildCoalMine
        | BuildMenuAction::BuildOilWell
        | BuildMenuAction::BuildFactory
        | BuildMenuAction::BuildForest => ToolbarGroup::Economy,
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
        let x = cell.col * mw / MINIMAP_COLS;
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
) {
    if !hud.minimap_visible || !mouse.just_pressed(MouseButton::Left) {
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
    let bottom = MINIMAP_BOTTOM;
    let local_x = cursor.x - left - MINIMAP_PAD;
    let local_y_from_bottom = cursor.y - bottom - MINIMAP_PAD;
    if local_x < 0.0
        || local_y_from_bottom < 0.0
        || local_x >= MINIMAP_COLS as f32 * MINIMAP_CELL
        || local_y_from_bottom >= MINIMAP_ROWS as f32 * MINIMAP_CELL
    {
        return None;
    }
    let col = (local_x / MINIMAP_CELL).floor() as u32;
    let row_from_bottom = (local_y_from_bottom / MINIMAP_CELL).floor() as u32;
    let row = MINIMAP_ROWS
        .saturating_sub(1)
        .saturating_sub(row_from_bottom);
    let x = (col * mw / MINIMAP_COLS) as i32;
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
    let half_w = window.width() * proj.scale * 0.5;
    let half_h = window.height() * proj.scale * 0.5;
    let center = Vec2::new(cam_tf.translation.x, cam_tf.translation.y);
    let corners = [
        center + Vec2::new(-half_w, -half_h),
        center + Vec2::new(half_w, -half_h),
        center + Vec2::new(-half_w, half_h),
        center + Vec2::new(half_w, half_h),
    ];
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for corner in corners {
        if let Some((x, y)) = world_pos_to_tile_coord(corner, map) {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if min_x == i32::MAX {
        return;
    }
    let min_x = min_x.clamp(0, mw.saturating_sub(1) as i32) as f32;
    let min_y = min_y.clamp(0, mh.saturating_sub(1) as i32) as f32;
    let max_x = max_x.clamp(0, mw.saturating_sub(1) as i32) as f32;
    let max_y = max_y.clamp(0, mh.saturating_sub(1) as i32) as f32;
    let left = MINIMAP_PAD + min_x / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL;
    let top = MINIMAP_PAD + min_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL;
    let width =
        ((max_x - min_x).max(1.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL).max(3.0);
    let height =
        ((max_y - min_y).max(1.0) / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL).max(3.0);
    let Ok(mut node) = viewport_q.single_mut() else {
        return;
    };
    node.left = Val::Px(left);
    node.top = Val::Px(top);
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
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    let Some(vehicle_id) = order_state.vehicle_id else {
        *vis = Visibility::Hidden;
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
    let mut out = format!(
        "Vehiculo #{} {:?}\ncargo {}/{} dest ({},{})",
        vehicle.id, vehicle.kind, vehicle.cargo, vehicle.capacity, vehicle.dest.x, vehicle.dest.y
    );
    if order_state.orders.is_empty() {
        out.push_str("\nSin ordenes");
    } else {
        for (i, order) in order_state.orders.iter().enumerate().take(8) {
            out.push_str(&format!("\n{}. ({},{})", i + 1, order.x, order.y));
        }
        if order_state.orders.len() > 8 {
            out.push_str("\n...");
        }
    }
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
                let _ = apply_command(
                    &mut sim.state,
                    &Command::SetVehicleOrders(vehicle_id, order_state.orders.clone()),
                );
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
        BuildMenuAction::Clear => Some(Command::ClearTile(pos)),
        BuildMenuAction::RoadDepot => Some(Command::PlaceRoadDepot(pos)),
        BuildMenuAction::RailDepot => Some(Command::PlaceRailDepot(pos)),
        BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::Orders => None,
        BuildMenuAction::BuildHouse => Some(Command::PlaceHouse(pos)),
        BuildMenuAction::BuildCoalMine => Some(Command::PlaceIndustryKind(
            pos,
            openttdrs_core::IndustryKind::CoalMine,
        )),
        BuildMenuAction::BuildOilWell => Some(Command::PlaceIndustryKind(
            pos,
            openttdrs_core::IndustryKind::OilWell,
        )),
        BuildMenuAction::BuildFactory => Some(Command::PlaceIndustryKind(
            pos,
            openttdrs_core::IndustryKind::Factory,
        )),
        BuildMenuAction::BuildForest => Some(Command::PlaceForest(pos)),
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
        if mouse.just_pressed(MouseButton::Left)
            && sim.state.map.get_kind(pos) == Some(TileKind::Industry)
        {
            industry_panel.open = true;
            industry_panel.focus_tile = Some(pos);
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
        order_state.orders.push(pos);
        let cmd = Command::SetVehicleOrders(vehicle_id, order_state.orders.clone());
        if apply_command(&mut sim.state, &cmd).is_ok() {
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
            let mut changed = false;
            if let Some(cmd) = command_for_line_action(action, &tiles) {
                changed |= apply_command(&mut sim.state, &cmd).is_ok();
            } else {
                for (x, y) in tiles {
                    if let Some(cmd) =
                        command_for_action(action, TileCoord::new(x, y), &station_state)
                    {
                        changed |= apply_command(&mut sim.state, &cmd).is_ok();
                    }
                }
            }
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

    if let Some(cmd) = command_for_action(action, TileCoord::new(tx, ty), &station_state) {
        if apply_command(&mut sim.state, &cmd).is_ok() {
            pending.pending = true;
        }
    }
}
