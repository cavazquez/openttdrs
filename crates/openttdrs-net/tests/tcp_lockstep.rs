//! Integración TCP: servidor + cliente aplican el mismo log y comparten hash.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::ErrorKind;
use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::{Command, CompanyId, GameState, TileCoord, apply_command};
use openttdrs_net::{ClientSession, ListenServer, NetError, SessionEvent, apply_session_event};

fn wait_event(client: &ClientSession, timeout: Duration) -> SessionEvent {
    let start = Instant::now();
    loop {
        if let Some(e) = client.try_recv() {
            match e {
                SessionEvent::PeerList { .. } | SessionEvent::Heartbeat { .. } => continue,
                other => return other,
            }
        }
        if start.elapsed() > timeout {
            panic!("timeout waiting for client event");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn maybe_start_server(bind: &str, snapshot: String) -> Option<ListenServer> {
    match ListenServer::start(bind, snapshot) {
        Ok(server) => Some(server),
        Err(NetError::Io(error)) if error.kind() == ErrorKind::PermissionDenied => None,
        Err(error) => panic!("ListenServer::start falló: {error}"),
    }
}

fn maybe_connect_client(bind: &str) -> Option<ClientSession> {
    match ClientSession::connect(bind) {
        Ok(client) => Some(client),
        Err(NetError::Io(error)) if error.kind() == ErrorKind::PermissionDenied => None,
        Err(error) => panic!("ClientSession::connect falló: {error}"),
    }
}

#[test]
fn two_peers_same_log_same_hash_over_tcp() {
    let mut host = GameState::new(32, 32);
    let snapshot = host.save_json().unwrap();

    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    thread::sleep(Duration::from_millis(50));

    let client = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let welcome = wait_event(&client, Duration::from_secs(2));
    let mut remote = GameState::new(1, 1);
    apply_session_event(&mut remote, &welcome).unwrap();
    assert_eq!(host.canonical_hash(), remote.canonical_hash());

    let cmd = Command::PlaceRail(TileCoord::new(4, 4));
    apply_command(&mut host, &cmd).unwrap();
    server.broadcast_commit(cmd).unwrap();

    let commit = wait_event(&client, Duration::from_secs(2));
    apply_session_event(&mut remote, &commit).unwrap();

    for _ in 0..10 {
        host.step();
    }
    server.broadcast_advance(10).unwrap();
    let advance = wait_event(&client, Duration::from_secs(2));
    apply_session_event(&mut remote, &advance).unwrap();

    let hash = host.canonical_hash();
    server.broadcast_hash(host.tick.get(), hash).unwrap();
    let check = wait_event(&client, Duration::from_secs(2));
    apply_session_event(&mut remote, &check).unwrap();
    assert_eq!(host.canonical_hash(), remote.canonical_hash());
}

#[test]
fn late_joiner_gets_live_snapshot_not_boot() {
    let mut host = GameState::new(32, 32);
    let boot = host.save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", boot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();

    apply_command(&mut host, &Command::PlaceRail(TileCoord::new(5, 5))).unwrap();
    for _ in 0..200 {
        host.step();
    }
    server.update_snapshot(host.save_json().unwrap());
    thread::sleep(Duration::from_millis(30));

    let client = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let welcome = wait_event(&client, Duration::from_secs(2));
    let mut remote = GameState::new(1, 1);
    apply_session_event(&mut remote, &welcome).unwrap();
    assert_eq!(
        host.tick.get(),
        remote.tick.get(),
        "late join debe recibir el tick actual"
    );
    assert_eq!(host.canonical_hash(), remote.canonical_hash());
}

#[test]
fn client_propose_reaches_host() {
    let host_state = GameState::new(24, 24);
    let snapshot = host_state.save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    thread::sleep(Duration::from_millis(50));

    let client = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let _welcome = wait_event(&client, Duration::from_secs(2));

    client
        .propose(Command::PlaceRail(TileCoord::new(3, 3)))
        .unwrap();

    let start = Instant::now();
    let mut got = None;
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(SessionEvent::Commit { command, .. }) = server.try_recv() {
            got = Some(command);
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(got, Some(Command::PlaceRail(TileCoord::new(3, 3))));
}

#[test]
fn peers_receive_exclusive_company_identity_and_commit_issuer() {
    let host_state = GameState::new(24, 24);
    let snapshot = host_state.save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    thread::sleep(Duration::from_millis(50));

    let first = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let second = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let _ = wait_event(&first, Duration::from_secs(2));
    let _ = wait_event(&second, Duration::from_secs(2));
    let first_company = first.handle().company_id();
    let second_company = second.handle().company_id();
    assert_ne!(first_company, CompanyId::PLAYER);
    assert_ne!(first_company, second_company);

    first
        .propose(Command::PlaceRail(TileCoord::new(3, 3)))
        .unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(SessionEvent::Commit { company_id, .. }) = server.try_recv() {
            assert_eq!(company_id, first_company);
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timeout esperando commit con issuer");
}

#[test]
fn invalid_client_propose_is_rejected_before_commit() {
    let host_state = GameState::new(24, 24);
    let snapshot = host_state.save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    thread::sleep(Duration::from_millis(50));

    let client = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let _welcome = wait_event(&client, Duration::from_secs(2));

    client
        .propose(Command::PlaceRail(TileCoord::new(99, 99)))
        .unwrap();

    let event = wait_event(&client, Duration::from_secs(2));
    match event {
        SessionEvent::CommandRejected { message } => {
            assert!(!message.is_empty());
        }
        other => panic!("se esperaba rechazo, llegó {other:?}"),
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(100) {
        assert!(
            !matches!(server.try_recv(), Some(SessionEvent::Commit { .. })),
            "una propuesta inválida no debe entrar al log"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn client_desync_report_reaches_host_and_peers() {
    let host_state = GameState::new(24, 24);
    let snapshot = host_state.save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    thread::sleep(Duration::from_millis(50));

    let reporter = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let _welcome = wait_event(&reporter, Duration::from_secs(2));
    let peer = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let _welcome = wait_event(&peer, Duration::from_secs(2));

    reporter.report_desync(37, 0x10, 0x2a).unwrap();

    let start = Instant::now();
    let mut host_report = false;
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(SessionEvent::Desync {
            tick,
            expected_hash,
            actual_hash,
        }) = server.try_recv()
        {
            assert_eq!((tick, expected_hash, actual_hash), (37, 0x10, 0x2a));
            host_report = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(host_report, "el servidor debe recibir el diagnóstico");

    let peer_report = wait_event(&peer, Duration::from_secs(2));
    assert!(matches!(
        peer_report,
        SessionEvent::Desync {
            tick: 37,
            expected_hash: 0x10,
            actual_hash: 0x2a,
        }
    ));
}
