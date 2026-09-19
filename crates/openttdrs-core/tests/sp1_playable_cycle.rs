//! SP1: ruta de carbón construida y operada exclusivamente mediante comandos públicos.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::parity::{
    FIRST_ROUTE_DELIVER_STOP, FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION,
    FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_POWER_STATION, FIRST_ROUTE_ROAD_END_X,
    FIRST_ROUTE_ROAD_START_X, FIRST_ROUTE_ROAD_Y, FIRST_ROUTE_VEHICLE_ID, build_scenario,
};
use openttdrs_core::{
    CargoType, Command, GameState, IndustrySpec, VehicleKind, VehicleOrder, apply_command,
};

const MAX_ROUTE_TICKS: usize = 40_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FirstRouteReport {
    vehicle_id: u32,
    order_destinations: [openttdrs_core::TileCoord; 2],
    loaded: u64,
    delivered: u64,
    income: u64,
    power_station_received_coal: bool,
    ticks_to_first_delivery: usize,
    canonical_hash: u64,
}

fn first_route_command_log() -> Vec<Command> {
    let mut commands = Vec::new();
    for x in FIRST_ROUTE_ROAD_START_X..=FIRST_ROUTE_ROAD_END_X {
        commands.push(Command::PlaceRoadBits(
            openttdrs_core::TileCoord::new(x, FIRST_ROUTE_ROAD_Y),
            0x0A,
        ));
    }
    commands.extend([
        Command::PlaceRoadDepotDir(FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION),
        Command::PlaceTruckStop(FIRST_ROUTE_LOAD_STOP, 1),
        Command::PlaceTruckStop(FIRST_ROUTE_DELIVER_STOP, 1),
        Command::BuildRoadVehicleAtDepot(FIRST_ROUTE_DEPOT, VehicleKind::Truck),
        Command::RefitVehicle {
            vehicle_id: FIRST_ROUTE_VEHICLE_ID,
            cargo: CargoType::Coal,
            unit_ids: Vec::new(),
        },
        Command::SetVehicleOrderList(
            FIRST_ROUTE_VEHICLE_ID,
            vec![
                VehicleOrder::station_with_flags(FIRST_ROUTE_LOAD_STOP, true, false),
                VehicleOrder::station(FIRST_ROUTE_DELIVER_STOP),
            ],
        ),
        Command::ToggleVehicleRunning(FIRST_ROUTE_VEHICLE_ID),
    ]);
    commands
}

fn play_first_route() -> FirstRouteReport {
    let mut state = build_scenario("first_route").expect("escenario first_route");
    assert!(state.vehicles.is_empty(), "el fixture no inyecta vehículos");
    assert!(state.stations.is_empty(), "el fixture no inyecta paradas");

    for command in first_route_command_log() {
        apply_command(&mut state, &command).expect("comando de primera ruta");
    }

    let vehicle = state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == FIRST_ROUTE_VEHICLE_ID)
        .expect("camión comprado por comando");
    assert_eq!(vehicle.kind, VehicleKind::Truck);
    assert_eq!(vehicle.cargo_type, Some(CargoType::Coal));
    assert!(vehicle.running);
    assert_eq!(vehicle.orders.len(), 2);
    let order_destinations = [
        vehicle.orders[0].destination(),
        vehicle.orders[1].destination(),
    ];

    let mut ticks_to_first_delivery = MAX_ROUTE_TICKS;
    for elapsed in 1..MAX_ROUTE_TICKS {
        state.step();
        if state.stats.cargo_units_delivered > 0 {
            ticks_to_first_delivery = elapsed;
            break;
        }
    }

    let power_station_received_coal = state.industries.iter().any(|industry| {
        industry.spec == Some(IndustrySpec::PowerStation)
            && industry.pos == FIRST_ROUTE_POWER_STATION
            && industry.was_cargo_delivered
            && industry.last_accepted_date(CargoType::Coal) > 0
    });
    let canonical_hash = state.canonical_hash();
    let roundtripped: GameState = serde_json::from_value(
        serde_json::to_value(&state).expect("estado con entrega serializable"),
    )
    .expect("estado con entrega deserializable");
    assert_eq!(
        roundtripped.canonical_hash(),
        canonical_hash,
        "el estado persistido tras la entrega conserva su hash canónico"
    );

    FirstRouteReport {
        vehicle_id: FIRST_ROUTE_VEHICLE_ID,
        order_destinations,
        loaded: state.stats.cargo_units_loaded,
        delivered: state.stats.cargo_units_delivered,
        income: state.stats.cargo_income_earned,
        power_station_received_coal,
        ticks_to_first_delivery,
        canonical_hash,
    }
}

#[test]
fn sp1_truck_loads_coal_delivers_to_power_station_and_is_deterministic() {
    let first = play_first_route();

    assert_eq!(first.vehicle_id, FIRST_ROUTE_VEHICLE_ID);
    assert_eq!(
        first.order_destinations,
        [FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_DELIVER_STOP],
        "el camión conserva las dos órdenes configuradas por comando"
    );
    assert!(
        first.loaded > 0,
        "el camión cargó carbón producido por la mina"
    );
    assert!(
        first.delivered > 0,
        "la central recibió una entrega antes del límite de ticks"
    );
    assert!(first.income > 0, "la entrega generó un pago real");
    assert!(
        first.power_station_received_coal,
        "la central eléctrica aceptó carbón de la ruta"
    );
    assert!(
        first.ticks_to_first_delivery < MAX_ROUTE_TICKS,
        "la primera entrega debe ocurrir antes de {MAX_ROUTE_TICKS} ticks"
    );

    let second = play_first_route();
    assert_eq!(
        first.canonical_hash, second.canonical_hash,
        "el mismo log de comandos debe producir el mismo estado canónico"
    );
    assert_eq!(
        first, second,
        "el reporte de dos ejecuciones debe coincidir"
    );
}
