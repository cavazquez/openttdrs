//! Integración TCP: servidor + cliente aplican el mismo log y comparten hash.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::{Command, GameState, TileCoord, apply_command};
use openttdrs_net::{
    apply_session_event, ClientSession, ListenServer, SessionEvent, DEFAULT_PORT,
};

fn wait_event(client: &ClientSession, timeout: Duration) -> SessionEvent {
    let start = Instant::now();
    loop {
        if let Some(e) = client.try_recv() {
            return e;
        }
        if start.elapsed() > timeout {
            panic!("timeout waiting for client event");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn two_peers_same_log_same_hash_over_tcp() {
    let mut host = GameState::new(32, 32);
    let snapshot = host.save_json().unwrap();

    // Puerto efímero para no chocar con un dedicated local.
    let port = u16::try_from(40_000 + (std::process::id() % 2000)).unwrap_or(DEFAULT_PORT);
    let bind = format!("127.0.0.1:{port}");
    let server = ListenServer::start(&bind, snapshot).unwrap();
    thread::sleep(Duration::from_millis(50));

    let client = ClientSession::connect(&bind).unwrap();
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
    server
        .broadcast_hash(host.tick.get(), hash)
        .unwrap();
    let check = wait_event(&client, Duration::from_secs(2));
    apply_session_event(&mut remote, &check).unwrap();
    assert_eq!(host.canonical_hash(), remote.canonical_hash());
}

#[test]
fn late_joiner_gets_live_snapshot_not_boot() {
    let mut host = GameState::new(32, 32);
    let boot = host.save_json().unwrap();
    let port = u16::try_from(44_000 + (std::process::id() % 2000)).unwrap_or(DEFAULT_PORT);
    let bind = format!("127.0.0.1:{port}");
    let server = ListenServer::start(&bind, boot).unwrap();

    apply_command(&mut host, &Command::PlaceRail(TileCoord::new(5, 5))).unwrap();
    for _ in 0..200 {
        host.step();
    }
    server.update_snapshot(host.save_json().unwrap());
    thread::sleep(Duration::from_millis(30));

    let client = ClientSession::connect(&bind).unwrap();
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
    let port = u16::try_from(42_000 + (std::process::id() % 2000)).unwrap_or(DEFAULT_PORT);
    let bind = format!("127.0.0.1:{port}");
    let server = ListenServer::start(&bind, snapshot).unwrap();
    thread::sleep(Duration::from_millis(50));

    let client = ClientSession::connect(&bind).unwrap();
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
