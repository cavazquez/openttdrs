//! Tests de comandos ferroviarios — órdenes, flota, refit y profit.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command};
use crate::test_fixtures::SandboxMap;
use crate::{
    CargoType, GameState, OrderConditionKind, OrderUnloadType, TileCoord, Vehicle, VehicleKind,
    VehicleOrder, pathfinder,
};

#[test]
fn train_order_through_waypoint_advances_without_full_stop() {
    let mut s = GameState::new(12, 12);
    let wp = TileCoord::new(5, 5);
    let end = TileCoord::new(8, 5);
    for x in 4..=8 {
        apply_command(&mut s, &Command::SetRailBits(TileCoord::new(x, 5), 0x01)).unwrap();
    }
    apply_command(&mut s, &Command::PlaceRailWaypoint(wp)).unwrap();
    s.vehicles.push(Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(4, 5),
        TileCoord::new(4, 5),
    ));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::waypoint(wp), VehicleOrder::tile(end)]),
    )
    .unwrap();
    s.vehicles[0].set_cruise_speed();
    s.vehicles[0].sync_order_destination(&s.map);
    let path = pathfinder::find_path(
        &s.map,
        TileCoord::new(4, 5),
        wp,
        pathfinder::PathNetwork::Rail,
    );
    assert!(path.is_some());
    assert!(path.unwrap().contains(&wp));
}

#[test]
fn remove_vehicle_order_at_adjusts_current_order() {
    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(2, 2);
    let b = TileCoord::new(4, 2);
    let c = TileCoord::new(6, 2);
    for x in 2..=6 {
        apply_command(&mut s, &Command::SetRailBits(TileCoord::new(x, 2), 0x01)).unwrap();
    }
    s.vehicles.push(Vehicle::new(1, VehicleKind::Train, a, a));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            1,
            vec![
                VehicleOrder::tile(a),
                VehicleOrder::tile(b),
                VehicleOrder::tile(c),
            ],
        ),
    )
    .unwrap();
    s.vehicles[0].current_order = 2;
    apply_command(
        &mut s,
        &Command::RemoveVehicleOrderAt {
            vehicle_id: 1,
            index: 1,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].orders.len(), 2);
    assert_eq!(s.vehicles[0].current_order, 1);
}

#[test]
fn skip_vehicle_order_advances_current() {
    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(2, 2);
    let b = TileCoord::new(4, 2);
    s.vehicles.push(Vehicle::new(1, VehicleKind::Bus, a, a));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::tile(a), VehicleOrder::tile(b)]),
    )
    .unwrap();
    assert_eq!(s.vehicles[0].current_order, 0);
    apply_command(&mut s, &Command::SkipVehicleOrder(1)).unwrap();
    assert_eq!(s.vehicles[0].current_order, 1);
}

#[test]
fn toggle_full_load_on_station_order() {
    let mut s = GameState::new(8, 8);
    let stop = TileCoord::new(3, 3);
    let road = TileCoord::new(3, 2);
    apply_command(&mut s, &Command::PlaceRoad(road)).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(stop, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, stop, stop));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::station(stop)]),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::ToggleVehicleOrderFullLoad {
            vehicle_id: 1,
            index: 0,
        },
    )
    .unwrap();
    assert!(s.vehicles[0].orders[0].full_load());
    for expected in [
        OrderUnloadType::Unload,
        OrderUnloadType::Transfer,
        OrderUnloadType::NoUnload,
        OrderUnloadType::UnloadIfPossible,
    ] {
        apply_command(
            &mut s,
            &Command::ToggleVehicleOrderNoUnload {
                vehicle_id: 1,
                index: 0,
            },
        )
        .unwrap();
        assert_eq!(s.vehicles[0].orders[0].unload_type(), expected);
    }
}

#[test]
fn append_goto_nearest_depot_adds_depot_order() {
    let mut s = GameState::new(10, 10);
    let depot = TileCoord::new(5, 5);
    let bus_pos = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(bus_pos)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(5, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, bus_pos, bus_pos));
    apply_command(&mut s, &Command::AppendGotoNearestDepot(1)).unwrap();
    assert_eq!(s.vehicles[0].orders.len(), 1);
    assert_eq!(s.vehicles[0].orders[0].destination(), depot);
}

#[test]
fn rename_vehicle_stores_trimmed_name() {
    let mut s = GameState::new(4, 4);
    s.vehicles.push(Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(1, 1),
        TileCoord::new(1, 1),
    ));
    apply_command(
        &mut s,
        &Command::RenameVehicle {
            vehicle_id: 1,
            name: Some("  Ruta 42  ".to_string()),
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].name.as_deref(), Some("Ruta 42"));
}

#[test]
fn rename_station_stores_trimmed_name() {
    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(2, 2);
    s.stations
        .push(crate::Station::new_with_kind(pos, crate::StopKind::BusStop));
    apply_command(
        &mut s,
        &Command::RenameStation {
            station_pos: pos,
            name: Some("  Central  ".to_string()),
        },
    )
    .unwrap();
    let station = s.stations.iter().find(|st| st.pos == pos).unwrap();
    assert_eq!(station.name.as_deref(), Some("Central"));
}

#[test]
fn move_vehicle_order_swaps_and_tracks_current() {
    use crate::command::OrderMoveDirection;

    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(2, 2);
    let b = TileCoord::new(4, 2);
    let c = TileCoord::new(6, 2);
    s.vehicles.push(Vehicle::new(1, VehicleKind::Bus, a, a));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            1,
            vec![
                VehicleOrder::tile(a),
                VehicleOrder::tile(b),
                VehicleOrder::tile(c),
            ],
        ),
    )
    .unwrap();
    s.vehicles[0].current_order = 1;
    apply_command(
        &mut s,
        &Command::MoveVehicleOrder {
            vehicle_id: 1,
            index: 1,
            direction: OrderMoveDirection::Up,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].orders[0].destination(), b);
    assert_eq!(s.vehicles[0].orders[1].destination(), a);
    assert_eq!(s.vehicles[0].current_order, 0);
}

#[test]
fn toggle_depot_stop_on_depot_order() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, depot, depot));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::depot(depot)]),
    )
    .unwrap();
    assert!(s.vehicles[0].orders[0].depot_stops());
    apply_command(
        &mut s,
        &Command::ToggleVehicleOrderDepotStop {
            vehicle_id: 1,
            index: 0,
        },
    )
    .unwrap();
    assert!(!s.vehicles[0].orders[0].depot_stops());
}

#[test]
fn turn_around_vehicle_reverses_train_heading() {
    use crate::vehicle::{DIR_N, DIR_S};

    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(2, 2);
    let mut train = Vehicle::new(1, VehicleKind::Train, pos, pos);
    train.direction = DIR_N;
    train.cur_speed = 80;
    s.vehicles.push(train);
    apply_command(&mut s, &Command::TurnAroundVehicle(1)).unwrap();
    assert_eq!(s.vehicles[0].direction, DIR_S);
    assert_eq!(s.vehicles[0].cur_speed, 0);
}

#[test]
fn refit_truck_in_depot_changes_cargo_type() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::RefitVehicle {
            vehicle_id: id,
            cargo: CargoType::Coal,
            unit_ids: vec![],
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].cargo_type, Some(CargoType::Coal));
}

#[test]
fn refit_rejects_with_cargo_on_board() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    s.vehicles[0].cargo = 5;
    let id = s.vehicles[0].id;
    assert_eq!(
        apply_command(
            &mut s,
            &Command::RefitVehicle {
                vehicle_id: id,
                cargo: CargoType::Coal,
                unit_ids: vec![],
            },
        ),
        Err(CommandError::RefitNotAllowed)
    );
}

#[test]
fn force_vehicle_proceed_sets_flag_on_train() {
    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(2, 2);
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Train, pos, pos));
    apply_command(&mut s, &Command::ForceVehicleProceed(1)).unwrap();
    assert!(s.vehicles[0].force_proceed);
    apply_command(&mut s, &Command::ForceVehicleProceed(2)).unwrap_err();
}

#[test]
fn autoreplace_upgrades_truck_in_depot() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetAutoReplaceRule {
            from_engine_id: crate::engine::ENGINE_TRUCK_MPS,
            to_engine_id: crate::engine::ENGINE_TRUCK_BALOGH_GOODS,
        },
    )
    .unwrap();
    // `try_autoreplace_vehicle` usa la economía de la compañía y respeta `engine_renew_money`.
    s.companies[0].economy.money = s.economy.money;
    s.companies[0].engine_renew_money = 0;
    assert!(crate::autoreplace::try_autoreplace_vehicle(&mut s, id).unwrap());
    assert_eq!(
        s.vehicles[0].engine_id,
        Some(crate::engine::ENGINE_TRUCK_BALOGH_GOODS)
    );
}

#[test]
fn vehicle_group_assign_and_save_v8_fields() {
    let mut s = GameState::new(8, 8);
    apply_command(
        &mut s,
        &Command::CreateVehicleGroup {
            name: "Buses centro".into(),
        },
    )
    .unwrap();
    let group_id = s.vehicle_groups[0].id;
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    assert_eq!(s.vehicles[0].build_tick, s.tick.get());
    apply_command(
        &mut s,
        &Command::AssignVehicleToGroup {
            vehicle_id: id,
            group_id: Some(group_id),
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].group_id, Some(group_id));
    assert_eq!(s.vehicles[0].vehicle_age_years(s.tick.get()), 0);
}

#[test]
fn timetable_lateness_clear_command() {
    let mut s = GameState::new(4, 4);
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 1),
    );
    v.timetable_lateness = 42;
    s.vehicles.push(v);
    apply_command(&mut s, &Command::ClearVehicleTimetableLateness(1)).unwrap();
    assert_eq!(s.vehicles[0].timetable_lateness, 0);
}

#[test]
fn autoreplace_only_when_old_skips_young_vehicle() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetAutoReplaceRule {
            from_engine_id: crate::engine::ENGINE_TRUCK_MPS,
            to_engine_id: crate::engine::ENGINE_TRUCK_BALOGH_GOODS,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::ToggleAutoReplaceOnlyWhenOld {
            from_engine_id: crate::engine::ENGINE_TRUCK_MPS,
        },
    )
    .unwrap();
    assert!(!crate::autoreplace::try_autoreplace_vehicle(&mut s, id).unwrap());
}

#[test]
fn shared_orders_sync_linked_vehicles() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    let a = s.vehicles[0].id;
    let b = s.vehicles[1].id;
    s.vehicles[0].orders = vec![VehicleOrder::tile(depot)];
    apply_command(&mut s, &Command::CreateSharedOrdersFromVehicle(a)).unwrap();
    let shared_id = s.vehicles[0].shared_order_id.unwrap();
    apply_command(
        &mut s,
        &Command::LinkVehicleToSharedOrders {
            vehicle_id: b,
            shared_id,
        },
    )
    .unwrap();
    s.shared_order_lists[0].orders = vec![
        VehicleOrder::tile(depot),
        VehicleOrder::tile(TileCoord::new(3, 2)),
    ];
    crate::shared_orders::sync_shared_orders_to_vehicles(&mut s, shared_id);
    assert_eq!(s.vehicles[0].orders.len(), 2);
    assert_eq!(s.vehicles[1].orders.len(), 2);
}

#[test]
fn conditional_order_jumps_when_cargo_above_threshold() {
    let pos = TileCoord::new(1, 1);
    let mut v = Vehicle::new(1, VehicleKind::Truck, pos, pos);
    v.cargo = 60;
    v.capacity = 100;
    v.orders = vec![
        VehicleOrder::conditional(OrderConditionKind::CargoLoadAbove, 50, 2),
        VehicleOrder::tile(TileCoord::new(0, 0)),
        VehicleOrder::tile(TileCoord::new(2, 2)),
    ];
    v.current_order = 0;
    v.resolve_conditional_orders();
    assert_eq!(v.current_order, 2);
}

#[test]
fn vehicle_profit_tracks_income_and_running_costs() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    s.vehicles[0].profit_this_year = 0;
    s.vehicles[0].running = true;
    s.vehicles[0].cur_speed = 10;
    for _ in 0..5_000 {
        if let Some(v) = s.vehicles.iter_mut().find(|v| v.id == id) {
            v.running = true;
            v.cur_speed = 10;
        }
        s.step();
    }
    let truck = s.vehicles.iter().find(|v| v.id == id).expect("camión");
    assert!(
        truck.profit_this_year < 0,
        "costes de marcha restan beneficio"
    );
    let after_cost = truck.profit_this_year;
    if let Some(v) = s.vehicles.iter_mut().find(|v| v.id == id) {
        v.profit_this_year = after_cost.saturating_add(500);
    }
    assert_eq!(
        s.vehicles
            .iter()
            .find(|v| v.id == id)
            .unwrap()
            .profit_this_year,
        after_cost + 500
    );
}

#[test]
fn refit_partial_consist_units() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    for x in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(4, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_KIRBY),
    )
    .unwrap();
    let head = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_WAGON_GOODS),
    )
    .unwrap();
    let wagon = s.vehicles.iter().find(|v| v.id != head).unwrap().id;
    apply_command(
        &mut s,
        &Command::AttachWagonToConsist {
            head_id: head,
            wagon_id: wagon,
        },
    )
    .unwrap();
    let wagon_before = s
        .vehicles
        .iter()
        .find(|v| v.id == wagon)
        .unwrap()
        .cargo_type;
    apply_command(
        &mut s,
        &Command::RefitVehicle {
            vehicle_id: head,
            cargo: CargoType::Oil,
            unit_ids: vec![head],
        },
    )
    .unwrap();
    assert_eq!(
        s.vehicles.iter().find(|v| v.id == head).unwrap().cargo_type,
        Some(CargoType::Oil)
    );
    assert_eq!(
        s.vehicles
            .iter()
            .find(|v| v.id == wagon)
            .unwrap()
            .cargo_type,
        wagon_before
    );
}

#[test]
fn cycle_depot_order_refit_and_apply_on_arrival() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(id, vec![VehicleOrder::depot(depot)]),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::CycleVehicleOrderDepotRefit {
            vehicle_id: id,
            index: 0,
        },
    )
    .unwrap();
    let cargo = s.vehicles[0].orders[0].depot_refit_cargo();
    assert!(cargo.is_some());
    s.vehicles[0].cargo = 0;
    s.vehicles[0].pending_depot_order_refit = cargo;
    s.step();
    assert_eq!(s.vehicles[0].cargo_type, cargo);
    assert!(s.vehicles[0].pending_depot_order_refit.is_none());
}
