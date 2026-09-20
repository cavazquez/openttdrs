//! V1 #596: primera entrega ferroviaria física de TransCargo.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::{
    CargoType, CompanyId, GameState, Industry, IndustryKind, IndustrySpec, PRODLEVEL_MAXIMUM,
    TileCoord, TileKind, VehicleKind, VehicleOrder,
};

const MAP_SIZE: u32 = 64;
const MAX_DELIVERY_TICKS: usize = 40_000;
const MINE: TileCoord = TileCoord::new(8, 32);
const POWER_STATION: TileCoord = TileCoord::new(48, 32);

#[derive(Debug, Clone, PartialEq, Eq)]
struct HumanAssets {
    money: i64,
    vehicles: Vec<u32>,
    stations: Vec<TileCoord>,
    infrastructure: Vec<TileCoord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransCargoDeliveryReport {
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

fn v1_fixture() -> (GameState, CompanyId) {
    let mut state = GameState::new(MAP_SIZE, MAP_SIZE);
    state.world_seed = 0x5960_0064;
    state.disasters_enabled = false;
    state.ai.enabled = true;
    state.ai.max_routes = 1;
    state.ensure_rival_transcargo();
    let ai_id = state
        .companies
        .iter()
        .find(|company| company.is_ai)
        .expect("rival TransCargo")
        .id;
    state.companies[ai_id.index()].economy.money = 2_000_000;

    let mut mine = Industry::with_tiles_spec(
        MINE,
        IndustryKind::CoalMine,
        IndustrySpec::CoalMine,
        vec![MINE],
        0,
    );
    mine.stock = 200;
    mine.prod_level = PRODLEVEL_MAXIMUM;
    let power_station = Industry::with_tiles_spec(
        POWER_STATION,
        IndustryKind::Factory,
        IndustrySpec::PowerStation,
        vec![POWER_STATION],
        0,
    );
    assert!(power_station.accepts_cargo(CargoType::Coal));
    state.industries = vec![mine, power_station];

    assert_eq!(state.map.dimensions(), (MAP_SIZE, MAP_SIZE));
    assert!(state.vehicles.is_empty());
    assert!(state.stations.is_empty());
    assert!(state.ai_build_queues.is_empty());
    (state, ai_id)
}

fn run_to_first_delivery() -> TransCargoDeliveryReport {
    let (mut state, ai_id) = v1_fixture();
    let (mut without_ai, _) = v1_fixture();
    without_ai.ai.enabled = false;
    let ai_money_before = state.companies[ai_id.index()].economy.money;
    let income_before = state.companies[ai_id.index()].cargo_income_earned;

    for _ in 0..MAX_DELIVERY_TICKS {
        state.step();
        without_ai.step();
        assert_eq!(
            human_assets(&state),
            human_assets(&without_ai),
            "TransCargo no puede alterar dinero ni activos de la compañía humana frente al control sin IA (tick {})",
            state.tick.get()
        );
        let ai = &state.companies[ai_id.index()];
        let coal_delivered = ai.cargo_units_delivered_by_type.units_for(CargoType::Coal);
        let delivery_income = ai.cargo_income_earned.saturating_sub(income_before);
        if coal_delivered == 0 || delivery_income == 0 {
            continue;
        }

        assert!(
            state.industries.iter().any(|industry| {
                industry.pos == POWER_STATION
                    && industry.was_cargo_delivered
                    && industry.last_accepted_date(CargoType::Coal) > 0
            }),
            "la central debe aceptar la entrega física de carbón"
        );
        let mut orders = state
            .vehicles
            .iter()
            .filter(|vehicle| {
                vehicle.owner == ai_id
                    && vehicle.kind == VehicleKind::Train
                    && vehicle.is_consist_head()
            })
            .map(|vehicle| (vehicle.id, vehicle.orders.clone()))
            .collect::<Vec<_>>();
        orders.sort_by_key(|(id, _)| *id);
        let orders = orders
            .into_iter()
            .map(|(_, orders)| orders)
            .collect::<Vec<_>>();
        assert!(
            orders.iter().all(|orders| !orders.is_empty()),
            "el scheduler debe comprar y ordenar cada tren antes de entregar"
        );

        return TransCargoDeliveryReport {
            first_delivery_tick: state.tick.get(),
            net_cost_at_first_delivery: ai_money_before - ai.economy.money,
            delivery_income,
            orders,
            canonical_hash: state.canonical_hash(),
        };
    }

    panic!("TransCargo no entregó carbón antes de {MAX_DELIVERY_TICKS} ticks");
}

#[test]
fn v1_transcargo_builds_and_pays_one_coal_route_deterministically() {
    let first = run_to_first_delivery();
    println!("V1-AIT first delivery: {first:?}");
    assert!(first.first_delivery_tick <= u64::try_from(MAX_DELIVERY_TICKS).unwrap());
    assert!(first.delivery_income > 0);
    assert!(!first.orders.is_empty());

    let second = run_to_first_delivery();
    assert_eq!(
        first, second,
        "dos ejecuciones con seed fijo deben coincidir"
    );
}
