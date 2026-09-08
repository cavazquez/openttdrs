//! Integración TCP: servidor + cliente aplican el mismo log y comparten hash.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::io::{ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::{Command, CompanyId, GameState, MAX_COMPANIES, TileCoord, apply_command};
use openttdrs_net::{
    ClientSession, ListenServer, NetError, NetMessage, PROTOCOL_VERSION, SessionEvent,
    SessionTimeouts, apply_session_event, read_message, write_message,
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

fn maybe_start_server_with_timeouts(
    bind: &str,
    snapshot: String,
    timeouts: SessionTimeouts,
) -> Option<ListenServer> {
    match ListenServer::start_with_timeouts(bind, snapshot, timeouts) {
        Ok(server) => Some(server),
        Err(NetError::Io(error)) if error.kind() == ErrorKind::PermissionDenied => None,
        Err(error) => panic!("ListenServer::start_with_timeouts falló: {error}"),
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

fn wait_heartbeat(client: &ClientSession, expected_tick: u64, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        match client.try_recv() {
            Some(SessionEvent::Heartbeat { tick }) => {
                assert_eq!(tick, expected_tick);
                return;
            }
            Some(SessionEvent::PeerList { .. }) => {}
            Some(other) => panic!("se esperaba Heartbeat, llegó {other:?}"),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            None => panic!("timeout esperando heartbeat {expected_tick}"),
        }
    }
}

fn assert_no_state_event_before_heartbeat(
    client: &ClientSession,
    expected_tick: u64,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    loop {
        match client.try_recv() {
            Some(SessionEvent::Heartbeat { tick }) => {
                assert_eq!(tick, expected_tick);
                return;
            }
            Some(SessionEvent::PeerList { .. }) => {}
            Some(SessionEvent::AdvanceTicks { count }) => {
                panic!("avance duplicado de {count} antes de la barrera heartbeat")
            }
            Some(SessionEvent::Commit { seq, .. }) => {
                panic!("commit duplicado seq={seq} antes de la barrera heartbeat")
            }
            Some(other) => panic!("evento inesperado antes de heartbeat: {other:?}"),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            None => panic!("timeout esperando heartbeat de frontera"),
        }
    }
}

fn wait_resync_welcome(client: &ClientSession, timeout: Duration) -> SessionEvent {
    let deadline = Instant::now() + timeout;
    loop {
        match client.try_recv() {
            Some(welcome @ SessionEvent::Welcome { .. }) => return welcome,
            Some(SessionEvent::Desync { .. } | SessionEvent::PeerList { .. }) => {}
            Some(SessionEvent::Heartbeat { .. }) => {}
            Some(other) => panic!("se esperaba Welcome de resync, llegó {other:?}"),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            None => panic!("timeout esperando Welcome de resync"),
        }
    }
}

fn wait_for_peer_count(server: &ListenServer, expected: usize, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while server.peer_ids().len() != expected && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        server.peer_ids().len(),
        expected,
        "cantidad de peers no alcanzó el valor esperado"
    );
}

fn short_timeouts() -> SessionTimeouts {
    SessionTimeouts {
        handshake: Duration::from_millis(150),
        stalled_peer: Duration::from_millis(150),
        poll_interval: Duration::from_millis(1),
    }
}

#[test]
fn silent_handshake_does_not_block_a_healthy_peer_or_shutdown() {
    let server = match maybe_start_server_with_timeouts(
        "127.0.0.1:0",
        GameState::new(16, 16).save_json().unwrap(),
        short_timeouts(),
    ) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    let healthy = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    assert!(matches!(
        wait_event(&healthy, Duration::from_secs(2)),
        SessionEvent::Welcome { .. }
    ));

    let _silent = TcpStream::connect(&bind).unwrap();
    // Dar una vuelta al accept para reproducir el punto donde la versión
    // previa se quedaba dentro del handshake bloqueante.
    thread::sleep(Duration::from_millis(20));
    server.broadcast_heartbeat(123).unwrap();
    wait_heartbeat(&healthy, 123, Duration::from_millis(700));

    let started = Instant::now();
    drop(server);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "Drop no debe esperar un Hello silencioso"
    );
}

#[test]
fn partial_peer_payload_expires_without_stalling_a_healthy_peer() {
    let server = match maybe_start_server_with_timeouts(
        "127.0.0.1:0",
        GameState::new(16, 16).save_json().unwrap(),
        short_timeouts(),
    ) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();
    let healthy = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    assert!(matches!(
        wait_event(&healthy, Duration::from_secs(2)),
        SessionEvent::Welcome { .. }
    ));

    let mut stalled = TcpStream::connect(&bind).unwrap();
    write_message(
        &mut stalled,
        &NetMessage::Hello {
            protocol: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    let welcome = read_message(&mut stalled).unwrap();
    let company_id = match welcome {
        NetMessage::Welcome { company_id, .. } => company_id,
        other => panic!("se esperaba Welcome, llegó {other:?}"),
    };
    let frame = framed_message(&NetMessage::Propose {
        company_id,
        command: Command::PlaceRoad(TileCoord::new(2, 2)),
    });
    stalled.write_all(&frame[..2]).unwrap();
    stalled.flush().unwrap();
    thread::sleep(Duration::from_millis(20));

    server.broadcast_heartbeat(456).unwrap();
    wait_heartbeat(&healthy, 456, Duration::from_millis(700));

    let deadline = Instant::now() + Duration::from_secs(1);
    while server.peer_ids().len() != 1 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        server.peer_ids().len(),
        1,
        "el peer con payload incompleto debe expirar solo"
    );

    server.broadcast_heartbeat(457).unwrap();
    wait_heartbeat(&healthy, 457, Duration::from_millis(700));
}

#[test]
fn client_handshake_timeout_and_drop_are_bounded_against_a_silent_server() {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => panic!("no se pudo crear listener de prueba: {error}"),
    };
    let bind = listener.local_addr().unwrap().to_string();
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();
    let silent_server = thread::spawn(move || {
        let (_stream, _) = listener.accept().expect("acepta cliente silencioso");
        accepted_tx.send(()).expect("notifica accept");
        stop_rx.recv().expect("termina servidor silencioso");
    });

    let client = match ClientSession::connect_with_timeouts(&bind, short_timeouts()) {
        Ok(client) => client,
        Err(NetError::Io(error)) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => panic!("ClientSession::connect_with_timeouts falló: {error}"),
    };
    accepted_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("servidor silencioso acepta el cliente");
    assert!(matches!(
        wait_event(&client, Duration::from_secs(1)),
        SessionEvent::Disconnected { reason } if reason.contains("handshake timed out")
    ));

    let started = Instant::now();
    drop(client);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "Drop de cliente no debe depender del servidor silencioso"
    );
    stop_tx.send(()).unwrap();
    silent_server.join().expect("servidor silencioso termina");
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
    server
        .synchronize()
        .expect("snapshot live publicado antes del late join");

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
fn snapshot_frontier_applies_commits_and_advances_exactly_once() {
    let mut host = GameState::new(16, 16);
    let server = match maybe_start_server("127.0.0.1:0", host.save_json().unwrap()) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();

    // Join antes del commit: debe recibir la operación por el log, no en el
    // snapshot con el que entró.
    let early = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let early_welcome = wait_event(&early, Duration::from_secs(2));
    let mut early_state = GameState::new(1, 1);
    apply_session_event(&mut early_state, &early_welcome).unwrap();

    let command = Command::PlaceRoad(TileCoord::new(3, 3));
    apply_command(&mut host, &command).unwrap();
    server
        .handle()
        .broadcast_commit_for_company_with_snapshot(
            CompanyId::PLAYER,
            command.clone(),
            host.save_json().unwrap(),
        )
        .unwrap();
    server.synchronize().unwrap();

    let commit = wait_event(&early, Duration::from_secs(2));
    assert!(matches!(
        commit,
        SessionEvent::Commit {
            seq: 1,
            company_id: CompanyId::PLAYER,
            command: Command::PlaceRoad(coord),
        } if coord == TileCoord::new(3, 3)
    ));
    apply_session_event(&mut early_state, &commit).unwrap();
    assert_eq!(early_state.canonical_hash(), host.canonical_hash());
    assert_eq!(server.next_seq(), 2);

    // Join después del commit: el Welcome ya lo contiene y su next_seq forma
    // parte de la misma frontera que el snapshot.
    let after_commit = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let after_commit_welcome = wait_event(&after_commit, Duration::from_secs(2));
    let SessionEvent::Welcome { next_seq, .. } = &after_commit_welcome else {
        panic!("se esperaba Welcome después del commit");
    };
    assert_eq!(*next_seq, server.next_seq());
    let mut after_commit_state = GameState::new(1, 1);
    apply_session_event(&mut after_commit_state, &after_commit_welcome).unwrap();
    assert_eq!(after_commit_state.canonical_hash(), host.canonical_hash());

    // Los peers que ya estaban conectados reciben el Advance una sola vez;
    // el snapshot publicado en la misma orden será para los joins posteriores.
    host.step();
    server
        .broadcast_advance_with_snapshot(1, host.save_json().unwrap())
        .unwrap();
    server.synchronize().unwrap();

    let early_advance = wait_event(&early, Duration::from_secs(2));
    let after_commit_advance = wait_event(&after_commit, Duration::from_secs(2));
    assert!(matches!(
        early_advance,
        SessionEvent::AdvanceTicks { count: 1 }
    ));
    assert!(matches!(
        after_commit_advance,
        SessionEvent::AdvanceTicks { count: 1 }
    ));
    apply_session_event(&mut early_state, &early_advance).unwrap();
    apply_session_event(&mut after_commit_state, &after_commit_advance).unwrap();
    assert_eq!(early_state.canonical_hash(), host.canonical_hash());
    assert_eq!(after_commit_state.canonical_hash(), host.canonical_hash());

    let after_advance = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    let after_advance_welcome = wait_event(&after_advance, Duration::from_secs(2));
    let mut after_advance_state = GameState::new(1, 1);
    apply_session_event(&mut after_advance_state, &after_advance_welcome).unwrap();
    assert_eq!(after_advance_state.canonical_hash(), host.canonical_hash());

    // El heartbeat es una barrera ordenada: un evento previo duplicado tendría
    // que aparecer antes y fallaría el test sin depender de una espera fija.
    server.broadcast_heartbeat(host.tick.get()).unwrap();
    server.synchronize().unwrap();
    assert_no_state_event_before_heartbeat(&early, host.tick.get(), Duration::from_secs(2));
    assert_no_state_event_before_heartbeat(&after_commit, host.tick.get(), Duration::from_secs(2));
    assert_no_state_event_before_heartbeat(&after_advance, host.tick.get(), Duration::from_secs(2));

    // Resync usa la misma frontera actual: el Welcome posterior reemplaza el
    // estado una vez, sin reaplicar el commit ni el advance incluidos.
    early.report_desync(host.tick.get(), 0x11, 0x22).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match server.try_recv() {
            Some(SessionEvent::Desync { .. }) => break,
            Some(_) => {}
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            None => panic!("timeout esperando desync en el servidor"),
        }
    }
    server.synchronize().unwrap();
    let resync = wait_resync_welcome(&early, Duration::from_secs(2));
    let SessionEvent::Welcome { next_seq, .. } = &resync else {
        panic!("se esperaba Welcome de resync");
    };
    assert_eq!(*next_seq, server.next_seq());
    let mut resynced = GameState::new(1, 1);
    apply_session_event(&mut resynced, &resync).unwrap();
    assert_eq!(resynced.canonical_hash(), host.canonical_hash());

    server.broadcast_heartbeat(host.tick.get()).unwrap();
    server.synchronize().unwrap();
    assert_no_state_event_before_heartbeat(&early, host.tick.get(), Duration::from_secs(2));
}

#[test]
fn pending_handshake_crossing_advance_frontier_never_replays_it() {
    let mut host = GameState::new(8, 8);
    let server = match maybe_start_server("127.0.0.1:0", host.save_json().unwrap()) {
        Some(server) => server,
        None => return,
    };
    let bind = server.local_addr().to_string();

    // Reproduce la carrera auditada: TCP ya aceptado, pero Hello todavía no
    // llegó. La barrera confirma que el hilo de listen vio esa conexión antes
    // de cruzar la frontera de simulación.
    let mut pending = TcpStream::connect(&bind).expect("conecta socket pendiente");
    pending
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("configura timeout socket pendiente");
    server.synchronize().unwrap();

    host.step();
    server
        .broadcast_advance_with_snapshot(1, host.save_json().unwrap())
        .unwrap();
    server.synchronize().unwrap();

    write_message(
        &mut pending,
        &NetMessage::Hello {
            protocol: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    let welcome = read_message(&mut pending).expect("recibe Welcome de frontera");
    let NetMessage::Welcome {
        snapshot_json,
        next_seq,
        ..
    } = welcome
    else {
        panic!("se esperaba Welcome después de la frontera")
    };
    let remote = GameState::load_json(&snapshot_json).expect("snapshot de Welcome válido");
    assert_eq!(remote.tick.get(), host.tick.get());
    assert_eq!(remote.canonical_hash(), host.canonical_hash());
    assert_eq!(next_seq, server.next_seq());

    // La promoción puede añadir PeerList, por eso Heartbeat funciona como
    // marcador ordenado del stream: AdvanceTicks antes de él sería la doble
    // aplicación que disparaba el bug original.
    server.broadcast_heartbeat(host.tick.get()).unwrap();
    server.synchronize().unwrap();
    let mut heartbeat_seen = false;
    for _ in 0..3 {
        match read_message(&mut pending).expect("drena frontera del handshake") {
            NetMessage::PeerList { .. } => {}
            NetMessage::Heartbeat { tick } => {
                assert_eq!(tick, host.tick.get());
                heartbeat_seen = true;
                break;
            }
            NetMessage::AdvanceTicks { count } => {
                panic!("avance duplicado de {count} para snapshot ya avanzado")
            }
            other => panic!("mensaje inesperado tras Welcome de frontera: {other:?}"),
        }
    }
    assert!(heartbeat_seen, "Heartbeat debe cerrar la barrera de stream");
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
fn company_pool_rejects_overflow_and_reuses_released_slot() {
    let server =
        match maybe_start_server("127.0.0.1:0", GameState::new(64, 64).save_json().unwrap()) {
            Some(server) => server,
            None => return,
        };
    let bind = server.local_addr().to_string();
    let client_capacity = usize::from(MAX_COMPANIES) - 1;
    let mut clients = Vec::with_capacity(client_capacity);

    for _ in 0..client_capacity {
        let Some(client) = maybe_connect_client(&bind) else {
            return;
        };
        assert!(matches!(
            wait_event(&client, Duration::from_secs(2)),
            SessionEvent::Welcome { .. }
        ));
        clients.push(client);
    }
    wait_for_peer_count(&server, client_capacity, Duration::from_secs(2));

    let company_ids: Vec<_> = clients
        .iter()
        .map(|client| client.handle().company_id())
        .collect();
    let unique_ids: BTreeSet<_> = company_ids.iter().map(|company| company.0).collect();
    assert_eq!(unique_ids.len(), client_capacity);
    assert!(
        company_ids
            .iter()
            .all(|company| { *company != CompanyId::PLAYER && company.0 < MAX_COMPANIES })
    );

    // El wire protocol debe rechazar el peer excedente antes de emitir Welcome.
    let mut overflow_wire = TcpStream::connect(&bind).expect("conecta peer excedente");
    overflow_wire
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("configura timeout del peer excedente");
    write_message(
        &mut overflow_wire,
        &NetMessage::Hello {
            protocol: PROTOCOL_VERSION,
        },
    )
    .expect("envía Hello excedente");
    match read_message(&mut overflow_wire).expect("recibe rechazo por capacidad") {
        NetMessage::Reject { message } => assert!(message.contains("compañías exclusivas")),
        other => panic!("el peer excedente no debe recibir Welcome: {other:?}"),
    }
    drop(overflow_wire);

    // El cliente de alto nivel transforma ese rechazo de handshake en una
    // desconexión explícita, en vez de dejarlo esperando un Welcome imposible.
    let rejected = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    assert!(matches!(
        wait_event(&rejected, Duration::from_secs(2)),
        SessionEvent::Disconnected { reason } if reason.contains("compañías exclusivas")
    ));
    assert_eq!(rejected.handle().company_id(), CompanyId::PLAYER);
    drop(rejected);
    wait_for_peer_count(&server, client_capacity, Duration::from_secs(2));

    // Cada identidad admitida puede materializar su propia compañía y emitir
    // una orden válida; ningún Commit debe heredar la compañía del host.
    for (index, client) in clients.iter().enumerate() {
        client
            .propose(Command::PlaceRoad(TileCoord::new(
                i32::try_from(index).expect("índice de cliente cabe en i32") + 2,
                2,
            )))
            .expect("encola propuesta de compañía válida");
    }
    let expected_ids: BTreeSet<_> = company_ids.iter().map(|company| company.0).collect();
    let mut committed_ids = BTreeSet::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while committed_ids.len() != expected_ids.len() && Instant::now() < deadline {
        match server.try_recv() {
            Some(SessionEvent::Commit { company_id, .. }) => {
                assert_ne!(company_id, CompanyId::PLAYER);
                assert!(expected_ids.contains(&company_id.0));
                committed_ids.insert(company_id.0);
            }
            Some(_) | None => thread::sleep(Duration::from_millis(5)),
        }
    }
    assert_eq!(committed_ids, expected_ids);

    let released_company = clients.remove(0).handle().company_id();
    wait_for_peer_count(&server, client_capacity - 1, Duration::from_secs(2));
    assert!(
        clients
            .iter()
            .all(|client| client.handle().company_id() != released_company)
    );

    let replacement = match maybe_connect_client(&bind) {
        Some(client) => client,
        None => return,
    };
    assert!(matches!(
        wait_event(&replacement, Duration::from_secs(2)),
        SessionEvent::Welcome { .. }
    ));
    assert_eq!(replacement.handle().company_id(), released_company);
    wait_for_peer_count(&server, client_capacity, Duration::from_secs(2));
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
