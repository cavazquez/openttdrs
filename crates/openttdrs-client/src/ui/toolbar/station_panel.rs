use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::{
    CargoType, Command, CommandError, MAX_STATION_NAME_CHARS, STATION_COVERAGE_RADIUS, TileCoord,
    VehicleOrder, apply_command, cargo_display_name, station_coverage_at, station_rating_for_cargo,
};

use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending};
use crate::state::{OrderPickState, SimWorld};
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::vehicle_list::VehicleListState;

use super::order_panel::apply_order_edit;
use super::{
    BuildMenuAction, BuildMenuUi, OrderEditState, StationBuildState, UiToolState,
    open_order_edit_for_vehicle,
};

#[derive(Resource, Default)]
pub(crate) struct StationCargoPanelState {
    pub(crate) station_pos: Option<TileCoord>,
    pub(crate) rename_editing: bool,
}

#[derive(Component)]
pub(crate) struct StationCargoPanelRoot;

#[derive(Component)]
pub(crate) struct StationCargoPanelText;

#[derive(Component)]
pub(crate) struct StationCargoRenameRow;

#[derive(Component)]
pub(crate) struct StationCargoRenameInput;

#[derive(Component, Clone, Copy)]
pub(crate) enum StationCargoRenameButton {
    Apply,
    Cancel,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum StationCargoPanelButton {
    AddToRoute,
    PickOrders,
    CenterCamera,
    Rename,
    ViewVehicles,
    /// Activa JoinStation con esta estación como `keep`.
    JoinWith,
    Close,
}

const CARGO_TYPES: [CargoType; 6] = [
    CargoType::Passengers,
    CargoType::Mail,
    CargoType::Goods,
    CargoType::Coal,
    CargoType::Wood,
    CargoType::Oil,
];

pub(crate) fn setup_station_cargo_panel(mut commands: Commands) {
    commands
        .spawn((
            StationCargoPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(300.0),
                width: Val::Px(400.0),
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
                    font_size: FontSize::Rem(0.85),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            ));
            panel
                .spawn((
                    StationCargoRenameRow,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Center,
                        display: Display::None,
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .with_children(|row| {
                    row.spawn((
                        StationCargoRenameInput,
                        EditableText::new(""),
                        TextFont {
                            font_size: FontSize::Rem(0.7),
                            ..default()
                        },
                        TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        Node {
                            flex_grow: 1.0,
                            height: Val::Px(22.0),
                            padding: UiRect::horizontal(Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
                    ));
                    spawn_rename_action(row, StationCargoRenameButton::Apply, "Guardar");
                    spawn_rename_action(row, StationCargoRenameButton::Cancel, "Cancelar");
                });
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_station_button(row, StationCargoPanelButton::AddToRoute, "Añadir a ruta");
                    spawn_station_button(
                        row,
                        StationCargoPanelButton::PickOrders,
                        "Editar órdenes",
                    );
                    spawn_station_button(row, StationCargoPanelButton::CenterCamera, "Centrar");
                    spawn_station_button(row, StationCargoPanelButton::Rename, "Renombrar");
                    spawn_station_button(
                        row,
                        StationCargoPanelButton::ViewVehicles,
                        "Ver vehículos",
                    );
                    spawn_station_button(row, StationCargoPanelButton::JoinWith, "Unir…");
                    spawn_station_button(row, StationCargoPanelButton::Close, "Cerrar");
                });
        });
}

fn spawn_rename_action(
    parent: &mut ChildSpawnerCommands,
    action: StationCargoRenameButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            min_width: Val::Px(72.0),
            height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
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
                font_size: FontSize::Rem(0.65),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_station_button(
    parent: &mut ChildSpawnerCommands,
    action: StationCargoPanelButton,
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
                font_size: FontSize::Rem(0.7),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

/// Vehículo activo para editar órdenes hacia esta estación.
#[must_use]
pub(crate) fn vehicle_id_for_station_panel(
    sim: &SimWorld,
    station_pos: TileCoord,
    preferred: Option<u32>,
) -> Option<u32> {
    if let Some(id) = preferred
        && sim.state.vehicles.iter().any(|v| v.id == id)
    {
        return Some(id);
    }
    sim.state
        .vehicles
        .iter()
        .find(|vehicle| {
            vehicle.orders.iter().any(|order| {
                matches!(order, VehicleOrder::Station { station, .. } if *station == station_pos)
            })
        })
        .map(|vehicle| vehicle.id)
}

pub(crate) fn try_append_station_order(
    state: &mut openttdrs_core::GameState,
    vehicle_id: u32,
    station_pos: TileCoord,
    orders: &mut Vec<VehicleOrder>,
) -> Result<(), CommandError> {
    let Some(station) = state.stations.iter().find(|s| s.pos == station_pos) else {
        return Err(CommandError::StationNotFound);
    };
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if !station.can_service_vehicle(vehicle.kind) {
        return Err(CommandError::IncompatibleStopForVehicle);
    }
    orders.push(VehicleOrder::station(station_pos));
    apply_order_edit(state, vehicle_id, orders)
}

fn station_display_name(station: &openttdrs_core::Station) -> String {
    station
        .name
        .clone()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            format!(
                "{} ({}, {})",
                station_kind_label(station.stop_kind),
                station.pos.x,
                station.pos.y
            )
        })
}

fn station_kind_label(kind: openttdrs_core::StopKind) -> &'static str {
    match kind {
        openttdrs_core::StopKind::BusStop => "Parada de bus",
        openttdrs_core::StopKind::TruckStop => "Parada de camión",
        openttdrs_core::StopKind::RailStation => "Estación de tren",
        openttdrs_core::StopKind::Dock => "Muelle",
        openttdrs_core::StopKind::Buoy => "Boya",
        openttdrs_core::StopKind::Airport => "Aeropuerto",
        openttdrs_core::StopKind::RailWaypoint => "Waypoint",
        openttdrs_core::StopKind::RoadWaypoint => "Waypoint road",
    }
}

fn vehicles_visiting(sim: &SimWorld, station_pos: TileCoord) -> Vec<u32> {
    sim.state
        .vehicles
        .iter()
        .filter(|vehicle| {
            vehicle.is_consist_head()
                && vehicle.orders.iter().any(|order| {
                    matches!(order, VehicleOrder::Station { station, .. } if *station == station_pos)
                })
        })
        .map(|vehicle| vehicle.id)
        .collect()
}

pub(crate) fn sync_station_cargo_panel(
    mut station_panel: ResMut<StationCargoPanelState>,
    order_state: Res<OrderEditState>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<StationCargoPanelRoot>>,
    mut text_q: Query<&mut Text, With<StationCargoPanelText>>,
    mut rename_row_q: Query<&mut Node, With<StationCargoRenameRow>>,
    mut last_pos: Local<Option<TileCoord>>,
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    if station_panel.station_pos != *last_pos {
        station_panel.rename_editing = false;
        *last_pos = station_panel.station_pos;
    }
    let Some(station_pos) = station_panel.station_pos else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    let Some(station) = sim.state.stations.iter().find(|st| st.pos == station_pos) else {
        return;
    };

    if let Ok(mut row) = rename_row_q.single_mut() {
        row.display = if station_panel.rename_editing {
            Display::Flex
        } else {
            Display::None
        };
    }

    let name = station_display_name(station);
    let owner_name = sim
        .state
        .companies
        .iter()
        .find(|c| c.id == station.owner)
        .map_or_else(
            || format!("Compañía {}", station.owner.0),
            |c| c.name.clone(),
        );
    let joined = station.joined_tiles.len();
    let coverage = station_coverage_at(
        &sim.state.map,
        &sim.state.industries,
        station_pos,
        STATION_COVERAGE_RADIUS,
    );
    let mut out = if station.is_waypoint() {
        format!(
            "{name}\nWaypoint · ({}, {}) · {owner_name}\nRating global: {}/255",
            station_pos.x, station_pos.y, station.rating
        )
    } else {
        let mut lines = vec![
            name,
            format!(
                "{} · ({}, {}) · {owner_name}",
                station_kind_label(station.stop_kind),
                station_pos.x,
                station_pos.y,
            ),
            format!(
                "Rating {}/255 · ingresos ${} · tiles unidas {}",
                station.rating, station.income, joined
            ),
            format!(
                "Cobertura r{}: casas {} · stock ind. {}",
                STATION_COVERAGE_RADIUS, coverage.house_tiles, coverage.supplied_stock
            ),
            "Carga en espera:".to_string(),
        ];
        for cargo in CARGO_TYPES {
            let waiting = station.cargo_stock.get(cargo);
            if waiting == 0 && !station.accepts_cargo(cargo) {
                continue;
            }
            let rating = station_rating_for_cargo(station, cargo);
            let days = station.time_since_pickup.get(cargo);
            let age = if waiting > 0 {
                format!(" · sin recogida {days}d")
            } else {
                String::new()
            };
            lines.push(format!(
                "  {} · espera {waiting} · rating {rating}/255{age}",
                cargo_display_name(cargo)
            ));
        }
        lines.push(format!(
            "Packets en cola: {}",
            station.cargo_packets.packets.len()
        ));
        lines.join("\n")
    };

    let visiting = vehicles_visiting(&sim, station_pos);
    if visiting.is_empty() {
        out.push_str("\nVehículos en ruta: ninguno");
    } else {
        let ids = visiting
            .iter()
            .take(8)
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ");
        let extra = if visiting.len() > 8 {
            format!(" (+{})", visiting.len() - 8)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "\nVehículos en ruta ({}): {ids}{extra}",
            visiting.len()
        ));
    }

    let active_vehicle = vehicle_id_for_station_panel(&sim, station_pos, order_state.vehicle_id);
    if let Some(vid) = active_vehicle {
        out.push_str(&format!("\nVehículo activo para órdenes: #{vid}"));
    } else if !station.is_waypoint() {
        out.push_str("\nSelecciona un vehículo o usa «Editar órdenes».");
    }

    if let Ok(mut text) = text_q.single_mut() {
        **text = out;
    }
}

fn apply_station_rename(
    station_panel: &mut StationCargoPanelState,
    sim: &mut SimWorld,
    hud_feedback: &mut HudBuildFeedback,
    rename_input_q: &Query<&EditableText, With<StationCargoRenameInput>>,
    elapsed_secs: f32,
) {
    let Some(station_pos) = station_panel.station_pos else {
        return;
    };
    let name = rename_input_q
        .single()
        .ok()
        .map(|e| e.value().to_string())
        .filter(|s| !s.trim().is_empty());
    match apply_command(
        &mut sim.state,
        &Command::RenameStation { station_pos, name },
    ) {
        Ok(()) => station_panel.rename_editing = false,
        Err(e) => push_build_command_error(hud_feedback, e, elapsed_secs),
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_station_cargo_panel_buttons(
    mut q: Query<(&Interaction, &StationCargoPanelButton), (Changed<Interaction>, With<Button>)>,
    mut station_panel: ResMut<StationCargoPanelState>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut tool_state: ResMut<UiToolState>,
    mut station_build: ResMut<StationBuildState>,
    mut vehicle_list: ResMut<VehicleListState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut rename_input_q: Query<&mut EditableText, With<StationCargoRenameInput>>,
    time: Res<Time>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(station_pos) = station_panel.station_pos else {
            continue;
        };
        match button {
            StationCargoPanelButton::Close => {
                station_panel.station_pos = None;
                station_panel.rename_editing = false;
            }
            StationCargoPanelButton::CenterCamera => {
                let height = sim.state.map.get(station_pos).map_or(0, |tile| tile.height);
                let world = tile_pos(station_pos.x, station_pos.y, height, 0.0);
                if let Ok(mut transform) = cam_q.single_mut() {
                    transform.translation.x = world.x;
                    transform.translation.y = world.y;
                }
            }
            StationCargoPanelButton::Rename => {
                station_panel.rename_editing = true;
                if let Some(station) = sim.state.stations.iter().find(|s| s.pos == station_pos)
                    && let Ok(mut editable) = rename_input_q.single_mut()
                {
                    let seed = station.name.as_deref().unwrap_or("");
                    editable.editor_mut().set_text(seed);
                }
            }
            StationCargoPanelButton::ViewVehicles => {
                if let Some(station) = sim.state.stations.iter().find(|s| s.pos == station_pos) {
                    vehicle_list.open_for_station(station.pos, station.stop_kind);
                }
            }
            StationCargoPanelButton::JoinWith => {
                station_build.join_keep = Some(station_pos);
                tool_state.active_tool = Some(BuildMenuAction::JoinStation);
            }
            StationCargoPanelButton::PickOrders => {
                let Some(vehicle_id) =
                    vehicle_id_for_station_panel(&sim, station_pos, order_state.vehicle_id)
                else {
                    push_build_command_error(
                        &mut hud_feedback,
                        CommandError::VehicleNotFound,
                        time.elapsed_secs(),
                    );
                    continue;
                };
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    open_order_edit_for_vehicle(&mut order_state, vehicle, &mut next_pick);
                    tool_state.active_tool = None;
                }
            }
            StationCargoPanelButton::AddToRoute => {
                let Some(vehicle_id) =
                    vehicle_id_for_station_panel(&sim, station_pos, order_state.vehicle_id)
                else {
                    push_build_command_error(
                        &mut hud_feedback,
                        CommandError::VehicleNotFound,
                        time.elapsed_secs(),
                    );
                    continue;
                };
                order_state.vehicle_id = Some(vehicle_id);
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    order_state.orders = vehicle.orders.clone();
                    order_state.selected_slot = if vehicle.orders.is_empty() {
                        None
                    } else {
                        Some(vehicle.current_order.min(vehicle.orders.len() - 1))
                    };
                }
                match try_append_station_order(
                    &mut sim.state,
                    vehicle_id,
                    station_pos,
                    &mut order_state.orders,
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        order_state.selected_slot = order_state.orders.len().checked_sub(1);
                    }
                    Err(e) => {
                        order_state.orders.pop();
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
        }
    }
}

pub(crate) fn handle_station_rename_buttons(
    mut buttons: Query<
        (&Interaction, &StationCargoRenameButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut station_panel: ResMut<StationCargoPanelState>,
    rename_input_q: Query<&EditableText, With<StationCargoRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, action) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            StationCargoRenameButton::Cancel => {
                station_panel.rename_editing = false;
            }
            StationCargoRenameButton::Apply => {
                apply_station_rename(
                    &mut station_panel,
                    &mut sim,
                    &mut hud_feedback,
                    &rename_input_q,
                    time.elapsed_secs(),
                );
            }
        }
    }
}

/// Enter aplica el nombre; Escape cancela edición.
pub(crate) fn station_rename_keyboard(
    mut station_panel: ResMut<StationCargoPanelState>,
    keys: Res<ButtonInput<KeyCode>>,
    rename_input_q: Query<&EditableText, With<StationCargoRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !station_panel.rename_editing {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        station_panel.rename_editing = false;
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        apply_station_rename(
            &mut station_panel,
            &mut sim,
            &mut hud_feedback,
            &rename_input_q,
            time.elapsed_secs(),
        );
    }
}

/// Teclas alfanuméricas en el campo de renombrado.
pub(crate) fn station_rename_editable_keyboard(
    station_panel: Res<StationCargoPanelState>,
    mut key_events: MessageReader<KeyboardInput>,
    mut rename_input_q: Query<&mut EditableText, With<StationCargoRenameInput>>,
) {
    if !station_panel.rename_editing {
        return;
    }
    let Ok(mut editable) = rename_input_q.single_mut() else {
        return;
    };
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            editable.queue_edit(bevy::text::TextEdit::Backspace);
            continue;
        }
        if matches!(ev.logical_key, Key::Delete) {
            editable.queue_edit(bevy::text::TextEdit::Delete);
            continue;
        }
        let Some(text) = &ev.text else {
            continue;
        };
        for c in text.chars() {
            if !c.is_control() && editable.value().chars().count() < MAX_STATION_NAME_CHARS {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(c.to_string()),
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
    use openttdrs_core::{GameState, Station, StopKind};

    fn fixture_resources(world: &mut World) {
        world.init_resource::<OrderEditState>();
        world.init_resource::<UiToolState>();
        world.init_resource::<StationBuildState>();
        world.init_resource::<VehicleListState>();
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(NextState::<OrderPickState>::default());
    }

    #[test]
    fn center_button_moves_camera() {
        let mut world = World::new();
        let mut state = GameState::new(16, 16);
        let pos = TileCoord::new(3, 4);
        state
            .stations
            .push(Station::new_with_kind(pos, StopKind::BusStop));
        let height = state.map.get(pos).map_or(0, |t| t.height);
        let expected = tile_pos(pos.x, pos.y, height, 0.0);
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.insert_resource(StationCargoPanelState {
            station_pos: Some(pos),
            rename_editing: false,
        });
        fixture_resources(&mut world);
        world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), PrimaryGameCamera));
        world.spawn((
            Button,
            StationCargoPanelButton::CenterCamera,
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_station_cargo_panel_buttons)
            .unwrap();
        let cam = world
            .query_filtered::<&Transform, With<PrimaryGameCamera>>()
            .single(&world)
            .unwrap();
        assert!((cam.translation.x - expected.x).abs() < 0.01);
        assert!((cam.translation.y - expected.y).abs() < 0.01);
    }

    #[test]
    fn view_vehicles_opens_filtered_list() {
        let mut world = World::new();
        let mut state = GameState::new(16, 16);
        let pos = TileCoord::new(5, 6);
        state
            .stations
            .push(Station::new_with_kind(pos, StopKind::RailStation));
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.insert_resource(StationCargoPanelState {
            station_pos: Some(pos),
            rename_editing: false,
        });
        fixture_resources(&mut world);
        world.spawn((
            Button,
            StationCargoPanelButton::ViewVehicles,
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_station_cargo_panel_buttons)
            .unwrap();
        let list = world.resource::<VehicleListState>();
        assert!(list.open);
        assert_eq!(list.station_filter, Some(pos));
        assert_eq!(list.kind, crate::ui::vehicle_list::VehicleListKind::Train);
    }

    #[test]
    fn rename_apply_stores_station_name() {
        let mut world = World::new();
        let mut state = GameState::new(16, 16);
        let pos = TileCoord::new(2, 2);
        state
            .stations
            .push(Station::new_with_kind(pos, StopKind::BusStop));
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.insert_resource(StationCargoPanelState {
            station_pos: Some(pos),
            rename_editing: true,
        });
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.spawn((StationCargoRenameInput, EditableText::new("Central")));
        world.spawn((
            Button,
            StationCargoRenameButton::Apply,
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_station_rename_buttons)
            .unwrap();
        let sim = world.resource::<SimWorld>();
        assert_eq!(
            sim.state
                .stations
                .iter()
                .find(|s| s.pos == pos)
                .and_then(|s| s.name.as_deref()),
            Some("Central")
        );
        assert!(!world.resource::<StationCargoPanelState>().rename_editing);
    }
}
