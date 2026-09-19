//! SP1: ruta de carbón construida y operada exclusivamente mediante comandos públicos.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::parity::{
    FIRST_ROUTE_DELIVER_STOP, FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION,
    FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_POWER_STATION, FIRST_ROUTE_ROAD_END_X,
    FIRST_ROUTE_ROAD_START_X, FIRST_ROUTE_ROAD_Y, FIRST_ROUTE_VEHICLE_ID, build_scenario,
};
use openttdrs_core::save::{load, save};
use openttdrs_core::{
    CargoStock, CargoType, Command, GameState, IndustrySpec, TileCoord, VehicleKind, VehicleOrder,
    apply_command,
};
use openttdrs_core::{
    cargo_packet::{StationCargoList, VehicleCargoList},
    company::CompanyId,
};

const MAX_ROUTE_TICKS: usize = 40_000;
const JSON_CONTINUATION_TICKS: usize = 2_000;

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

/// Proyección explícita de las piezas que una continuación de ruta no puede
/// perder al cruzar el formato JSON. El hash canónico cubre el estado entero;
/// esta vista hace que una divergencia indique qué parte operativa cambió.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteContinuationSnapshot {
    canonical_hash: u64,
    vehicle: RouteVehicleSnapshot,
    industry_stocks: Vec<IndustryStockSnapshot>,
    station_stocks: Vec<StationStockSnapshot>,
    income: IncomeSnapshot,
    random_state: [u32; 2],
    interactive_random_state: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteVehicleSnapshot {
    id: u32,
    orders: Vec<VehicleOrder>,
    current_order: usize,
    cargo: u32,
    cargo_type: Option<CargoType>,
    cargo_packets: VehicleCargoList,
    cargo_source: Option<TileCoord>,
    cargo_transit_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndustryStockSnapshot {
    pos: TileCoord,
    stock: u32,
    secondary_stock: u32,
    accepted_waiting: CargoStock,
    extra_produced: CargoStock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StationStockSnapshot {
    pos: TileCoord,
    stock: u32,
    cargo_stock: CargoStock,
    cargo_packets: StationCargoList,
    income: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IncomeSnapshot {
    cargo_income_earned: u64,
    cargo_units_delivered: u64,
    active_company_money: i64,
    companies: Vec<(CompanyId, i64, u64, u64)>,
}

impl RouteContinuationSnapshot {
    fn capture(state: &GameState) -> Self {
        let vehicle = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == FIRST_ROUTE_VEHICLE_ID)
            .expect("camión del fixture vial");

        Self {
            canonical_hash: state.canonical_hash(),
            vehicle: RouteVehicleSnapshot {
                id: vehicle.id,
                orders: vehicle.orders.clone(),
                current_order: vehicle.current_order,
                cargo: vehicle.cargo,
                cargo_type: vehicle.cargo_type,
                cargo_packets: vehicle.cargo_packets.clone(),
                cargo_source: vehicle.cargo_source,
                cargo_transit_ticks: vehicle.cargo_transit_ticks,
            },
            industry_stocks: state
                .industries
                .iter()
                .map(|industry| IndustryStockSnapshot {
                    pos: industry.pos,
                    stock: industry.stock,
                    secondary_stock: industry.secondary_stock,
                    accepted_waiting: industry.newgrf_accepted_cargo_waiting,
                    extra_produced: industry.newgrf_extra_produced_cargo,
                })
                .collect(),
            station_stocks: state
                .stations
                .iter()
                .map(|station| StationStockSnapshot {
                    pos: station.pos,
                    stock: station.stock,
                    cargo_stock: station.cargo_stock,
                    cargo_packets: station.cargo_packets.clone(),
                    income: station.income,
                })
                .collect(),
            income: IncomeSnapshot {
                cargo_income_earned: state.stats.cargo_income_earned,
                cargo_units_delivered: state.stats.cargo_units_delivered,
                active_company_money: state.economy.money,
                companies: state
                    .companies
                    .iter()
                    .map(|company| {
                        (
                            company.id,
                            company.economy.money,
                            company.cargo_income_earned,
                            company.cargo_deliveries,
                        )
                    })
                    .collect(),
            },
            random_state: state.random.state,
            interactive_random_state: state.interactive_random.state,
        }
    }

    fn first_difference(&self, other: &Self) -> Option<&'static str> {
        if self.vehicle.id != other.vehicle.id {
            return Some("vehicle.id");
        }
        if self.vehicle.orders != other.vehicle.orders {
            return Some("vehicle.orders");
        }
        if self.vehicle.current_order != other.vehicle.current_order {
            return Some("vehicle.current_order");
        }
        if self.vehicle.cargo != other.vehicle.cargo {
            return Some("vehicle.cargo");
        }
        if self.vehicle.cargo_type != other.vehicle.cargo_type {
            return Some("vehicle.cargo_type");
        }
        if self.vehicle.cargo_packets != other.vehicle.cargo_packets {
            return Some("vehicle.cargo_packets");
        }
        if self.vehicle.cargo_source != other.vehicle.cargo_source {
            return Some("vehicle.cargo_source");
        }
        if self.vehicle.cargo_transit_ticks != other.vehicle.cargo_transit_ticks {
            return Some("vehicle.cargo_transit_ticks");
        }
        if self.industry_stocks != other.industry_stocks {
            return Some("industry_stocks");
        }
        if self.station_stocks != other.station_stocks {
            return Some("station_stocks");
        }
        if self.income != other.income {
            return Some("income");
        }
        if self.random_state != other.random_state {
            return Some("random_state");
        }
        if self.interactive_random_state != other.interactive_random_state {
            return Some("interactive_random_state");
        }
        if self.canonical_hash != other.canonical_hash {
            return Some("canonical_hash");
        }
        None
    }
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

fn configured_first_route() -> GameState {
    let mut state = build_scenario("first_route").expect("escenario first_route");
    assert!(state.vehicles.is_empty(), "el fixture no inyecta vehículos");
    assert!(state.stations.is_empty(), "el fixture no inyecta paradas");

    for command in first_route_command_log() {
        apply_command(&mut state, &command).expect("comando del fixture vial");
    }
    state
}

fn play_first_route() -> FirstRouteReport {
    let mut state = configured_first_route();

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

#[test]
fn sp1_json_save_load_continues_loaded_coal_route_tick_for_tick() {
    let mut uninterrupted = configured_first_route();
    let ticks_to_cargo_onboard = (1..MAX_ROUTE_TICKS)
        .find(|_| {
            uninterrupted.step();
            uninterrupted
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == FIRST_ROUTE_VEHICLE_ID)
                .is_some_and(|vehicle| vehicle.cargo > 0 && !vehicle.cargo_packets.is_empty())
        })
        .expect("la ruta debe tener carbón a bordo antes del límite");
    assert_eq!(
        uninterrupted.stats.cargo_units_delivered, 0,
        "el punto de guardado ocurre antes de la primera entrega pagada"
    );

    let delivered_before_save = uninterrupted.stats.cargo_units_delivered;
    let income_before_save = uninterrupted.stats.cargo_income_earned;
    let directory = tempfile::tempdir().expect("directorio temporal para save JSON");
    let save_path = directory.path().join("first-route-with-coal-onboard.json");
    save(&uninterrupted, &save_path).expect("guardar ruta productiva como JSON versionado");
    let persisted = std::fs::read_to_string(&save_path).expect("leer save JSON versionado");
    assert!(
        persisted.contains("\"version\""),
        "la API productiva debe escribir el envoltorio versionado"
    );
    let mut reloaded = load(&save_path).expect("recargar ruta productiva desde JSON");

    for offset in 1..=JSON_CONTINUATION_TICKS {
        uninterrupted.step();
        reloaded.step();
        let control = RouteContinuationSnapshot::capture(&uninterrupted);
        let restored = RouteContinuationSnapshot::capture(&reloaded);
        if let Some(field) = control.first_difference(&restored) {
            panic!(
                "divergencia tras JSON en tick {} ({} tras guardar, carbón a bordo en {}): {}; hash continuo={:#018x}, hash recargado={:#018x}",
                uninterrupted.tick.get(),
                offset,
                ticks_to_cargo_onboard,
                field,
                control.canonical_hash,
                restored.canonical_hash,
            );
        }
    }

    assert!(
        reloaded.stats.cargo_units_delivered > delivered_before_save,
        "la rama recargada debe completar otra entrega de carbón"
    );
    assert!(
        reloaded.stats.cargo_income_earned > income_before_save,
        "la entrega posterior a recargar debe producir un pago real"
    );
}
