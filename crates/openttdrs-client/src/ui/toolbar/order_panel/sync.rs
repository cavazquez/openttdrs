use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::state::{OrderPickState, SimWorld, order_pick_active};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText,
};
use crate::ui::toolbar::{OrderEditState, OrderSlotState};
use crate::ui::vehicle_chain::{MAX_VEHICLE_CHAIN_SLOTS, VehicleChainSlot, vehicle_window_key};

use super::{ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

/// TitleText → contenedor → title bar → FloatingWindow root.
fn title_root_entity(child_of: &ChildOf, parents: &Query<&ChildOf>) -> Option<Entity> {
    let center = child_of.parent();
    let bar = parents.get(center).ok()?.parent();
    parents.get(bar).ok().map(ChildOf::parent)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_order_panel(
    mut order_state: ResMut<OrderEditState>,
    pick_state: Res<State<OrderPickState>>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    sim: Res<SimWorld>,
    mut root_q: Query<(
        Entity,
        &mut FloatingWindow,
        &VehicleChainSlot,
        &mut Visibility,
    )>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut row_q: Query<(
        &VehicleChainSlot,
        &OrderPanelRow,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &Interaction,
    )>,
    mut row_text_q: Query<
        (&VehicleChainSlot, &OrderPanelRowText, &mut Text),
        Without<FloatingWindowTitleText>,
    >,
) {
    // Refrescar órdenes desde sim para cada slot abierto.
    for slot in &mut order_state.slots {
        refresh_slot_from_sim(slot, &sim);
    }

    let any_open = order_state.slots.iter().any(|s| s.vehicle_id.is_some());
    if !any_open && order_pick_active(&pick_state) {
        next_pick.set(OrderPickState::Idle);
    }

    for (root_entity, mut win, chain_slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::Orders {
            continue;
        }
        let idx = chain_slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        let slot_state = order_state.slots[idx].clone();
        let vehicle_id = slot_state.vehicle_id;
        win.key = vehicle_window_key(FloatingWindowId::Orders, vehicle_id.unwrap_or(0));
        let Some(vehicle_id) = vehicle_id else {
            *vis = Visibility::Hidden;
            hide_order_rows_for_slot(&mut row_q, chain_slot.0);
            continue;
        };
        let Some(vehicle) = sim
            .state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == vehicle_id)
        else {
            *vis = Visibility::Hidden;
            hide_order_rows_for_slot(&mut row_q, chain_slot.0);
            continue;
        };

        *vis = Visibility::Visible;
        let pick_hint = if order_pick_active(&pick_state) && order_state.focused == Some(vehicle_id)
        {
            " · clic en parada"
        } else {
            ""
        };
        let title_name = {
            let shared = vehicle
                .shared_order_id
                .map_or_else(String::new, |id| format!(" · pool #{id}"));
            format!("{} (Órdenes){shared}{pick_hint}", vehicle.display_name())
        };
        for (title, mut text, child_of) in &mut title_q {
            if title.0 != FloatingWindowId::Orders {
                continue;
            }
            if title_root_entity(child_of, &parents) == Some(root_entity) {
                **text = title_name.clone();
            }
        }

        let drag_from = slot_state.list_drag_from;
        for (row_chain, row, mut node, mut bg, mut border, interaction) in &mut row_q {
            if row_chain.0 != chain_slot.0 {
                continue;
            }
            let has_content = row.slot == 0 && slot_state.orders.is_empty()
                || row.slot < slot_state.orders.len().min(ORDER_PANEL_ROWS);
            node.display = if has_content {
                Display::Flex
            } else {
                Display::None
            };
            let is_current = !slot_state.orders.is_empty()
                && row.slot
                    == vehicle
                        .current_order
                        .min(slot_state.orders.len().saturating_sub(1));
            let is_selected =
                slot_state.selected_slot == Some(row.slot) && row.slot < slot_state.orders.len();
            let is_drag_source = drag_from == Some(row.slot);
            let is_drop_target = drag_from.is_some_and(|from| {
                from != row.slot
                    && row.slot < slot_state.orders.len()
                    && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
            });
            *bg = if is_drop_target {
                BackgroundColor(Color::srgb(0.42, 0.48, 0.28))
            } else if is_drag_source {
                BackgroundColor(Color::srgb(0.62, 0.54, 0.34))
            } else if is_selected {
                BackgroundColor(Color::srgb(0.28, 0.32, 0.42))
            } else if is_current {
                BackgroundColor(Color::srgb(0.42, 0.35, 0.22))
            } else {
                BackgroundColor(Color::srgb(0.22, 0.18, 0.12))
            };
            *border = if is_drop_target {
                BorderColor::all(Color::srgb(0.72, 0.88, 0.42))
            } else if is_selected || is_drag_source {
                BorderColor::all(Color::srgb(0.55, 0.72, 0.95))
            } else if is_current {
                BorderColor::all(Color::srgb(0.88, 0.74, 0.46))
            } else {
                BorderColor::all(Color::srgb(0.45, 0.39, 0.27))
            };
        }
        let current_slot = if slot_state.orders.is_empty() {
            0usize
        } else {
            vehicle
                .current_order
                .min(slot_state.orders.len().saturating_sub(1))
        };
        for (text_chain, row_text, mut text) in &mut row_text_q {
            if text_chain.0 != chain_slot.0 {
                continue;
            }
            **text = if slot_state.orders.is_empty() && row_text.slot == 0 {
                "Sin órdenes — «Ir a» y clic en una parada del mapa.".to_string()
            } else if let Some(order) = slot_state.orders.get(row_text.slot) {
                let stuck_here = vehicle.no_network_route_to_order && row_text.slot == current_slot;
                order_row_label(row_text.slot, *order, vehicle, &sim, stuck_here)
            } else {
                String::new()
            };
        }
    }
}

fn refresh_slot_from_sim(slot: &mut OrderSlotState, sim: &SimWorld) {
    let Some(vehicle_id) = slot.vehicle_id else {
        return;
    };
    let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        *slot = OrderSlotState::default();
        return;
    };
    slot.orders = vehicle.orders.clone();
    if let Some(sel) = slot.selected_slot
        && sel >= slot.orders.len()
    {
        slot.selected_slot = slot
            .orders
            .len()
            .checked_sub(1)
            .or(if slot.orders.is_empty() {
                None
            } else {
                Some(0)
            });
    }
}

/// Limpia el estado al cerrar con ✕ / Esc (solo esa instancia).
pub(crate) fn order_panel_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
) {
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::Orders {
            continue;
        }
        let vehicle_id = msg.0.instance;
        if vehicle_id == 0 {
            continue;
        }
        order_state.close_vehicle(vehicle_id);
        if order_state.vehicle_id().is_none() {
            next_pick.set(OrderPickState::Idle);
        }
    }
}

fn hide_order_rows_for_slot(
    row_q: &mut Query<(
        &VehicleChainSlot,
        &OrderPanelRow,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &Interaction,
    )>,
    chain_slot: u8,
) {
    for (row_chain, _, mut node, _, _, _) in row_q.iter_mut() {
        if row_chain.0 == chain_slot {
            node.display = Display::None;
        }
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
                    "Depósito vía (parar)"
                } else {
                    "Depósito vía (serv. si hace falta)"
                }
            } else if stop {
                "Depósito (parar)"
            } else {
                "Depósito (serv. si hace falta)"
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
            comparator: _,
        } => {
            let cond = match condition {
                openttdrs_core::OrderConditionKind::CargoLoadAbove => "carga>",
                openttdrs_core::OrderConditionKind::CargoLoadBelow => "carga<",
                openttdrs_core::OrderConditionKind::LoadPercentage => "carga%",
                openttdrs_core::OrderConditionKind::Reliability => "fiab",
                openttdrs_core::OrderConditionKind::MaxSpeed => "vmax",
                openttdrs_core::OrderConditionKind::Age => "edad",
                openttdrs_core::OrderConditionKind::RequiresService => "serv",
                openttdrs_core::OrderConditionKind::Unconditionally => "siempre",
                openttdrs_core::OrderConditionKind::RemainingLifetime => "vida",
                openttdrs_core::OrderConditionKind::MaxReliability => "fiabmáx",
                openttdrs_core::OrderConditionKind::DrivingBackwards => "marcha atrás",
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
        wait_ticks,
        travel_ticks,
        ..
    } = order
    {
        let load_label = match order.load_type() {
            openttdrs_core::OrderLoadType::LoadIfPossible => "cargar si posible",
            openttdrs_core::OrderLoadType::FullLoad => "carga completa",
            openttdrs_core::OrderLoadType::FullLoadAny => "completar una carga",
            openttdrs_core::OrderLoadType::NoLoad => "no cargar",
        };
        let unload_label = match order.unload_type() {
            openttdrs_core::OrderUnloadType::UnloadIfPossible => "descargar si posible",
            openttdrs_core::OrderUnloadType::Unload => "descarga forzada",
            openttdrs_core::OrderUnloadType::Transfer => "transferir",
            openttdrs_core::OrderUnloadType::NoUnload => "no descargar",
        };
        line.push_str(&format!(" · {load_label} · {unload_label}"));
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
    use openttdrs_core::prelude::*;

    use crate::state::SimWorld;
    use crate::ui::toolbar::OrderEditState;
    use crate::ui::vehicle_chain::VehicleChainRegistry;

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

    #[test]
    fn order_row_labels_complete_load_and_unload_modes() {
        let sim = SimWorld::default();
        let stop = TileCoord::new(2, 2);
        let vehicle = Vehicle::new(1, VehicleKind::Truck, stop, stop);
        let order = VehicleOrder::station_with_types(
            stop,
            openttdrs_core::OrderLoadType::FullLoadAny,
            openttdrs_core::OrderUnloadType::Unload,
            openttdrs_core::OrderNonStop::NonStopDestination,
        );

        let label = order_row_label(0, order, &vehicle, &sim, false);
        assert!(label.contains("completar una carga"));
        assert!(label.contains("descarga forzada"));
    }

    #[test]
    fn two_orders_open_with_distinct_vehicle_ids() {
        let mut chain = VehicleChainRegistry::default();
        let s0 = chain.open_or_focus(10);
        let s1 = chain.open_or_focus(20);
        let mut state = OrderEditState::default();
        state.bind_slot(s0, 10, vec![], None);
        state.bind_slot(
            s1,
            20,
            vec![VehicleOrder::station(TileCoord::new(1, 1))],
            Some(0),
        );
        assert_eq!(state.slots[0].vehicle_id, Some(10));
        assert_eq!(state.slots[1].vehicle_id, Some(20));
        assert!(state.is_open_for(10));
        assert!(state.is_open_for(20));
        assert_eq!(state.vehicle_id(), Some(20));
    }

    #[test]
    fn closing_one_orders_keeps_the_other() {
        let mut chain = VehicleChainRegistry::default();
        let s0 = chain.open_or_focus(1);
        let s1 = chain.open_or_focus(2);
        let mut state = OrderEditState::default();
        state.bind_slot(s0, 1, vec![], None);
        state.bind_slot(s1, 2, vec![], None);
        state.close_vehicle(1);
        assert!(!state.is_open_for(1));
        assert!(state.is_open_for(2));
        assert_eq!(state.focused, Some(2));
    }
}
