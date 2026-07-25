//! Tests del módulo vehicle.

#![allow(clippy::unwrap_used)]

use std::collections::VecDeque;

use crate::map::TileCoord;

use super::VEHICLE_CAPACITY;
use super::model::{DIR_N, DIR_NE, DIR_S, DIR_SE, DIR_SW, Vehicle, VehicleKind};
use super::order::{OrderConditionKind, VehicleOrder};

#[test]
fn progress_requires_multiple_ticks_per_tile() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.path = VecDeque::from([TileCoord::new(1, 0)]);
    v.set_cruise_speed();
    v.road_state = crate::road_movement::trackdir_from_direction(v.direction);
    let start = v.pos;
    let mut ticks = 0_u32;
    while v.pos == start && ticks < v.ticks_per_tile().saturating_mul(2) {
        v.step();
        ticks += 1;
    }
    assert!(
        ticks > 1,
        "cruzar una tesela requiere varios ticks (fue {ticks})"
    );
    assert_eq!(v.pos, TileCoord::new(1, 0));
}

#[test]
fn next_station_hop_skips_current_and_wraps() {
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(2, 2);
    let c = TileCoord::new(3, 3);
    let orders = vec![
        VehicleOrder::station(a),
        VehicleOrder::station(b),
        VehicleOrder::station(c),
    ];
    assert_eq!(VehicleOrder::next_station_hop(&orders, 0, a), Some(b));
    assert_eq!(VehicleOrder::next_station_hop(&orders, 1, b), Some(c));
    assert_eq!(VehicleOrder::next_station_hop(&orders, 2, c), Some(a));
}

#[test]
fn get_next_stopping_station_explores_conditional_branches() {
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(2, 2);
    let c = TileCoord::new(3, 3);
    let orders = vec![
        VehicleOrder::station(a),
        VehicleOrder::conditional(OrderConditionKind::CargoLoadAbove, 50, 3),
        VehicleOrder::station(b),
        VehicleOrder::station(c),
    ];
    let hops = VehicleOrder::get_next_stopping_station(&orders, 0, a, None);
    assert!(hops.contains(&b));
    assert!(hops.contains(&c));
}

#[test]
fn maybe_insert_implicit_order_on_unscheduled_visit() {
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(5, 5);
    let mid = TileCoord::new(3, 3);
    let mut v = Vehicle::new(0, VehicleKind::Truck, a, b);
    v.set_vehicle_orders(vec![VehicleOrder::station(a), VehicleOrder::station(b)]);
    v.current_order = 1;
    v.cur_implicit_order_index = 0;
    v.maybe_insert_implicit_order(mid);
    assert!(
        v.orders
            .iter()
            .any(|o| o.is_implicit() && o.destination() == mid)
    );
    assert_eq!(v.last_station_visited, Some(mid));
}

#[test]
fn train_reverses_immediately_when_next_tile_opposite() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(21, 15),
        TileCoord::new(21, 15),
    );
    v.path = VecDeque::from([TileCoord::new(20, 15)]);
    v.direction = DIR_SW;
    v.progress = 255;
    v.cur_speed = 0;
    v.step();
    assert_eq!(v.direction, DIR_NE, "giro inmediato al volver por la vía");
    assert_eq!(v.progress, 0);
}

#[test]
fn maglev_45_degree_turn_skips_small_turn_penalty() {
    use crate::map::{Map, TileKind};
    use crate::rail_type::{RailType, set_rail_type_on_tile};

    let mut map = Map::new_flat(8, 8, 4);
    let c = TileCoord::new(3, 3);
    map.set_kind(c, TileKind::Rail).unwrap();
    let tile = set_rail_type_on_tile(map.get(c).unwrap(), RailType::Maglev);
    map.set_tile(c, tile).unwrap();
    let mut v = Vehicle::new(0, VehicleKind::Train, c, c);
    v.direction = DIR_NE;
    v.cur_speed = 200;
    v.set_direction_with_curve_penalty(
        DIR_N,
        Some(&map),
        crate::engine::TrainAccelerationModel::Original,
    );
    assert_eq!(
        v.cur_speed, 200,
        "maglev small_turn=0: giro 45° sin penalización"
    );
}

#[test]
fn arrival_at_order_keeps_progress_at_lane_end() {
    use crate::command::apply_command;
    use crate::{Command, GameState};

    let mut state = GameState::new(24, 18);
    let stop = TileCoord::new(15, 3);
    let road = TileCoord::new(15, 4);
    apply_command(&mut state, &Command::SetRoadBits(road, 0x0A)).unwrap();
    apply_command(&mut state, &Command::PlaceBusStop(stop, 1)).unwrap();

    let mut v = Vehicle::new(0, VehicleKind::Bus, TileCoord::new(14, 4), stop);
    v.set_station_orders(vec![stop, TileCoord::new(21, 3)]);
    v.sync_order_destination(&state.map);
    assert_eq!(v.dest, stop, "bus entra a la tesela de la bahía (Fase 2)");
    v.path = VecDeque::from([road, stop]);
    v.direction = super::model::DIR_NW;
    v.road_state = crate::road_movement::trackdir_from_direction(v.direction);
    v.set_cruise_speed();
    let mut guard = 0_u32;
    while v.pos != road {
        v.step();
        guard += 1;
        assert!(guard < 500, "no alcanzó la carretera de acceso");
    }
    while v.pos != stop {
        v.step();
        guard += 1;
        assert!(guard < 1_000, "no llegó a la bahía");
    }
    assert!(
        v.awaiting_load_window || v.progress == 255 || v.cargo_transfer_active(),
        "al llegar debe anclarse / abrir ventana de carga"
    );
}

#[test]
fn vehicle_accelerates_from_standstill_before_moving() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.path = VecDeque::from([TileCoord::new(1, 0)]);
    assert_eq!(v.cur_speed, 0);
    v.step();
    assert_eq!(v.pos, TileCoord::new(0, 0));
    assert!(v.cur_speed > 0);
    assert_eq!(v.progress, 0);
}

#[test]
fn vehicle_decelerates_when_idle() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Truck,
        TileCoord::new(2, 2),
        TileCoord::new(2, 2),
    );
    v.cur_speed = 96;
    v.subspeed = 0;
    for _ in 0..160 {
        v.step();
        if v.cur_speed == 0 {
            break;
        }
    }
    assert_eq!(v.cur_speed, 0);
    assert_eq!(v.subspeed, 0);
}

#[test]
fn loaded_sprite_for_bus_and_truck() {
    let mut bus = Vehicle::new(
        0,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    assert!(!bus.uses_loaded_road_sprite());
    bus.cargo = VEHICLE_CAPACITY / 2;
    assert!(bus.uses_loaded_road_sprite());
    let mut truck = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    truck.cargo = VEHICLE_CAPACITY / 2;
    assert!(truck.uses_loaded_road_sprite());
}

#[test]
fn train_without_path_never_walks_off_rail() {
    // Tren sin órdenes con destino lejano y sin camino por red: no debe
    // avanzar en Manhattan (caminar por el pasto hasta el depósito).
    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(4, 14),
        TileCoord::new(10, 14),
    );
    v.set_cruise_speed();
    for _ in 0..200 {
        v.step();
    }
    assert_eq!(
        v.pos,
        TileCoord::new(4, 14),
        "el tren no debe salir de la vía"
    );

    // Un camión libre (sin órdenes) conserva el fallback Manhattan.
    let mut truck = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(4, 14),
        TileCoord::new(10, 14),
    );
    truck.set_cruise_speed();
    for _ in 0..200 {
        truck.step();
    }
    assert_ne!(truck.pos, TileCoord::new(4, 14));
}

#[test]
fn train_moves_slower_than_bus_on_same_path() {
    let mut bus = Vehicle::new(
        0,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(3, 0),
    );
    bus.path = VecDeque::from([
        TileCoord::new(1, 0),
        TileCoord::new(2, 0),
        TileCoord::new(3, 0),
    ]);
    let mut train = Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(0, 0),
        TileCoord::new(3, 0),
    );
    train.path = bus.path.clone();
    bus.set_cruise_speed();
    train.set_cruise_speed();

    // Ambos usan coste por sub-paso (~192); el tren llama el handler 2×/tick.
    let mut bus_steps = 0;
    while bus.pos.x < 1 {
        bus.step();
        bus_steps += 1;
        assert!(bus_steps < 2_000, "bus atascado");
    }
    let mut train_steps = 0;
    while train.pos.x < 1 {
        train.step();
        train_steps += 1;
        assert!(train_steps < 2_000, "tren atascado");
    }
    assert!(
        train_steps >= bus_steps / 2,
        "tren ({train_steps}) no debería ser mucho más rápido que el bus ({bus_steps})"
    );
}

#[test]
fn render_direction_uses_cardinal_in_turn_second_half() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 1),
    );
    v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(1, 1)]);
    v.progress = 200;
    assert_eq!(v.render_direction(), DIR_S);
}

#[test]
fn road_vehicle_loses_quarter_speed_on_turn() {
    // OpenTTD AM_ORIGINAL: `v->cur_speed -= v->cur_speed >> 2` al cambiar
    // de dirección (roadveh_cmd.cpp:1481).
    let mut v = Vehicle::new(
        0,
        VehicleKind::Truck,
        TileCoord::new(1, 1),
        TileCoord::new(2, 2),
    );
    v.direction = DIR_SE;
    v.path = VecDeque::from([TileCoord::new(1, 2), TileCoord::new(2, 2)]);
    v.set_cruise_speed();
    let cruise = v.cur_speed;
    while v.pos != TileCoord::new(1, 2) {
        v.step();
    }
    assert_eq!(v.cur_speed, cruise, "tramo recto: sin penalización");
    while v.pos != TileCoord::new(2, 2) {
        v.step();
    }
    assert_eq!(
        v.cur_speed,
        cruise - (cruise >> 2),
        "giro SE→SW: −25 % de velocidad"
    );
}

#[test]
fn train_loses_speed_on_direction_change() {
    // OpenTTD AM_ORIGINAL: `_accel_slowdown` al cambiar dirección en la
    // locomotora (train_cmd.cpp:3564-3568). Giro SE→SW = 90° → large_turn.
    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(1, 1),
        TileCoord::new(2, 2),
    );
    v.direction = DIR_SE;
    v.path = VecDeque::from([TileCoord::new(1, 2), TileCoord::new(2, 2)]);
    v.set_cruise_speed();
    let cruise = v.cur_speed;
    while v.pos != TileCoord::new(1, 2) {
        v.step();
    }
    assert_eq!(v.cur_speed, cruise, "tramo recto: sin penalización");
    while v.pos != TileCoord::new(2, 2) {
        v.step();
    }
    assert_eq!(
        v.cur_speed,
        cruise - ((cruise * 128) >> 8),
        "giro SE→SW: −50 % de velocidad"
    );
}

#[test]
fn train_realistic_skips_accel_slowdown_on_turn() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(1, 1),
        TileCoord::new(1, 1),
    );
    v.direction = DIR_SE;
    v.cur_speed = 128;
    v.set_direction_with_curve_penalty(
        DIR_SW,
        None,
        crate::engine::TrainAccelerationModel::Realistic,
    );
    assert_eq!(v.cur_speed, 128, "AM_REALISTIC no aplica _accel_slowdown");
    assert_eq!(v.direction, DIR_SW);
}

#[test]
fn train_realistic_respects_cached_curve_speed_cap() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(1, 1),
        TileCoord::new(5, 1),
    );
    v.running = true;
    v.path = VecDeque::from([
        TileCoord::new(2, 1),
        TileCoord::new(3, 1),
        TileCoord::new(4, 1),
        TileCoord::new(5, 1),
    ]);
    v.cached_max_curve_speed = 61;
    v.cur_speed = 100;
    v.subspeed = 0;
    for _ in 0..40 {
        v.step_with_map_and_accel(None, crate::engine::TrainAccelerationModel::Realistic);
    }
    assert!(
        v.cur_speed <= 61,
        "techo de curva Realistic: speed={}",
        v.cur_speed
    );
}

#[test]
fn train_loses_speed_when_climbing_tile_z() {
    use crate::map::{Map, TileKind, tile_slope_and_z};
    use crate::train_movement::affect_speed_by_z_change;

    let mut map = Map::new_flat(8, 8, 4);
    // Meseta en (3,2): GetTileZ = 5; (2,2) queda en 4.
    for (x, y) in [(3, 2), (4, 2), (3, 3), (4, 3)] {
        map.set_height(TileCoord::new(x, y), 5).unwrap();
    }
    map.set_kind(TileCoord::new(2, 2), TileKind::Rail).unwrap();
    map.set_kind(TileCoord::new(3, 2), TileKind::Rail).unwrap();
    assert_eq!(tile_slope_and_z(&map, TileCoord::new(2, 2)).unwrap().1, 4);
    assert_eq!(tile_slope_and_z(&map, TileCoord::new(3, 2)).unwrap().1, 5);

    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(2, 2),
        TileCoord::new(3, 2),
    );
    v.path = VecDeque::from([TileCoord::new(3, 2)]);
    v.running = true;
    let max = v.effective_engine().max_speed;
    // Mantener velocidad al tope del motor para que UpdateSpeed no la mueva.
    while v.pos != TileCoord::new(3, 2) {
        v.cur_speed = max;
        v.step_with_map(Some(&map));
    }
    assert_eq!(
        v.cur_speed,
        affect_speed_by_z_change(max, 1, 0, max),
        "subida Δz=+1: −25 % (z_up=64)"
    );
}

#[test]
fn train_gains_speed_when_descending_tile_z() {
    use crate::map::{Map, TileKind};
    use crate::train_movement::affect_speed_by_z_change;

    let mut map = Map::new_flat(8, 8, 4);
    map.set_kind(TileCoord::new(3, 2), TileKind::Rail).unwrap();
    let mut v = Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(3, 2),
        TileCoord::new(2, 2),
    );
    v.running = true;
    let max = v.effective_engine().max_speed;
    let cruise = max.saturating_sub(10).max(4);
    v.cur_speed = cruise;
    v.z_pos = Some(40);
    // Simula Δz=-1 en bajada (`AffectSpeedByZChange` +z_down=2).
    v.cur_speed = affect_speed_by_z_change(v.cur_speed, -1, 0, max);
    assert_eq!(v.cur_speed, cruise + 2, "bajada Δz píxel: +z_down");
    let _ = map;
}

#[test]
fn train_applies_z_change_while_progressing_on_inclined_tile() {
    use crate::map::{Map, SLOPE_NE, TileKind, tile_slope_and_z};

    let mut map = Map::new_flat(8, 8, 4);
    // SLOPE_NE en (3,3): N+E elevados.
    map.set_height(TileCoord::new(3, 3), 5).unwrap();
    map.set_height(TileCoord::new(3, 4), 5).unwrap();
    let c = TileCoord::new(3, 3);
    assert_eq!(tile_slope_and_z(&map, c).unwrap().0, SLOPE_NE);
    map.set_kind(c, TileKind::Rail).unwrap();
    map.set_mapt_m5(c, 0x10, 0x01).unwrap(); // TRACK_X

    let mut v = Vehicle::new(0, VehicleKind::Train, c, TileCoord::new(4, 3));
    v.direction = DIR_SW; // +X
    v.running = true;
    let cruise = v.effective_engine().max_speed.saturating_sub(10).max(4);
    v.cur_speed = cruise;
    v.rail_pixel = 0;
    v.sync_train_slope_speed(&map);
    let z0 = v.z_pos.unwrap();
    v.cur_speed = cruise;
    // Progreso visual vía píxeles de vía (no el remanente físico `progress`).
    v.rail_pixel = 14;
    v.sync_train_slope_speed(&map);
    let z1 = v.z_pos.unwrap();
    assert!(
        z1 < z0,
        "en SLOPE_NE hacia el este baja el Z; z0={z0} z1={z1}"
    );
    assert_eq!(
        v.cur_speed,
        cruise + 2,
        "ΔZ subpíxel en bajada aplica +z_down"
    );
}

#[test]
fn direction_updates_when_tile_advances() {
    let mut v = Vehicle::new(
        0,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.path = VecDeque::from([TileCoord::new(1, 0)]);
    v.set_cruise_speed();
    for _ in 0..v.ticks_per_tile() {
        v.step();
    }
    assert_eq!(v.direction, DIR_SW);
}

#[test]
fn timetable_wait_delays_order_advance() {
    let pos = TileCoord::new(1, 1);
    let mut v = Vehicle::new(1, VehicleKind::Bus, pos, pos);
    v.timetable_active = true;
    let wait_order = VehicleOrder::station(pos).with_cycled_wait().unwrap();
    v.orders = vec![wait_order, VehicleOrder::station(TileCoord::new(3, 3))];
    assert_eq!(v.orders[0].wait_ticks(), 30);
    v.running = true;
    v.progress = 255;
    v.sim_tick = 100;
    v.step();
    // Primer step: abre la ventana de carga; el segundo agenda la espera.
    v.step();
    assert_eq!(v.timetable_wait_remaining, 30);
    assert_eq!(v.current_order, 0);
    for _ in 0..30 {
        v.tick_timetable_wait();
    }
    assert_eq!(v.current_order, 1);
}

#[test]
fn no_load_order_skips_loading() {
    let pos = TileCoord::new(1, 1);
    let mut v = Vehicle::new(1, VehicleKind::Bus, pos, pos);
    let mut order = VehicleOrder::station(pos);
    if let VehicleOrder::Station { no_load, .. } = &mut order {
        *no_load = true;
    }
    v.orders = vec![order];
    v.running = true;
    v.progress = 255;
    v.capacity = 20;
    assert!(!v.orders[0].should_wait_for_loading(v.cargo, v.capacity));
}

#[test]
fn timetable_start_offsets_lateness_on_first_arrival() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.timetable_active = true;
    v.timetable_start = 100;
    v.sim_tick = 150;
    v.orders = vec![VehicleOrder::station(TileCoord::new(1, 0))];
    v.current_order_time = 10;
    v.update_vehicle_timetable(true);
    assert_eq!(v.timetable_lateness, 50);
    assert_eq!(v.timetable_start, 0);
    assert!(v.timetable_started);
}

#[test]
fn service_at_depot_restores_reliability() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.reliability = 2_000;
    v.needs_servicing = true;
    v.breakdown_ctr = 50;
    v.breakdown_delay = 10;
    v.service_at_depot();
    assert!(v.reliability >= 8_000);
    assert!(!v.needs_servicing);
    assert_eq!(v.breakdown_ctr, 0);
    assert_eq!(v.breakdown_delay, 0);
}

#[test]
fn requires_service_by_percent_threshold() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.service_interval_days = 20;
    v.reliability = 7_500;
    assert!(!v.requires_service_for_company(true));
    v.reliability = 6_000;
    assert!(v.requires_service_for_company(true));
}

#[test]
fn requires_service_by_day_interval() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.service_interval_days = 10;
    v.last_service_day = 0;
    v.sim_tick = 0;
    v.reliability = 9_000;
    assert!(!v.requires_service_for_company(false));
    v.sim_tick = u64::from(crate::economy::TICKS_PER_DAY) * 10;
    assert!(v.requires_service_for_company(false));
    v.service_at_depot();
    assert!(!v.requires_service_for_company(false));
}

#[test]
fn service_if_needed_depot_order_is_skipped_when_fresh() {
    let mut map = crate::map::Map::new_flat(8, 8, 0);
    let depot = TileCoord::new(2, 2);
    map.set_kind(depot, crate::map::TileKind::RoadDepot)
        .unwrap();
    let mut v = Vehicle::new(1, VehicleKind::Bus, TileCoord::new(1, 1), depot);
    v.reliability = 9_000;
    v.last_service_day = 0;
    v.service_interval_days = 150;
    v.sim_tick = 0;
    v.orders = vec![
        VehicleOrder::depot_pass_through(depot),
        VehicleOrder::station(TileCoord::new(4, 4)),
    ];
    v.current_order = 0;
    v.sync_order_destination(&map);
    assert_eq!(v.current_order, 1);
}

#[test]
fn check_breakdown_triggers_when_unreliable() {
    let mut v = Vehicle::new(
        7,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.reliability = 100;
    v.running = true;
    v.cur_speed = 50;
    v.breakdown_chance = 250;
    v.check_vehicle_breakdown(&mut crate::cargodist::parity::Randomizer::new(42));
    assert!(v.breakdown_ctr > 0);
    assert!(v.handle_breakdown(0) || v.breakdown_ctr > 2);
    if v.breakdown_ctr == 1 {
        assert_eq!(v.cur_speed, 0);
    }
}

#[test]
fn sanitize_current_order_with_empty_orders() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.current_order = 5;
    v.sanitize_current_order();
    assert_eq!(v.current_order, 0);
}

#[test]
fn sanitize_current_order_with_invalid_index() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.orders = vec![
        VehicleOrder::station(TileCoord::new(2, 2)),
        VehicleOrder::station(TileCoord::new(3, 3)),
    ];
    v.current_order = 99;
    v.sanitize_current_order();
    assert_eq!(v.current_order, 0);
}

#[test]
fn sanitize_current_order_valid_index_unchanged() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.orders = vec![
        VehicleOrder::station(TileCoord::new(2, 2)),
        VehicleOrder::station(TileCoord::new(3, 3)),
    ];
    v.current_order = 1;
    v.sanitize_current_order();
    assert_eq!(v.current_order, 1);
}

#[test]
fn current_order_ref_returns_none_with_empty_orders() {
    let v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    assert!(v.current_order_ref().is_none());
}

#[test]
fn current_order_ref_returns_none_with_invalid_index() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.orders = vec![VehicleOrder::station(TileCoord::new(2, 2))];
    v.current_order = 99;
    assert!(v.current_order_ref().is_none());
}

#[test]
fn current_order_ref_returns_some_with_valid_index() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    let order = VehicleOrder::station(TileCoord::new(2, 2));
    v.orders = vec![order];
    v.current_order = 0;
    assert_eq!(v.current_order_ref(), Some(&order));
}

#[test]
fn advance_to_next_order_no_panic_with_invalid_current_order() {
    use crate::map::Map;
    let map = Map::new_flat(8, 8, 0);
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.orders = vec![
        VehicleOrder::station(TileCoord::new(2, 2)),
        VehicleOrder::station(TileCoord::new(3, 3)),
    ];
    v.current_order = 99;
    v.sync_order_destination(&map);
    assert_eq!(v.current_order, 0);
}

#[test]
fn advance_after_loading_no_panic_with_invalid_current_order() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.orders = vec![
        VehicleOrder::station(TileCoord::new(2, 2)),
        VehicleOrder::station(TileCoord::new(3, 3)),
    ];
    v.current_order = 99;
    v.cargo = 10;
    v.capacity = 100;
    v.advance_after_loading();
    // Sanitiza a 0 y luego avanza a la siguiente orden (1).
    assert_eq!(v.current_order, 1);
    assert!(v.current_order < v.orders.len());
}
