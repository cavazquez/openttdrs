//! V1 #597: primer servicio de pasajeros físico de RoadHaul.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::station::StopKind;
use openttdrs_core::town::Town;
use openttdrs_core::{
    CargoType, Climate, CompanyId, GameState, TileCoord, TileKind, VehicleKind, VehicleOrder,
};

const MAP_SIZE: u32 = 64;
const MAX_DELIVERY_TICKS: usize = 40_000;
const WORLD_SEED: u64 = 0x5970_0064;
const TOWN_A: TileCoord = TileCoord::new(24, 20);
const TOWN_B: TileCoord = TileCoord::new(32, 20);

#[derive(Debug, Clone, PartialEq, Eq)]
struct HumanAssets {
    money: i64,
    vehicles: Vec<u32>,
    stations: Vec<TileCoord>,
    infrastructure: Vec<TileCoord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RoadHaulDeliveryReport {
    world_seed: u64,
    route_built_tick: u64,
    bus_purchase_tick: u64,
    orders_ready_tick: u64,
    first_delivery_tick: u64,
    net_cost_at_first_delivery: i64,
    delivery_income: u64,
    orders: Vec<Vec<VehicleOrder>>,
    canonical_hash: u64,
}

fn human_assets(state: &GameState) -> HumanAssets {
    let (width, height) = state.map.dimensions();
    let mut infrastructure = Vec::new();
    for y in 0..i32::try_from(height).unwrap() {
        for x in 0..i32::try_from(width).unwrap() {
            let pos = TileCoord::new(x, y);
            let Some(tile) = state.map.get(pos) else {
                continue;
            };
            if matches!(
                tile.kind,
                TileKind::Rail
                    | TileKind::Road
                    | TileKind::RailDepot
                    | TileKind::RoadDepot
                    | TileKind::ShipDepot
            ) && CompanyId::from_tile_m1(tile.m1, state.companies.len()) == CompanyId::PLAYER
            {
                infrastructure.push(pos);
            }
        }
    }
    HumanAssets {
        money: state.companies[CompanyId::PLAYER.index()].economy.money,
        vehicles: state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.owner == CompanyId::PLAYER)
            .map(|vehicle| vehicle.id)
            .collect(),
        stations: state
            .stations
            .iter()
            .filter(|station| station.owner == CompanyId::PLAYER)
            .map(|station| station.pos)
            .collect(),
        infrastructure,
    }
}

fn add_houses(state: &mut GameState, town: TileCoord, town_id: u32) {
    for (dx, dy) in [(-1, 3), (0, 3), (1, 3), (-1, 4), (0, 4), (1, 4)] {
        let house = TileCoord::new(town.x + dx, town.y + dy);
        state.map.set_completed_house(house, 4, 0).unwrap();
        state.map.set_house_town_id(house, town_id).unwrap();
    }
}

fn roadhaul_route_built(state: &GameState, ai_id: CompanyId) -> bool {
    let bus_stops = state
        .stations
        .iter()
        .filter(|station| station.owner == ai_id && station.stop_kind == StopKind::BusStop)
        .count();
    let (width, height) = state.map.dimensions();
    let has_depot = (0..i32::try_from(height).unwrap()).any(|y| {
        (0..i32::try_from(width).unwrap()).any(|x| {
            state.map.get(TileCoord::new(x, y)).is_some_and(|tile| {
                tile.kind == TileKind::RoadDepot
                    && CompanyId::from_tile_m1(tile.m1, state.companies.len()) == ai_id
            })
        })
    });
    bus_stops >= 2 && has_depot
}

fn roadhaul_bus_bought(state: &GameState, ai_id: CompanyId) -> bool {
    state
        .vehicles
        .iter()
        .any(|vehicle| vehicle.owner == ai_id && vehicle.kind == VehicleKind::Bus)
}

fn roadhaul_orders_ready(state: &GameState, ai_id: CompanyId) -> bool {
    state.vehicles.iter().any(|vehicle| {
        vehicle.owner == ai_id
            && vehicle.kind == VehicleKind::Bus
            && vehicle
                .orders
                .iter()
                .filter(|order| matches!(order, VehicleOrder::Station { .. }))
                .count()
                >= 2
    })
}

fn v1_fixture() -> (GameState, CompanyId) {
    let mut state = GameState::new(MAP_SIZE, MAP_SIZE);
    state.climate = Climate::Temperate;
    state.world_seed = WORLD_SEED;
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;
    state.ai.enabled = true;
    state.ensure_rival_roadhaul();
    let ai_id = state
        .companies
        .iter()
        .find(|company| company.is_ai)
        .expect("rival RoadHaul")
        .id;
    state.companies[ai_id.index()].economy.money = 2_000_000;
    state.towns = vec![
        Town {
            id: 1,
            pos: TOWN_A,
            name: "Norte V1".into(),
            population: 1_000,
            ..Town::default()
        },
        Town {
            id: 2,
            pos: TOWN_B,
            name: "Sur V1".into(),
            population: 900,
            ..Town::default()
        },
    ];
    add_houses(&mut state, TOWN_A, 1);
    add_houses(&mut state, TOWN_B, 2);

    assert_eq!(state.map.dimensions(), (MAP_SIZE, MAP_SIZE));
    assert_eq!(state.climate, Climate::Temperate);
    assert_eq!(state.world_seed, WORLD_SEED);
    assert!(!state.disasters_enabled);
    assert_eq!(state.vehicle_breakdowns, 0);
    assert!(state.vehicles.is_empty());
    assert!(state.stations.is_empty());
    assert!(state.ai_build_queues.is_empty());
    assert_eq!(
        state.companies[ai_id.index()]
            .cargo_units_delivered_by_type
            .units_for(CargoType::Passengers),
        0
    );
    assert_eq!(state.companies[ai_id.index()].cargo_income_earned, 0);
    (state, ai_id)
}

fn run_to_first_delivery() -> RoadHaulDeliveryReport {
    let (mut state, ai_id) = v1_fixture();
    let (mut without_ai, _) = v1_fixture();
    without_ai.ai.enabled = false;
    let ai_money_before = state.companies[ai_id.index()].economy.money;
    let income_before = state.companies[ai_id.index()].cargo_income_earned;
    let mut route_built_tick = None;
    let mut bus_purchase_tick = None;
    let mut orders_ready_tick = None;

    for _ in 0..MAX_DELIVERY_TICKS {
        state.step();
        without_ai.step();
        assert_eq!(
            human_assets(&state),
            human_assets(&without_ai),
            "RoadHaul no puede alterar dinero ni activos de la compañía humana frente al control sin IA (tick {})",
            state.tick.get()
        );
        if route_built_tick.is_none() && roadhaul_route_built(&state, ai_id) {
            route_built_tick = Some(state.tick.get());
        }
        if bus_purchase_tick.is_none() && roadhaul_bus_bought(&state, ai_id) {
            bus_purchase_tick = Some(state.tick.get());
        }
        if orders_ready_tick.is_none() && roadhaul_orders_ready(&state, ai_id) {
            orders_ready_tick = Some(state.tick.get());
        }
        let ai = &state.companies[ai_id.index()];
        let passengers = ai
            .cargo_units_delivered_by_type
            .units_for(CargoType::Passengers);
        let delivery_income = ai.cargo_income_earned.saturating_sub(income_before);
        if passengers == 0 || delivery_income == 0 {
            continue;
        }

        let mut orders = state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.owner == ai_id && vehicle.kind == VehicleKind::Bus)
            .map(|vehicle| (vehicle.id, vehicle.orders.clone()))
            .collect::<Vec<_>>();
        orders.sort_by_key(|(id, _)| *id);
        let orders = orders
            .into_iter()
            .map(|(_, orders)| orders)
            .collect::<Vec<_>>();
        assert!(!orders.is_empty(), "RoadHaul debe comprar un bus");
        assert!(
            orders.iter().all(|orders| !orders.is_empty()),
            "el scheduler debe crear las órdenes antes de entregar"
        );

        return RoadHaulDeliveryReport {
            world_seed: state.world_seed,
            route_built_tick: route_built_tick.expect("RoadHaul debe construir su ruta"),
            bus_purchase_tick: bus_purchase_tick.expect("RoadHaul debe comprar un bus"),
            orders_ready_tick: orders_ready_tick.expect("RoadHaul debe crear sus órdenes"),
            first_delivery_tick: state.tick.get(),
            net_cost_at_first_delivery: ai_money_before - ai.economy.money,
            delivery_income,
            orders,
            canonical_hash: state.canonical_hash(),
        };
    }

    let buses = state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.owner == ai_id && vehicle.kind == VehicleKind::Bus)
        .map(|vehicle| {
            (
                vehicle.id,
                vehicle.pos,
                vehicle.dest,
                vehicle.current_order,
                vehicle.cargo,
                vehicle.orders.clone(),
            )
        })
        .collect::<Vec<_>>();
    let stops = state
        .stations
        .iter()
        .filter(|station| station.owner == ai_id)
        .map(|station| {
            (
                station.pos,
                station.cargo_stock.passengers,
                station.goods.get(CargoType::Passengers).last_speed,
            )
        })
        .collect::<Vec<_>>();
    panic!(
        "RoadHaul no entregó pasajeros antes de {MAX_DELIVERY_TICKS} ticks; selectgoods={}; generated={}; buses={buses:?}; stops={stops:?}",
        state.order.selectgoods, state.stats.town_passengers_generated,
    );
}

#[test]
fn v1_roadhaul_builds_and_pays_one_passenger_route_deterministically() {
    let first = run_to_first_delivery();
    println!("V1-AIRH first delivery: {first:?}");
    assert_eq!(first.world_seed, WORLD_SEED);
    assert!(first.route_built_tick < first.first_delivery_tick);
    assert!(first.bus_purchase_tick < first.first_delivery_tick);
    assert!(first.orders_ready_tick < first.first_delivery_tick);
    assert!(first.first_delivery_tick <= u64::try_from(MAX_DELIVERY_TICKS).unwrap());
    assert!(first.delivery_income > 0);
    assert!(!first.orders.is_empty());

    let second = run_to_first_delivery();
    assert_eq!(
        first, second,
        "dos ejecuciones con seed fijo deben coincidir"
    );
}
