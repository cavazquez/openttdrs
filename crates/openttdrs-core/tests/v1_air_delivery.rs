//! V1 #594: primera entrega pagada de pasajeros entre aeropuertos Country.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::{
    AirportSpecId, CargoType, Climate, Command, GameState, OrderLoadType, OrderNonStop,
    OrderUnloadType, SimEvent, TileCoord, VehicleKind, VehicleOrder, apply_command,
    station_uses_country_fta,
};

const MAP_SIZE: u32 = 64;
const WORLD_SEED: u64 = 0x5940_0064;
const MAX_DELIVERY_TICKS: usize = 40_000;
const PASSENGER_DEMAND: u32 = 50;
const AIRCRAFT_ID: u32 = 1;
const SOURCE_AIRPORT: TileCoord = TileCoord::new(4, 4);
const DESTINATION_AIRPORT: TileCoord = TileCoord::new(42, 42);

#[derive(Debug, Clone, PartialEq, Eq)]
enum AirEventKind {
    FtaEnter,
    FtaLeave,
    Takeoff,
    Landing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AirEvent {
    tick: u64,
    kind: AirEventKind,
    at: TileCoord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AirDeliveryReport {
    events: Vec<AirEvent>,
    first_delivery_tick: usize,
    delivered: u64,
    cargo_income: u64,
    canonical_hash: u64,
}

fn airport_command_log() -> Vec<Command> {
    vec![
        Command::PlaceAirportArea {
            origin: SOURCE_AIRPORT,
            axis_y: false,
            spec: AirportSpecId::Small,
        },
        Command::PlaceAirportArea {
            origin: DESTINATION_AIRPORT,
            axis_y: false,
            spec: AirportSpecId::Small,
        },
    ]
}

fn aircraft_command_log(source: TileCoord, destination: TileCoord) -> Vec<Command> {
    vec![
        Command::BuildVehicleAtDepot(source, openttdrs_core::ENGINE_AIRCRAFT_DAKOTA),
        Command::SetVehicleOrderList(
            AIRCRAFT_ID,
            vec![
                VehicleOrder::station_with_types(
                    source,
                    OrderLoadType::FullLoad,
                    OrderUnloadType::NoUnload,
                    OrderNonStop::NonStopDestination,
                ),
                VehicleOrder::station_with_types(
                    destination,
                    OrderLoadType::NoLoad,
                    OrderUnloadType::UnloadIfPossible,
                    OrderNonStop::NonStopDestination,
                ),
            ],
        ),
        Command::ToggleVehicleRunning(AIRCRAFT_ID),
    ]
}

fn configured_air_route() -> GameState {
    let mut state = GameState::new(MAP_SIZE, MAP_SIZE);
    state.climate = Climate::Temperate;
    state.world_seed = WORLD_SEED;
    state.tick = openttdrs_core::news::tick_for_calendar_year(1950);
    state.sync_timers_from_tick();
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;

    for command in airport_command_log() {
        apply_command(&mut state, &command).expect("comando público de aeropuerto Country");
    }
    let source_airport = airport_anchor(&state, SOURCE_AIRPORT);
    let destination_airport = airport_anchor(&state, DESTINATION_AIRPORT);
    let source = state
        .stations
        .iter_mut()
        .find(|station| station.pos == source_airport)
        .expect("aeropuerto de origen creado por comando");
    source.add_waiting_cargo(CargoType::Passengers, PASSENGER_DEMAND);

    for command in aircraft_command_log(source_airport, destination_airport) {
        apply_command(&mut state, &command).expect("comando público de servicio aéreo");
    }
    assert_route_assets(&state);
    state
}

fn aircraft(state: &GameState) -> &openttdrs_core::Vehicle {
    state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == AIRCRAFT_ID)
        .expect("Dakota creado por comando")
}

fn airport_anchor(state: &GameState, origin: TileCoord) -> TileCoord {
    state
        .stations
        .iter()
        .find(|station| station.covers_tile(origin))
        .map(|station| station.pos)
        .expect("aeropuerto Country creado por comando")
}

fn assert_route_assets(state: &GameState) {
    assert_eq!(state.map.dimensions(), (MAP_SIZE, MAP_SIZE));
    assert_eq!(state.climate, Climate::Temperate);
    assert_eq!(state.world_seed, WORLD_SEED);
    assert!(!state.disasters_enabled);
    assert_eq!(state.vehicle_breakdowns, 0);
    for origin in [SOURCE_AIRPORT, DESTINATION_AIRPORT] {
        let station = state
            .stations
            .iter()
            .find(|station| station.covers_tile(origin))
            .expect("aeropuerto Country creado por comando");
        assert_eq!(station.airport_spec, AirportSpecId::Small);
        assert!(station_uses_country_fta(station));
    }
    let plane = aircraft(state);
    assert_eq!(plane.kind, VehicleKind::Aircraft);
    assert_eq!(plane.cargo_type, Some(CargoType::Passengers));
    assert!(plane.capacity > 0, "la capacidad viene del Dakota vanilla");
    assert!(plane.airport_fta_active, "la compra inicia el FTA Country");
    assert_eq!(plane.orders.len(), 2, "la ruta tiene dos destinos públicos");
    assert_eq!(
        plane.orders[0].destination(),
        airport_anchor(state, SOURCE_AIRPORT)
    );
    assert_eq!(
        plane.orders[1].destination(),
        airport_anchor(state, DESTINATION_AIRPORT)
    );
    assert_eq!(
        state
            .stations
            .iter()
            .find(|station| station.pos == airport_anchor(state, SOURCE_AIRPORT))
            .expect("origen")
            .cargo_stock
            .get(CargoType::Passengers),
        PASSENGER_DEMAND,
        "la demanda se fija antes de iniciar la operación"
    );
}

fn passenger_waiting(state: &GameState) -> u64 {
    state
        .stations
        .iter()
        .map(|station| u64::from(station.cargo_stock.get(CargoType::Passengers)))
        .sum()
}

fn passenger_onboard(state: &GameState) -> u64 {
    state
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.cargo_packets.packets.iter())
        .filter(|packet| packet.cargo == CargoType::Passengers)
        .map(|packet| u64::from(packet.count))
        .sum()
}

/// El bloque de una estación pertenece exactamente a los aviones que lo
/// declaran en su FTA actual; cualquier bit restante sería una reserva huérfana.
fn assert_fta_block_ownership(state: &GameState) {
    for station in state
        .stations
        .iter()
        .filter(|station| station_uses_country_fta(station))
    {
        let held = state
            .vehicles
            .iter()
            .filter(|vehicle| {
                vehicle.kind == VehicleKind::Aircraft
                    && vehicle.airport_fta_station == Some(station.pos)
            })
            .fold(0_u64, |blocks, vehicle| {
                blocks | vehicle.airport_blocks_held
            });
        assert_eq!(
            station.airport_blocks, held,
            "la reserva FTA de {:?} no puede quedar huérfana",
            station.pos
        );
    }
}

fn play_air_route() -> AirDeliveryReport {
    let mut state = configured_air_route();
    let initial_waiting = passenger_waiting(&state);
    let mut events = Vec::new();
    let mut saw_reserved_blocks = false;
    let mut saw_source_release = false;
    let mut delivery = None;

    for elapsed in 1..=MAX_DELIVERY_TICKS {
        let before = aircraft(&state);
        let before_fta_active = before.airport_fta_active;
        let before_fta_station = before.airport_fta_station;
        let before_pos = before.pos;
        let delivered_before = state.stats.cargo_units_final_delivered;
        state.step();

        let (after_fta_active, after_fta_station, after_pos) = {
            let plane = aircraft(&state);
            (
                plane.airport_fta_active,
                plane.airport_fta_station,
                plane.pos,
            )
        };
        let tick = state.tick.get();
        if before_fta_active != after_fta_active {
            let (kind, at) = if after_fta_active {
                (
                    AirEventKind::FtaEnter,
                    after_fta_station.unwrap_or(after_pos),
                )
            } else {
                (
                    AirEventKind::FtaLeave,
                    before_fta_station.unwrap_or(before_pos),
                )
            };
            events.push(AirEvent { tick, kind, at });
        }
        for event in state.runtime.pending_sim_events.drain() {
            match event {
                SimEvent::AircraftTakeoff { vehicle_id, at, .. } if vehicle_id == AIRCRAFT_ID => {
                    events.push(AirEvent {
                        tick,
                        kind: AirEventKind::Takeoff,
                        at,
                    });
                }
                SimEvent::AircraftLanding { vehicle_id, at, .. } if vehicle_id == AIRCRAFT_ID => {
                    events.push(AirEvent {
                        tick,
                        kind: AirEventKind::Landing,
                        at,
                    });
                }
                SimEvent::AircraftCrash { vehicle_id, .. } if vehicle_id == AIRCRAFT_ID => {
                    panic!("el Dakota no debe estrellarse con crashes deshabilitados");
                }
                _ => {}
            }
        }

        assert_fta_block_ownership(&state);
        saw_reserved_blocks |= state
            .stations
            .iter()
            .filter(|station| station_uses_country_fta(station))
            .any(|station| station.airport_blocks != 0);
        if before_fta_active && !after_fta_active {
            saw_source_release = true;
            assert_eq!(
                state
                    .stations
                    .iter()
                    .find(|station| station.pos == airport_anchor(&state, SOURCE_AIRPORT))
                    .expect("aeropuerto de origen")
                    .airport_blocks,
                0,
                "el despegue libera los bloques Country de origen"
            );
        }

        let delivered_delta = state
            .stats
            .cargo_units_final_delivered
            .saturating_sub(delivered_before);
        if delivered_delta > 0 {
            assert!(
                events
                    .iter()
                    .any(|event| event.kind == AirEventKind::Takeoff),
                "la entrega necesita un despegue FTA"
            );
            assert!(
                events
                    .iter()
                    .any(|event| event.kind == AirEventKind::Landing),
                "la entrega necesita un aterrizaje FTA"
            );
            delivery = Some((elapsed, delivered_delta));
            break;
        }
    }

    let (first_delivery_tick, delivered) =
        delivery.expect("el Dakota debe entregar pasajeros antes del límite");
    assert!(
        saw_reserved_blocks,
        "el FTA debe reservar bloques de aeropuerto"
    );
    assert!(
        saw_source_release,
        "el despegue debe liberar sus bloques FTA"
    );
    assert_eq!(
        initial_waiting,
        passenger_waiting(&state)
            .saturating_add(passenger_onboard(&state))
            .saturating_add(delivered),
        "demanda V1: pasajeros iniciales = espera + a bordo + entrega final"
    );
    assert_eq!(
        state.companies[state.active_company.index()]
            .cargo_units_delivered_by_type
            .units_for(CargoType::Passengers),
        delivered,
        "la entrega física acredita pasajeros a la compañía"
    );
    assert!(
        state.stats.cargo_income_earned > 0,
        "la primera entrega de pasajeros debe ser pagada"
    );

    AirDeliveryReport {
        events,
        first_delivery_tick,
        delivered,
        cargo_income: state.stats.cargo_income_earned,
        canonical_hash: state.canonical_hash(),
    }
}

#[test]
fn v1_country_airport_service_delivers_passengers_deterministically() {
    let first = play_air_route();
    println!("V1-AIR first delivery: {first:?}");
    assert!(first.first_delivery_tick <= MAX_DELIVERY_TICKS);
    assert!(first.delivered > 0);
    assert!(first.cargo_income > 0);
    assert!(
        first
            .events
            .iter()
            .any(|event| event.kind == AirEventKind::FtaEnter)
    );
    assert!(
        first
            .events
            .iter()
            .any(|event| event.kind == AirEventKind::FtaLeave)
    );

    let second = play_air_route();
    assert_eq!(
        first, second,
        "dos ejecuciones del mismo log deben conservar eventos y hash canónico"
    );
}
