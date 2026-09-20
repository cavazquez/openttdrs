//! V1 #593: primera entrega marítima pagada de carbón vía boya.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::parity::{FIRST_ROUTE_COAL_MINE, FIRST_ROUTE_POWER_STATION, build_scenario};
use openttdrs_core::{
    CargoType, Command, GameState, OrderLoadType, OrderNonStop, OrderUnloadType, StopKind,
    TileCoord, TileKind, VehicleKind, VehicleOrder, apply_command,
};

const MAP_SIZE: u32 = 64;
const SHIP_ID: u32 = 1;
const MAX_ROUTE_TICKS: usize = 40_000;
const DEPOT: TileCoord = TileCoord::new(5, 13);
const SOURCE_DOCK: TileCoord = TileCoord::new(12, 11);
const BUOY: TileCoord = TileCoord::new(32, 13);
const DESTINATION_DOCK: TileCoord = TileCoord::new(52, 11);

/// Prepara la única franja de mar del fixture. La infraestructura jugable se
/// construye después mediante `Command`; aquí no se inyectan vehículos, carga
/// ni dinero.
fn prepare_sea(state: &mut GameState) {
    assert_eq!(state.map.dimensions(), (MAP_SIZE, MAP_SIZE));
    for y in 12..=14 {
        for x in 4..=57 {
            openttdrs_core::map::make_water_tile(
                &mut state.map,
                TileCoord::new(x, y),
                openttdrs_core::map::WaterClass::Sea,
            )
            .expect("franja de mar V1");
        }
    }

    for land in [SOURCE_DOCK, DESTINATION_DOCK] {
        assert_eq!(state.map.get_kind(land), Some(TileKind::Grass));
        set_dock_land_slope(&mut state.map, land, 1, 1);
    }
}

/// Reproduce la pendiente costera nativa que necesita `PlaceDock` para una
/// entrada SW (`dir = 1`).
fn set_dock_land_slope(map: &mut openttdrs_core::Map, land: TileCoord, dir: u8, base: u8) {
    for corner in [
        TileCoord::new(land.x, land.y),
        TileCoord::new(land.x + 1, land.y),
        TileCoord::new(land.x, land.y + 1),
        TileCoord::new(land.x + 1, land.y + 1),
    ] {
        map.set_height(corner, base).expect("altura de costa");
    }
    let high = match openttdrs_core::map::opposite_diag_dir(dir) {
        0 => [
            TileCoord::new(land.x, land.y),
            TileCoord::new(land.x, land.y + 1),
        ],
        1 => [
            TileCoord::new(land.x, land.y + 1),
            TileCoord::new(land.x + 1, land.y + 1),
        ],
        2 => [
            TileCoord::new(land.x + 1, land.y),
            TileCoord::new(land.x + 1, land.y + 1),
        ],
        3 => [
            TileCoord::new(land.x, land.y),
            TileCoord::new(land.x + 1, land.y),
        ],
        _ => unreachable!("opposite_diag_dir devuelve una dirección válida"),
    };
    for corner in high {
        map.set_height(corner, base + 1)
            .expect("arista elevada de costa");
    }
}

fn ship_command_log() -> Vec<Command> {
    vec![
        Command::PlaceShipDepotDir(DEPOT, 2),
        Command::PlaceDock(SOURCE_DOCK, 1),
        Command::PlaceDock(DESTINATION_DOCK, 1),
        Command::PlaceBuoy(BUOY),
        Command::BuildVehicleAtDepot(DEPOT, openttdrs_core::ENGINE_SHIP_COAL),
        Command::SetVehicleOrderList(
            SHIP_ID,
            vec![
                VehicleOrder::station_with_types(
                    SOURCE_DOCK,
                    OrderLoadType::FullLoad,
                    OrderUnloadType::NoUnload,
                    OrderNonStop::NonStopDestination,
                ),
                VehicleOrder::waypoint(BUOY),
                VehicleOrder::station_with_types(
                    DESTINATION_DOCK,
                    OrderLoadType::NoLoad,
                    OrderUnloadType::UnloadIfPossible,
                    OrderNonStop::NonStopDestination,
                ),
            ],
        ),
        Command::ToggleVehicleRunning(SHIP_ID),
    ]
}

fn configured_ship_route() -> GameState {
    let mut state = build_scenario("first_route").expect("fixture Temperate fijo");
    assert!(state.vehicles.is_empty(), "el fixture no inyecta vehículos");
    assert!(
        state.stations.is_empty(),
        "el fixture no inyecta estaciones"
    );
    prepare_sea(&mut state);
    for command in ship_command_log() {
        apply_command(&mut state, &command).expect("comando público de la ruta naval");
    }
    assert_route_assets(&state);
    state
}

fn ship(state: &GameState) -> &openttdrs_core::Vehicle {
    state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == SHIP_ID)
        .expect("barco de carbón creado por comando")
}

fn assert_route_assets(state: &GameState) {
    let vessel = ship(state);
    assert_eq!(vessel.kind, VehicleKind::Ship);
    assert_eq!(vessel.cargo_type, Some(CargoType::Coal));
    assert_eq!(
        vessel.orders.len(),
        3,
        "la ruta tiene tres órdenes públicas"
    );
    assert_eq!(vessel.orders[0].destination(), SOURCE_DOCK);
    assert!(matches!(
        vessel.orders[1],
        VehicleOrder::Waypoint { waypoint, .. } if waypoint == BUOY
    ));
    assert_eq!(vessel.orders[2].destination(), DESTINATION_DOCK);
    assert_eq!(state.map.get_kind(DEPOT), Some(TileKind::ShipDepot));
    for dock in [SOURCE_DOCK, DESTINATION_DOCK] {
        assert_eq!(state.map.get_kind(dock), Some(TileKind::Station));
        assert!(
            state
                .stations
                .iter()
                .any(|station| station.pos == dock && station.stop_kind == StopKind::Dock)
        );
    }
    assert!(
        state
            .stations
            .iter()
            .any(|station| station.pos == BUOY && station.stop_kind == StopKind::Buoy)
    );
}

fn coal_mine(state: &GameState) -> &openttdrs_core::Industry {
    state
        .industries
        .iter()
        .find(|industry| industry.pos == FIRST_ROUTE_COAL_MINE)
        .expect("mina de carbón del fixture")
}

fn coal_waiting(state: &GameState) -> u64 {
    u64::from(coal_mine(state).stock)
        + state
            .stations
            .iter()
            .map(|station| u64::from(station.cargo_stock.get(CargoType::Coal)))
            .sum::<u64>()
}

fn coal_onboard(state: &GameState) -> u64 {
    state
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.cargo_packets.packets.iter())
        .filter(|packet| packet.cargo == CargoType::Coal)
        .map(|packet| u64::from(packet.count))
        .sum()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShipDeliveryReport {
    buoy_tick: usize,
    onboard_at_buoy: u64,
    delivery_tick: usize,
    delivered: u64,
    cargo_income: u64,
    canonical_hash: u64,
}

fn play_ship_route() -> ShipDeliveryReport {
    let mut state = configured_ship_route();
    let initial_stock = u64::from(coal_mine(&state).stock);
    let initial_produced = coal_mine(&state).produced_total;
    let mut buoy_trace = None;
    let mut delivery_trace = None;

    for elapsed in 1..=MAX_ROUTE_TICKS {
        let before = ship(&state);
        let order_before = before.current_order;
        let position_before = before.pos;
        let cargo_before = coal_onboard(&state);
        let delivered_before = state.stats.cargo_units_final_delivered;
        state.step();

        let after = ship(&state);
        if buoy_trace.is_none() && order_before == 1 && after.current_order == 2 {
            assert!(
                position_before == BUOY || after.pos == BUOY,
                "la transición a destino sólo puede ocurrir al visitar la boya"
            );
            assert!(
                cargo_before > 0 || coal_onboard(&state) > 0,
                "el barco debe conservar carbón a bordo al pasar la boya"
            );
            let buoy = state
                .stations
                .iter()
                .find(|station| station.pos == BUOY)
                .expect("boya creada por comando");
            assert_eq!(buoy.stop_kind, StopKind::Buoy);
            assert_eq!(
                buoy.cargo_stock.get(CargoType::Coal),
                0,
                "la boya no debe descargar ni acumular carbón"
            );
            buoy_trace = Some((elapsed, coal_onboard(&state)));
        }

        let delivered_delta = state
            .stats
            .cargo_units_final_delivered
            .saturating_sub(delivered_before);
        if delivered_delta > 0 {
            assert!(
                buoy_trace.is_some(),
                "la entrega debe pasar antes por la boya"
            );
            assert_eq!(
                after.current_order, 2,
                "la entrega se produce al cumplir la orden del muelle de destino"
            );
            delivery_trace = Some((elapsed, delivered_delta));
            break;
        }
    }

    let (buoy_tick, onboard_at_buoy) =
        buoy_trace.expect("el barco visita la boya antes del límite");
    let (delivery_tick, delivered) =
        delivery_trace.expect("el barco entrega carbón pagado antes del límite");
    assert!(
        state.industries.iter().any(|industry| {
            industry.pos == FIRST_ROUTE_POWER_STATION
                && industry.was_cargo_delivered
                && industry.last_accepted_date(CargoType::Coal) > 0
        }),
        "la central debe aceptar físicamente el carbón del barco"
    );

    let produced = coal_mine(&state)
        .produced_total
        .saturating_sub(initial_produced);
    let waiting = coal_waiting(&state);
    let onboard = coal_onboard(&state);
    assert_eq!(
        initial_stock.saturating_add(produced),
        waiting.saturating_add(onboard).saturating_add(delivered),
        "balance V1: stock inicial + producción = espera + a bordo + entrega final"
    );
    assert_eq!(
        state.companies[state.active_company.index()]
            .cargo_units_delivered_by_type
            .units_for(CargoType::Coal),
        delivered,
        "la entrega física acredita exactamente carbón a la compañía"
    );
    assert!(
        state.stats.cargo_income_earned > 0,
        "la primera entrega final debe ser pagada"
    );

    ShipDeliveryReport {
        buoy_tick,
        onboard_at_buoy,
        delivery_tick,
        delivered,
        cargo_income: state.stats.cargo_income_earned,
        canonical_hash: state.canonical_hash(),
    }
}

#[test]
fn v1_ship_delivers_paid_coal_via_buoy_deterministically() {
    let first = play_ship_route();
    println!("V1-SHIP first delivery: {first:?}");
    assert!(first.buoy_tick < first.delivery_tick);
    assert!(first.onboard_at_buoy > 0);
    assert!(first.delivered > 0);
    assert!(first.cargo_income > 0);
    assert!(first.delivery_tick <= MAX_ROUTE_TICKS);

    let second = play_ship_route();
    assert_eq!(
        first, second,
        "dos ejecuciones del mismo log deben conservar eventos y hash canónico"
    );
}
