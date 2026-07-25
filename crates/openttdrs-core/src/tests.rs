#![allow(clippy::expect_used, clippy::unwrap_used)]
use crate::link_graph::LinkEdgeKey;
use crate::tnbp_decode::JgrTunnelRecord;
use std::collections::VecDeque;

use crate::industry::{
    INDUSTRY_PRODUCE_AMOUNT, INDUSTRY_PRODUCE_TICKS, industry_produce_period_ticks,
};

use crate::vehicle::VEHICLE_CAPACITY;

use super::*;
use crate::test_fixtures::SimHarness;

fn advance_vehicle(v: &mut Vehicle, tiles: u32) {
    for _ in 0..tiles {
        let max_ticks = v.ticks_per_tile() * 2;
        let start = v.pos;
        for _ in 0..max_ticks {
            v.step();
            if v.pos != start {
                break;
            }
        }
    }
}

#[test]
fn new_map_has_expected_dimensions() {
    let s = GameState::new(8, 8);
    assert_eq!(s.map.dimensions(), (8, 8));
}

#[test]
fn step_increments_tick() {
    let mut s = GameState::new(4, 4);
    assert_eq!(s.tick.get(), 0);
    s.step();
    assert_eq!(s.tick.get(), 1);
    s.step();
    assert_eq!(s.tick.get(), 2);
}

#[test]
fn tile_height_roundtrip() {
    let mut s = GameState::new(3, 3);
    let c = TileCoord::new(1, 1);
    s.map.set_height(c, 5).unwrap();
    assert_eq!(s.map.get(c).unwrap().height, 5);
}

#[test]
fn tile_kind_default_is_grass() {
    let s = GameState::new(4, 4);
    for y in 0..4_i32 {
        for x in 0..4_i32 {
            let c = TileCoord::new(x, y);
            assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
        }
    }
}

#[test]
fn tile_kind_roundtrip() {
    let mut s = GameState::new(4, 4);
    let c = TileCoord::new(2, 1);
    s.map.set_kind(c, TileKind::Water).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
    s.map.set_kind(c, TileKind::Forest).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
    s.map.set_kind(c, TileKind::CoalField).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::CoalField));
}

#[test]
fn bfs_finds_path_on_straight_road() {
    let mut m = Map::new_flat(8, 8, 0);
    for x in 0..=4_i32 {
        m.set_kind(TileCoord::new(x, 0), TileKind::Road).unwrap();
    }
    let path = pathfinder::find_path(
        &m,
        TileCoord::new(0, 0),
        TileCoord::new(4, 0),
        pathfinder::PathNetwork::Road,
    );
    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(*path.last().unwrap(), TileCoord::new(4, 0));
    assert_eq!(path.len(), 4);
}

#[test]
fn bfs_returns_none_when_blocked() {
    let m = Map::new_flat(8, 8, 0); // todo Grass, sin carreteras
    let path = pathfinder::find_path(
        &m,
        TileCoord::new(0, 0),
        TileCoord::new(4, 0),
        pathfinder::PathNetwork::Road,
    );
    assert!(path.is_none());
}

#[test]
fn bfs_rail_network_ignores_road_only_corridor() {
    let mut m = Map::new_flat(8, 8, 0);
    for x in 0..=4_i32 {
        m.set_kind(TileCoord::new(x, 0), TileKind::Road).unwrap();
    }
    let from = TileCoord::new(0, 0);
    let to = TileCoord::new(4, 0);
    assert!(pathfinder::find_path(&m, from, to, pathfinder::PathNetwork::Road).is_some());
    assert!(pathfinder::find_path(&m, from, to, pathfinder::PathNetwork::Rail).is_none());
}

#[test]
fn bfs_rail_finds_rail_line() {
    let mut m = Map::new_flat(8, 8, 0);
    for x in 0..=4_i32 {
        m.set_kind(TileCoord::new(x, 1), TileKind::Rail).unwrap();
    }
    let path = pathfinder::find_path(
        &m,
        TileCoord::new(0, 1),
        TileCoord::new(4, 1),
        pathfinder::PathNetwork::Rail,
    );
    assert!(path.is_some());
}

#[test]
fn vehicle_follows_path() {
    let mut s = GameState::new(8, 8);
    for x in 0..=4_i32 {
        s.map
            .set_kind(TileCoord::new(x, 0), TileKind::Road)
            .unwrap();
    }
    let start = TileCoord::new(0, 0);
    let dest = TileCoord::new(4, 0);
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, start, dest));

    let expected = pathfinder::find_path(&s.map, start, dest, pathfinder::PathNetwork::Road)
        .expect("hay carretera");

    for (i, &tile) in expected.iter().enumerate() {
        SimHarness::advance_vehicle_tiles(&mut s, 1);
        assert_eq!(
            s.vehicles[0].pos,
            tile,
            "tesela {} posición incorrecta",
            i + 1
        );
    }
}

#[test]
fn vehicle_loads_from_industry() {
    let mut s = GameState::new(8, 8);
    let ipos = TileCoord::new(0, 0);
    let spos = TileCoord::new(4, 0);
    let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
    ind.stock = 50;
    s.industries.push(ind);
    s.stations.push(Station::new(spos));
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));

    // Carga gradual desde industria.
    let want = VEHICLE_CAPACITY.min(50);
    SimHarness::until_vehicle_cargo(&mut s, 0, want, 16);
    assert_eq!(s.vehicles[0].cargo, want);
    assert_eq!(s.industries[0].stock, 50 - want);
}

#[test]
fn vehicle_loads_from_industry_covered_by_nearby_station() {
    let mut s = GameState::new(12, 8);
    let ipos = TileCoord::new(2, 2);
    let station_pos = TileCoord::new(6, 2);
    let vehicle_pos = ipos;
    let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
    ind.stock = 50;
    s.industries.push(ind);
    s.stations.push(Station::new(station_pos));
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Truck,
        vehicle_pos,
        station_pos,
    ));

    let want = VEHICLE_CAPACITY.min(50);
    SimHarness::until_vehicle_cargo(&mut s, 0, want, 16);

    assert_eq!(s.vehicles[0].cargo, want);
    assert_eq!(s.industries[0].stock, 50 - want);
}

#[test]
fn vehicle_delivers_to_station() {
    let mut s = GameState::new(8, 8);
    let ipos = TileCoord::new(0, 0);
    let spos = TileCoord::new(1, 0);
    let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
    ind.stock = 20;
    s.industries.push(ind);
    s.stations.push(Station::new(spos));
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));

    // Carga en industria, luego un tile de viaje hasta la estación.
    SimHarness::step_until(&mut s, 16, "vehicle loaded and transfer idle", |s| {
        s.vehicles[0].cargo > 0 && !s.vehicles[0].cargo_transfer_active()
    });
    assert!(s.vehicles[0].cargo > 0);

    SimHarness::advance_vehicle_tiles(&mut s, 1);
    assert_eq!(s.vehicles[0].pos, spos);
    SimHarness::until_vehicle_cargo(&mut s, 0, 0, 16);
    assert_eq!(s.vehicles[0].cargo, 0);
    assert!(s.stations[0].income > 0);
    assert!(s.stats.cargo_income_earned > 0, "pago por entrega TTD");
    assert!(!s.runtime.pending_income_popups.is_empty());
    assert!(s.runtime.pending_income_popups.iter().any(|p| p.amount > 0));
    assert!(s.runtime.pending_income_popups.iter().any(|p| p.at == spos));
}

#[test]
fn vehicle_delivers_when_inside_station_coverage() {
    let mut s = GameState::new(12, 8);
    let station_pos = TileCoord::new(6, 2);
    s.stations.push(Station::new(station_pos));
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Truck,
        station_pos,
        station_pos,
    ));
    s.vehicles[0].cargo = 17;
    s.vehicles[0].cargo_type = Some(CargoType::Coal);
    s.vehicles[0].mark_cargo_loaded(TileCoord::new(0, 0));
    s.vehicles[0].ensure_packets_from_legacy();

    SimHarness::until_vehicle_cargo(&mut s, 0, 0, 16);

    assert_eq!(s.vehicles[0].cargo, 0);
    assert_eq!(s.stations[0].stock, 17);
    assert!(
        s.stations[0].income > 0,
        "pago TTD por distancia y tipo de carga"
    );
}

#[test]
fn sim_stats_do_not_count_freight_transfer_as_delivery() {
    let mut s = GameState::new(8, 8);
    let ipos = TileCoord::new(0, 0);
    let spos = TileCoord::new(1, 0);
    let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
    ind.stock = 20;
    s.industries.push(ind);
    s.stations.push(Station::new(spos));
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));
    assert_eq!(s.stats.cargo_pickups, 0);
    assert_eq!(s.stats.cargo_deliveries, 0);
    SimHarness::step_until(&mut s, 16, "vehicle loaded and transfer idle", |s| {
        s.vehicles[0].cargo > 0 && !s.vehicles[0].cargo_transfer_active()
    });
    assert_eq!(s.stats.cargo_pickups, 1);
    assert!(s.stats.cargo_units_loaded > 0);
    SimHarness::advance_vehicle_tiles(&mut s, 1);
    SimHarness::until_vehicle_cargo(&mut s, 0, 0, 16);
    assert_eq!(s.stats.cargo_deliveries, 1);
    assert!(s.stats.cargo_units_delivered > 0);
    assert!(
        !s.news.items.iter().any(|item| {
            matches!(
                item.news_type,
                crate::news::NewsType::CargoDelivered | crate::news::NewsType::FirstCargoDelivered
            )
        }),
        "un trasbordo de carbón no debe publicar una noticia de entrega"
    );
}

#[test]
fn game_state_json_roundtrip() {
    let mut s = GameState::new(4, 4);
    s.industries
        .push(Industry::new(TileCoord::new(0, 0), IndustryKind::Forest));
    s.industries
        .push(Industry::new(TileCoord::new(1, 0), IndustryKind::Factory));
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Train,
        TileCoord::new(0, 1),
        TileCoord::new(2, 1),
    ));
    s.jgr_tunnels_from_footer.push(JgrTunnelRecord {
        tile_n: 0,
        tile_s: 1,
        height: 2,
        is_chunnel: false,
        style_n: None,
        style_s: None,
    });
    let j = s.save_json().expect("json");
    assert!(j.contains("jgr_tunnels_from_footer"));
    let s2 = GameState::load_json(&j).expect("parse");
    assert_eq!(s2.map.dimensions(), (4, 4));
    assert_eq!(s2.industries.len(), 2);
    assert_eq!(s2.industries[0].kind, IndustryKind::Forest);
    assert_eq!(s2.industries[1].kind, IndustryKind::Factory);
    assert_eq!(s2.vehicles[0].kind, VehicleKind::Train);
    assert_eq!(s2.jgr_tunnels_from_footer.len(), 1);
    assert_eq!(s2.jgr_tunnels_from_footer[0].tile_n, 0);
}

#[test]
fn factory_produces_half_as_often_as_mine() {
    assert_eq!(
        industry_produce_period_ticks(IndustryKind::Factory),
        industry_produce_period_ticks(IndustryKind::CoalMine) * 2
    );
    let mut coal = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
    let mut fact = Industry::new(TileCoord::new(1, 0), IndustryKind::Factory);
    let coal_amount = coal.produce_amount();
    coal.produce(256);
    fact.produce(256);
    assert_eq!(coal.stock, coal_amount);
    assert_eq!(fact.stock, 0);
    fact.produce(512);
    assert_eq!(fact.stock, 0, "fábrica sin insumos en estación no produce");
}

#[test]
fn factory_chain_produces_goods_from_delivered_cargo() {
    let mut s = GameState::new(16, 16);
    let fact_pos = TileCoord::new(4, 4);
    let stop_pos = TileCoord::new(5, 4);
    s.industries.push(Industry::with_tiles_spec(
        fact_pos,
        IndustryKind::Factory,
        IndustrySpec::Factory,
        vec![fact_pos],
        0,
    ));
    s.stations
        .push(Station::new_with_kind(stop_pos, StopKind::TruckStop));
    s.stations[0].cargo_stock.wood = FACTORY_WOOD_INPUT;
    s.stations[0].cargo_stock.coal = FACTORY_COAL_INPUT;

    for _ in 0..512 {
        s.step();
    }

    assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);
    assert_eq!(s.stations[0].cargo_stock.wood, 0);
    assert_eq!(s.stations[0].cargo_stock.coal, 0);
}

#[test]
fn truck_loads_freight_waiting_at_station_hub() {
    let mut s = GameState::new(12, 8);
    let hub = TileCoord::new(4, 0);
    s.stations.push(Station::new(hub));
    s.stations[0].cargo_stock.coal = 14;
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Truck,
        hub,
        TileCoord::new(8, 0),
    ));

    SimHarness::until_vehicle_cargo(&mut s, 0, 14, 8);

    assert_eq!(s.vehicles[0].cargo, 14);
    assert_eq!(s.vehicles[0].cargo_type, Some(CargoType::Coal));
    assert_eq!(s.stations[0].cargo_stock.coal, 0);
    assert!(!s.vehicles[0].cargo_packets.is_empty());
}

#[test]
fn locomotive_without_wagon_does_not_load_station_freight() {
    let mut s = GameState::new(12, 8);
    let hub = TileCoord::new(3, 2);
    s.stations
        .push(Station::new_with_kind(hub, StopKind::RailStation));
    s.stations[0].cargo_stock.goods = 9;
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Train,
        hub,
        TileCoord::new(7, 2),
    ));
    for _ in 0..16 {
        s.step();
    }
    assert_eq!(s.vehicles[0].cargo, 0);
    assert_eq!(s.stations[0].cargo_stock.goods, 9);
}

#[test]
fn truck_does_not_load_coal_from_station_while_passing_nearby() {
    use crate::command::apply_command;

    let mut s = GameState::new(16, 12);
    let stop = TileCoord::new(10, 5);
    let road_y = 6_i32;
    for x in 5..=15 {
        s.map
            .set_kind(TileCoord::new(x, road_y), TileKind::Road)
            .expect("road");
    }
    apply_command(&mut s, &Command::PlaceStationDir(stop, 1)).expect("station");
    let stop_idx = s
        .stations
        .iter()
        .position(|st| st.pos == stop)
        .expect("stop");
    s.stations[stop_idx].cargo_stock.coal = 20;

    let far_road = TileCoord::new(5, road_y);
    let mut truck = Vehicle::new(1, VehicleKind::Truck, far_road, TileCoord::new(15, road_y));
    truck.running = true;
    truck.set_station_orders(vec![stop]);
    truck.sync_order_destination(&s.map);
    s.vehicles.push(truck);

    for _ in 0..80 {
        s.step();
        let v = &s.vehicles[0];
        if v.cargo > 0 {
            let at_stop = v.pos == stop || road_stop_approach_tile(&s.map, stop) == Some(v.pos);
            assert!(
                at_stop,
                "no debe cargar carbón lejos de la parada (pos {:?}, cargo {})",
                v.pos, v.cargo
            );
        }
    }
}

#[test]
fn truck_prefers_industry_over_station_waiting_cargo() {
    let mut s = GameState::new(12, 8);
    let ipos = TileCoord::new(4, 0);
    let hub = TileCoord::new(4, 0);
    let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
    ind.stock = 11;
    s.industries.push(ind);
    s.stations.push(Station::new(hub));
    s.stations[0].cargo_stock.coal = 50;
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Truck,
        hub,
        TileCoord::new(8, 0),
    ));

    SimHarness::until_vehicle_cargo(&mut s, 0, 11, 8);

    assert_eq!(s.vehicles[0].cargo, 11);
    assert_eq!(s.industries[0].stock, 0);
    assert_eq!(s.stations[0].cargo_stock.coal, 50);
}

#[test]
fn two_truck_transfer_via_station_hub() {
    let mut s = GameState::new(16, 8);
    let hub = TileCoord::new(6, 0);
    let dest = TileCoord::new(10, 0);
    for x in 0..=10_i32 {
        s.map
            .set_kind(TileCoord::new(x, 0), TileKind::Road)
            .unwrap();
    }
    s.stations.push(Station::new(hub));

    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, hub, hub));
    s.vehicles[0].cargo = 16;
    // Goods: transferencia en hub (no bloqueada como bulk de mina).
    s.vehicles[0].cargo_type = Some(CargoType::Goods);
    s.vehicles[0].mark_cargo_loaded(TileCoord::new(0, 0));
    s.vehicles[0].ensure_packets_from_legacy();

    SimHarness::until_vehicle_cargo(&mut s, 0, 0, 8);
    assert_eq!(s.vehicles[0].cargo, 0);
    assert_eq!(s.stations[0].cargo_stock.goods, 16);
    s.vehicles.clear();

    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, hub, dest));
    SimHarness::until_vehicle_cargo(&mut s, 0, 16, 8);
    assert_eq!(s.vehicles[0].cargo, 16);
    assert_eq!(s.stations[0].cargo_stock.goods, 0);
}

#[test]
fn link_graph_records_station_flow_on_unload() {
    let mut s = GameState::new(16, 8);
    let from = TileCoord::new(2, 0);
    let dest = TileCoord::new(10, 0);
    s.stations.push(Station::new(dest));
    let mut truck = Vehicle::new(1, VehicleKind::Truck, dest, dest);
    truck.cargo = 10;
    truck.cargo_type = Some(CargoType::Goods);
    truck.mark_cargo_loaded(from);
    truck.ensure_packets_from_legacy();
    truck.last_pickup_station = Some(from);
    s.vehicles.push(truck);
    SimHarness::until_vehicle_cargo(&mut s, 0, 0, 8);
    assert_eq!(s.vehicles[0].cargo, 0);
    let key = LinkEdgeKey {
        from,
        to: dest,
        cargo: CargoType::Goods,
    };
    assert_eq!(
        s.link_graph.edges.get(&key).map(|e| e.units_total),
        Some(10)
    );
    assert_eq!(s.link_graph.edges[&key].units_month, 10);
    s.link_graph.rollover_month();
    assert_eq!(s.link_graph.edges[&key].units_month, 0);
    assert_eq!(s.link_graph.edges[&key].units_total, 10);
}

#[test]
fn link_graph_sets_pickup_when_loading_waiting_cargo() {
    let mut s = GameState::new(12, 8);
    let hub = TileCoord::new(4, 0);
    s.stations.push(Station::new(hub));
    s.stations[0].cargo_stock.goods = 14;
    s.vehicles.push(Vehicle::new(
        0,
        VehicleKind::Truck,
        hub,
        TileCoord::new(8, 0),
    ));

    SimHarness::until_vehicle_cargo(&mut s, 0, 14, 8);
    assert_eq!(s.vehicles[0].cargo, 14);
    assert_eq!(s.vehicles[0].last_pickup_station, Some(hub));
}

#[test]
fn delivery_income_scales_with_haul_distance() {
    let station = TileCoord::new(10, 0);
    let cases = [(TileCoord::new(9, 0), 0_u64), (TileCoord::new(0, 0), 0_u64)];
    let mut incomes = [0_u64; 2];

    for (idx, (source, _)) in cases.iter().enumerate() {
        let mut s = GameState::new(16, 8);
        s.stations.push(Station::new(station));
        let mut truck = Vehicle::new(1, VehicleKind::Truck, station, station);
        truck.cargo = 10;
        truck.cargo_type = Some(CargoType::Coal);
        truck.mark_cargo_loaded(*source);
        truck.ensure_packets_from_legacy();
        s.vehicles.push(truck);
        SimHarness::until_vehicle_cargo(&mut s, 0, 0, 8);
        incomes[idx] = s.stations[0].income;
    }

    assert!(
        incomes[1] > incomes[0],
        "origen lejano {} vs cercano {}",
        incomes[1],
        incomes[0]
    );
}

#[test]
fn economic_cycle_roundtrip() {
    let mut s = GameState::new(16, 16);
    let ipos = TileCoord::new(0, 0);
    let spos = TileCoord::new(2, 0);
    for x in 0..=2_i32 {
        s.map
            .set_kind(TileCoord::new(x, 0), TileKind::Road)
            .unwrap();
    }

    // Industria con stock suficiente para varios ciclos.
    let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
    ind.stock = 1000;
    s.industries.push(ind);
    s.stations.push(Station::new(spos));
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));

    // Ciclo completo con descarga al llegar (manhattan_to_dest == 0).
    for _ in 0..80 {
        s.step();
    }
    assert!(
        s.stations[0].income > 0,
        "debe haber income tras varios ticks"
    );
}

#[test]
fn station_coverage_counts_nearby_cargo_sources_and_acceptors() {
    let mut s = GameState::new(16, 16);
    let station_pos = TileCoord::new(8, 8);
    let coal_pos = TileCoord::new(10, 8);
    let house_pos = TileCoord::new(7, 7);
    let far_forest_pos = TileCoord::new(14, 8);

    s.map.set_kind(coal_pos, TileKind::Industry).unwrap();
    s.map.set_kind(house_pos, TileKind::House).unwrap();
    s.map.set_kind(far_forest_pos, TileKind::Industry).unwrap();

    let mut coal = Industry::new(coal_pos, IndustryKind::CoalMine);
    coal.stock = 42;
    s.industries.push(coal);
    s.industries
        .push(Industry::new(far_forest_pos, IndustryKind::Forest));

    let coverage = station_coverage_at(&s.map, &s.industries, station_pos, STATION_COVERAGE_RADIUS);
    assert_eq!(coverage.house_tiles, 1);
    assert_eq!(coverage.accepts_mail, 1);
    assert_eq!(coverage.accepts_goods, 1);
    assert_eq!(coverage.supplies_coal, 1);
    assert_eq!(coverage.supplies_wood, 0);
    assert_eq!(coverage.supplied_stock, 42);
    assert!(coverage.accepts_anything());
    assert!(coverage.supplies_anything());
}

#[test]
fn town_generates_passengers_at_bus_stop_near_houses() {
    let mut s = GameState::new(16, 16);
    let stop_pos = TileCoord::new(8, 8);
    s.map
        .set_kind(TileCoord::new(7, 8), TileKind::House)
        .unwrap();
    s.stations
        .push(Station::new_with_kind(stop_pos, StopKind::BusStop));
    // Sin un bus que haya intentado cargar, selectgoods no mueve pasajeros al andén.
    s.stations[0]
        .goods
        .get_mut(CargoType::Passengers)
        .last_speed = 1;

    for _ in 0..TOWN_PRODUCE_TICKS {
        s.step();
    }

    assert!(
        s.stations[0].cargo_stock.passengers > 0,
        "debe haber pasajeros en la parada"
    );
    assert!(s.stats.town_passengers_generated > 0);
}

#[test]
fn bus_loads_and_delivers_passengers_for_income() {
    let mut s = GameState::new(16, 16);
    let origin = TileCoord::new(4, 0);
    let dest = TileCoord::new(8, 0);
    for x in 0..=8_i32 {
        s.map
            .set_kind(TileCoord::new(x, 0), TileKind::Road)
            .unwrap();
    }
    s.stations
        .push(Station::new_with_kind(origin, StopKind::BusStop));
    s.stations
        .push(Station::new_with_kind(dest, StopKind::BusStop));
    s.stations[0].cargo_stock.passengers = 15;
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Bus, origin, dest));

    SimHarness::until_vehicle_cargo(&mut s, 0, 15, 8);
    assert_eq!(s.vehicles[0].cargo, 15);
    assert_eq!(s.stations[0].cargo_stock.passengers, 0);

    SimHarness::advance_vehicle_tiles(&mut s, 4);
    assert_eq!(s.vehicles[0].pos, dest);
    SimHarness::until_vehicle_cargo(&mut s, 0, 0, 8);
    assert_eq!(s.vehicles[0].cargo, 0);
    assert!(
        s.stats.cargo_income_earned > 0,
        "entrega de pasajeros debe pagar"
    );
    assert_eq!(
        s.stations[1].cargo_stock.passengers, 0,
        "pasajeros entregados no quedan en cola de destino"
    );
}

#[test]
fn vehicle_moves_toward_dest() {
    let mut s = GameState::new(8, 8);
    let start = TileCoord::new(0, 0);
    let dest = TileCoord::new(5, 0);
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, start, dest));

    let dist_before = s.vehicles[0].manhattan_to_dest();
    SimHarness::advance_vehicle_tiles(&mut s, 1);
    let dist_after = s.vehicles[0].manhattan_to_dest();
    assert!(dist_after < dist_before, "debe acercarse al destino");
}

#[test]
fn vehicle_without_orders_waits_at_arrival_without_road_network() {
    let mut s = GameState::new(8, 8);
    let start = TileCoord::new(0, 0);
    let dest = TileCoord::new(3, 0);
    s.vehicles
        .push(Vehicle::new(0, VehicleKind::Truck, start, dest));

    // Avanzar hasta llegar al destino (3 tiles Manhattan sin red).
    SimHarness::advance_vehicle_tiles(&mut s, 3);
    assert_eq!(s.vehicles[0].pos, dest);
    assert_eq!(s.vehicles[0].dest, dest);

    for _ in 0..=3 {
        s.step();
    }
    assert_eq!(s.vehicles[0].dest, dest);
}

#[test]
fn vehicle_with_orders_does_not_use_manhattan_without_network() {
    let mut s = GameState::new(8, 8);
    let start = TileCoord::new(0, 0);
    let far = TileCoord::new(5, 0);
    let mut v = Vehicle::new(0, VehicleKind::Truck, start, start);
    v.set_orders(vec![far]);
    s.vehicles.push(v);

    s.step();

    assert!(s.vehicles[0].no_network_route_to_order);
    assert_eq!(s.vehicles[0].pos, start);
}

#[test]
fn train_without_orders_keeps_moving_on_rail() {
    let mut s = GameState::new(16, 8);
    for x in 0..10 {
        s.map
            .set_kind(TileCoord::new(x, 3), TileKind::Rail)
            .unwrap();
    }
    let start = TileCoord::new(1, 3);
    let mut v = Vehicle::new(11, VehicleKind::Train, start, start);
    v.running = true;
    v.set_cruise_speed();
    s.vehicles.push(v);

    SimHarness::advance_vehicle_tiles(&mut s, 2);

    assert_ne!(s.vehicles[0].pos, start);
    assert_eq!(s.map.get_kind(s.vehicles[0].pos), Some(TileKind::Rail));
}

#[test]
fn vehicle_without_orders_wanders_on_road_network() {
    let mut s = GameState::new(8, 8);
    for x in 1..=3 {
        s.map
            .set_kind(TileCoord::new(x, 1), TileKind::Road)
            .unwrap();
    }
    let start = TileCoord::new(1, 1);
    s.vehicles
        .push(Vehicle::new(7, VehicleKind::Truck, start, start));

    SimHarness::advance_vehicle_tiles(&mut s, 1);

    assert_ne!(s.vehicles[0].pos, start);
    assert_eq!(s.map.get_kind(s.vehicles[0].pos), Some(TileKind::Road));
}

#[test]
fn vehicle_with_orders_cycles_destinations() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.set_orders(vec![TileCoord::new(1, 0), TileCoord::new(1, 1)]);
    v.path = VecDeque::from([TileCoord::new(1, 0)]);
    v.set_cruise_speed();
    advance_vehicle(&mut v, 1);
    assert_eq!(v.pos, TileCoord::new(1, 0));
    assert_eq!(v.dest, TileCoord::new(1, 1));
    v.path = VecDeque::from([TileCoord::new(1, 1)]);
    advance_vehicle(&mut v, 1);
    assert_eq!(v.pos, TileCoord::new(1, 1));
    assert_eq!(v.dest, TileCoord::new(1, 0));
}

#[test]
fn vehicle_with_station_orders_cycles_station_destinations() {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.set_station_orders(vec![TileCoord::new(1, 0), TileCoord::new(1, 1)]);
    assert!(matches!(v.orders[0], VehicleOrder::Station { .. }));
    v.path = VecDeque::from([TileCoord::new(1, 0)]);
    v.set_cruise_speed();
    advance_vehicle(&mut v, 1);
    assert_eq!(v.pos, TileCoord::new(1, 0));
    // La orden avanza en el tick siguiente a la llegada (ventana de carga).
    v.step();
    assert_eq!(v.dest, TileCoord::new(1, 1));
}

#[test]
fn legacy_tile_orders_deserialize_as_tile_orders() {
    let json = r#"{
        "id": 1,
        "kind": "Truck",
        "pos": {"x": 0, "y": 0},
        "origin": {"x": 0, "y": 0},
        "dest": {"x": 1, "y": 0},
        "cargo": 0,
        "capacity": 20,
        "path": [],
        "orders": [{"x": 1, "y": 0}],
        "current_order": 0
    }"#;

    let vehicle: Vehicle = serde_json::from_str(json).expect("legacy vehicle json");

    assert!(matches!(vehicle.orders[0], VehicleOrder::Tile(_)));
    assert_eq!(vehicle.orders[0].destination(), TileCoord::new(1, 0));
}

#[test]
fn two_worlds_same_vehicles_same_position() {
    let start = TileCoord::new(0, 0);
    let dest = TileCoord::new(4, 3);
    let mut a = GameState::new(8, 8);
    let mut b = GameState::new(8, 8);
    for s in [&mut a, &mut b] {
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, start, dest));
    }
    for _ in 0..50 {
        a.step();
        b.step();
    }
    assert_eq!(a.vehicles[0].pos, b.vehicles[0].pos);
}

/// El pago decae por periodo de tránsito, y un periodo son 185 ticks (~2,5 días),
/// no un día de calendario: con 74 la carga envejecería 2,5× más rápido.
#[test]
fn onboard_cargo_ages_every_185_ticks() {
    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(1, 1);
    let mut truck = Vehicle::new(0, VehicleKind::Truck, pos, pos);
    truck.cargo = 10;
    truck.cargo_type = Some(CargoType::Coal);
    s.vehicles.push(truck);

    for _ in 0..u64::from(TICKS_PER_DAY) {
        s.step();
    }
    assert_eq!(s.vehicles[0].cargo_packets.max_periods_in_transit(), 0);

    while s.tick.get() < u64::from(CARGO_AGING_TICKS) {
        s.step();
    }
    assert_eq!(s.vehicles[0].cargo_packets.max_periods_in_transit(), 1);
}

/// Tras producir, la mina reparte el carbón a las estaciones de su cobertura: el rating
/// decide la tajada de cada una (`TransportIndustryGoods` / `MoveGoodsToStation`).
#[test]
fn mine_production_splits_between_competing_stations() {
    let mut s = GameState::new(16, 16);
    let mine_pos = TileCoord::new(8, 8);
    s.industries
        .push(Industry::new(mine_pos, IndustryKind::CoalMine));
    let mut good = Station::new(TileCoord::new(6, 8));
    let mut bad = Station::new(TileCoord::new(10, 8));
    good.goods.get_mut(CargoType::Coal).last_speed = 1;
    bad.goods.get_mut(CargoType::Coal).last_speed = 1;
    good.goods.get_mut(CargoType::Coal).rating = 220;
    bad.goods.get_mut(CargoType::Coal).rating = 40;
    s.stations.push(good);
    s.stations.push(bad);

    for _ in 0..=INDUSTRY_PRODUCE_TICKS {
        s.step();
    }

    assert_eq!(
        s.industries[0].stock, 0,
        "la producción se mueve a las estaciones, no se queda en la mina"
    );
    assert!(
        s.stations[0].cargo_stock.coal > s.stations[1].cargo_stock.coal,
        "la estación bien servida se lleva más: {} vs {}",
        s.stations[0].cargo_stock.coal,
        s.stations[1].cargo_stock.coal
    );
}

/// Una estación abandonada pierde rating barrido a barrido en la simulación completa,
/// y con él la cantidad que un vehículo puede llevarse (`load_amount_for_rating`).
#[test]
fn abandoned_station_loses_rating_over_time() {
    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(2, 2);
    let mut station = crate::station::Station::new(pos);
    station.add_waiting_cargo(CargoType::Coal, 200);
    s.stations.push(station);

    let initial = crate::station::station_rating_for_cargo(&s.stations[0], CargoType::Coal);
    assert_eq!(initial, crate::station::INITIAL_STATION_RATING);

    let sweep = u64::from(crate::economy::STATION_RATING_TICKS);
    while s.tick.get() < sweep * 20 {
        s.step();
    }

    let decayed = crate::station::station_rating_for_cargo(&s.stations[0], CargoType::Coal);
    assert!(
        decayed < initial,
        "sin vehículos que la sirvan la estación debe empeorar, quedó en {decayed}"
    );
    assert!(
        crate::station::load_amount_for_rating(100, decayed) < 100,
        "un rating peor limita lo que se puede cargar"
    );
}

#[test]
fn industry_produces_on_schedule() {
    let mut s = GameState::new(8, 8);
    s.industries
        .push(Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine));
    let per_cycle = s.industries[0].produce_amount();

    // Sin ticks no hay producción.
    assert_eq!(s.industries[0].stock, 0);

    // Avanzar exactamente INDUSTRY_PRODUCE_TICKS ticks.
    for _ in 0..INDUSTRY_PRODUCE_TICKS {
        s.step();
    }
    assert_eq!(s.industries[0].stock, per_cycle);

    // Un segundo ciclo completo.
    for _ in 0..INDUSTRY_PRODUCE_TICKS {
        s.step();
    }
    assert_eq!(s.industries[0].stock, per_cycle * 2);
}

#[test]
fn industry_does_not_exceed_capacity() {
    let mut s = GameState::new(8, 8);
    let mut ind = Industry::new(TileCoord::new(0, 0), IndustryKind::Forest);
    let per_cycle = ind.produce_amount();
    ind.capacity = per_cycle; // capacidad mínima: un ciclo
    s.industries.push(ind);

    // Primer ciclo llena hasta capacity.
    for _ in 0..INDUSTRY_PRODUCE_TICKS {
        s.step();
    }
    assert_eq!(s.industries[0].stock, per_cycle);

    // Segundo ciclo: stock saturado, no supera capacity.
    for _ in 0..INDUSTRY_PRODUCE_TICKS {
        s.step();
    }
    assert_eq!(s.industries[0].stock, per_cycle);
}

#[test]
fn two_worlds_same_industries_same_stock() {
    let mut a = GameState::new(8, 8);
    let mut b = GameState::new(8, 8);
    for state in [&mut a, &mut b] {
        state
            .industries
            .push(Industry::new(TileCoord::new(1, 2), IndustryKind::CoalMine));
        state
            .industries
            .push(Industry::new(TileCoord::new(3, 4), IndustryKind::Forest));
    }
    for _ in 0..INDUSTRY_PRODUCE_TICKS * 3 {
        a.step();
        b.step();
    }
    assert_eq!(a.industries[0].stock, b.industries[0].stock);
    assert_eq!(a.industries[1].stock, b.industries[1].stock);
}

#[test]
fn tile_height_and_kind_are_independent() {
    let mut s = GameState::new(4, 4);
    let c = TileCoord::new(1, 2);
    s.map.set_height(c, 7).unwrap();
    s.map.set_kind(c, TileKind::Forest).unwrap();
    assert_eq!(s.map.get(c).unwrap().height, 7);
    assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
    // Cambiar altura no afecta el tipo.
    s.map.set_height(c, 3).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
    // Cambiar tipo no afecta la altura.
    s.map.set_kind(c, TileKind::Water).unwrap();
    assert_eq!(s.map.get(c).unwrap().height, 3);
}
