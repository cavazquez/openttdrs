//! Integración TCP: servidor + cliente aplican el mismo log y comparten hash.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::{Command, CompanyId, GameState, TileCoord, apply_command};
use openttdrs_net::{
    ClientSession, ListenServer, NetError, NetMessage, PROTOCOL_VERSION, SessionEvent,
    apply_session_event, read_message, write_message,
};

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

fn framed_message(message: &NetMessage) -> Vec<u8> {
    let payload = serde_json::to_vec(message).expect("serializa el mensaje de prueba");
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("payload de prueba cabe en u32")
            .to_le_bytes(),
    );
    frame.extend_from_slice(&payload);
    frame
}

#[test]
fn fragmented_propose_reaches_listen_server_with_its_original_sequence() {
    let snapshot = GameState::new(24, 24).save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    let mut peer = TcpStream::connect(&bind).unwrap();
    write_message(
        &mut peer,
        &NetMessage::Hello {
            protocol: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    let welcome = read_message(&mut peer).unwrap();
    let assigned_company = match welcome {
        NetMessage::Welcome { company_id, .. } => company_id,
        other => panic!("se esperaba Welcome, llegó {other:?}"),
    };

    let command = Command::PlaceRail(TileCoord::new(3, 3));
    let frame = framed_message(&NetMessage::Propose {
        company_id: assigned_company,
        command: command.clone(),
    });
    // Forzar al servidor a observar un header incompleto. El decoder unitario
    // ya controla el WouldBlock de forma determinista; esta pausa comprueba el
    // recorrido real de TCP contra el fallo reproducido en el audit.
    peer.write_all(&frame[..1]).unwrap();
    peer.flush().unwrap();
    thread::sleep(Duration::from_millis(30));
    let payload_split = 4 + (frame.len() - 4) / 2;
    peer.write_all(&frame[1..payload_split]).unwrap();
    peer.flush().unwrap();
    thread::sleep(Duration::from_millis(30));
    peer.write_all(&frame[payload_split..]).unwrap();
    peer.flush().unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match server.try_recv() {
            Some(SessionEvent::Commit {
                seq,
                company_id,
                command: committed,
            }) => {
                assert_eq!(seq, 1);
                assert_eq!(company_id, assigned_company);
                assert_eq!(committed, command);
                break;
            }
            Some(_) => {}
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            None => panic!("timeout esperando el commit fragmentado"),
        }
    }

    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut delivered = false;
    for _ in 0..3 {
        match read_message(&mut peer).unwrap() {
            NetMessage::PeerList { .. } => {}
            NetMessage::Commit {
                seq,
                company_id,
                command: committed,
            } => {
                assert_eq!(seq, 1);
                assert_eq!(company_id, assigned_company);
                assert_eq!(committed, command);
                delivered = true;
                break;
            }
            other => panic!("se esperaba Commit, llegó {other:?}"),
        }
    }
    assert!(delivered, "el peer debe recibir su commit una sola vez");
}

#[test]
fn client_session_receives_a_fragmented_server_commit() {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => panic!("no se pudo crear listener de prueba: {error}"),
    };
    let bind = listener.local_addr().unwrap().to_string();
    let snapshot = GameState::new(16, 16).save_json().unwrap();
    let expected = NetMessage::Commit {
        seq: 7,
        company_id: CompanyId::PLAYER,
        command: Command::PlaceRoad(TileCoord::new(2, 2)),
    };
    let (start_tx, start_rx) = mpsc::channel();
    let (first_header_tx, first_header_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();
    let server_thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("acepta cliente de prueba");
        assert!(matches!(
            read_message(&mut stream).expect("lee Hello"),
            NetMessage::Hello {
                protocol: PROTOCOL_VERSION
            }
        ));
        write_message(
            &mut stream,
            &NetMessage::Welcome {
                protocol: PROTOCOL_VERSION,
                snapshot_json: snapshot,
                next_seq: 1,
                peer_id: 1,
                company_id: CompanyId::PLAYER,
            },
        )
        .expect("envía Welcome");

        start_rx.recv().expect("cliente listo para el frame");
        let frame = framed_message(&expected);
        stream.write_all(&frame[..2]).expect("envía header parcial");
        stream.flush().expect("flush header parcial");
        first_header_tx.send(()).expect("notifica header parcial");
        finish_rx.recv().expect("autoriza resto del frame");
        stream
            .write_all(&frame[2..])
            .expect("envía resto del frame");
        stream.flush().expect("flush frame completo");
    });

    let client = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    assert!(matches!(
        wait_event(&client, Duration::from_secs(2)),
        SessionEvent::Welcome { .. }
    ));
    start_tx.send(()).unwrap();
    first_header_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("servidor envía header parcial");
    // El test unitario inyecta WouldBlock sin tiempo; aquí damos al hilo de
    // sesión una vuelta real antes de enviar el resto del stream.
    thread::sleep(Duration::from_millis(30));
    finish_tx.send(()).unwrap();

    assert!(matches!(
        wait_event(&client, Duration::from_secs(2)),
        SessionEvent::Commit {
            seq: 7,
            company_id: CompanyId::PLAYER,
            command: Command::PlaceRoad(coord),
        } if coord == TileCoord::new(2, 2)
    ));
    drop(client);
    server_thread.join().expect("servidor de prueba termina");
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
fn client_reports_a_closed_server_once_then_the_drain_finishes() {
    let snapshot = GameState::new(16, 16).save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    let client = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    assert!(matches!(
        wait_event(&client, Duration::from_secs(2)),
        SessionEvent::Welcome { .. }
    ));

    drop(server);

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match client.try_recv() {
            Some(SessionEvent::Disconnected { .. }) => break,
            Some(_) => {}
            None if Instant::now() >= deadline => {
                panic!("timeout esperando el cierre del servidor")
            }
            None => thread::sleep(Duration::from_millis(5)),
        }
    }
    for _ in 0..100 {
        assert!(
            client.try_recv().is_none(),
            "el cierre ya comunicado no debe prolongar el drenaje"
        );
    }
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
fn server_rejects_a_spoofed_company_before_commit() {
    let host_state = GameState::new(24, 24);
    let snapshot = host_state.save_json().unwrap();
    let server = match maybe_start_server("127.0.0.1:0", snapshot) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    thread::sleep(Duration::from_millis(50));

    // Un peer que saltee `ClientSessionHandle` no puede elegir el issuer del
    // commit: el servidor asigna la compañía durante el handshake y comprueba
    // el campo de `Propose` antes de reservar una secuencia.
    let mut peer = TcpStream::connect(&bind).unwrap();
    write_message(
        &mut peer,
        &NetMessage::Hello {
            protocol: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    let welcome = read_message(&mut peer).unwrap();
    let assigned_company = match welcome {
        NetMessage::Welcome { company_id, .. } => company_id,
        other => panic!("se esperaba Welcome, llegó {other:?}"),
    };
    assert_ne!(assigned_company, CompanyId::PLAYER);

    write_message(
        &mut peer,
        &NetMessage::Propose {
            company_id: CompanyId::PLAYER,
            command: Command::PlaceRail(TileCoord::new(3, 3)),
        },
    )
    .unwrap();

    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut rejected = false;
    for _ in 0..3 {
        match read_message(&mut peer).unwrap() {
            NetMessage::Reject { message } => {
                assert!(message.contains("no puede emitir"));
                rejected = true;
                break;
            }
            NetMessage::PeerList { .. } => {}
            other => panic!("se esperaba Reject, llegó {other:?}"),
        }
    }
    assert!(
        rejected,
        "la propuesta con issuer falsificado debe rechazarse"
    );

    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(100) {
        assert!(
            !matches!(server.try_recv(), Some(SessionEvent::Commit { .. })),
            "un issuer falsificado no debe entrar al log"
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

    // El peer que reporta recibe primero el diagnóstico difundido y luego el
    // Welcome con el snapshot autoritativo para reparar su estado sin
    // reconectar manualmente.
    let reporter_report = wait_event(&reporter, Duration::from_secs(2));
    assert!(matches!(
        reporter_report,
        SessionEvent::Desync {
            tick: 37,
            expected_hash: 0x10,
            actual_hash: 0x2a,
        }
    ));
    let reporter_resync = wait_event(&reporter, Duration::from_secs(2));
    let SessionEvent::Welcome { snapshot_json, .. } = reporter_resync else {
        panic!("el emisor del desync debe recibir snapshot de reconciliación");
    };
    let repaired = GameState::load_json(&snapshot_json).expect("snapshot reparable");
    assert_eq!(repaired.canonical_hash(), host_state.canonical_hash());
}
