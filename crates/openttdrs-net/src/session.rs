//! Sesiones listen-server y cliente (protocolo v3 / ADR 0004).

use std::collections::VecDeque;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use openttdrs_core::prelude::*;
use openttdrs_core::{Command, CompanyId};

use crate::codec::{FrameDecoder, FrameWriter};
use crate::protocol::{NetError, NetMessage, PROTOCOL_VERSION};

/// Elige el nuevo host: menor `peer_id` vivo (ADR 0004).
#[must_use]
pub fn elect_new_host(alive: &[u64]) -> Option<u64> {
    alive.iter().copied().min()
}

/// Eventos hacia el hilo de UI / simulación.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Snapshot inicial (solo cliente, tras Welcome).
    Welcome {
        snapshot_json: String,
        next_seq: u64,
        peer_id: u64,
    },
    /// Comando autorizado a aplicar.
    Commit {
        seq: u64,
        company_id: CompanyId,
        command: Command,
    },
    /// Avanzar ticks.
    AdvanceTicks { count: u32 },
    /// Comprobar hash; si diverge, emitir desync.
    HashCheck { tick: u64, hash: u64 },
    /// Desync detectado.
    Desync {
        tick: u64,
        expected_hash: u64,
        actual_hash: u64,
    },
    /// Anuncio de nuevo listen-server (failover).
    HostAnnounce {
        bind: String,
        next_seq: u64,
        new_host_peer_id: u64,
    },
    /// Lista de peers vivos (elección de host).
    PeerList { peer_ids: Vec<u64> },
    /// Keep-alive del host.
    Heartbeat { tick: u64 },
    /// La autoridad rechazó una propuesta antes de incorporarla al log.
    CommandRejected { message: String },
    /// Peer desconectado o error fatal.
    Disconnected { reason: String },
}

/// Drena un evento de sesión y sintetiza el fin del hilo una sola vez.
///
/// `mpsc::Receiver::try_recv` devuelve `Disconnected` en cada consulta una vez
/// que se cerró el canal. Los consumidores drenan hasta `None`, por lo que
/// convertir ese estado directamente en un evento infinito bloquea su frame.
/// Los eventos encolados siguen saliendo primero; después se entrega una única
/// desconexión terminal y las consultas posteriores devuelven `None`.
fn try_recv_session_event(
    event_rx: &Receiver<SessionEvent>,
    terminal_delivered: &AtomicBool,
    terminal_reason: &str,
) -> Option<SessionEvent> {
    match event_rx.try_recv() {
        Ok(event) => {
            if matches!(&event, SessionEvent::Disconnected { .. }) {
                terminal_delivered.store(true, Ordering::Relaxed);
            }
            Some(event)
        }
        Err(TryRecvError::Disconnected) if !terminal_delivered.swap(true, Ordering::Relaxed) => {
            Some(SessionEvent::Disconnected {
                reason: terminal_reason.into(),
            })
        }
        Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
    }
}

enum ServerCmd {
    LocalCommit {
        company_id: CompanyId,
        command: Command,
    },
    Advance(u32),
    HashCheck {
        tick: u64,
        hash: u64,
    },
    Heartbeat {
        tick: u64,
    },
    HostAnnounce {
        bind: String,
        next_seq: u64,
        new_host_peer_id: u64,
    },
    Shutdown,
}

/// Snapshot vivo compartido con el hilo de accept (late join).
type LiveSnapshot = Arc<Mutex<String>>;
type SharedSeq = Arc<Mutex<u64>>;
type SharedPeerIds = Arc<Mutex<Vec<u64>>>;

/// Límites de progreso para el transporte TCP de una sesión.
///
/// Los plazos se reinician con cada byte leído o escrito. Por tanto un save
/// grande puede continuar mientras haya progreso, pero una conexión que queda
/// detenida a mitad de handshake, frame o envío no inmoviliza el resto de la
/// sesión.
#[derive(Debug, Clone, Copy)]
pub struct SessionTimeouts {
    /// Máximo sin progreso mientras se espera Hello/Welcome.
    pub handshake: Duration,
    /// Máximo sin progreso para un frame o una cola de salida ya iniciados.
    pub stalled_peer: Duration,
    /// Pausa cooperativa entre sondeos non-blocking sin trabajo pendiente.
    pub poll_interval: Duration,
}

impl Default for SessionTimeouts {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(10),
            stalled_peer: Duration::from_secs(30),
            poll_interval: Duration::from_millis(2),
        }
    }
}

/// Handle clonable para emitir commits/ticks desde el hilo de UI.
#[derive(Clone)]
pub struct ListenServerHandle {
    cmd_tx: Sender<ServerCmd>,
    live_snapshot: LiveSnapshot,
    next_seq: SharedSeq,
    peer_ids: SharedPeerIds,
}

impl ListenServerHandle {
    pub fn broadcast_commit(&self, command: Command) -> Result<(), NetError> {
        self.broadcast_commit_for_company(CompanyId::PLAYER, command)
    }

    /// Publica un comando originado por una compañía concreta del host.
    pub fn broadcast_commit_for_company(
        &self,
        company_id: CompanyId,
        command: Command,
    ) -> Result<(), NetError> {
        self.cmd_tx
            .send(ServerCmd::LocalCommit {
                company_id,
                command,
            })
            .map_err(|_| NetError::Closed)
    }

    pub fn broadcast_advance(&self, count: u32) -> Result<(), NetError> {
        self.cmd_tx
            .send(ServerCmd::Advance(count))
            .map_err(|_| NetError::Closed)
    }

    pub fn broadcast_hash(&self, tick: u64, hash: u64) -> Result<(), NetError> {
        self.cmd_tx
            .send(ServerCmd::HashCheck { tick, hash })
            .map_err(|_| NetError::Closed)
    }

    /// Keep-alive para que los clientes detecten un host caído.
    pub fn broadcast_heartbeat(&self, tick: u64) -> Result<(), NetError> {
        self.cmd_tx
            .send(ServerCmd::Heartbeat { tick })
            .map_err(|_| NetError::Closed)
    }

    /// Retransmite [`NetMessage::HostAnnounce`] a los peers conectados.
    pub fn broadcast_host_announce(
        &self,
        bind: String,
        next_seq: u64,
        new_host_peer_id: u64,
    ) -> Result<(), NetError> {
        self.cmd_tx
            .send(ServerCmd::HostAnnounce {
                bind,
                next_seq,
                new_host_peer_id,
            })
            .map_err(|_| NetError::Closed)
    }

    /// Actualiza de inmediato el JSON de `Welcome` (visible al próximo accept).
    pub fn update_snapshot(&self, snapshot_json: String) {
        let mut guard = self
            .live_snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = snapshot_json;
    }

    /// Próximo `seq` de commit (para continuidad tras failover).
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        *self
            .next_seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `peer_id` de clientes actualmente conectados.
    #[must_use]
    pub fn peer_ids(&self) -> Vec<u64> {
        self.peer_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Listen-server: acepta clientes y retransmite commits/ticks/hashes.
pub struct ListenServer {
    handle: ListenServerHandle,
    event_rx: Receiver<SessionEvent>,
    terminal_delivered: AtomicBool,
    join: Option<JoinHandle<()>>,
    local_addr: SocketAddr,
}

impl ListenServer {
    /// Arranca el servidor en un hilo con `next_seq = 1`.
    pub fn start(bind: &str, snapshot_json: String) -> Result<Self, NetError> {
        Self::start_with_timeouts(bind, snapshot_json, SessionTimeouts::default())
    }

    /// Arranca un servidor con plazos de transporte explícitos.
    ///
    /// Es útil para hosts que quieren una política distinta y para pruebas que
    /// verifican la expiración de peers detenidos sin esperar los valores de
    /// producción.
    pub fn start_with_timeouts(
        bind: &str,
        snapshot_json: String,
        timeouts: SessionTimeouts,
    ) -> Result<Self, NetError> {
        Self::start_with_seq_and_timeouts(bind, snapshot_json, 1, timeouts)
    }

    /// Arranca el servidor continuando desde `initial_next_seq` (failover ADR 0004).
    ///
    /// `snapshot_json` es el Welcome inicial; el host debe llamar
    /// [`ListenServerHandle::update_snapshot`] tras cada avance de sim para que
    /// los late-joiners reciban el estado **actual** (no el del arranque).
    pub fn start_with_seq(
        bind: &str,
        snapshot_json: String,
        initial_next_seq: u64,
    ) -> Result<Self, NetError> {
        Self::start_with_seq_and_timeouts(
            bind,
            snapshot_json,
            initial_next_seq,
            SessionTimeouts::default(),
        )
    }

    fn start_with_seq_and_timeouts(
        bind: &str,
        snapshot_json: String,
        initial_next_seq: u64,
        timeouts: SessionTimeouts,
    ) -> Result<Self, NetError> {
        let listener = crate::listen(bind)?;
        let local_addr = listener.local_addr()?;
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let live: LiveSnapshot = Arc::new(Mutex::new(snapshot_json));
        let next_seq: SharedSeq = Arc::new(Mutex::new(initial_next_seq.max(1)));
        let peer_ids: SharedPeerIds = Arc::new(Mutex::new(Vec::new()));
        let live_thread = Arc::clone(&live);
        let next_seq_thread = Arc::clone(&next_seq);
        let peer_ids_thread = Arc::clone(&peer_ids);
        let bind_owned = bind.to_string();
        let join = thread::Builder::new()
            .name("openttdrs-listen".into())
            .spawn(move || {
                server_thread(
                    listener,
                    live_thread,
                    next_seq_thread,
                    peer_ids_thread,
                    cmd_rx,
                    event_tx,
                    &bind_owned,
                    timeouts,
                );
            })
            .map_err(NetError::Io)?;
        Ok(Self {
            handle: ListenServerHandle {
                cmd_tx,
                live_snapshot: live,
                next_seq,
                peer_ids,
            },
            event_rx,
            terminal_delivered: AtomicBool::new(false),
            join: Some(join),
            local_addr,
        })
    }

    /// Dirección efectiva del listener, útil cuando se eligió un puerto efímero.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    #[must_use]
    pub fn handle(&self) -> ListenServerHandle {
        self.handle.clone()
    }

    /// Comando originado en el host (listen-server): se aplica localmente en el caller
    /// y se retransmite a los peers.
    pub fn broadcast_commit(&self, command: Command) -> Result<(), NetError> {
        self.handle.broadcast_commit(command)
    }

    /// Publica un comando originado por una compañía concreta del host.
    pub fn broadcast_commit_for_company(
        &self,
        company_id: CompanyId,
        command: Command,
    ) -> Result<(), NetError> {
        self.handle
            .broadcast_commit_for_company(company_id, command)
    }

    pub fn broadcast_advance(&self, count: u32) -> Result<(), NetError> {
        self.handle.broadcast_advance(count)
    }

    pub fn broadcast_hash(&self, tick: u64, hash: u64) -> Result<(), NetError> {
        self.handle.broadcast_hash(tick, hash)
    }

    pub fn broadcast_heartbeat(&self, tick: u64) -> Result<(), NetError> {
        self.handle.broadcast_heartbeat(tick)
    }

    pub fn broadcast_host_announce(
        &self,
        bind: String,
        next_seq: u64,
        new_host_peer_id: u64,
    ) -> Result<(), NetError> {
        self.handle
            .broadcast_host_announce(bind, next_seq, new_host_peer_id)
    }

    pub fn update_snapshot(&self, snapshot_json: String) {
        self.handle.update_snapshot(snapshot_json);
    }

    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.handle.next_seq()
    }

    #[must_use]
    pub fn peer_ids(&self) -> Vec<u64> {
        self.handle.peer_ids()
    }

    /// Eventos remotos (p.ej. Propose ya convertido en Commit por el hilo).
    pub fn try_recv(&self) -> Option<SessionEvent> {
        try_recv_session_event(
            &self.event_rx,
            &self.terminal_delivered,
            "server thread ended",
        )
    }
}

impl Drop for ListenServer {
    fn drop(&mut self) {
        let _ = self.handle.cmd_tx.send(ServerCmd::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

const MAX_PENDING_HANDSHAKES: usize = 64;
const MAX_ACCEPTS_PER_TICK: usize = 16;

struct ClientSlot {
    stream: TcpStream,
    decoder: FrameDecoder,
    writer: FrameWriter,
    peer_id: u64,
    company_id: CompanyId,
    last_progress: Instant,
}

struct PendingHandshake {
    stream: TcpStream,
    decoder: FrameDecoder,
    writer: FrameWriter,
    peer_id: u64,
    company_id: Option<CompanyId>,
    last_progress: Instant,
}

enum PendingHandshakePoll {
    Keep,
    Promote,
    Drop(String),
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn server_thread(
    listener: TcpListener,
    live_snapshot: LiveSnapshot,
    shared_next_seq: SharedSeq,
    shared_peer_ids: SharedPeerIds,
    cmd_rx: Receiver<ServerCmd>,
    event_tx: Sender<SessionEvent>,
    bind: &str,
    timeouts: SessionTimeouts,
) {
    if let Err(e) = listener.set_nonblocking(true) {
        let _ = event_tx.send(SessionEvent::Disconnected {
            reason: format!("set_nonblocking: {e}"),
        });
        return;
    }
    let mut clients: Vec<ClientSlot> = Vec::new();
    let mut pending_handshakes: Vec<PendingHandshake> = Vec::new();
    let mut next_peer_id: u64 = 1;
    // Copia autoritativa para validar propuestas antes de asignarles secuencia.
    // El host publica snapshots; cuando cambian, esta copia se realinea para
    // incluir ticks y mutaciones locales.
    let mut authority_snapshot = live_snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let mut authority_state = GameState::load_json(&authority_snapshot).ok();
    eprintln!("openttdrs-net: listen-server on {bind}");

    loop {
        let current_snapshot = live_snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if current_snapshot != authority_snapshot
            && let Ok(state) = GameState::load_json(&current_snapshot)
        {
            authority_state = Some(state);
            authority_snapshot.clone_from(&current_snapshot);
        }
        let next_seq = *shared_next_seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Err(error) =
            accept_pending_handshakes(&listener, &mut pending_handshakes, &mut next_peer_id)
        {
            let _ = event_tx.send(SessionEvent::Disconnected {
                reason: format!("accept: {error}"),
            });
            return;
        }
        poll_pending_handshakes(
            &mut pending_handshakes,
            &mut clients,
            &current_snapshot,
            next_seq,
            &shared_peer_ids,
            timeouts,
        );

        let now = Instant::now();
        let mut i = 0;
        while i < clients.len() {
            let peer_id = clients[i].peer_id;
            let incoming = poll_client_transport(&mut clients[i], now, timeouts);
            match incoming {
                Ok(Some(NetMessage::Propose {
                    company_id,
                    command,
                })) => {
                    let assigned_company = clients[i].company_id;
                    if company_id != assigned_company {
                        let message = format!(
                            "peer {} no puede emitir como compañía {} (asignada {})",
                            peer_id, company_id.0, assigned_company.0
                        );
                        let response = NetMessage::Reject {
                            message: message.clone(),
                        };
                        if let Err(error) = enqueue_for_client(&mut clients[i], &response) {
                            eprintln!(
                                "openttdrs-net: reject queue failed peer_id={peer_id}: {error}"
                            );
                            remove_client(&mut clients, &shared_peer_ids, i);
                            broadcast_peer_list(&mut clients, &shared_peer_ids);
                            continue;
                        }
                        let _ = event_tx.send(SessionEvent::CommandRejected { message });
                        i += 1;
                        continue;
                    }
                    if let Some(state) = authority_state.as_mut()
                        && let Err(error) = apply_command_as_company(state, company_id, &command)
                    {
                        let response = NetMessage::Reject {
                            message: error.clone(),
                        };
                        if let Err(queue_error) = enqueue_for_client(&mut clients[i], &response) {
                            eprintln!(
                                "openttdrs-net: reject queue failed peer_id={peer_id}: {queue_error}"
                            );
                            remove_client(&mut clients, &shared_peer_ids, i);
                            broadcast_peer_list(&mut clients, &shared_peer_ids);
                            continue;
                        }
                        let _ = event_tx.send(SessionEvent::CommandRejected { message: error });
                        i += 1;
                        continue;
                    }
                    let seq = reserve_next_seq(&shared_next_seq);
                    let commit = NetMessage::Commit {
                        seq,
                        company_id,
                        command: command.clone(),
                    };
                    broadcast_and_refresh_peer_list(&mut clients, &shared_peer_ids, &commit);
                    let _ = event_tx.send(SessionEvent::Commit {
                        seq,
                        company_id,
                        command,
                    });
                    if clients.get(i).map(|client| client.peer_id) == Some(peer_id) {
                        i += 1;
                    }
                }
                Ok(Some(NetMessage::Desync {
                    tick,
                    expected_hash,
                    actual_hash,
                })) => {
                    // Propagar el diagnóstico para que todos los peers lo hagan
                    // visible y reparar al emisor con el último snapshot
                    // autoritativo, sin realizar una escritura bloqueante.
                    let report = NetMessage::Desync {
                        tick,
                        expected_hash,
                        actual_hash,
                    };
                    broadcast_and_refresh_peer_list(&mut clients, &shared_peer_ids, &report);
                    if let Some(index) = clients.iter().position(|client| client.peer_id == peer_id)
                    {
                        let resync = NetMessage::Welcome {
                            protocol: PROTOCOL_VERSION,
                            snapshot_json: current_snapshot.clone(),
                            next_seq,
                            peer_id,
                            company_id: clients[index].company_id,
                        };
                        if let Err(error) = enqueue_for_client(&mut clients[index], &resync) {
                            eprintln!(
                                "openttdrs-net: desync resync queue failed peer_id={peer_id}: {error}"
                            );
                            remove_client(&mut clients, &shared_peer_ids, index);
                            broadcast_peer_list(&mut clients, &shared_peer_ids);
                        }
                    }
                    let _ = event_tx.send(SessionEvent::Desync {
                        tick,
                        expected_hash,
                        actual_hash,
                    });
                    if clients.get(i).map(|client| client.peer_id) == Some(peer_id) {
                        i += 1;
                    }
                }
                Ok(None | Some(NetMessage::Hello { .. })) => i += 1,
                Ok(Some(other)) => {
                    eprintln!("openttdrs-net: unexpected from client: {other:?}");
                    i += 1;
                }
                Err(error) => {
                    eprintln!("openttdrs-net: client dropped peer_id={peer_id}: {error}");
                    remove_client(&mut clients, &shared_peer_ids, i);
                    broadcast_peer_list(&mut clients, &shared_peer_ids);
                }
            }
        }

        match cmd_rx.try_recv() {
            Ok(ServerCmd::LocalCommit {
                company_id,
                command,
            }) => {
                if let Some(state) = authority_state.as_mut() {
                    // El host ya aplicó la mutación en su hilo de simulación;
                    // esta copia sólo necesita avanzar para validar propuestas
                    // posteriores. Un error aquí indica snapshot atrasado.
                    let _ = apply_command_as_company(state, company_id, &command);
                }
                let commit = NetMessage::Commit {
                    seq: reserve_next_seq(&shared_next_seq),
                    company_id,
                    command,
                };
                broadcast_and_refresh_peer_list(&mut clients, &shared_peer_ids, &commit);
            }
            Ok(ServerCmd::Advance(count)) => {
                if let Some(state) = authority_state.as_mut() {
                    for _ in 0..count {
                        state.step();
                    }
                }
                broadcast_and_refresh_peer_list(
                    &mut clients,
                    &shared_peer_ids,
                    &NetMessage::AdvanceTicks { count },
                );
            }
            Ok(ServerCmd::HashCheck { tick, hash }) => {
                broadcast_and_refresh_peer_list(
                    &mut clients,
                    &shared_peer_ids,
                    &NetMessage::HashCheck { tick, hash },
                );
            }
            Ok(ServerCmd::Heartbeat { tick }) => {
                broadcast_and_refresh_peer_list(
                    &mut clients,
                    &shared_peer_ids,
                    &NetMessage::Heartbeat { tick },
                );
            }
            Ok(ServerCmd::HostAnnounce {
                bind,
                next_seq,
                new_host_peer_id,
            }) => {
                broadcast_and_refresh_peer_list(
                    &mut clients,
                    &shared_peer_ids,
                    &NetMessage::HostAnnounce {
                        bind,
                        next_seq,
                        new_host_peer_id,
                    },
                );
            }
            Ok(ServerCmd::Shutdown) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => thread::sleep(timeouts.poll_interval),
        }
    }
}

fn accept_pending_handshakes(
    listener: &TcpListener,
    pending_handshakes: &mut Vec<PendingHandshake>,
    next_peer_id: &mut u64,
) -> Result<(), NetError> {
    for _ in 0..MAX_ACCEPTS_PER_TICK {
        match listener.accept() {
            Ok((stream, addr)) => {
                eprintln!("openttdrs-net: client connected {addr}");
                if pending_handshakes.len() >= MAX_PENDING_HANDSHAKES {
                    eprintln!("openttdrs-net: pending handshake limit reached; dropping {addr}");
                    continue;
                }
                configure_stream(&stream)?;
                let peer_id = *next_peer_id;
                *next_peer_id = next_peer_id.saturating_add(1);
                pending_handshakes.push(PendingHandshake {
                    stream,
                    decoder: FrameDecoder::default(),
                    writer: FrameWriter::default(),
                    peer_id,
                    company_id: None,
                    last_progress: Instant::now(),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(NetError::Io(error)),
        }
    }
    Ok(())
}

fn poll_pending_handshakes(
    pending_handshakes: &mut Vec<PendingHandshake>,
    clients: &mut Vec<ClientSlot>,
    snapshot_json: &str,
    next_seq: u64,
    shared_peer_ids: &SharedPeerIds,
    timeouts: SessionTimeouts,
) {
    let now = Instant::now();
    let mut i = 0;
    while i < pending_handshakes.len() {
        let candidate_company = allocate_company_id(clients, pending_handshakes);
        let outcome = poll_pending_handshake(
            &mut pending_handshakes[i],
            snapshot_json,
            next_seq,
            candidate_company,
            now,
            timeouts,
        );
        match outcome {
            PendingHandshakePoll::Keep => i += 1,
            PendingHandshakePoll::Drop(reason) => {
                eprintln!(
                    "openttdrs-net: handshake dropped peer_id={}: {reason}",
                    pending_handshakes[i].peer_id
                );
                pending_handshakes.remove(i);
            }
            PendingHandshakePoll::Promote => {
                let pending = pending_handshakes.remove(i);
                let Some(company_id) = pending.company_id else {
                    eprintln!(
                        "openttdrs-net: handshake promotion without company peer_id={}",
                        pending.peer_id
                    );
                    continue;
                };
                let peer_id = pending.peer_id;
                shared_peer_ids
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(peer_id);
                clients.push(ClientSlot {
                    stream: pending.stream,
                    decoder: pending.decoder,
                    writer: pending.writer,
                    peer_id,
                    company_id,
                    last_progress: pending.last_progress,
                });
                broadcast_peer_list(clients, shared_peer_ids);
            }
        }
    }
}

fn poll_pending_handshake(
    pending: &mut PendingHandshake,
    snapshot_json: &str,
    next_seq: u64,
    candidate_company: CompanyId,
    now: Instant,
    timeouts: SessionTimeouts,
) -> PendingHandshakePoll {
    let incoming = match pending.decoder.try_read_with_progress(&mut pending.stream) {
        Ok(incoming) => incoming,
        Err(error) => return PendingHandshakePoll::Drop(error.to_string()),
    };
    if incoming.made_progress {
        pending.last_progress = now;
    }
    if let Some(message) = incoming.message {
        if pending.company_id.is_some() {
            return PendingHandshakePoll::Drop(
                "message received before Welcome was flushed".into(),
            );
        }
        match message {
            NetMessage::Hello {
                protocol: PROTOCOL_VERSION,
            } => {
                let welcome = NetMessage::Welcome {
                    protocol: PROTOCOL_VERSION,
                    snapshot_json: snapshot_json.to_string(),
                    next_seq,
                    peer_id: pending.peer_id,
                    company_id: candidate_company,
                };
                if let Err(error) = pending.writer.queue(&welcome) {
                    return PendingHandshakePoll::Drop(error.to_string());
                }
                pending.company_id = Some(candidate_company);
                pending.last_progress = now;
            }
            NetMessage::Hello { protocol } => {
                return PendingHandshakePoll::Drop(format!("unsupported protocol {protocol}"));
            }
            other => {
                return PendingHandshakePoll::Drop(format!("expected Hello, got {other:?}"));
            }
        }
    }

    let output = match pending.writer.try_flush(&mut pending.stream) {
        Ok(output) => output,
        Err(error) => return PendingHandshakePoll::Drop(error.to_string()),
    };
    if output.made_progress {
        pending.last_progress = now;
    }
    if pending.company_id.is_some() && output.is_empty {
        return PendingHandshakePoll::Promote;
    }
    if now.duration_since(pending.last_progress) >= timeouts.handshake {
        return PendingHandshakePoll::Drop("handshake timed out without progress".into());
    }
    PendingHandshakePoll::Keep
}

fn poll_client_transport(
    client: &mut ClientSlot,
    now: Instant,
    timeouts: SessionTimeouts,
) -> Result<Option<NetMessage>, NetError> {
    let incoming = client.decoder.try_read_with_progress(&mut client.stream)?;
    if incoming.made_progress {
        client.last_progress = now;
    }
    let output = client.writer.try_flush(&mut client.stream)?;
    if output.made_progress {
        client.last_progress = now;
    }
    if (client.decoder.has_partial_frame() || client.writer.has_pending())
        && now.duration_since(client.last_progress) >= timeouts.stalled_peer
    {
        return Err(NetError::Protocol(
            "peer stalled without transport progress".into(),
        ));
    }
    Ok(incoming.message)
}

fn reserve_next_seq(shared_next_seq: &SharedSeq) -> u64 {
    let mut guard = shared_next_seq
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let seq = *guard;
    *guard = seq.saturating_add(1);
    seq
}

fn remove_peer_id(shared: &SharedPeerIds, peer_id: u64) {
    let mut guard = shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.retain(|id| *id != peer_id);
}

fn remove_client(clients: &mut Vec<ClientSlot>, shared_peer_ids: &SharedPeerIds, index: usize) {
    let peer_id = clients[index].peer_id;
    clients.remove(index);
    remove_peer_id(shared_peer_ids, peer_id);
}

/// Asigna una compañía exclusiva a cada peer conectado. La compañía 0 queda
/// reservada al host; los clientes reciben el primer id libre del pool de
/// `OpenTTD` (0..15). Los handshakes que ya enviaron Welcome también reservan
/// su id para que dos conexiones simultáneas no reciban la misma compañía.
fn allocate_company_id(clients: &[ClientSlot], pending: &[PendingHandshake]) -> CompanyId {
    (1..=15)
        .map(CompanyId)
        .find(|candidate| {
            clients.iter().all(|slot| slot.company_id != *candidate)
                && pending
                    .iter()
                    .all(|handshake| handshake.company_id != Some(*candidate))
        })
        .unwrap_or(CompanyId::PLAYER)
}

fn enqueue_for_client(client: &mut ClientSlot, message: &NetMessage) -> Result<(), NetError> {
    client.writer.queue(message)?;
    // Encolar una operación inicia un nuevo plazo de progreso, incluso si el
    // peer llevaba tiempo idle antes de recibir este broadcast.
    client.last_progress = Instant::now();
    Ok(())
}

/// Encola un broadcast y elimina sólo los peers cuya cola ya no puede
/// aceptar el frame. Nunca realiza I/O bloqueante dentro del loop compartido.
fn broadcast_raw(
    clients: &mut Vec<ClientSlot>,
    shared_peer_ids: &SharedPeerIds,
    message: &NetMessage,
) -> bool {
    let mut dead = Vec::new();
    for (index, client) in clients.iter_mut().enumerate() {
        if let Err(error) = enqueue_for_client(client, message) {
            eprintln!(
                "openttdrs-net: broadcast queue failed peer_id={}: {error}",
                client.peer_id
            );
            dead.push(index);
        }
    }
    let removed = !dead.is_empty();
    for index in dead.into_iter().rev() {
        remove_client(clients, shared_peer_ids, index);
    }
    removed
}

fn broadcast_and_refresh_peer_list(
    clients: &mut Vec<ClientSlot>,
    shared_peer_ids: &SharedPeerIds,
    message: &NetMessage,
) {
    if broadcast_raw(clients, shared_peer_ids, message) {
        broadcast_peer_list(clients, shared_peer_ids);
    }
}

fn broadcast_peer_list(clients: &mut Vec<ClientSlot>, shared_peer_ids: &SharedPeerIds) {
    // Un peer puede agotar su cola al intentar enviarle la primera lista; una
    // segunda vuelta informa la lista final a quienes siguen sanos sin entrar
    // en recursión si varios peers fallan a la vez.
    for _ in 0..2 {
        let peer_ids = shared_peer_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if !broadcast_raw(clients, shared_peer_ids, &NetMessage::PeerList { peer_ids }) {
            break;
        }
    }
}

fn configure_stream(stream: &TcpStream) -> Result<(), NetError> {
    stream.set_nodelay(true)?;
    stream.set_nonblocking(true)?;
    Ok(())
}

enum ClientCmd {
    Propose {
        company_id: CompanyId,
        command: Command,
    },
    ReportDesync {
        tick: u64,
        expected_hash: u64,
        actual_hash: u64,
    },
    Shutdown,
}

/// Handle clonable para proponer comandos desde la UI.
#[derive(Clone)]
pub struct ClientSessionHandle {
    cmd_tx: Sender<ClientCmd>,
    company_id: std::sync::Arc<std::sync::Mutex<CompanyId>>,
}

impl ClientSessionHandle {
    pub fn propose(&self, command: Command) -> Result<(), NetError> {
        let company_id = *self
            .company_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.cmd_tx
            .send(ClientCmd::Propose {
                company_id,
                command,
            })
            .map_err(|_| NetError::Closed)
    }

    /// Compañía asignada por el servidor durante el handshake.
    #[must_use]
    pub fn company_id(&self) -> CompanyId {
        *self
            .company_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Informa al servidor de una divergencia detectada por este peer.
    pub fn report_desync(
        &self,
        tick: u64,
        expected_hash: u64,
        actual_hash: u64,
    ) -> Result<(), NetError> {
        self.cmd_tx
            .send(ClientCmd::ReportDesync {
                tick,
                expected_hash,
                actual_hash,
            })
            .map_err(|_| NetError::Closed)
    }
}

/// Cliente TCP: recibe commits/ticks y puede proponer comandos.
pub struct ClientSession {
    handle: ClientSessionHandle,
    event_rx: Receiver<SessionEvent>,
    terminal_delivered: AtomicBool,
    join: Option<JoinHandle<()>>,
}

impl ClientSession {
    pub fn connect(addr: &str) -> Result<Self, NetError> {
        Self::connect_with_timeouts(addr, SessionTimeouts::default())
    }

    /// Conecta un cliente con plazos de progreso explícitos.
    pub fn connect_with_timeouts(addr: &str, timeouts: SessionTimeouts) -> Result<Self, NetError> {
        let stream = crate::connect(addr)?;
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let company_id = std::sync::Arc::new(std::sync::Mutex::new(CompanyId::PLAYER));
        let company_id_thread = std::sync::Arc::clone(&company_id);
        let addr_owned = addr.to_string();
        let join = thread::Builder::new()
            .name("openttdrs-client-net".into())
            .spawn(move || {
                if let Err(e) = client_thread(stream, cmd_rx, event_tx, company_id_thread, timeouts)
                {
                    eprintln!("openttdrs-net: client ended ({addr_owned}): {e}");
                }
            })
            .map_err(NetError::Io)?;
        Ok(Self {
            handle: ClientSessionHandle { cmd_tx, company_id },
            event_rx,
            terminal_delivered: AtomicBool::new(false),
            join: Some(join),
        })
    }

    #[must_use]
    pub fn handle(&self) -> ClientSessionHandle {
        self.handle.clone()
    }

    pub fn propose(&self, command: Command) -> Result<(), NetError> {
        self.handle.propose(command)
    }

    pub fn report_desync(
        &self,
        tick: u64,
        expected_hash: u64,
        actual_hash: u64,
    ) -> Result<(), NetError> {
        self.handle.report_desync(tick, expected_hash, actual_hash)
    }

    pub fn try_recv(&self) -> Option<SessionEvent> {
        try_recv_session_event(
            &self.event_rx,
            &self.terminal_delivered,
            "client thread ended",
        )
    }
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        let _ = self.handle.cmd_tx.send(ClientCmd::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

const MAX_PENDING_CLIENT_COMMANDS: usize = 256;

#[allow(clippy::too_many_lines)]
fn client_thread(
    mut stream: TcpStream,
    cmd_rx: Receiver<ClientCmd>,
    event_tx: Sender<SessionEvent>,
    company_id: std::sync::Arc<std::sync::Mutex<CompanyId>>,
    timeouts: SessionTimeouts,
) -> Result<(), NetError> {
    configure_stream(&stream)?;
    let mut decoder = FrameDecoder::default();
    let mut writer = FrameWriter::default();
    writer.queue(&NetMessage::Hello {
        protocol: PROTOCOL_VERSION,
    })?;
    let mut waiting_for_welcome = true;
    let mut pending_commands = VecDeque::new();
    let mut last_progress = Instant::now();

    loop {
        let now = Instant::now();
        for _ in 0..MAX_PENDING_CLIENT_COMMANDS {
            match cmd_rx.try_recv() {
                Ok(ClientCmd::Shutdown) | Err(TryRecvError::Disconnected) => return Ok(()),
                Ok(command) => {
                    if pending_commands.len() >= MAX_PENDING_CLIENT_COMMANDS {
                        let reason = "client command queue limit exceeded".into();
                        let _ = event_tx.send(SessionEvent::Disconnected { reason });
                        return Ok(());
                    }
                    pending_commands.push_back(command);
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        if !waiting_for_welcome {
            while let Some(command) = pending_commands.pop_front() {
                let message = match command {
                    ClientCmd::Propose {
                        company_id,
                        command,
                    } => NetMessage::Propose {
                        company_id,
                        command,
                    },
                    ClientCmd::ReportDesync {
                        tick,
                        expected_hash,
                        actual_hash,
                    } => NetMessage::Desync {
                        tick,
                        expected_hash,
                        actual_hash,
                    },
                    ClientCmd::Shutdown => return Ok(()),
                };
                writer.queue(&message)?;
                last_progress = now;
            }
        }

        let output = match writer.try_flush(&mut stream) {
            Ok(output) => output,
            Err(error) => {
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: format!("server write failed: {error}"),
                });
                return Err(error);
            }
        };
        if output.made_progress {
            last_progress = now;
        }

        let incoming = match decoder.try_read_with_progress(&mut stream) {
            Ok(incoming) => incoming,
            Err(NetError::Closed) => {
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: "server closed".into(),
                });
                return Ok(());
            }
            Err(error) => {
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: format!("server read failed: {error}"),
                });
                return Err(error);
            }
        };
        if incoming.made_progress {
            last_progress = now;
        }

        match incoming.message {
            Some(NetMessage::Welcome {
                protocol,
                snapshot_json,
                next_seq,
                peer_id,
                company_id: assigned_company,
            }) if protocol == PROTOCOL_VERSION => {
                *company_id
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = assigned_company;
                let _ = event_tx.send(SessionEvent::Welcome {
                    snapshot_json,
                    next_seq,
                    peer_id,
                });
                waiting_for_welcome = false;
            }
            Some(NetMessage::Welcome { protocol, .. }) => {
                let reason = format!("unsupported protocol {protocol}");
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: reason.clone(),
                });
                return Err(NetError::Protocol(reason));
            }
            Some(other) if waiting_for_welcome => {
                let reason = format!("expected welcome, got {other:?}");
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: reason.clone(),
                });
                return Err(NetError::Protocol(reason));
            }
            Some(NetMessage::Commit {
                seq,
                company_id,
                command,
            }) => {
                let _ = event_tx.send(SessionEvent::Commit {
                    seq,
                    company_id,
                    command,
                });
            }
            Some(NetMessage::AdvanceTicks { count }) => {
                let _ = event_tx.send(SessionEvent::AdvanceTicks { count });
            }
            Some(NetMessage::HashCheck { tick, hash }) => {
                let _ = event_tx.send(SessionEvent::HashCheck { tick, hash });
            }
            Some(NetMessage::HostAnnounce {
                bind,
                next_seq,
                new_host_peer_id,
            }) => {
                let _ = event_tx.send(SessionEvent::HostAnnounce {
                    bind,
                    next_seq,
                    new_host_peer_id,
                });
            }
            Some(NetMessage::PeerList { peer_ids }) => {
                let _ = event_tx.send(SessionEvent::PeerList { peer_ids });
            }
            Some(NetMessage::Heartbeat { tick }) => {
                let _ = event_tx.send(SessionEvent::Heartbeat { tick });
            }
            Some(NetMessage::Reject { message }) => {
                let _ = event_tx.send(SessionEvent::CommandRejected { message });
            }
            Some(NetMessage::Desync {
                tick,
                expected_hash,
                actual_hash,
            }) => {
                let _ = event_tx.send(SessionEvent::Desync {
                    tick,
                    expected_hash,
                    actual_hash,
                });
            }
            Some(NetMessage::Error { message }) => {
                let _ = event_tx.send(SessionEvent::Disconnected { reason: message });
                return Ok(());
            }
            Some(other) => {
                eprintln!("openttdrs-net: ignore msg {other:?}");
            }
            None => {}
        }

        let deadline = if waiting_for_welcome {
            timeouts.handshake
        } else {
            timeouts.stalled_peer
        };
        if (waiting_for_welcome || decoder.has_partial_frame() || writer.has_pending())
            && now.duration_since(last_progress) >= deadline
        {
            let reason = if waiting_for_welcome {
                "handshake timed out without progress"
            } else {
                "server stalled without transport progress"
            };
            let _ = event_tx.send(SessionEvent::Disconnected {
                reason: reason.into(),
            });
            return Ok(());
        }
        thread::sleep(timeouts.poll_interval);
    }
}

/// Aplica commits y ticks a un [`GameState`] (útil en tests / dedicated).
///
/// La compañía del commit es contexto de autoridad y no forma parte del
/// `GameState` persistido como una selección local de UI.
pub fn apply_command_as_company(
    state: &mut GameState,
    company_id: CompanyId,
    command: &Command,
) -> Result<(), String> {
    ensure_company_slot(state, company_id)?;
    let previous = state.active_company;
    if previous != company_id && !state.set_active_company(company_id) {
        return Err(format!("compañía inexistente: {}", company_id.0));
    }
    let result = apply_command(state, command).map_err(|error| error.to_string());
    if previous != company_id {
        let _ = state.set_active_company(previous);
    }
    result
}

/// Materializa slots de compañía asignados por la sesión si el snapshot era
/// un mapa mínimo que todavía sólo contenía al jugador. Los slots creados por
/// peers son empresas humanas, no IA, y quedan dentro del estado replicado.
fn ensure_company_slot(state: &mut GameState, company_id: CompanyId) -> Result<(), String> {
    const MAX_COMPANIES: usize = 15;
    let index = company_id.index();
    if index >= MAX_COMPANIES {
        return Err(format!("compañía fuera de rango: {}", company_id.0));
    }
    state.ensure_companies();
    while state.companies.len() <= index {
        let id =
            CompanyId(u8::try_from(state.companies.len()).map_err(|_| "pool de compañías lleno")?);
        let colour = openttdrs_core::company::first_free_company_colour(&state.companies);
        let mut company =
            openttdrs_core::Company::player(openttdrs_core::CompanyEconomy::default(), colour);
        company.id = id;
        company.name = format!("Compañía {}", u16::from(id.0) + 1);
        state.companies.push(company);
    }
    Ok(())
}

/// Aplica commits y ticks a un [`GameState`] (útil en tests / dedicated).
pub fn apply_session_event(state: &mut GameState, event: &SessionEvent) -> Result<(), String> {
    match event {
        SessionEvent::Welcome { snapshot_json, .. } => {
            *state = GameState::load_json(snapshot_json).map_err(|e| e.to_string())?;
            Ok(())
        }
        SessionEvent::Commit {
            company_id,
            command,
            ..
        } => apply_command_as_company(state, *company_id, command),
        SessionEvent::AdvanceTicks { count } => {
            for _ in 0..*count {
                state.step();
            }
            Ok(())
        }
        SessionEvent::HashCheck { tick, hash } => {
            let local_tick = state.tick.get();
            if local_tick != *tick {
                // Late-join / cola: solo comparar cuando el tick coincide.
                return Ok(());
            }
            let actual = state.canonical_hash();
            if actual != *hash {
                return Err(format!(
                    "desync at tick {tick}: expected {hash:#x} got {actual:#x}"
                ));
            }
            Ok(())
        }
        SessionEvent::HostAnnounce { .. }
        | SessionEvent::PeerList { .. }
        | SessionEvent::Heartbeat { .. } => Ok(()),
        SessionEvent::CommandRejected { message } => Err(message.clone()),
        SessionEvent::Desync {
            tick,
            expected_hash,
            actual_hash,
        } => Err(format!(
            "remote desync tick={tick} expected={expected_hash:#x} actual={actual_hash:#x}"
        )),
        SessionEvent::Disconnected { reason } => Err(reason.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    use super::{SessionEvent, apply_command_as_company, elect_new_host, try_recv_session_event};
    use openttdrs_core::{Command, CompanyId, GameState, TileCoord};

    #[test]
    fn elect_new_host_picks_minimum_peer_id() {
        assert_eq!(elect_new_host(&[3, 1, 7]), Some(1));
        assert_eq!(elect_new_host(&[]), None);
        assert_eq!(elect_new_host(&[9]), Some(9));
    }

    #[test]
    fn command_runs_under_issuer_without_changing_local_selection() {
        let mut state = GameState::new(16, 16);
        assert_eq!(state.active_company, CompanyId::PLAYER);
        assert!(
            apply_command_as_company(
                &mut state,
                CompanyId(1),
                &Command::PlaceRail(TileCoord::new(3, 3)),
            )
            .is_ok(),
            "issuer company should be accepted"
        );
        assert_eq!(state.active_company, CompanyId::PLAYER);
        assert_eq!(
            state.map.get(TileCoord::new(3, 3)).map(|tile| tile.m1),
            Some(1)
        );
    }

    #[test]
    fn closed_receiver_delivers_pending_events_then_one_terminal_event() {
        let (sender, receiver) = mpsc::channel();
        assert!(sender.send(SessionEvent::Heartbeat { tick: 7 }).is_ok());
        drop(sender);
        let terminal_delivered = AtomicBool::new(false);

        assert!(matches!(
            try_recv_session_event(&receiver, &terminal_delivered, "thread ended"),
            Some(SessionEvent::Heartbeat { tick: 7 })
        ));
        assert!(matches!(
            try_recv_session_event(&receiver, &terminal_delivered, "thread ended"),
            Some(SessionEvent::Disconnected { reason }) if reason == "thread ended"
        ));
        assert!(try_recv_session_event(&receiver, &terminal_delivered, "thread ended").is_none());
    }

    #[test]
    fn explicit_disconnect_is_not_followed_by_a_synthetic_one() {
        let (sender, receiver) = mpsc::channel();
        assert!(
            sender
                .send(SessionEvent::Disconnected {
                    reason: "socket closed".into(),
                })
                .is_ok()
        );
        drop(sender);
        let terminal_delivered = AtomicBool::new(false);

        assert!(matches!(
            try_recv_session_event(&receiver, &terminal_delivered, "thread ended"),
            Some(SessionEvent::Disconnected { reason }) if reason == "socket closed"
        ));
        assert!(try_recv_session_event(&receiver, &terminal_delivered, "thread ended").is_none());
    }
}
