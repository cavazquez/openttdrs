use bevy::prelude::*;
use openttdrs_core::{Station, StopKind, TileKind, Vehicle, VehicleOrder};

use crate::state::{OrderPickState, SimWorld, order_pick_active};
use crate::ui::toolbar::{OrderEditState, OrderPanelRoot, OrderPanelTitle};

use super::{ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_order_panel(
    order_state: Res<OrderEditState>,
    pick_state: Res<State<OrderPickState>>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<OrderPanelRoot>>,
    mut title_q: Query<&mut Text, With<OrderPanelTitle>>,
    mut row_q: Query<(
        &OrderPanelRow,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut row_text_q: Query<(&OrderPanelRowText, &mut Text), Without<OrderPanelTitle>>,
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    let Some(vehicle_id) = order_state.vehicle_id else {
        if order_pick_active(&pick_state) {
            next_pick.set(OrderPickState::Idle);
        }
        *vis = Visibility::Hidden;
        hide_order_rows(&mut row_q);
        return;
    };
    let Some(vehicle) = sim
        .state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == vehicle_id)
    else {
        *vis = Visibility::Hidden;
        hide_order_rows(&mut row_q);
        return;
    };

    *vis = Visibility::Visible;
    let pick_hint = if order_pick_active(&pick_state) {
        " · clic en parada"
    } else {
        ""
    };
    if let Ok(mut text) = title_q.single_mut() {
        let shared = vehicle
            .shared_order_id
            .map_or_else(String::new, |id| format!(" · pool #{id}"));
        **text = format!("{} (Órdenes){shared}{pick_hint}", vehicle.display_name());
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
        let is_selected =
            order_state.selected_slot == Some(row.slot) && row.slot < order_state.orders.len();
        *bg = if is_selected {
            BackgroundColor(Color::srgb(0.28, 0.32, 0.42))
        } else if is_current {
            BackgroundColor(Color::srgb(0.42, 0.35, 0.22))
        } else {
            BackgroundColor(Color::srgb(0.22, 0.18, 0.12))
        };
        *border = if is_selected {
            BorderColor::all(Color::srgb(0.55, 0.72, 0.95))
        } else if is_current {
            BorderColor::all(Color::srgb(0.88, 0.74, 0.46))
        } else {
            BorderColor::all(Color::srgb(0.45, 0.39, 0.27))
        };
    }
    let current_slot = if order_state.orders.is_empty() {
        0usize
    } else {
        vehicle
            .current_order
            .min(order_state.orders.len().saturating_sub(1))
    };
    for (row_text, mut text) in &mut row_text_q {
        **text = if order_state.orders.is_empty() && row_text.slot == 0 {
            "Sin órdenes — «Ir a» y clic en una parada del mapa.".to_string()
        } else if let Some(order) = order_state.orders.get(row_text.slot) {
            let stuck_here = vehicle.no_network_route_to_order && row_text.slot == current_slot;
            order_row_label(row_text.slot, *order, vehicle, &sim, stuck_here)
        } else {
            String::new()
        };
    }
}

fn hide_order_rows(
    row_q: &mut Query<(
        &OrderPanelRow,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (_, mut node, _, _) in row_q {
        node.display = Display::None;
    }
}

fn station_at_tile(sim: &SimWorld, pos: openttdrs_core::TileCoord) -> Option<&Station> {
    openttdrs_core::station_at_tile(&sim.state.map, &sim.state.stations, pos)
}

fn stop_kind_mismatch_note(vehicle: &Vehicle, station: &Station) -> Option<&'static str> {
    if station.can_service_vehicle(vehicle.kind) {
        return None;
    }
    Some(match station.stop_kind {
        StopKind::BusStop => " — incompatible: solo buses",
        StopKind::TruckStop => " — incompatible: solo camiones/carga",
        StopKind::Dock | StopKind::Buoy => " — incompatible: solo barcos",
        StopKind::Airport => " — incompatible: solo aviones",
        StopKind::RailStation | StopKind::RailWaypoint => " — incompatible: solo trenes",
        StopKind::RoadWaypoint => " — incompatible: solo vehículos de carretera",
    })
}

fn order_row_label(
    index: usize,
    order: VehicleOrder,
    vehicle: &Vehicle,
    sim: &SimWorld,
    stuck_here: bool,
) -> String {
    let pos = order.destination();
    let current = if !vehicle.orders.is_empty() && vehicle.current_order == index {
        ">"
    } else {
        " "
    };
    let label = match order {
        VehicleOrder::Station { .. } => match station_at_tile(sim, pos).map(|s| s.stop_kind) {
            Some(StopKind::BusStop) => "Parada bus",
            Some(StopKind::TruckStop) => "Parada carga",
            Some(StopKind::RailStation) => "Estacion tren",
            Some(StopKind::RailWaypoint) => "Waypoint",
            Some(StopKind::RoadWaypoint) => "Waypoint road",
            Some(StopKind::Dock) => "Muelle",
            Some(StopKind::Buoy) => "Boya",
            Some(StopKind::Airport) => "Aeropuerto",
            None => "Estación",
        },
        VehicleOrder::Waypoint { .. } => "Waypoint",
        VehicleOrder::Depot { depot, stop, .. } => {
            if sim.state.map.get_kind(depot) == Some(TileKind::RailDepot) {
                if stop {
                    "Depósito vía"
                } else {
                    "Depósito vía (paso)"
                }
            } else if stop {
                "Depósito"
            } else {
                "Depósito (paso)"
            }
        }
        VehicleOrder::Tile(tile) if sim.state.map.get_kind(tile) == Some(TileKind::RoadDepot) => {
            "Depósito"
        }
        VehicleOrder::Tile(tile) if sim.state.map.get_kind(tile) == Some(TileKind::RailDepot) => {
            "Depósito vía"
        }
        VehicleOrder::Tile(_) => "Casilla",
        VehicleOrder::Conditional {
            condition,
            value,
            jump_to,
        } => {
            let cond = match condition {
                openttdrs_core::OrderConditionKind::CargoLoadAbove => "carga>",
                openttdrs_core::OrderConditionKind::CargoLoadBelow => "carga<",
            };
            return format!(
                "{current} {:>2}. Cond. {cond}{value}% → ord.{}",
                index + 1,
                jump_to + 1
            );
        }
    };
    let mut line = format!("{current} {:>2}. {label} ({}, {})", index + 1, pos.x, pos.y);
    if let VehicleOrder::Station {
        full_load,
        no_unload,
        wait_ticks,
        travel_ticks,
        ..
    } = order
    {
        if full_load {
            line.push_str(" · carga completa");
        }
        if no_unload {
            line.push_str(" · no descargar");
        }
        if wait_ticks > 0 {
            line.push_str(&format!(" · esp.{wait_ticks}"));
        }
        if travel_ticks > 0 {
            line.push_str(&format!(" · viaje {travel_ticks}"));
        }
    } else if let VehicleOrder::Depot {
        stop,
        wait_ticks,
        travel_ticks,
        refit_cargo,
        ..
    } = order
    {
        if stop {
            line.push_str(" · parar");
        } else {
            line.push_str(" · servicio");
        }
        if let Some(cargo) = refit_cargo {
            line.push_str(&format!(
                " · refit {}",
                openttdrs_core::cargo_display_name(cargo)
            ));
        }
        if wait_ticks > 0 {
            line.push_str(&format!(" · esp.{wait_ticks}"));
        }
        if travel_ticks > 0 {
            line.push_str(&format!(" · viaje {travel_ticks}"));
        }
    } else if let VehicleOrder::Waypoint { travel_ticks, .. } = order
        && travel_ticks > 0
    {
        line.push_str(&format!(" · viaje {travel_ticks}"));
    }
    if let Some(st) = station_at_tile(sim, pos)
        && let Some(note) = stop_kind_mismatch_note(vehicle, st)
    {
        line.push_str(note);
    }
    if stuck_here {
        line.push_str(" · sin ruta por red");
    }
    line
}

#[cfg(test)]
mod tests {
    use openttdrs_core::{TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder};

    use crate::state::SimWorld;

    use super::order_row_label;

    #[test]
    fn order_row_labels_depots() {
        let mut sim = SimWorld::default();
        let depot = TileCoord::new(1, 2);
        assert!(
            sim.state.map.set_kind(depot, TileKind::RoadDepot).is_ok(),
            "depot tile should be valid in default map"
        );
        let vehicle = Vehicle::new(1, VehicleKind::Bus, depot, depot);

        assert!(
            order_row_label(0, VehicleOrder::tile(depot), &vehicle, &sim, false)
                .contains("Depósito")
        );

        let rail_depot = TileCoord::new(2, 3);
        assert!(
            sim.state
                .map
                .set_kind(rail_depot, TileKind::RailDepot)
                .is_ok(),
            "rail depot tile should be valid in default map"
        );
        let train = Vehicle::new(2, VehicleKind::Train, rail_depot, rail_depot);
        assert!(
            order_row_label(0, VehicleOrder::tile(rail_depot), &train, &sim, false)
                .contains("Depósito vía")
        );
    }
}
