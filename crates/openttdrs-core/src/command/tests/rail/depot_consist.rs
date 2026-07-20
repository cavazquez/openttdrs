//! Tests de comandos ferroviarios — depósito y consists.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command};
use crate::test_fixtures::SandboxMap;
use crate::{GameState, TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder};

#[test]
fn set_depot_vehicles_running_toggles_all_in_tile() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(3, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, depot, depot));
    s.vehicles[0].running = true;
    s.vehicles
        .push(Vehicle::new(2, VehicleKind::Truck, depot, depot));
    s.vehicles[1].running = true;
    apply_command(
        &mut s,
        &Command::SetDepotVehiclesRunning {
            depot_pos: depot,
            running: false,
        },
    )
    .unwrap();
    assert!(!s.vehicles[0].running);
    assert!(!s.vehicles[1].running);
}

#[test]
fn sell_vehicle_requires_depot_tile() {
    let mut s = GameState::new(8, 8);
    let road = TileCoord::new(2, 2);
    s.map.set_kind(road, TileKind::Road).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Truck, road, road));
    assert_eq!(
        apply_command(&mut s, &Command::SellVehicle(1)),
        Err(CommandError::VehicleNotInDepot)
    );
}

#[test]
fn rail_depot_beside_x_line_connects_exit_tile() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = GameState::new(12, 12);
    // Línea recta en eje X (y=4) y depósito al sur con la boca hacia la vía (NW).
    for x in 2..=8_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(5, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    // La tesela de salida gana las curvas de empalme hacia la boca del depósito:
    // X (recta NE↔SW) + LOWER (SE↔SW) + RIGHT (NE↔SE) = 0x29.
    let exit = s.map.get(TileCoord::new(5, 4)).unwrap();
    assert_eq!(
        exit.m5 & 0x3F,
        0x29,
        "empalme esperado X|LOWER|RIGHT: m5={:#04x}",
        exit.m5
    );

    // Un tren en la línea puede llegar al depósito y salir de él.
    assert!(
        find_path(&s.map, TileCoord::new(2, 4), depot, PathNetwork::Rail).is_some(),
        "línea → depósito"
    );
    assert!(
        find_path(&s.map, depot, TileCoord::new(8, 4), PathNetwork::Rail).is_some(),
        "depósito → línea"
    );
}

#[test]
fn train_consist_attach_wagon_grows_capacity_and_length() {
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
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_WAGON_PASSENGER),
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
    let head_v = s.vehicles.iter().find(|v| v.id == head).unwrap();
    assert_eq!(head_v.next_unit, Some(wagon));
    assert!(head_v.cached_total_length >= 16);
    assert!(head_v.capacity >= 40);
    assert_eq!(
        crate::consist_unit_ids(&s.vehicles, head),
        vec![head, wagon]
    );
    let tiles = crate::consist_occupied_tiles(&s.vehicles, head);
    assert!(!tiles.is_empty());
    assert_eq!(tiles[0], head_v.pos);
}

#[test]
fn train_consist_sell_head_sells_chain() {
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
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_WAGON_COAL),
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
    apply_command(&mut s, &Command::SellVehicle(head)).unwrap();
    assert!(s.vehicles.is_empty());
}

#[test]
fn two_trains_leave_same_rail_depot_sequentially() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    for x in 2..=8_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(5, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let id1 = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let id2 = s.vehicles[1].id;

    let orders = vec![TileCoord::new(8, 4)];
    apply_command(&mut s, &Command::SetVehicleOrders(id1, orders.clone())).unwrap();
    apply_command(&mut s, &Command::SetVehicleOrders(id2, orders)).unwrap();
    apply_command(&mut s, &Command::ToggleVehicleRunning(id1)).unwrap();
    apply_command(&mut s, &Command::ToggleVehicleRunning(id2)).unwrap();

    let mut saw_exclusive = false;
    let mut both_left = false;
    for _ in 0..20_000 {
        s.step();
        let v1 = s.vehicles.iter().find(|v| v.id == id1).unwrap();
        let v2 = s.vehicles.iter().find(|v| v.id == id2).unwrap();
        if (v1.depot_leave_cleared || v1.pos != depot) && v2.pos == depot && !v2.depot_leave_cleared
        {
            saw_exclusive = true;
        }
        if v1.pos != depot && v2.pos != depot {
            both_left = true;
            break;
        }
    }
    assert!(
        saw_exclusive,
        "la reserva de depósito debe serializar la salida"
    );
    assert!(
        both_left,
        "ambos trenes deben acabar saliendo del depósito compartido"
    );
}

#[test]
fn build_vehicle_at_rail_depot_creates_train_with_engine() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    let money_before = s.economy.money;
    let engine = crate::engine_by_id(crate::engine::ENGINE_TRAIN_GINZU_A4).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].kind, VehicleKind::Train);
    assert_eq!(
        s.vehicles[0].engine_id,
        Some(crate::engine::ENGINE_TRAIN_GINZU_A4)
    );
    assert!(!s.vehicles[0].running);
    assert_eq!(s.vehicles[0].direction, crate::DIR_NE);
    assert_eq!(s.economy.money, money_before - engine.price);
}

#[test]
fn build_vehicle_at_depot_rejects_insufficient_funds() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    s.economy.money = 10;
    let e = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap_err();
    assert_eq!(e, CommandError::InsufficientFunds);
    assert!(s.vehicles.is_empty());
    assert_eq!(s.economy.money, 10, "sin cobro al fallar");
}

#[test]
fn build_vehicle_at_depot_charges_model_price() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    let money_before = s.economy.money;
    let engine = crate::engine_by_id(crate::engine::ENGINE_BUS_FOSTER).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_FOSTER),
    )
    .unwrap();
    assert_eq!(s.economy.money, money_before - engine.price);
    assert_eq!(s.vehicles[0].capacity, engine.capacity);
}

#[test]
fn build_vehicle_at_depot_rejects_unknown_engine() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    let e = apply_command(&mut s, &Command::BuildVehicleAtDepot(depot, 9_999)).unwrap_err();
    assert_eq!(e, CommandError::EngineNotFound);
}

#[test]
fn build_vehicle_at_depot_buys_newgrf_train_from_catalog() {
    use crate::engine::{EngineDef, NEWGRF_ENGINE_ID_BASE};

    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(2, 3))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    let id = NEWGRF_ENGINE_ID_BASE;
    s.engine_catalog.push(EngineDef {
        id,
        kind: VehicleKind::Train,
        name: "NewGRF Express".into(),
        max_speed: 140,
        price: 50_000,
        running_cost_year: 2_000,
        capacity: 0,
        cargo: None,
        power_hp: 2_000,
        weight_t: 90,
        intro_year: 1960,
        reliability_pct: 85,
        train_image_index: 2,
        dual_headed: false,
        rail_tilts: false,
        curve_speed_mod: 0,
        from_newgrf: true,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
    });
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::BuildVehicleAtDepot(depot, id)).unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].engine_id, Some(id));
    assert_eq!(s.economy.money, money_before - 50_000);
}

#[test]
fn build_manley_morel_creates_dual_head_pair() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_MANLEY_MOREL),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 2);
    let head = &s.vehicles[0];
    let rear = &s.vehicles[1];
    assert_eq!(head.next_unit, Some(rear.id));
    assert_eq!(rear.prev_unit, Some(head.id));
    assert_eq!(head.other_multiheaded_part, Some(rear.id));
    assert_eq!(rear.other_multiheaded_part, Some(head.id));
    assert_eq!(head.cached_total_length, 16);
    // Potencia total = valor del motor (cada cabina aporta la mitad).
    assert_eq!(head.cached_power_hp, 600);
    assert_eq!(head.capacity, 76);
}

#[test]
fn build_wagon_then_attach_updates_consist() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let head = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_WAGON_PASSENGER),
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
    let h = s.vehicles.iter().find(|v| v.id == head).unwrap();
    assert_eq!(h.next_unit, Some(wagon));
    assert!(h.capacity >= 40);
    assert_eq!(h.cached_total_length, 16);

    apply_command(&mut s, &Command::DetachConsistUnit(wagon)).unwrap();
    let h = s.vehicles.iter().find(|v| v.id == head).unwrap();
    assert!(h.next_unit.is_none());
    let w = s.vehicles.iter().find(|v| v.id == wagon).unwrap();
    assert!(w.prev_unit.is_none());
    assert!(w.is_consist_head());
}

#[test]
fn move_rail_vehicle_transfers_wagon_between_consists() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let head_a = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let head_b = s.vehicles.iter().find(|v| v.id != head_a).unwrap().id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_WAGON_PASSENGER),
    )
    .unwrap();
    let wagon = s
        .vehicles
        .iter()
        .find(|v| v.id != head_a && v.id != head_b)
        .unwrap()
        .id;
    apply_command(
        &mut s,
        &Command::AttachWagonToConsist {
            head_id: head_a,
            wagon_id: wagon,
        },
    )
    .unwrap();
    assert_eq!(
        s.vehicles
            .iter()
            .find(|v| v.id == head_a)
            .unwrap()
            .next_unit,
        Some(wagon)
    );

    apply_command(
        &mut s,
        &Command::MoveRailVehicle {
            head_id: head_b,
            unit_id: wagon,
            after_id: None,
            move_chain: false,
        },
    )
    .unwrap();
    assert!(
        s.vehicles
            .iter()
            .find(|v| v.id == head_a)
            .unwrap()
            .next_unit
            .is_none()
    );
    assert_eq!(
        s.vehicles
            .iter()
            .find(|v| v.id == head_b)
            .unwrap()
            .next_unit,
        Some(wagon)
    );
    let w = s.vehicles.iter().find(|v| v.id == wagon).unwrap();
    assert_eq!(w.prev_unit, Some(head_b));
}

#[test]
fn move_rail_vehicle_chain_keeps_tail_wagons() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let head_a = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let head_b = s.vehicles.iter().find(|v| v.id != head_a).unwrap().id;
    for _ in 0..2 {
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_WAGON_PASSENGER),
        )
        .unwrap();
    }
    let wagons: Vec<u32> = s
        .vehicles
        .iter()
        .filter(|v| v.id != head_a && v.id != head_b)
        .map(|v| v.id)
        .collect();
    assert_eq!(wagons.len(), 2);
    let (w1, w2) = (wagons[0], wagons[1]);
    apply_command(
        &mut s,
        &Command::AttachWagonToConsist {
            head_id: head_a,
            wagon_id: w1,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::AttachWagonToConsist {
            head_id: head_a,
            wagon_id: w2,
        },
    )
    .unwrap();
    assert_eq!(
        crate::consist_unit_ids(&s.vehicles, head_a),
        vec![head_a, w1, w2]
    );

    apply_command(
        &mut s,
        &Command::MoveRailVehicle {
            head_id: head_b,
            unit_id: w1,
            after_id: None,
            move_chain: true,
        },
    )
    .unwrap();
    assert_eq!(crate::consist_unit_ids(&s.vehicles, head_a), vec![head_a]);
    assert_eq!(
        crate::consist_unit_ids(&s.vehicles, head_b),
        vec![head_b, w1, w2]
    );
}

#[test]
fn clone_vehicle_at_depot_copies_engine_and_orders() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_FOSTER),
    )
    .unwrap();
    let source_id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(source_id, vec![VehicleOrder::tile(TileCoord::new(3, 3))]),
    )
    .unwrap();
    let money_before = s.economy.money;
    apply_command(
        &mut s,
        &Command::CloneVehicleAtDepot {
            source_vehicle_id: source_id,
            depot_pos: depot,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 2);
    assert_eq!(
        s.vehicles[1].engine_id,
        Some(crate::engine::ENGINE_BUS_FOSTER)
    );
    assert_eq!(s.vehicles[1].orders, s.vehicles[0].orders);
    assert!(s.economy.money < money_before);
}

#[test]
fn sell_all_vehicles_at_depot_empties_depot() {
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
    assert_eq!(s.vehicles.len(), 2);
    apply_command(&mut s, &Command::SellAllVehiclesAtDepot(depot)).unwrap();
    assert!(s.vehicles.is_empty());
}

#[test]
fn depot_reorder_vehicle_slot_updates_display_order() {
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
    apply_command(
        &mut s,
        &Command::DepotReorderVehicleSlot {
            depot_pos: depot,
            from_slot: 0,
            to_slot: 1,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].depot_display_slot, Some(1));
    assert_eq!(s.vehicles[1].depot_display_slot, Some(0));
}

#[test]
fn build_vehicle_at_depot_rejects_other_company_depot() {
    use crate::test_fixtures::SandboxMap;

    let mut s = SandboxMap::flat_rich(12, 12, 1);
    s.ensure_rival_transcargo();

    // Compañía B crea un depósito.
    let rival = crate::company::CompanyId(1);
    assert!(s.set_active_company(rival));
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();

    // Verificar que el depósito pertenece a la compañía rival.
    let tile = s.map.get(depot).unwrap();
    let owner = crate::company::CompanyId::from_tile_m1(tile.m1, s.companies.len());
    assert_eq!(owner, rival);

    // Compañía A (jugador) intenta comprar un vehículo en el depósito de B.
    assert!(s.set_active_company(crate::company::CompanyId::PLAYER));
    let result = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_KIRBY),
    );
    assert_eq!(result, Err(CommandError::TileNotOwned));
    assert!(s.vehicles.is_empty(), "no debe crear vehículo");
}

#[test]
fn build_vehicle_at_depot_allows_own_depot() {
    use crate::test_fixtures::SandboxMap;

    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();

    // Verificar que el depósito pertenece al jugador.
    let tile = s.map.get(depot).unwrap();
    let owner = crate::company::CompanyId::from_tile_m1(tile.m1, s.companies.len());
    assert_eq!(owner, crate::company::CompanyId::PLAYER);

    // El jugador puede comprar en su propio depósito.
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_KIRBY),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].owner, crate::company::CompanyId::PLAYER);
}

#[test]
fn build_road_vehicle_at_depot_rejects_other_company_depot() {
    use crate::test_fixtures::SandboxMap;

    let mut s = SandboxMap::flat_rich(12, 12, 1);
    s.ensure_rival_transcargo();

    // Compañía B crea un depósito de carretera.
    let rival = crate::company::CompanyId(1);
    assert!(s.set_active_company(rival));
    let depot = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();

    // Compañía A (jugador) intenta comprar un vehículo en el depósito de B.
    assert!(s.set_active_company(crate::company::CompanyId::PLAYER));
    let result = apply_command(
        &mut s,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
    );
    assert_eq!(result, Err(CommandError::TileNotOwned));
    assert!(s.vehicles.is_empty(), "no debe crear vehículo");
}
