use bevy::prelude::*;
use openttdrs_core::{Station, StopKind, TileKind, Vehicle, VehicleKind, VehicleOrder};

use crate::state::SimWorld;
use crate::ui::toolbar::{OrderEditState, OrderPanelRoot, OrderPanelText};

use super::{ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

pub(crate) fn sync_order_panel(
    mut order_state: ResMut<OrderEditState>,
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
    let Some(vehicle) = sim
        .state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == vehicle_id)
    else {
        order_state.vehicle_id = None;
        order_state.orders.clear();
        *vis = Visibility::Hidden;
        for (_, mut node, _, _) in &mut row_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;
    let route_note = if vehicle.no_network_route_to_order {
        " · sin ruta por red al destino actual"
    } else {
        ""
    };
    let out = format!(
        "Vehículo #{} ({}) · carga {}/{} · destino ({},{}){route_note}",
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
    let current_slot = if order_state.orders.is_empty() {
        0usize
    } else {
        vehicle
            .current_order
            .min(order_state.orders.len().saturating_sub(1))
    };
    for (row_text, mut text) in &mut row_text_q {
        **text = if order_state.orders.is_empty() && row_text.slot == 0 {
            "Pendiente: clica el mapa para añadir paradas (estación o depósito).".to_string()
        } else if let Some(order) = order_state.orders.get(row_text.slot) {
            let stuck_here = vehicle.no_network_route_to_order && row_text.slot == current_slot;
            order_row_label(row_text.slot, *order, vehicle, &sim, stuck_here)
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

fn station_at_tile(sim: &SimWorld, pos: openttdrs_core::TileCoord) -> Option<&Station> {
    sim.state.stations.iter().find(|s| s.pos == pos)
}

fn stop_kind_mismatch_note(vehicle: &Vehicle, station: &Station) -> Option<&'static str> {
    if station.can_service_vehicle(vehicle.kind) {
        return None;
    }
    Some(match station.stop_kind {
        StopKind::BusStop => " — incompatible: solo buses",
        StopKind::TruckStop => " — incompatible: solo camiones/carga",
        StopKind::RailStation => " — incompatible: solo trenes",
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
            None => "Estación",
        },
        VehicleOrder::Tile(tile) if sim.state.map.get_kind(tile) == Some(TileKind::RoadDepot) => {
            "Depósito"
        }
        VehicleOrder::Tile(_) => "Casilla",
    };
    let mut line = format!("{current} {:>2}. {label} ({}, {})", index + 1, pos.x, pos.y);
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
    }
}
