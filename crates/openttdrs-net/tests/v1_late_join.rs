//! Contrato V1-NET: dos peers TCP y un late join sobre una ruta de carbón.
//!
//! El servidor es un [`ListenServer`] headless, la misma autoridad que usa el
//! binario `openttdrs-dedicated`. El test mantiene el transporte real de
//! loopback: no sustituye sockets por mocks ni acepta `PermissionDenied`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::env;
use std::io::ErrorKind;
use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::parity::{
    FIRST_ROUTE_DELIVER_STOP, FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION,
    FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_ROAD_END_X, FIRST_ROUTE_ROAD_START_X, FIRST_ROUTE_ROAD_Y,
    FIRST_ROUTE_VEHICLE_ID, build_scenario,
};
use openttdrs_core::{
    CargoType, Command, CompanyId, GameState, TileCoord, VehicleKind, VehicleOrder, apply_command,
};
use openttdrs_net::{
    ClientSession, ListenServer, NetError, SessionEvent, SessionTimeouts, apply_command_as_company,
    apply_session_event,
};

const TOTAL_TICKS: u64 = 2_000;
const LATE_JOIN_TICK: u64 = 500;
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_PAUSE: Duration = Duration::from_millis(1);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct CommitRecord {
    seq: u64,
    issuer: CompanyId,
    command: Command,
}

#[derive(Debug, Clone, serde::Serialize)]
struct FirstDisagreement {
    session_tick: u64,
    world_tick: u64,
    client: &'static str,
    server_hash: u64,
    client_hash: u64,
    json_difference: String,
}

#[derive(Debug, serde::Serialize)]
struct LateJoinReport {
    source_sha: String,
    ticks_requested: u64,
    ticks_observed: u64,
    client_count: usize,
    late_join_session_tick: u64,
    late_join_world_tick: u64,
    server_command_log: Vec<CommitRecord>,
    first_disagreement: Option<FirstDisagreement>,
    final_hash: u64,
}

fn mandatory_network<T>(operation: &str, result: Result<T, NetError>) -> T {
    match result {
        Ok(value) => value,
        Err(NetError::Io(error)) if error.kind() == ErrorKind::PermissionDenied => {
            panic!("red TCP obligatoria: {operation} recibió PermissionDenied ({error})")
        }
        Err(error) => panic!("red TCP obligatoria: {operation} falló ({error})"),
    }
}

fn start_server(snapshot: String) -> ListenServer {
    mandatory_network(
        "ListenServer::start_with_timeouts",
        ListenServer::start_with_timeouts("127.0.0.1:0", snapshot, fast_loopback_timeouts()),
    )
}

fn connect_client(bind: &str) -> ClientSession {
    mandatory_network(
        "ClientSession::connect_with_timeouts",
        ClientSession::connect_with_timeouts(bind, fast_loopback_timeouts()),
    )
}

fn fast_loopback_timeouts() -> SessionTimeouts {
    SessionTimeouts {
        handshake: NETWORK_TIMEOUT,
        stalled_peer: NETWORK_TIMEOUT,
        poll_interval: POLL_PAUSE,
    }
}

fn wait_event_until(client: &ClientSession, deadline: Instant, waiting_for: &str) -> SessionEvent {
    loop {
        match client.try_recv() {
            Some(SessionEvent::PeerList { .. } | SessionEvent::Heartbeat { .. }) => {}
            Some(event) => return event,
            None if Instant::now() < deadline => thread::sleep(POLL_PAUSE),
            None => panic!("timeout V1-NET de 120 s esperando {waiting_for}"),
        }
    }
}

fn wait_for_peer_count(server: &ListenServer, expected: usize, deadline: Instant) {
    while server.peer_ids().len() != expected && Instant::now() < deadline {
        thread::sleep(POLL_PAUSE);
    }
    assert_eq!(
        server.peer_ids().len(),
        expected,
        "cantidad de peers no alcanzó {expected} antes del timeout V1-NET"
    );
}

fn wait_server_commit(server: &ListenServer, deadline: Instant) -> CommitRecord {
    loop {
        match server.try_recv() {
            Some(SessionEvent::Commit {
                seq,
                company_id,
                command,
            }) => {
                return CommitRecord {
                    seq,
                    issuer: company_id,
                    command,
                };
            }
            Some(other) => panic!("evento inesperado del dedicated: {other:?}"),
            None if Instant::now() < deadline => thread::sleep(POLL_PAUSE),
            None => panic!("timeout V1-NET de 120 s esperando Commit del dedicated"),
        }
    }
}

fn receive_expected_commit(
    client: &ClientSession,
    state: &mut GameState,
    expected: &CommitRecord,
    deadline: Instant,
    client_name: &str,
) -> CommitRecord {
    let event = wait_event_until(client, deadline, &format!("Commit en {client_name}"));
    let received = match &event {
        SessionEvent::Commit {
            seq,
            company_id,
            command,
        } => CommitRecord {
            seq: *seq,
            issuer: *company_id,
            command: command.clone(),
        },
        other => panic!("{client_name} debía recibir Commit, recibió {other:?}"),
    };
    assert_eq!(received, *expected, "Commit distinto en {client_name}");
    apply_session_event(state, &event).expect("aplica Commit recibido");
    received
}

fn receive_advance(client: &ClientSession, state: &mut GameState, deadline: Instant, name: &str) {
    let event = wait_event_until(client, deadline, &format!("AdvanceTicks en {name}"));
    assert!(
        matches!(event, SessionEvent::AdvanceTicks { count: 1 }),
        "{name} debía recibir exactamente un AdvanceTicks(1), recibió {event:?}"
    );
    apply_session_event(state, &event).expect("aplica AdvanceTicks recibido");
}

fn receive_hash(
    client: &ClientSession,
    state: &mut GameState,
    expected_tick: u64,
    expected_hash: u64,
    deadline: Instant,
    name: &str,
) {
    let event = wait_event_until(client, deadline, &format!("HashCheck en {name}"));
    assert!(
        matches!(
            event,
            SessionEvent::HashCheck { tick, hash } if tick == expected_tick && hash == expected_hash
        ),
        "{name} recibió un HashCheck distinto: {event:?}"
    );
    apply_session_event(state, &event)
        .unwrap_or_else(|error| panic!("{name} no coincide con el HashCheck remoto: {error}"));
}

fn configured_coal_route() -> GameState {
    let mut state = build_scenario("first_route").expect("fixture Temperate first_route");
    let (width, height) = state.map.dimensions();
    assert_eq!((width, height), (64, 64));
    assert!(state.vehicles.is_empty(), "el fixture no inyecta vehículos");
    assert!(
        state.stations.is_empty(),
        "el fixture no inyecta estaciones"
    );

    for x in FIRST_ROUTE_ROAD_START_X..=FIRST_ROUTE_ROAD_END_X {
        apply_command(
            &mut state,
            &Command::PlaceRoadBits(TileCoord::new(x, FIRST_ROUTE_ROAD_Y), 0x0A),
        )
        .expect("construye corredor de carbón");
    }
    for command in [
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
    ] {
        apply_command(&mut state, &command).expect("comando público de la ruta de carbón");
    }
    state
}

fn expected_log_from(server_log: &[CommitRecord], next_seq: u64) -> Vec<CommitRecord> {
    server_log
        .iter()
        .filter(|record| record.seq >= next_seq)
        .cloned()
        .collect()
}

fn assert_log_matches(
    server_log: &[CommitRecord],
    next_seq: u64,
    client_log: &[CommitRecord],
    name: &str,
    session_tick: u64,
) {
    assert_eq!(
        client_log,
        expected_log_from(server_log, next_seq),
        "log de {name} perdido/duplicado en tick de sesión {session_tick}"
    );
}

fn first_disagreement(
    session_tick: u64,
    server: &GameState,
    first: &GameState,
    late: &GameState,
) -> Option<FirstDisagreement> {
    let server_hash = server.canonical_hash();
    for (client, state) in [("cliente inicial", first), ("late join", late)] {
        let client_hash = state.canonical_hash();
        if client_hash != server_hash {
            return Some(FirstDisagreement {
                session_tick,
                world_tick: server.tick.get(),
                client,
                server_hash,
                client_hash,
                json_difference: first_json_difference(
                    &serde_json::to_value(server).expect("serializa server para diagnóstico"),
                    &serde_json::to_value(state).expect("serializa cliente para diagnóstico"),
                    "$",
                )
                .unwrap_or_else(|| "orden interno no canónico".into()),
            });
        }
    }
    None
}

fn first_json_difference(
    left: &serde_json::Value,
    right: &serde_json::Value,
    path: &str,
) -> Option<String> {
    match (left, right) {
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            let keys: std::collections::BTreeSet<_> = left.keys().chain(right.keys()).collect();
            for key in keys {
                let next_path = format!("{path}.{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(difference) = first_json_difference(left, right, &next_path) {
                            return Some(difference);
                        }
                    }
                    (Some(_), None) | (None, Some(_)) => return Some(next_path),
                    (None, None) => unreachable!("la clave procede de uno de los dos mapas"),
                }
            }
            None
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!("{path}.len: {} != {}", left.len(), right.len()));
            }
            left.iter()
                .zip(right)
                .enumerate()
                .find_map(|(index, (left, right))| {
                    first_json_difference(left, right, &format!("{path}[{index}]"))
                })
        }
        _ if left == right => None,
        _ => Some(format!("{path}: {left:?} != {right:?}")),
    }
}

fn source_sha() -> String {
    env::var("OPENTTDRS_SOURCE_SHA")
        .or_else(|_| env::var("GITHUB_SHA"))
        .unwrap_or_else(|_| "local-worktree".into())
}

fn assert_company_materialized(state: &GameState, company: CompanyId, name: &str) {
    assert!(
        state
            .companies
            .iter()
            .any(|candidate| candidate.id == company),
        "{name} no materializó la compañía asignada {}",
        company.0
    );
}

#[test]
fn v1_late_join_two_clients_stay_lockstep_for_2000_ticks() {
    let deadline = Instant::now() + NETWORK_TIMEOUT;
    let source_sha = source_sha();
    let mut host = configured_coal_route();
    let server = start_server(host.save_json().expect("serializa snapshot inicial"));
    let bind = server.local_addr().to_string();

    let first = connect_client(&bind);
    let first_welcome = wait_event_until(&first, deadline, "Welcome del cliente inicial");
    let first_next_seq = match &first_welcome {
        SessionEvent::Welcome { next_seq, .. } => *next_seq,
        other => panic!("el cliente inicial debía recibir Welcome, recibió {other:?}"),
    };
    let mut first_state = GameState::new(1, 1);
    apply_session_event(&mut first_state, &first_welcome).expect("aplica Welcome inicial");
    let first_company = first.handle().company_id();
    assert_ne!(first_company, CompanyId::PLAYER);
    wait_for_peer_count(&server, 1, deadline);

    let first_command = Command::PlaceRoad(TileCoord::new(2, 50));
    first
        .propose(first_command.clone())
        .expect("envía propuesta del cliente inicial");
    let first_commit = wait_server_commit(&server, deadline);
    assert_eq!(first_commit.seq, 1);
    assert_eq!(first_commit.issuer, first_company);
    assert_eq!(first_commit.command, first_command);
    apply_command_as_company(&mut host, first_commit.issuer, &first_commit.command)
        .expect("dedicated aplica el Commit inicial");
    server.synchronize().expect("publica el Commit inicial");
    let mut server_log = vec![first_commit.clone()];
    let mut first_log = vec![receive_expected_commit(
        &first,
        &mut first_state,
        &first_commit,
        deadline,
        "cliente inicial",
    )];
    assert_eq!(host.canonical_hash(), first_state.canonical_hash());
    assert_log_matches(
        &server_log,
        first_next_seq,
        &first_log,
        "cliente inicial",
        0,
    );

    let mut late: Option<ClientSession> = None;
    let mut late_state: Option<GameState> = None;
    let mut late_log = Vec::new();
    let mut late_next_seq = 0;
    let mut late_company = CompanyId::PLAYER;
    let mut late_join_world_tick = 0;
    let mut ticks_observed = 0;
    let mut first_difference = None;

    for session_tick in 1..=TOTAL_TICKS {
        host.step();
        let world_tick = host.tick.get();
        server
            .broadcast_advance_with_snapshot(
                1,
                host.save_json().expect("serializa frontera de tick"),
            )
            .expect("publica avance del dedicated");

        receive_advance(&first, &mut first_state, deadline, "cliente inicial");
        if let (Some(client), Some(state)) = (late.as_ref(), late_state.as_mut()) {
            receive_advance(client, state, deadline, "late join");
        }
        if let Some(state) = late_state.as_ref() {
            first_difference = first_disagreement(session_tick, &host, &first_state, state);
            if first_difference.is_some() {
                ticks_observed = session_tick;
                break;
            }
        } else {
            assert_eq!(host.canonical_hash(), first_state.canonical_hash());
        }
        ticks_observed = session_tick;

        if session_tick == LATE_JOIN_TICK {
            server
                .synchronize()
                .expect("fija la frontera del snapshot de late join");
            let client = connect_client(&bind);
            let welcome = wait_event_until(&client, deadline, "Welcome del late join");
            late_next_seq = match &welcome {
                SessionEvent::Welcome { next_seq, .. } => *next_seq,
                other => panic!("el late join debía recibir Welcome, recibió {other:?}"),
            };
            assert_eq!(late_next_seq, server.next_seq());
            late_join_world_tick = world_tick;
            let mut state = GameState::new(1, 1);
            apply_session_event(&mut state, &welcome).expect("aplica Welcome del late join");
            late_company = client.handle().company_id();
            assert_ne!(late_company, CompanyId::PLAYER);
            assert_ne!(late_company, first_company);
            wait_for_peer_count(&server, 2, deadline);
            assert_eq!(
                host.canonical_hash(),
                state.canonical_hash(),
                "snapshot de late join difiere en {}",
                first_json_difference(
                    &serde_json::to_value(&host).expect("serializa host para diagnóstico"),
                    &serde_json::to_value(&state).expect("serializa late join para diagnóstico"),
                    "$",
                )
                .unwrap_or_else(|| "orden interno no canónico".into())
            );
            assert_log_matches(
                &server_log,
                late_next_seq,
                &late_log,
                "late join",
                session_tick,
            );

            let late_command = Command::PlaceRoad(TileCoord::new(3, 50));
            client
                .propose(late_command.clone())
                .expect("envía propuesta del late join");
            let late_commit = wait_server_commit(&server, deadline);
            assert_eq!(late_commit.seq, 2);
            assert_eq!(late_commit.issuer, late_company);
            assert_eq!(late_commit.command, late_command);
            apply_command_as_company(&mut host, late_commit.issuer, &late_commit.command)
                .expect("dedicated aplica el Commit del late join");
            server
                .synchronize()
                .expect("publica el Commit del late join");
            server_log.push(late_commit.clone());
            first_log.push(receive_expected_commit(
                &first,
                &mut first_state,
                &late_commit,
                deadline,
                "cliente inicial",
            ));
            late_log.push(receive_expected_commit(
                &client,
                &mut state,
                &late_commit,
                deadline,
                "late join",
            ));
            late = Some(client);
            late_state = Some(state);
        }

        assert_log_matches(
            &server_log,
            first_next_seq,
            &first_log,
            "cliente inicial",
            session_tick,
        );
        if let Some(state) = late_state.as_ref() {
            assert_log_matches(
                &server_log,
                late_next_seq,
                &late_log,
                "late join",
                session_tick,
            );
            first_difference = first_disagreement(session_tick, &host, &first_state, state);
            if first_difference.is_some() {
                break;
            }
        } else {
            assert_eq!(host.canonical_hash(), first_state.canonical_hash());
        }
    }

    assert!(
        late.is_some(),
        "el segundo cliente debía unirse en tick 500"
    );
    if first_difference.is_none() {
        let final_tick = host.tick.get();
        let final_hash = host.canonical_hash();
        server
            .broadcast_hash(final_tick, final_hash)
            .expect("publica HashCheck final del dedicated");
        server
            .synchronize()
            .expect("sincroniza HashCheck final del dedicated");
        receive_hash(
            &first,
            &mut first_state,
            final_tick,
            final_hash,
            deadline,
            "cliente inicial",
        );
        let client = late.as_ref().expect("cliente late join");
        let state = late_state.as_mut().expect("estado late join");
        receive_hash(client, state, final_tick, final_hash, deadline, "late join");
    }
    let report = LateJoinReport {
        source_sha,
        ticks_requested: TOTAL_TICKS,
        ticks_observed,
        client_count: 2,
        late_join_session_tick: LATE_JOIN_TICK,
        late_join_world_tick,
        server_command_log: server_log.clone(),
        first_disagreement: first_difference,
        final_hash: host.canonical_hash(),
    };
    eprintln!(
        "V1-NET late join report:\n{}",
        serde_json::to_string_pretty(&report).expect("serializa evidencia V1-NET")
    );

    assert!(
        report.first_disagreement.is_none(),
        "primer desacuerdo V1-NET: {report:#?}"
    );
    assert_eq!(
        ticks_observed, TOTAL_TICKS,
        "la sesión debe recorrer 2.000 ticks"
    );
    assert_eq!(
        server_log.len(),
        2,
        "no puede faltar ni duplicarse un Commit"
    );
    assert_eq!(server_log[0].issuer, first_company);
    assert_eq!(server_log[1].issuer, late_company);
    assert_eq!(server.next_seq(), 3);
    assert_company_materialized(&host, first_company, "dedicated");
    assert_company_materialized(&host, late_company, "dedicated");
    assert_company_materialized(&first_state, first_company, "cliente inicial");
    assert_company_materialized(&first_state, late_company, "cliente inicial");
    let state = late_state.as_ref().expect("estado del late join");
    assert_company_materialized(state, first_company, "late join");
    assert_company_materialized(state, late_company, "late join");
}
