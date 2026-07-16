//! Failover listen-server orquestado (ADR 0004 / protocolo v2).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::{Command, GameState, TileCoord, apply_command};
use openttdrs_net::{
    ClientSession, DEFAULT_PORT, ListenServer, SessionEvent, apply_session_event, elect_new_host,
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

fn wait_welcome(client: &ClientSession, timeout: Duration) -> (GameState, u64, u64) {
    match wait_event(client, timeout) {
        SessionEvent::Welcome {
            snapshot_json,
            next_seq,
            peer_id,
        } => {
            let state = GameState::load_json(&snapshot_json).unwrap();
            (state, next_seq, peer_id)
        }
        other => panic!("expected Welcome, got {other:?}"),
    }
}

fn drain_until_disconnected(client: &ClientSession, timeout: Duration) {
    let start = Instant::now();
    loop {
        match client.try_recv() {
            Some(SessionEvent::Disconnected { .. }) => return,
            Some(_) => {}
            None if start.elapsed() > timeout => return,
            None => thread::sleep(Duration::from_millis(5)),
        }
    }
}

#[test]
fn listen_server_failover_promotes_min_peer_and_preserves_hash() {
    let mut host = GameState::new(32, 32);
    let snapshot = host.save_json().unwrap();

    let port_a = u16::try_from(46_000 + (std::process::id() % 1000)).unwrap_or(DEFAULT_PORT);
    let bind_a = format!("127.0.0.1:{port_a}");
    let server = ListenServer::start(&bind_a, snapshot).unwrap();
    thread::sleep(Duration::from_millis(40));

    let client_lo = ClientSession::connect(&bind_a).unwrap();
    let (mut state_lo, _, peer_lo) = wait_welcome(&client_lo, Duration::from_secs(2));
    let client_hi = ClientSession::connect(&bind_a).unwrap();
    let (mut state_hi, _, peer_hi) = wait_welcome(&client_hi, Duration::from_secs(2));

    assert_ne!(peer_lo, peer_hi);
    let winner = elect_new_host(&[peer_lo, peer_hi]).unwrap();
    assert_eq!(winner, peer_lo.min(peer_hi));

    let cmd = Command::PlaceRail(TileCoord::new(6, 6));
    apply_command(&mut host, &cmd).unwrap();
    server.broadcast_commit(cmd).unwrap();

    let commit_lo = wait_event(&client_lo, Duration::from_secs(2));
    apply_session_event(&mut state_lo, &commit_lo).unwrap();
    let commit_hi = wait_event(&client_hi, Duration::from_secs(2));
    apply_session_event(&mut state_hi, &commit_hi).unwrap();

    for _ in 0..12 {
        host.step();
    }
    server.broadcast_advance(12).unwrap();
    let advance_lo = wait_event(&client_lo, Duration::from_secs(2));
    apply_session_event(&mut state_lo, &advance_lo).unwrap();
    let advance_hi = wait_event(&client_hi, Duration::from_secs(2));
    apply_session_event(&mut state_hi, &advance_hi).unwrap();

    assert_eq!(host.canonical_hash(), state_lo.canonical_hash());
    assert_eq!(host.canonical_hash(), state_hi.canonical_hash());

    let next_seq = server.next_seq();
    let failover_snapshot = host.save_json().unwrap();
    drop(server);
    drain_until_disconnected(&client_lo, Duration::from_secs(2));
    drain_until_disconnected(&client_hi, Duration::from_secs(2));
    drop(client_lo);
    drop(client_hi);

    // Nuevo listen-server; el peer ganador actúa como host local (sin ClientSession).
    let port_b = port_a.saturating_add(1);
    let bind_b = format!("127.0.0.1:{port_b}");
    let new_server = ListenServer::start_with_seq(&bind_b, failover_snapshot, next_seq).unwrap();
    thread::sleep(Duration::from_millis(40));

    let mut new_host_state = host;
    let rejoiner = ClientSession::connect(&bind_b).unwrap();
    let (mut rejoiner_state, welcome_seq, _) = wait_welcome(&rejoiner, Duration::from_secs(2));
    assert_eq!(welcome_seq, next_seq);
    assert_eq!(
        new_host_state.canonical_hash(),
        rejoiner_state.canonical_hash()
    );

    let cmd2 = Command::PlaceRail(TileCoord::new(7, 7));
    apply_command(&mut new_host_state, &cmd2).unwrap();
    new_server.broadcast_commit(cmd2).unwrap();
    let commit2 = wait_event(&rejoiner, Duration::from_secs(2));
    apply_session_event(&mut rejoiner_state, &commit2).unwrap();

    for _ in 0..8 {
        new_host_state.step();
    }
    new_server.broadcast_advance(8).unwrap();
    let advance2 = wait_event(&rejoiner, Duration::from_secs(2));
    apply_session_event(&mut rejoiner_state, &advance2).unwrap();

    assert_eq!(
        new_host_state.canonical_hash(),
        rejoiner_state.canonical_hash()
    );
    assert!(new_server.next_seq() > next_seq);
}
