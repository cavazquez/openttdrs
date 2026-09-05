use bevy::prelude::*;
use openttdrs_core::CargoType;
use openttdrs_core::prelude::*;

use crate::i18n::{Locale, text as localized};
use crate::settings::ClientPreferences;
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
    prefs: Option<Res<ClientPreferences>>,
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
    let locale = prefs.as_ref().map_or(Locale::Es, |prefs| prefs.locale());

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
        let title_name = order_panel_title(
            locale,
            vehicle,
            order_pick_active(&pick_state) && order_state.focused == Some(vehicle_id),
        );
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
                empty_order_hint(locale).to_owned()
            } else if let Some(order) = slot_state.orders.get(row_text.slot) {
                let stuck_here = vehicle.no_network_route_to_order && row_text.slot == current_slot;
                order_row_label(locale, row_text.slot, *order, vehicle, &sim, stuck_here)
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

fn order_panel_title(locale: Locale, vehicle: &Vehicle, pick_active: bool) -> String {
    let pick_hint = if pick_active {
        localized(locale, " · clic en parada")
    } else {
        ""
    };
    let shared = vehicle.shared_order_id.map_or_else(String::new, |id| {
        format!("{}{id}", localized(locale, " · pool #"))
    });
    format!(
        "{} ({}){shared}{pick_hint}",
        vehicle.display_name(),
        localized(locale, "Órdenes")
    )
}

fn empty_order_hint(locale: Locale) -> &'static str {
    localized(
        locale,
        "Sin órdenes — «Ir a» y clic en una parada del mapa.",
    )
}

fn order_cargo_label(locale: Locale, cargo: CargoType) -> &'static str {
    if locale == Locale::Es {
        return cargo.display_name();
    }
    match cargo {
        CargoType::Passengers => "passengers",
        CargoType::Coal => "coal",
        CargoType::Mail => "mail",
        CargoType::Oil => "oil",
        CargoType::Livestock => "livestock",
        CargoType::Goods => "goods",
        CargoType::Grain => "grain",
        CargoType::Wood => "wood",
        CargoType::IronOre => "iron ore",
        CargoType::Steel => "steel",
        CargoType::Valuables => "valuables",
        CargoType::Wheat => "wheat",
        CargoType::Paper => "paper",
        CargoType::Gold => "gold",
        CargoType::Food => "food",
        CargoType::Rubber => "rubber",
        CargoType::Fruit => "fruit",
        CargoType::Maize => "maize",
        CargoType::CopperOre => "copper ore",
        CargoType::Water => "water",
        CargoType::Diamonds => "diamonds",
        CargoType::Sugar => "sugar",
        CargoType::Toys => "toys",
        CargoType::Batteries => "batteries",
        CargoType::Candy => "candy",
        CargoType::Toffee => "toffee",
        CargoType::Cola => "cola",
        CargoType::CottonCandy => "cotton candy",
        CargoType::Bubbles => "bubbles",
        CargoType::Plastic => "plastic",
        CargoType::FizzyDrinks => "fizzy drinks",
        CargoType::Custom(_) => "custom cargo",
    }
}

fn station_at_tile(sim: &SimWorld, pos: openttdrs_core::TileCoord) -> Option<&Station> {
    openttdrs_core::station_at_tile(&sim.state.map, &sim.state.stations, pos)
}

fn stop_kind_mismatch_note(
    locale: Locale,
    vehicle: &Vehicle,
    station: &Station,
) -> Option<&'static str> {
    if station.can_service_vehicle(vehicle.kind) {
        return None;
    }
    let source = match station.stop_kind {
        StopKind::BusStop => " — incompatible: solo buses",
        StopKind::TruckStop => " — incompatible: solo camiones/carga",
        StopKind::Dock | StopKind::Buoy => " — incompatible: solo barcos",
        StopKind::Airport => " — incompatible: solo aviones",
        StopKind::RailStation | StopKind::RailWaypoint => " — incompatible: solo trenes",
        StopKind::RoadWaypoint => " — incompatible: solo vehículos de carretera",
    };
    Some(localized(locale, source))
}

fn append_order_times(line: &mut String, locale: Locale, wait_ticks: u32, travel_ticks: u32) {
    if wait_ticks > 0 {
        match locale {
            Locale::Es => line.push_str(&format!(" · esp.{wait_ticks}")),
            Locale::En => line.push_str(&format!(" · wait {wait_ticks}")),
        }
    }
    if travel_ticks > 0 {
        match locale {
            Locale::Es => line.push_str(&format!(" · viaje {travel_ticks}")),
            Locale::En => line.push_str(&format!(" · travel {travel_ticks}")),
        }
    }
}

fn order_row_label(
    locale: Locale,
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
    let label_source = match order {
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
                "{current} {:>2}. {} {}{value}% → {}{}",
                index + 1,
                localized(locale, "Cond."),
                localized(locale, cond),
                localized(locale, "ord."),
                jump_to + 1
            );
        }
    };
    let label = localized(locale, label_source);
    let mut line = format!("{current} {:>2}. {label} ({}, {})", index + 1, pos.x, pos.y);
    if let VehicleOrder::Station {
        wait_ticks,
        travel_ticks,
        ..
    } = order
    {
        let load_source = match order.load_type() {
            openttdrs_core::OrderLoadType::LoadIfPossible => "cargar si posible",
            openttdrs_core::OrderLoadType::FullLoad => "carga completa",
            openttdrs_core::OrderLoadType::FullLoadAny => "completar una carga",
            openttdrs_core::OrderLoadType::NoLoad => "no cargar",
        };
        let unload_source = match order.unload_type() {
            openttdrs_core::OrderUnloadType::UnloadIfPossible => "descargar si posible",
            openttdrs_core::OrderUnloadType::Unload => "descarga forzada",
            openttdrs_core::OrderUnloadType::Transfer => "transferir",
            openttdrs_core::OrderUnloadType::NoUnload => "no descargar",
        };
        let non_stop_source = if order.non_stop_destination() {
            "sin paradas intermedias"
        } else {
            "paradas intermedias"
        };
        let stop_location_source = match order.stop_location() {
            openttdrs_core::OrderStopLocation::NearEnd => "andén cercano",
            openttdrs_core::OrderStopLocation::Middle => "andén central",
            openttdrs_core::OrderStopLocation::FarEnd => "andén lejano",
        };
        line.push_str(&format!(
            " · {} · {} · {} · {}",
            localized(locale, load_source),
            localized(locale, unload_source),
            localized(locale, non_stop_source),
            localized(locale, stop_location_source),
        ));
        append_order_times(&mut line, locale, wait_ticks, travel_ticks);
    } else if let VehicleOrder::Depot {
        stop,
        wait_ticks,
        travel_ticks,
        refit_cargo,
        ..
    } = order
    {
        if stop {
            line.push_str(" · ");
            line.push_str(localized(locale, "parar"));
        } else {
            line.push_str(" · ");
            line.push_str(localized(locale, "servicio"));
        }
        if let Some(cargo) = refit_cargo {
            line.push_str(&format!(" · refit {}", order_cargo_label(locale, cargo)));
        }
        append_order_times(&mut line, locale, wait_ticks, travel_ticks);
    } else if let VehicleOrder::Waypoint { travel_ticks, .. } = order
        && travel_ticks > 0
    {
        append_order_times(&mut line, locale, 0, travel_ticks);
    }
    if let Some(st) = station_at_tile(sim, pos)
        && let Some(note) = stop_kind_mismatch_note(locale, vehicle, st)
    {
        line.push_str(note);
    }
    if stuck_here {
        line.push_str(localized(locale, " · sin ruta por red"));
    }
    line
}

#[cfg(test)]
mod tests {
    use openttdrs_core::prelude::*;
    use openttdrs_core::{CargoType, OrderConditionKind};

    use crate::i18n::Locale;
    use crate::state::SimWorld;
    use crate::ui::toolbar::OrderEditState;
    use crate::ui::vehicle_chain::VehicleChainRegistry;

    use super::{
        empty_order_hint, order_cargo_label, order_panel_title, order_row_label,
        stop_kind_mismatch_note,
    };

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
            order_row_label(
                Locale::Es,
                0,
                VehicleOrder::tile(depot),
                &vehicle,
                &sim,
                false
            )
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
            order_row_label(
                Locale::Es,
                0,
                VehicleOrder::tile(rail_depot),
                &train,
                &sim,
                false,
            )
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

        let label = order_row_label(Locale::Es, 0, order, &vehicle, &sim, false);
        assert!(label.contains("completar una carga"));
        assert!(label.contains("descarga forzada"));
        assert!(label.contains("sin paradas intermedias"));
        assert!(label.contains("andén central"));
    }

    #[test]
    fn order_row_labels_intermediate_stops_and_platform_end() {
        let sim = SimWorld::default();
        let stop = TileCoord::new(2, 2);
        let vehicle = Vehicle::new(1, VehicleKind::Train, stop, stop);
        let Some(order) = VehicleOrder::station_with_types(
            stop,
            openttdrs_core::OrderLoadType::LoadIfPossible,
            openttdrs_core::OrderUnloadType::UnloadIfPossible,
            openttdrs_core::OrderNonStop::StopAtIntermediate,
        )
        .with_cycled_stop_location() else {
            panic!("station order should support platform position");
        };

        let label = order_row_label(Locale::Es, 0, order, &vehicle, &sim, false);
        assert!(label.contains("paradas intermedias"));
        assert!(label.contains("andén lejano"));
    }

    #[test]
    fn order_panel_dynamic_text_follows_the_active_locale() {
        let sim = SimWorld::default();
        let stop = TileCoord::new(2, 2);
        let mut vehicle = Vehicle::new(1, VehicleKind::Truck, stop, stop);
        vehicle.shared_order_id = Some(7);
        let Some(order) = VehicleOrder::station_with_types(
            stop,
            openttdrs_core::OrderLoadType::FullLoadAny,
            openttdrs_core::OrderUnloadType::Unload,
            openttdrs_core::OrderNonStop::NonStopDestination,
        )
        .with_cycled_wait() else {
            panic!("station order should support a wait timetable");
        };
        let order = order.with_cycled_travel();

        let title = order_panel_title(Locale::En, &vehicle, true);
        assert!(title.contains("(Orders)"));
        assert!(title.contains("shared pool #7"));
        assert!(title.contains("click a stop"));
        assert_eq!(
            empty_order_hint(Locale::En),
            "No orders — “Go to” and click a stop on the map."
        );

        let label = order_row_label(Locale::En, 0, order, &vehicle, &sim, true);
        assert!(label.contains("Station"));
        assert!(label.contains("full load any cargo"));
        assert!(label.contains("unload all"));
        assert!(label.contains("non-stop"));
        assert!(label.contains("middle of platform"));
        assert!(label.contains("wait 30"));
        assert!(label.contains("travel 60"));
        assert!(label.contains("no network route"));
        let Some(depot_refit) =
            VehicleOrder::depot(stop).with_cycled_depot_refit(&[CargoType::Passengers])
        else {
            panic!("depot order should support cargo refit");
        };
        let depot_label = order_row_label(Locale::En, 1, depot_refit, &vehicle, &sim, false);
        assert!(depot_label.contains("refit passengers"));
        assert_eq!(
            order_cargo_label(Locale::En, CargoType::Custom(17)),
            "custom cargo"
        );

        let conditional = VehicleOrder::conditional(OrderConditionKind::CargoLoadAbove, 50, 2);
        let conditional_label = order_row_label(Locale::En, 2, conditional, &vehicle, &sim, false);
        assert!(conditional_label.contains("If load>50% → order3"));

        let bus_stop = Station::new_with_kind(stop, StopKind::BusStop);
        assert_eq!(
            stop_kind_mismatch_note(Locale::En, &vehicle, &bus_stop),
            Some(" — incompatible: buses only")
        );
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
