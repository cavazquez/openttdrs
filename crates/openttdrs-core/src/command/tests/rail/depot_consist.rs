//! Tests de comandos ferroviarios — depósito y consists.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command};
use crate::map::{RAIL_TB_LOWER, RAIL_TB_RIGHT};
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
fn rail_depot_keeps_prebuilt_exit_junction_unchanged() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = GameState::new(12, 12);
    // Línea recta en eje X (y=4) y depósito al sur con la boca hacia la vía (NW).
    for x in 2..=8_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(5, 5);
    let exit_pos = TileCoord::new(5, 4);
    // En OpenTTD el empalme se construye explícitamente antes o después del depósito.
    apply_command(
        &mut s,
        &Command::PlaceRailBits(exit_pos, RAIL_TB_LOWER | RAIL_TB_RIGHT),
    )
    .unwrap();
    let exit_before = s.map.get(exit_pos).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    // X (recta NE↔SW) + LOWER (SE↔SW) + RIGHT (NE↔SE) = 0x29.
    let exit = s.map.get(exit_pos).unwrap();
    assert_eq!(
        exit, exit_before,
        "el depósito no debe reescribir la salida"
    );
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
fn rail_depot_connects_exit_without_touching_parallel_neighbors() {
    let mut s = GameState::new(12, 12);
    for y in [4, 5] {
        for x in 2..=8_i32 {
            apply_command(
                &mut s,
                &Command::PlaceRailBits(TileCoord::new(x, y), crate::map::RAIL_TB_X),
            )
            .unwrap();
        }
    }

    let depot = TileCoord::new(5, 6);
    let before: Vec<_> = {
        let map = &s.map;
        (3..=7)
            .flat_map(|y| {
                (1..=9).map(move |x| {
                    let pos = TileCoord::new(x, y);
                    (pos, map.get(pos))
                })
            })
            .collect()
    };
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    for (pos, tile_before) in before {
        if pos == depot {
            continue;
        }
        if pos == TileCoord::new(5, 5) {
            let exit = s.map.get(pos).unwrap();
            assert_eq!(exit.m5 & 0x3F, 0x29, "empalme automático de salida");
            continue;
        }
        assert_eq!(
            s.map.get(pos),
            tile_before,
            "el depósito modificó una tesela vecina en {pos:?}"
        );
    }
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
fn attach_newgrf_wagon_refreshes_callback_consist_cache() {
    use crate::engine::engines_table;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };

    let mut state = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    state.map.set_kind(depot, TileKind::RailDepot).unwrap();

    let mut runtime = TrainSpriteGraphics::default();
    runtime.assigns.push(TrainSpriteAssign {
        local_id: 0,
        set_id: 2,
    });
    runtime.action2_var.insert(
        2,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x1A,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: 77,
                    ..Action2VarAdjust::default()
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        },
    );
    let mut wagon_engine = engines_table()
        .iter()
        .find(|candidate| candidate.is_wagon())
        .cloned()
        .unwrap();
    wagon_engine.id = 65_105;
    wagon_engine.newgrf_grfid = 0x5741_474E;
    wagon_engine.newgrf_local_id = 0;
    wagon_engine.newgrf_runtime = Some(Box::new(runtime));
    state.engine_catalog.push(wagon_engine.clone());

    let head_id = 1;
    let wagon_id = 2;
    let mut head = Vehicle::new(head_id, VehicleKind::Train, depot, depot);
    head.engine_id = Some(crate::engine::ENGINE_TRAIN_KIRBY);
    let mut wagon = Vehicle::new(wagon_id, VehicleKind::Train, depot, depot);
    wagon.engine_id = Some(wagon_engine.id);
    state.vehicles.extend([head, wagon]);

    apply_command(
        &mut state,
        &Command::AttachWagonToConsist { head_id, wagon_id },
    )
    .unwrap();

    let head = state.vehicles.iter().find(|v| v.id == head_id).unwrap();
    assert_eq!(head.next_unit, Some(wagon_id));
    assert_eq!(head.capacity, 77);
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
        reliability_spd_dec: crate::engine::DEFAULT_RELIABILITY_SPD_DEC,
        lifelength_years: 30,
        model_life_years: u8::MAX,
        load_amount: 0,
        train_image_index: 2,
        dual_headed: false,
        rail_engine_class: 0,
        rail_is_mu: false,
        uses_2cc: false,
        rail_tilts: false,
        curve_speed_mod: 0,
        pow_wag_power: 0,
        pow_wag_weight: 0,
        from_newgrf: true,
        tractive_effort: 0,
        air_drag: 0,
        shorten_factor: 0,
        required_rail_type: None,
        refit_mask: 0,
        ctt_include_cargos: Vec::new(),
        ctt_exclude_cargos: Vec::new(),
        cargo_classes_allowed: 0,
        cargo_classes_disallowed: 0,
        cargo_classes_required: 0,
        cargo_classes_specified: false,
        is_helicopter: false,
        is_large_aircraft: false,
        sprite_stack: false,
        ocean_speed_frac: 0,
        canal_speed_frac: 0,
        sound_effect: 0,
        visual_effect: crate::engine::VEHICLE_VISUAL_EFFECT_DEFAULT,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
        vehicle_callback_mask: 0,
        badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
    });
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::BuildVehicleAtDepot(depot, id)).unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].engine_id, Some(id));
    assert_eq!(s.economy.money, money_before - 50_000);
}

#[test]
#[allow(clippy::too_many_lines)]
fn build_newgrf_train_materializes_articulated_parts_from_callback() {
    use crate::engine::{EngineDef, NEWGRF_ENGINE_ID_BASE};
    use crate::newgrf_config::NewGrfEntry;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm, TrainSpriteAssign,
        TrainSpriteGraphics,
    };

    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(2, 3))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();

    let grfid = 0x4152_5443;
    let front_id = NEWGRF_ENGINE_ID_BASE + 20;
    let part_id = front_id + 1;
    let mut callback = TrainSpriteGraphics::default();
    callback.assigns.push(TrainSpriteAssign {
        local_id: 0,
        set_id: 2,
    });
    let literal = |value: u8| Action2VarTerm {
        variable: 0x1A,
        param: None,
        adjust: Action2VarAdjust {
            and_mask: u32::from(value),
            ..Action2VarAdjust::default()
        },
    };
    // The first node selects a value by callback index.  Index 1 branches to
    // a callback value of 1 (the articulated wagon); subsequent indexes branch
    // to 0x7FFF, the modern terminator.
    callback.action2_var.insert(
        2,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x10,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: u32::from(u8::MAX),
                    ..Action2VarAdjust::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(3, 1, 1)],
            default: 4,
        },
    );
    callback.action2_var.insert(
        3,
        Action2VarEntry {
            first: literal(1),
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        },
    );
    callback.action2_var.insert(
        4,
        Action2VarEntry {
            first: literal(0xFF),
            ops: vec![
                Action2VarOp {
                    operator: 0x0A,
                    rhs: literal(0x80),
                },
                Action2VarOp {
                    operator: 0x00,
                    rhs: literal(0x7F),
                },
            ],
            ranges: Vec::new(),
            default: 0,
        },
    );

    let mut front: EngineDef = crate::engine::engine_by_id(crate::engine::ENGINE_TRAIN_KIRBY)
        .unwrap()
        .clone();
    front.id = front_id;
    front.name = "Articulated front".into();
    front.price = 50_000;
    front.from_newgrf = true;
    front.newgrf_local_id = 0;
    front.newgrf_grfid = grfid;
    front.vehicle_callback_mask = 1 << 4;
    front.newgrf_runtime = Some(Box::new(callback));

    let mut part: EngineDef = crate::engine::engine_by_id(crate::engine::ENGINE_WAGON_PASSENGER)
        .unwrap()
        .clone();
    part.id = part_id;
    part.name = "Articulated passenger module".into();
    part.price = 0;
    part.from_newgrf = true;
    part.newgrf_local_id = 1;
    part.newgrf_grfid = grfid;
    s.engine_catalog.push(front);
    s.engine_catalog.push(part);
    s.newgrf_stack.push(NewGrfEntry {
        filename: "articulated-test.grf".into(),
        grfid,
        name: "Articulated test".into(),
        description: String::new(),
        grf_version: 8,
        enabled: true,
        is_static: false,
        params: Vec::new(),
    });

    apply_command(&mut s, &Command::BuildVehicleAtDepot(depot, front_id)).unwrap();

    assert_eq!(s.vehicles.len(), 2);
    let head = s
        .vehicles
        .iter()
        .find(|v| v.engine_id == Some(front_id))
        .unwrap();
    let part = s
        .vehicles
        .iter()
        .find(|v| v.engine_id == Some(part_id))
        .unwrap();
    assert_eq!(head.next_unit, Some(part.id));
    assert_eq!(part.prev_unit, Some(head.id));
    assert_eq!(part.capacity, 40);
    assert_eq!(head.capacity, 40);
    assert!(head.cached_total_length >= 16);
}

#[test]
#[allow(clippy::too_many_lines)]
fn build_newgrf_road_vehicle_materializes_articulated_parts_from_callback() {
    use crate::engine::{ENGINE_BUS_FOSTER, ENGINE_BUS_MPS, EngineDef, NEWGRF_ENGINE_ID_BASE};
    use crate::newgrf_config::NewGrfEntry;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm, TrainSpriteAssign,
        TrainSpriteGraphics,
    };

    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(2, 3))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();

    let grfid = 0x4152_5444;
    let front_id = NEWGRF_ENGINE_ID_BASE + 30;
    let part_id = front_id + 1;
    let mut callback = TrainSpriteGraphics::default();
    callback.assigns.push(TrainSpriteAssign {
        local_id: 0,
        set_id: 2,
    });
    let literal = |value: u8| Action2VarTerm {
        variable: 0x1A,
        param: None,
        adjust: Action2VarAdjust {
            and_mask: u32::from(value),
            ..Action2VarAdjust::default()
        },
    };
    callback.action2_var.insert(
        2,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x10,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: u32::from(u8::MAX),
                    ..Action2VarAdjust::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(4, 1, 1)],
            default: u16::from(u8::MAX),
        },
    );
    callback.action2_var.insert(
        3,
        Action2VarEntry {
            first: literal(1),
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        },
    );
    callback.action2_var.insert(
        4,
        Action2VarEntry {
            first: literal(0x80),
            ops: vec![
                Action2VarOp {
                    operator: 0x0A,
                    rhs: literal(0x80),
                },
                Action2VarOp {
                    operator: 0x00,
                    rhs: literal(0x01),
                },
            ],
            ranges: Vec::new(),
            default: 0,
        },
    );

    let mut front: EngineDef = crate::engine::engine_by_id(ENGINE_BUS_MPS).unwrap().clone();
    front.id = front_id;
    front.name = "Articulated road front".into();
    front.price = 50_000;
    front.from_newgrf = true;
    front.newgrf_local_id = 0;
    front.newgrf_grfid = grfid;
    front.vehicle_callback_mask = 1 << 4;
    front.newgrf_runtime = Some(Box::new(callback));

    let mut part: EngineDef = crate::engine::engine_by_id(ENGINE_BUS_FOSTER)
        .unwrap()
        .clone();
    part.id = part_id;
    part.name = "Articulated road module".into();
    part.price = 0;
    part.from_newgrf = true;
    part.newgrf_local_id = 1;
    part.newgrf_grfid = grfid;
    s.engine_catalog.push(front);
    s.engine_catalog.push(part);
    s.newgrf_stack.push(NewGrfEntry {
        filename: "articulated-road-test.grf".into(),
        grfid,
        name: "Articulated road test".into(),
        description: String::new(),
        grf_version: 8,
        enabled: true,
        is_static: false,
        params: Vec::new(),
    });

    apply_command(&mut s, &Command::BuildVehicleAtDepot(depot, front_id)).unwrap();

    assert_eq!(s.vehicles.len(), 2);
    let head = s
        .vehicles
        .iter()
        .find(|v| v.engine_id == Some(front_id))
        .unwrap();
    let part = s
        .vehicles
        .iter()
        .find(|v| v.engine_id == Some(part_id))
        .unwrap();
    assert_eq!(head.kind, VehicleKind::Bus);
    assert_eq!(part.kind, VehicleKind::Bus);
    assert_eq!(head.next_unit, Some(part.id));
    assert_eq!(part.prev_unit, Some(head.id));
    assert!(part.newgrf_articulated);
    assert!(part.newgrf_mirrored);
    assert!(!part.is_consist_head());
    assert_eq!(
        part.road_depot_phase,
        crate::vehicle::RoadDepotPhase::InDepot
    );
    assert_eq!(part.road_state, crate::road_movement::RVSB_IN_DEPOT);
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
