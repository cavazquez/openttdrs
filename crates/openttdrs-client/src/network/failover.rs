//! Auto-promote / reconnect tras caída del listen-server (ADR 0004 / #171).

use std::time::{Duration, Instant};

use bevy::prelude::*;
use openttdrs_net::{
    ClientSession, ListenServer, elect_new_host, failover_connect_addr, failover_listen_bind,
};

use crate::network::dispatch::{install_client, install_server};
use crate::network::plugin::{NetworkRole, NetworkRuntime, NetworkStatus};
use crate::state::SimWorld;

/// Timeout sin mensajes del host antes de intentar failover.
pub const HOST_SILENCE_TIMEOUT: Duration = Duration::from_secs(2);

/// Estado de cliente para elección y reconnect.
#[derive(Resource, Debug, Clone)]
pub struct ClientFailoverState {
    pub peer_id: Option<u64>,
    pub known_peers: Vec<u64>,
    pub next_seq: u64,
    /// Addr original de `--client` (para derivar puerto+1).
    pub server_addr: String,
    pub last_rx: Instant,
    pub failover_attempted: bool,
}

impl ClientFailoverState {
    #[must_use]
    pub fn new(server_addr: String) -> Self {
        Self {
            peer_id: None,
            known_peers: Vec::new(),
            next_seq: 1,
            server_addr,
            last_rx: Instant::now(),
            failover_attempted: false,
        }
    }

    pub fn note_rx(&mut self) {
        self.last_rx = Instant::now();
    }
}

/// Tras silencio o `Disconnected`, promover o reconectar.
pub fn try_client_failover(
    net: &mut NetworkRuntime,
    status: &mut NetworkStatus,
    failover: &mut ClientFailoverState,
    sim: &SimWorld,
) -> bool {
    if failover.failover_attempted || net.role() != NetworkRole::Client {
        return false;
    }
    let Some(my_id) = failover.peer_id else {
        return false;
    };
    let mut alive = failover.known_peers.clone();
    if !alive.contains(&my_id) {
        alive.push(my_id);
    }
    let Some(winner) = elect_new_host(&alive) else {
        return false;
    };
    failover.failover_attempted = true;

    if winner == my_id {
        promote_local_host(net, status, failover, sim)
    } else {
        reconnect_to_failover_host(net, status, failover)
    }
}

fn promote_local_host(
    net: &mut NetworkRuntime,
    status: &mut NetworkStatus,
    failover: &ClientFailoverState,
    sim: &SimWorld,
) -> bool {
    let Some(bind) = failover_listen_bind(&failover.server_addr) else {
        error!(
            "failover: no se pudo derivar bind desde {}",
            failover.server_addr
        );
        return false;
    };
    let snapshot = match sim.state.save_json() {
        Ok(s) => s,
        Err(e) => {
            error!("failover: snapshot falló: {e}");
            return false;
        }
    };
    // Cerrar cliente antes de escuchar.
    *net = NetworkRuntime::offline();
    match ListenServer::start_with_seq(&bind, snapshot, failover.next_seq) {
        Ok(server) => {
            let announce =
                failover_connect_addr(&failover.server_addr).unwrap_or_else(|| bind.clone());
            if let Some(peer_id) = failover.peer_id {
                let _ =
                    server.broadcast_host_announce(announce.clone(), failover.next_seq, peer_id);
            }
            install_server(server.handle());
            info!(
                "failover: promovido a listen-server en {bind} (announce {announce}) peer_id={:?} next_seq={}",
                failover.peer_id, failover.next_seq
            );
            status.label = format!("server {bind} (failover)");
            status.desync = None;
            *net = NetworkRuntime::listen_server(server);
            true
        }
        Err(e) => {
            error!("failover: no se pudo bind {bind}: {e}");
            false
        }
    }
}

fn reconnect_to_failover_host(
    net: &mut NetworkRuntime,
    status: &mut NetworkStatus,
    failover: &mut ClientFailoverState,
) -> bool {
    let Some(addr) = failover_connect_addr(&failover.server_addr) else {
        error!(
            "failover: no se pudo derivar addr desde {}",
            failover.server_addr
        );
        return false;
    };
    *net = NetworkRuntime::offline();
    // Reintentos cortos: el nuevo host puede tardar en bind.
    for attempt in 1..=20 {
        match ClientSession::connect(&addr) {
            Ok(client) => {
                install_client(client.handle());
                info!("failover: reconectado a {addr} (intento {attempt})");
                status.label = format!("client {addr} (failover)");
                status.desync = None;
                failover.server_addr = addr.clone();
                failover.failover_attempted = false;
                failover.note_rx();
                *net = NetworkRuntime::client(client);
                return true;
            }
            Err(e) => {
                if attempt == 20 {
                    error!("failover: no se pudo conectar a {addr}: {e}");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    false
}
