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

const RECONNECT_ATTEMPTS: u32 = 20;
const RECONNECT_RETRY_GAP: Duration = Duration::from_millis(100);

/// Fase visible en el banner de failover.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FailoverUiPhase {
    #[default]
    Idle,
    Promoting {
        bind: String,
    },
    Reconnecting {
        addr: String,
        attempt: u32,
    },
    Failed {
        reason: String,
    },
}

/// Estado de cliente para elección y reconnect.
#[derive(Resource, Debug, Clone)]
pub struct ClientFailoverState {
    pub peer_id: Option<u64>,
    pub known_peers: Vec<u64>,
    pub next_seq: u64,
    /// Addr original de `--client` (para derivar puerto+1).
    pub server_addr: String,
    /// Destino explícito de [`HostAnnounce`] (no vuelve a sumar puerto).
    pub reconnect_addr: Option<String>,
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
            reconnect_addr: None,
            last_rx: Instant::now(),
            failover_attempted: false,
        }
    }

    pub fn note_rx(&mut self) {
        self.last_rx = Instant::now();
    }

    /// Destino de reconnect: anuncio explícito o convención puerto+1.
    #[must_use]
    pub fn resolve_reconnect_addr(&self) -> Option<String> {
        self.reconnect_addr
            .clone()
            .or_else(|| failover_connect_addr(&self.server_addr))
    }
}

/// Reintentos de reconnect repartidos por frames (no bloquea el hilo de Bevy).
#[derive(Resource, Debug)]
pub struct PendingFailoverReconnect {
    pub addr: String,
    pub attempts_left: u32,
    pub next_try: Instant,
    pub attempt: u32,
}

/// Tras silencio o `Disconnected`, promover o programar reconnect.
pub fn try_client_failover(
    commands: &mut Commands,
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
        promote_local_host(commands, net, status, failover, sim)
    } else {
        schedule_reconnect(commands, net, status, failover)
    }
}

fn promote_local_host(
    commands: &mut Commands,
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
        status.failover_phase = FailoverUiPhase::Failed {
            reason: "no se pudo derivar puerto de listen".into(),
        };
        return false;
    };
    status.failover_phase = FailoverUiPhase::Promoting {
        bind: bind.clone(),
    };
    let snapshot = match sim.state.save_json() {
        Ok(s) => s,
        Err(e) => {
            error!("failover: snapshot falló: {e}");
            status.failover_phase = FailoverUiPhase::Failed {
                reason: format!("snapshot: {e}"),
            };
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
            status.failover_phase = FailoverUiPhase::Idle;
            *net = NetworkRuntime::listen_server(server);
            commands.remove_resource::<ClientFailoverState>();
            commands.remove_resource::<PendingFailoverReconnect>();
            true
        }
        Err(e) => {
            error!("failover: no se pudo bind {bind}: {e}");
            status.failover_phase = FailoverUiPhase::Failed {
                reason: format!("bind {bind}: {e}"),
            };
            false
        }
    }
}

fn schedule_reconnect(
    commands: &mut Commands,
    net: &mut NetworkRuntime,
    status: &mut NetworkStatus,
    failover: &ClientFailoverState,
) -> bool {
    let Some(addr) = failover.resolve_reconnect_addr() else {
        error!(
            "failover: no se pudo derivar addr desde {}",
            failover.server_addr
        );
        status.failover_phase = FailoverUiPhase::Failed {
            reason: "no se pudo derivar addr de reconnect".into(),
        };
        return false;
    };
    *net = NetworkRuntime::offline();
    status.label = format!("reconectando {addr}");
    status.failover_phase = FailoverUiPhase::Reconnecting {
        addr: addr.clone(),
        attempt: 1,
    };
    commands.insert_resource(PendingFailoverReconnect {
        addr,
        attempts_left: RECONNECT_ATTEMPTS,
        next_try: Instant::now(),
        attempt: 1,
    });
    true
}

/// Un intento de reconnect por frame (o tras el gap).
pub fn tick_pending_reconnect(
    mut commands: Commands,
    pending: Option<ResMut<PendingFailoverReconnect>>,
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<NetworkStatus>,
    mut failover: Option<ResMut<ClientFailoverState>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    if Instant::now() < pending.next_try {
        return;
    }
    let addr = pending.addr.clone();
    match ClientSession::connect(&addr) {
        Ok(client) => {
            install_client(client.handle());
            info!(
                "failover: reconectado a {addr} (intento {})",
                pending.attempt
            );
            status.label = format!("client {addr} (failover)");
            status.desync = None;
            status.failover_phase = FailoverUiPhase::Idle;
            if let Some(fo) = failover.as_mut() {
                fo.server_addr = addr;
                fo.reconnect_addr = None;
                fo.failover_attempted = false;
                fo.note_rx();
            }
            *net = NetworkRuntime::client(client);
            commands.remove_resource::<PendingFailoverReconnect>();
        }
        Err(e) => {
            pending.attempts_left = pending.attempts_left.saturating_sub(1);
            if pending.attempts_left == 0 {
                error!("failover: no se pudo conectar a {addr}: {e}");
                status.label = format!("failover fallido: {addr}");
                status.failover_phase = FailoverUiPhase::Failed {
                    reason: format!("no se pudo conectar a {addr}"),
                };
                commands.remove_resource::<PendingFailoverReconnect>();
            } else {
                pending.attempt = pending.attempt.saturating_add(1);
                pending.next_try = Instant::now() + RECONNECT_RETRY_GAP;
                status.failover_phase = FailoverUiPhase::Reconnecting {
                    addr: addr.clone(),
                    attempt: pending.attempt,
                };
                status.label = format!("reconectando {addr} ({})", pending.attempt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ClientFailoverState;

    #[test]
    fn reconnect_prefers_host_announce_over_port_bump() {
        let mut fo = ClientFailoverState::new("127.0.0.1:3979".into());
        fo.reconnect_addr = Some("127.0.0.1:3980".into());
        assert_eq!(
            fo.resolve_reconnect_addr().as_deref(),
            Some("127.0.0.1:3980")
        );
        // Sin anuncio: convención puerto+1.
        fo.reconnect_addr = None;
        assert_eq!(
            fo.resolve_reconnect_addr().as_deref(),
            Some("127.0.0.1:3980")
        );
        // Anuncio ya en puerto final: no debe sumar otro +1.
        fo.server_addr = "127.0.0.1:3980".into();
        fo.reconnect_addr = Some("127.0.0.1:3980".into());
        assert_eq!(
            fo.resolve_reconnect_addr().as_deref(),
            Some("127.0.0.1:3980")
        );
    }
}
