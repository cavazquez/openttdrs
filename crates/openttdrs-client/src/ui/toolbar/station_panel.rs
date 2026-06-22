use bevy::prelude::*;
use openttdrs_core::{CommandError, TileCoord, VehicleOrder};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};

use super::order_panel::apply_order_edit;
use super::{BuildMenuUi, OrderEditState, UiToolState};

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
    AddToRoute,
    PickOrders,
    Close,
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
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    spawn_station_button(row, StationCargoPanelButton::AddToRoute, "Añadir a ruta");
                    spawn_station_button(
                        row,
                        StationCargoPanelButton::PickOrders,
                        "Editar órdenes",
                    );
                    spawn_station_button(row, StationCargoPanelButton::Close, "Cerrar");
                });
        });
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
                font_size: FontSize::Px(11.0),
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
                matches!(order, VehicleOrder::Station { station } if *station == station_pos)
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

pub(crate) fn sync_station_cargo_panel(
    station_panel: Res<StationCargoPanelState>,
    order_state: Res<OrderEditState>,
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
            vehicle.orders.iter().any(|order| {
                matches!(order, VehicleOrder::Station { station } if *station == station_pos)
            })
        })
        .count();
    out.push_str(&format!("\nVehículos en ruta a esta estación: {en_route}"));
    let active_vehicle = vehicle_id_for_station_panel(&sim, station_pos, order_state.vehicle_id);
    if let Some(vid) = active_vehicle {
        out.push_str(&format!("\nVehículo activo para órdenes: #{vid}"));
    } else {
        out.push_str("\nSelecciona un vehículo en el depósito (Órdenes) o usa «Editar órdenes».");
    }
    if let Ok(mut text) = text_q.single_mut() {
        **text = out;
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_station_cargo_panel_buttons(
    mut q: Query<(&Interaction, &StationCargoPanelButton), (Changed<Interaction>, With<Button>)>,
    mut station_panel: ResMut<StationCargoPanelState>,
    mut order_state: ResMut<OrderEditState>,
    mut tool_state: ResMut<UiToolState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
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
                    order_state.vehicle_id = Some(vehicle_id);
                    order_state.orders = vehicle.orders.clone();
                    order_state.picking_destination = false;
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
                }
                match try_append_station_order(
                    &mut sim.state,
                    vehicle_id,
                    station_pos,
                    &mut order_state.orders,
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        order_state.orders.pop();
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use openttdrs_core::{Command, GameState, TileCoord, Vehicle, VehicleKind, apply_command};

    use super::try_append_station_order;

    #[test]
    fn append_station_order_checks_vehicle_kind() {
        let mut state = GameState::new(8, 8);
        let stop = TileCoord::new(2, 2);
        let road = TileCoord::new(1, 2);
        apply_command(&mut state, &Command::PlaceRoad(road)).unwrap();
        apply_command(&mut state, &Command::PlaceBusStop(stop, 0)).unwrap();
        let bus = Vehicle::new(1, VehicleKind::Bus, road, road);
        state.vehicles.push(bus);
        let truck = Vehicle::new(2, VehicleKind::Truck, road, road);
        state.vehicles.push(truck);
        let mut orders = Vec::new();
        assert!(
            try_append_station_order(&mut state, 1, stop, &mut orders).is_ok(),
            "bus en parada bus"
        );
        let mut orders2 = Vec::new();
        assert_eq!(
            try_append_station_order(&mut state, 2, stop, &mut orders2).unwrap_err(),
            openttdrs_core::CommandError::IncompatibleStopForVehicle
        );
    }

    #[test]
    fn append_station_order_requires_station_entity() {
        let mut state = GameState::new(4, 4);
        state.vehicles.push(Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        ));
        let mut orders = Vec::new();
        assert_eq!(
            try_append_station_order(&mut state, 0, TileCoord::new(1, 1), &mut orders).unwrap_err(),
            openttdrs_core::CommandError::StationNotFound
        );
    }
}
