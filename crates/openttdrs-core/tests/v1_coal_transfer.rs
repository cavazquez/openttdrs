//! V1 #587: transferencia real de carbón por dos camiones y tres paradas.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::parity::{
    FIRST_ROUTE_DELIVER_STOP, FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION,
    FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_ROAD_END_X, FIRST_ROUTE_ROAD_START_X, FIRST_ROUTE_ROAD_Y,
    build_scenario,
};
use openttdrs_core::{
    CargoType, Command, GameState, OrderLoadType, OrderNonStop, OrderUnloadType, TileCoord,
    VehicleKind, VehicleOrder, apply_command, cargo_packet::CargoPacket, company::feeder_share_of,
};

const TRANSFER_STOP: TileCoord = TileCoord::new(32, 11);
const FIRST_TRUCK_ID: u32 = 1;
const SECOND_TRUCK_ID: u32 = 2;
const MAX_ROUTE_TICKS: usize = 40_000;
const V1_INFLATION_PAYMENT: u64 = 1 << 16;
const NATIVE_ORACLE_TRACE: &str = include_str!("fixtures/parity/coal_transfer_15_3.tsv");

fn transfer_command_log() -> Vec<Command> {
    let mut commands = Vec::new();
    for x in FIRST_ROUTE_ROAD_START_X..=FIRST_ROUTE_ROAD_END_X {
        commands.push(Command::PlaceRoadBits(
            TileCoord::new(x, FIRST_ROUTE_ROAD_Y),
            0x0A,
        ));
    }
    commands.extend([
        Command::PlaceRoadDepotDir(FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION),
        Command::PlaceTruckStop(FIRST_ROUTE_LOAD_STOP, 1),
        Command::PlaceTruckStop(TRANSFER_STOP, 1),
        Command::PlaceTruckStop(FIRST_ROUTE_DELIVER_STOP, 1),
        Command::BuildRoadVehicleAtDepot(FIRST_ROUTE_DEPOT, VehicleKind::Truck),
        Command::BuildRoadVehicleAtDepot(FIRST_ROUTE_DEPOT, VehicleKind::Truck),
        Command::RefitVehicle {
            vehicle_id: FIRST_TRUCK_ID,
            cargo: CargoType::Coal,
            unit_ids: Vec::new(),
        },
        Command::RefitVehicle {
            vehicle_id: SECOND_TRUCK_ID,
            cargo: CargoType::Coal,
            unit_ids: Vec::new(),
        },
        Command::SetVehicleOrderList(
            FIRST_TRUCK_ID,
            vec![
                VehicleOrder::station_with_types(
                    FIRST_ROUTE_LOAD_STOP,
                    OrderLoadType::FullLoad,
                    OrderUnloadType::NoUnload,
                    OrderNonStop::NonStopDestination,
                ),
                VehicleOrder::station_with_types(
                    TRANSFER_STOP,
                    OrderLoadType::NoLoad,
                    OrderUnloadType::Transfer,
                    OrderNonStop::NonStopDestination,
                ),
            ],
        ),
        Command::SetVehicleOrderList(
            SECOND_TRUCK_ID,
            vec![
                VehicleOrder::station_with_types(
                    TRANSFER_STOP,
                    OrderLoadType::LoadIfPossible,
                    OrderUnloadType::NoUnload,
                    OrderNonStop::NonStopDestination,
                ),
                VehicleOrder::station_with_types(
                    FIRST_ROUTE_DELIVER_STOP,
                    OrderLoadType::NoLoad,
                    OrderUnloadType::UnloadIfPossible,
                    OrderNonStop::NonStopDestination,
                ),
            ],
        ),
        Command::ToggleVehicleRunning(FIRST_TRUCK_ID),
        Command::ToggleVehicleRunning(SECOND_TRUCK_ID),
    ]);
    commands
}

fn configured_transfer_route() -> GameState {
    let mut state = build_scenario("first_route").expect("escenario Temperate fijo");
    assert!(state.vehicles.is_empty(), "el fixture no inyecta vehículos");
    assert!(state.stations.is_empty(), "el fixture no inyecta paradas");
    // El contrato V1-PKT fija la misma inflación 16.16 del oracle antes de
    // aplicar el log público. Congelarla evita que un rollover mensual cambie
    // el pago entre los dos tramos durante la ejecución.
    state.global_economy.inflation_payment = V1_INFLATION_PAYMENT;
    state.global_economy.inflation_enabled = false;
    for command in transfer_command_log() {
        apply_command(&mut state, &command).expect("comando público de la ruta de transferencia");
    }
    state
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleSlice {
    units: u16,
    distance: u32,
    transit_periods: u16,
    income: i64,
    feeder_share: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransferLedgerTrace {
    transfer_tick: usize,
    transfer: OracleSlice,
    final_tick: usize,
    final_slices: Vec<OracleSlice>,
    final_income: i64,
    final_feeder: i64,
    final_deliverer: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoalTransferReport {
    ticks_to_final_delivery: usize,
    initial_stock: u64,
    produced: u64,
    waiting: u64,
    onboard: u64,
    delivered: u64,
    canonical_hash: u64,
    trace: TransferLedgerTrace,
}

#[derive(Debug, Clone, Copy)]
struct NativeOracleCase {
    units: u16,
    distance: u32,
    transit_periods: u16,
    income: i64,
}

fn native_oracle_case(phase: &str) -> NativeOracleCase {
    let mut lines = NATIVE_ORACLE_TRACE.lines();
    assert_eq!(
        lines.next(),
        Some("phase\tcargo\tcount\tdistance\ttransit_days\tincome"),
        "encabezado de la traza nativa V1-COAL-TRANSFER"
    );
    let rows = lines.collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        2,
        "la traza acotada debe conservar sus dos tramos nativos"
    );
    let matches = rows
        .iter()
        .filter(|row| row.split('\t').next() == Some(phase))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "la traza debe tener un único tramo nativo {phase}"
    );
    let columns = matches[0].split('\t').collect::<Vec<_>>();
    assert_eq!(columns.len(), 6, "fila V1-COAL-TRANSFER completa");
    assert_eq!(columns[0], phase);
    assert_eq!(columns[1], "COAL");
    NativeOracleCase {
        units: columns[2].parse().expect("unidades nativas"),
        distance: columns[3].parse().expect("distancia nativa"),
        transit_periods: columns[4].parse().expect("edad nativa"),
        income: columns[5].parse().expect("pago nativo"),
    }
}

fn truck(state: &GameState, id: u32) -> &openttdrs_core::Vehicle {
    state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == id)
        .unwrap_or_else(|| panic!("camión {id} creado por comando"))
}

fn coal_mine(state: &GameState) -> &openttdrs_core::Industry {
    state
        .industries
        .iter()
        .find(|industry| industry.output_cargo() == CargoType::Coal)
        .expect("mina de carbón Temperate")
}

fn oracle_slice(
    packet: &CargoPacket,
    at: TileCoord,
    expected: NativeOracleCase,
    feeder_share: i64,
) -> OracleSlice {
    assert_eq!(
        packet.cargo,
        CargoType::Coal,
        "la traza sólo certifica carbón"
    );
    assert_eq!(
        packet.count, expected.units,
        "cantidad de la traza debe coincidir con el oracle nativo"
    );
    assert_eq!(
        packet.get_distance(at),
        expected.distance,
        "distancia de la traza debe coincidir con el oracle nativo"
    );
    assert_eq!(
        packet.periods_in_transit, expected.transit_periods,
        "edad de la traza debe coincidir con el oracle nativo"
    );
    OracleSlice {
        units: packet.count,
        distance: packet.get_distance(at),
        transit_periods: packet.periods_in_transit,
        income: expected.income,
        feeder_share,
    }
}

fn leading_portions(packets: &[CargoPacket], units: u32) -> Vec<CargoPacket> {
    let mut remaining = units;
    let mut portions = Vec::new();
    for packet in packets {
        if remaining == 0 {
            break;
        }
        let count = remaining.min(u32::from(packet.count));
        let mut portion = packet.clone();
        if count < u32::from(packet.count) {
            let count = u16::try_from(count).expect("porción de packet de carbón");
            portion.count = count;
            portion.feeder_share = packet.feeder_share_of(count);
        }
        remaining -= u32::from(portion.count);
        portions.push(portion);
    }
    assert_eq!(
        remaining, 0,
        "la descarga final no puede exceder los packets de carbón que llevaba el segundo camión"
    );
    portions
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

fn play_transfer_route() -> CoalTransferReport {
    let mut state = configured_transfer_route();
    let initial_stock = u64::from(coal_mine(&state).stock);
    let initial_produced = coal_mine(&state).produced_total;
    let mut transfer_trace = None;
    let mut final_trace = None;

    for elapsed in 1..=MAX_ROUTE_TICKS {
        let income_before = state.stats.cargo_income_earned;
        let final_before = state.stats.cargo_units_final_delivered;
        let second_before = truck(&state, SECOND_TRUCK_ID);
        let second_order_before = second_before.current_order;
        let second_profit_before = second_before.profit_this_year;
        let second_packets_before = second_before
            .cargo_packets
            .packets
            .iter()
            .filter(|packet| packet.cargo == CargoType::Coal)
            .cloned()
            .collect::<Vec<_>>();
        state.step();

        if transfer_trace.is_none()
            && let Some(packet) = state
                .stations
                .iter()
                .find(|station| station.pos == TRANSFER_STOP)
                .and_then(|station| {
                    station.cargo_packets.packets().find(|packet| {
                        packet.cargo == CargoType::Coal
                            && packet.first_station == Some(FIRST_ROUTE_LOAD_STOP)
                            && packet.feeder_share > 0
                    })
                })
        {
            let expected = native_oracle_case("transfer");
            let expected_share = feeder_share_of(expected.income);
            let trace = oracle_slice(packet, TRANSFER_STOP, expected, expected_share);
            assert_eq!(
                packet.feeder_share, expected_share,
                "el packet transferido debe conservar el feeder share del oracle"
            );
            assert_eq!(
                state.stats.cargo_income_earned, income_before,
                "un transfer no puede acreditar una segunda entrega final"
            );
            let payment = state
                .cargo_payments
                .iter()
                .find(|payment| payment.front_vehicle_id == Some(FIRST_TRUCK_ID))
                .expect("ledger temporal del primer camión");
            assert_eq!(payment.route_profit, 0, "transfer no liquida route_profit");
            assert_eq!(
                payment.visual_profit, 0,
                "transfer no liquida visual_profit"
            );
            assert_eq!(payment.visual_transfer, expected_share);
            transfer_trace = Some((elapsed, trace));
        }

        let final_delta = state
            .stats
            .cargo_units_final_delivered
            .saturating_sub(final_before);
        if final_trace.is_none() && final_delta > 0 {
            let expected = native_oracle_case("final");
            assert_eq!(
                final_delta,
                u64::from(expected.units),
                "la entrega final debe liquidar exactamente el tramo nativo"
            );
            assert_eq!(
                second_order_before, 1,
                "la entrega final debe salir de la segunda orden del segundo camión"
            );
            assert!(
                !second_packets_before.is_empty(),
                "el segundo camión debe llevar el packet transferido antes de entregar"
            );
            let portions = leading_portions(
                &second_packets_before,
                u32::try_from(final_delta).expect("unidades finales"),
            );
            assert_eq!(
                portions.len(),
                1,
                "el contrato V1 fija un único packet de carbón en la entrega final"
            );
            let slices = portions
                .iter()
                .map(|packet| {
                    oracle_slice(
                        packet,
                        FIRST_ROUTE_DELIVER_STOP,
                        expected,
                        packet.feeder_share,
                    )
                })
                .collect::<Vec<_>>();
            let final_income = slices.iter().map(|slice| slice.income).sum::<i64>();
            let final_feeder = slices.iter().map(|slice| slice.feeder_share).sum::<i64>();
            let final_deliverer = final_income.saturating_sub(final_feeder);
            let payment = state
                .cargo_payments
                .iter()
                .find(|payment| payment.front_vehicle_id == Some(SECOND_TRUCK_ID))
                .expect("ledger temporal del segundo camión");
            assert_eq!(
                state.stats.cargo_income_earned - income_before,
                u64::try_from(final_income).expect("ingreso final no negativo"),
                "la liquidación final debe acreditar una única vez el bruto del oracle"
            );
            assert_eq!(payment.route_profit, final_income);
            assert_eq!(payment.visual_profit, final_deliverer);
            assert_eq!(payment.visual_transfer, 0);
            assert_eq!(
                truck(&state, SECOND_TRUCK_ID).profit_this_year - second_profit_before,
                final_deliverer,
                "el entregador recibe bruto menos feeder, sin segundo cobro"
            );
            assert!(
                final_feeder > 0,
                "la entrega final debe liquidar feeder_share"
            );
            final_trace = Some((elapsed, slices, final_income, final_feeder, final_deliverer));
        }

        if transfer_trace.is_some() && final_trace.is_some() {
            break;
        }
    }

    let (transfer_tick, transfer) = transfer_trace.expect("transfer de carbón antes del límite");
    let (final_tick, final_slices, final_income, final_feeder, final_deliverer) =
        final_trace.expect("entrega final pagada antes del límite");
    assert!(
        state.industries.iter().any(|industry| {
            industry.pos == openttdrs_core::parity::FIRST_ROUTE_POWER_STATION
                && industry.was_cargo_delivered
                && industry.last_accepted_date(CargoType::Coal) > 0
        }),
        "la central debe aceptar el carbón entregado por el segundo tramo"
    );

    let produced = coal_mine(&state)
        .produced_total
        .saturating_sub(initial_produced);
    let waiting = coal_waiting(&state);
    let onboard = coal_onboard(&state);
    let delivered = state.stats.cargo_units_final_delivered;
    assert_eq!(
        initial_stock.saturating_add(produced),
        waiting.saturating_add(onboard).saturating_add(delivered),
        "balance V1 de carbón: producido + stock inicial = esperando + a bordo + entrega final"
    );

    CoalTransferReport {
        ticks_to_final_delivery: final_tick,
        initial_stock,
        produced,
        waiting,
        onboard,
        delivered,
        canonical_hash: state.canonical_hash(),
        trace: TransferLedgerTrace {
            transfer_tick,
            transfer,
            final_tick,
            final_slices,
            final_income,
            final_feeder,
            final_deliverer,
        },
    }
}

#[test]
fn v1_two_truck_coal_transfer_conserves_cargo_and_matches_ledger_twice() {
    let first = play_transfer_route();
    let transfer_oracle = native_oracle_case("transfer");
    let final_oracle = native_oracle_case("final");
    assert!(
        first.ticks_to_final_delivery < MAX_ROUTE_TICKS,
        "la entrega final debe ocurrir antes de {MAX_ROUTE_TICKS} ticks"
    );
    assert!(
        first.delivered > 0,
        "debe existir una entrega final de carbón"
    );
    assert_eq!(first.trace.transfer.income, transfer_oracle.income);
    assert_eq!(
        first.trace.transfer.feeder_share,
        feeder_share_of(transfer_oracle.income),
        "el primer tramo aplica el porcentaje feeder fijo del contrato"
    );
    assert_eq!(first.trace.final_income, final_oracle.income);
    assert_eq!(
        first.trace.final_feeder, first.trace.transfer.feeder_share,
        "el único feeder acumulado se liquida una vez al entregar"
    );
    assert_eq!(
        first.trace.final_income,
        first.trace.final_deliverer + first.trace.final_feeder,
        "el ledger final se descompone exactamente en entregador + feeder"
    );

    let second = play_transfer_route();
    assert_eq!(
        first, second,
        "dos ejecuciones del mismo log deben conservar la misma traza y hash canónico"
    );
}

#[test]
fn v1_raw_primary_coal_cannot_bypass_the_transfer_provenance_guard() {
    let mut state = configured_transfer_route();
    let transfer = state
        .stations
        .iter_mut()
        .find(|station| station.pos == TRANSFER_STOP)
        .expect("parada de transferencia");
    transfer.add_waiting_cargo(CargoType::Coal, 4);
    assert!(
        transfer
            .cargo_packets
            .packets()
            .all(|packet| packet.first_station == Some(TRANSFER_STOP)),
        "la carga primaria de este control no tiene procedencia de otro tramo"
    );

    for _ in 0..1_000 {
        state.step();
    }

    assert_eq!(
        truck(&state, SECOND_TRUCK_ID).cargo,
        0,
        "un camión no puede recoger carbón primario de una parada intermedia"
    );
    assert_eq!(
        state
            .stations
            .iter()
            .find(|station| station.pos == TRANSFER_STOP)
            .expect("parada de transferencia")
            .cargo_stock
            .get(CargoType::Coal),
        4,
        "el paquete primario debe seguir esperando hasta una recogida válida"
    );
    assert_eq!(state.stats.cargo_units_final_delivered, 0);
}
