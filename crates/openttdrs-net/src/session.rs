//! Sesiones listen-server y cliente (protocolo v3 / ADR 0004).

use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use openttdrs_core::prelude::*;
use openttdrs_core::{Command, CompanyId};

use crate::codec::{read_message, write_message};
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
    join: Option<JoinHandle<()>>,
    local_addr: SocketAddr,
}

impl ListenServer {
    /// Arranca el servidor en un hilo con `next_seq = 1`.
    pub fn start(bind: &str, snapshot_json: String) -> Result<Self, NetError> {
        Self::start_with_seq(bind, snapshot_json, 1)
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
        match self.event_rx.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(SessionEvent::Disconnected {
                reason: "server thread ended".into(),
            }),
        }
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

struct ClientSlot {
    stream: TcpStream,
    peer_id: u64,
    company_id: CompanyId,
}

#[allow(clippy::too_many_lines)]
fn server_thread(
    listener: TcpListener,
    live_snapshot: LiveSnapshot,
    shared_next_seq: SharedSeq,
    shared_peer_ids: SharedPeerIds,
    cmd_rx: Receiver<ServerCmd>,
    event_tx: Sender<SessionEvent>,
    bind: &str,
) {
    if let Err(e) = listener.set_nonblocking(true) {
        let _ = event_tx.send(SessionEvent::Disconnected {
            reason: format!("set_nonblocking: {e}"),
        });
        return;
    }
    let mut clients: Vec<ClientSlot> = Vec::new();
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
            authority_snapshot = current_snapshot;
        }
        let next_seq = *shared_next_seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        match listener.accept() {
            Ok((stream, addr)) => {
                eprintln!("openttdrs-net: client connected {addr}");
                if let Err(e) = configure_stream(&stream) {
                    eprintln!("openttdrs-net: configure failed: {e}");
                    continue;
                }
                let mut stream = stream;
                let snapshot = live_snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                let peer_id = next_peer_id;
                next_peer_id = next_peer_id.saturating_add(1);
                let company_id = allocate_company_id(&clients);
                match handshake_server(&mut stream, &snapshot, next_seq, peer_id, company_id) {
                    Ok(()) => {
                        // Si el host avanzó durante el handshake, reenviar snapshot vivo.
                        let fresh = live_snapshot
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        if fresh != snapshot
                            && let Err(e) = write_message(
                                &mut stream,
                                &NetMessage::Welcome {
                                    protocol: PROTOCOL_VERSION,
                                    snapshot_json: fresh,
                                    next_seq,
                                    peer_id,
                                    company_id,
                                },
                            )
                        {
                            eprintln!("openttdrs-net: late-join resync failed: {e}");
                            continue;
                        }
                        shared_peer_ids
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(peer_id);
                        clients.push(ClientSlot {
                            stream,
                            peer_id,
                            company_id,
                        });
                        broadcast_peer_list(&mut clients, &shared_peer_ids);
                    }
                    Err(e) => eprintln!("openttdrs-net: handshake failed: {e}"),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: format!("accept: {e}"),
                });
                return;
            }
        }

        // Propuestas de clientes (non-blocking peek via set_nonblocking on clients).
        let mut i = 0;
        while i < clients.len() {
            match try_read_client(&mut clients[i].stream) {
                Ok(Some(NetMessage::Propose {
                    company_id,
                    command,
                })) => {
                    let assigned_company = clients[i].company_id;
                    if company_id != assigned_company {
                        let message = format!(
                            "peer {} no puede emitir como compañía {} (asignada {})",
                            clients[i].peer_id, company_id.0, assigned_company.0
                        );
                        let _ = write_message(
                            &mut clients[i].stream,
                            &NetMessage::Reject {
                                message: message.clone(),
                            },
                        );
                        let _ = event_tx.send(SessionEvent::CommandRejected { message });
                        i += 1;
                        continue;
                    }
                    if let Some(state) = authority_state.as_mut()
                        && let Err(error) = apply_command_as_company(state, company_id, &command)
                    {
                        let message = error;
                        let _ = write_message(
                            &mut clients[i].stream,
                            &NetMessage::Reject {
                                message: message.clone(),
                            },
                        );
                        let _ = event_tx.send(SessionEvent::CommandRejected { message });
                        i += 1;
                        continue;
                    }
                    let seq = {
                        let mut guard = shared_next_seq
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let seq = *guard;
                        *guard = seq.saturating_add(1);
                        seq
                    };
                    let commit = NetMessage::Commit {
                        seq,
                        company_id,
                        command: command.clone(),
                    };
                    broadcast_raw(&mut clients, &shared_peer_ids, &commit);
                    let _ = event_tx.send(SessionEvent::Commit {
                        seq,
                        company_id,
                        command,
                    });
                    i += 1;
                }
                Ok(Some(NetMessage::Desync {
                    tick,
                    expected_hash,
                    actual_hash,
                })) => {
                    // Propagar el diagnóstico para que todos los peers lo hagan
                    // visible y, además, reparar al emisor con el último snapshot
                    // autoritativo. Antes sólo se registraba el hash: el cliente
                    // quedaba divergido hasta reconectar manualmente.
                    let report = NetMessage::Desync {
                        tick,
                        expected_hash,
                        actual_hash,
                    };
                    broadcast_raw(&mut clients, &shared_peer_ids, &report);
                    let snapshot = live_snapshot
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    let resync = NetMessage::Welcome {
                        protocol: PROTOCOL_VERSION,
                        snapshot_json: snapshot,
                        next_seq,
                        peer_id: clients[i].peer_id,
                        company_id: clients[i].company_id,
                    };
                    if let Err(error) = write_message(&mut clients[i].stream, &resync) {
                        eprintln!(
                            "openttdrs-net: desync resync failed peer_id={}: {error}",
                            clients[i].peer_id
                        );
                    }
                    let _ = event_tx.send(SessionEvent::Desync {
                        tick,
                        expected_hash,
                        actual_hash,
                    });
                    i += 1;
                }
                Ok(None | Some(NetMessage::Hello { .. })) => i += 1,
                Ok(Some(other)) => {
                    eprintln!("openttdrs-net: unexpected from client: {other:?}");
                    i += 1;
                }
                Err(NetError::Closed | NetError::Io(_)) => {
                    let peer_id = clients[i].peer_id;
                    eprintln!("openttdrs-net: client dropped peer_id={peer_id}");
                    remove_peer_id(&shared_peer_ids, peer_id);
                    clients.remove(i);
                    broadcast_peer_list(&mut clients, &shared_peer_ids);
                }
                Err(e) => {
                    let peer_id = clients[i].peer_id;
                    eprintln!("openttdrs-net: client read error peer_id={peer_id}: {e}");
                    remove_peer_id(&shared_peer_ids, peer_id);
                    clients.remove(i);
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
                let seq = {
                    let mut guard = shared_next_seq
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let seq = *guard;
                    *guard = seq.saturating_add(1);
                    seq
                };
                let commit = NetMessage::Commit {
                    seq,
                    company_id,
                    command: command.clone(),
                };
                broadcast_raw(&mut clients, &shared_peer_ids, &commit);
            }
            Ok(ServerCmd::Advance(count)) => {
                if let Some(state) = authority_state.as_mut() {
                    for _ in 0..count {
                        state.step();
                    }
                }
                broadcast_raw(
                    &mut clients,
                    &shared_peer_ids,
                    &NetMessage::AdvanceTicks { count },
                );
            }
            Ok(ServerCmd::HashCheck { tick, hash }) => {
                broadcast_raw(
                    &mut clients,
                    &shared_peer_ids,
                    &NetMessage::HashCheck { tick, hash },
                );
            }
            Ok(ServerCmd::Heartbeat { tick }) => {
                broadcast_raw(
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
                broadcast_raw(
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
            Err(TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

fn remove_peer_id(shared: &SharedPeerIds, peer_id: u64) {
    let mut guard = shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.retain(|id| *id != peer_id);
}

/// Asigna una compañía exclusiva a cada peer conectado. La compañía 0 queda
/// reservada al host; los clientes reciben el primer id libre del pool de
/// `OpenTTD` (0..15).
fn allocate_company_id(clients: &[ClientSlot]) -> CompanyId {
    (1..=15)
        .map(CompanyId)
        .find(|candidate| clients.iter().all(|slot| slot.company_id != *candidate))
        .unwrap_or(CompanyId::PLAYER)
}

fn broadcast_peer_list(clients: &mut Vec<ClientSlot>, shared_peer_ids: &SharedPeerIds) {
    let peer_ids = shared_peer_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    broadcast_raw(clients, shared_peer_ids, &NetMessage::PeerList { peer_ids });
}

fn handshake_server(
    stream: &mut TcpStream,
    snapshot_json: &str,
    next_seq: u64,
    peer_id: u64,
    company_id: CompanyId,
) -> Result<(), NetError> {
    let hello = read_message(stream)?;
    match hello {
        NetMessage::Hello { protocol } if protocol == PROTOCOL_VERSION => {}
        NetMessage::Hello { protocol } => {
            return Err(NetError::Protocol(format!(
                "unsupported protocol {protocol}"
            )));
        }
        other => {
            return Err(NetError::Protocol(format!("expected hello, got {other:?}")));
        }
    }
    write_message(
        stream,
        &NetMessage::Welcome {
            protocol: PROTOCOL_VERSION,
            snapshot_json: snapshot_json.to_string(),
            next_seq,
            peer_id,
            company_id,
        },
    )?;
    Ok(())
}

fn try_read_client(stream: &mut TcpStream) -> Result<Option<NetMessage>, NetError> {
    stream.set_nonblocking(true)?;
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf) {
        Ok(()) => {
            stream.set_nonblocking(false)?;
            let len = u32::from_le_bytes(len_buf) as usize;
            if len > 64 * 1024 * 1024 {
                return Err(NetError::Protocol(format!("frame too large: {len}")));
            }
            let mut payload = vec![0u8; len];
            stream.read_exact(&mut payload)?;
            Ok(Some(serde_json::from_slice(&payload)?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            stream.set_nonblocking(false)?;
            Ok(None)
        }
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Err(NetError::Closed),
        Err(e) => Err(NetError::Io(e)),
    }
}

fn broadcast_raw(clients: &mut Vec<ClientSlot>, shared_peer_ids: &SharedPeerIds, msg: &NetMessage) {
    let mut dead = Vec::new();
    for (i, client) in clients.iter_mut().enumerate() {
        if let Err(e) = write_message(&mut client.stream, msg) {
            eprintln!("openttdrs-net: broadcast failed: {e}");
            dead.push(i);
        }
    }
    for i in dead.into_iter().rev() {
        let peer_id = clients[i].peer_id;
        remove_peer_id(shared_peer_ids, peer_id);
        clients.remove(i);
    }
}

fn configure_stream(stream: &TcpStream) -> Result<(), NetError> {
    stream.set_nodelay(true)?;
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
    join: Option<JoinHandle<()>>,
}

impl ClientSession {
    pub fn connect(addr: &str) -> Result<Self, NetError> {
        let stream = crate::connect(addr)?;
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let company_id = std::sync::Arc::new(std::sync::Mutex::new(CompanyId::PLAYER));
        let company_id_thread = std::sync::Arc::clone(&company_id);
        let addr_owned = addr.to_string();
        let join = thread::Builder::new()
            .name("openttdrs-client-net".into())
            .spawn(move || {
                if let Err(e) = client_thread(stream, cmd_rx, event_tx, company_id_thread) {
                    eprintln!("openttdrs-net: client ended ({addr_owned}): {e}");
                }
            })
            .map_err(NetError::Io)?;
        Ok(Self {
            handle: ClientSessionHandle { cmd_tx, company_id },
            event_rx,
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
        match self.event_rx.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(SessionEvent::Disconnected {
                reason: "client thread ended".into(),
            }),
        }
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

#[allow(clippy::too_many_lines)]
fn client_thread(
    mut stream: TcpStream,
    cmd_rx: Receiver<ClientCmd>,
    event_tx: Sender<SessionEvent>,
    company_id: std::sync::Arc<std::sync::Mutex<CompanyId>>,
) -> Result<(), NetError> {
    write_message(
        &mut stream,
        &NetMessage::Hello {
            protocol: PROTOCOL_VERSION,
        },
    )?;
    let welcome = read_message(&mut stream)?;
    match welcome {
        NetMessage::Welcome {
            protocol,
            snapshot_json,
            next_seq,
            peer_id,
            company_id: assigned_company,
        } if protocol == PROTOCOL_VERSION => {
            *company_id
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = assigned_company;
            let _ = event_tx.send(SessionEvent::Welcome {
                snapshot_json,
                next_seq,
                peer_id,
            });
        }
        NetMessage::Welcome { protocol, .. } => {
            return Err(NetError::Protocol(format!(
                "unsupported protocol {protocol}"
            )));
        }
        other => {
            return Err(NetError::Protocol(format!(
                "expected welcome, got {other:?}"
            )));
        }
    }

    stream.set_nonblocking(true)?;
    loop {
        match cmd_rx.try_recv() {
            Ok(ClientCmd::Propose {
                company_id,
                command,
            }) => {
                stream.set_nonblocking(false)?;
                write_message(
                    &mut stream,
                    &NetMessage::Propose {
                        company_id,
                        command,
                    },
                )?;
                stream.set_nonblocking(true)?;
            }
            Ok(ClientCmd::ReportDesync {
                tick,
                expected_hash,
                actual_hash,
            }) => {
                stream.set_nonblocking(false)?;
                write_message(
                    &mut stream,
                    &NetMessage::Desync {
                        tick,
                        expected_hash,
                        actual_hash,
                    },
                )?;
                stream.set_nonblocking(true)?;
            }
            Ok(ClientCmd::Shutdown) | Err(TryRecvError::Disconnected) => return Ok(()),
            Err(TryRecvError::Empty) => {}
        }

        match try_read_client(&mut stream) {
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Ok(Some(NetMessage::Welcome {
                protocol,
                snapshot_json,
                next_seq,
                peer_id,
                company_id: assigned_company,
            })) if protocol == PROTOCOL_VERSION => {
                *company_id
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = assigned_company;
                // Resync post-handshake (host avanzó durante el Welcome).
                let _ = event_tx.send(SessionEvent::Welcome {
                    snapshot_json,
                    next_seq,
                    peer_id,
                });
            }
            Ok(Some(NetMessage::Commit {
                seq,
                company_id,
                command,
            })) => {
                let _ = event_tx.send(SessionEvent::Commit {
                    seq,
                    company_id,
                    command,
                });
            }
            Ok(Some(NetMessage::AdvanceTicks { count })) => {
                let _ = event_tx.send(SessionEvent::AdvanceTicks { count });
            }
            Ok(Some(NetMessage::HashCheck { tick, hash })) => {
                let _ = event_tx.send(SessionEvent::HashCheck { tick, hash });
            }
            Ok(Some(NetMessage::HostAnnounce {
                bind,
                next_seq,
                new_host_peer_id,
            })) => {
                let _ = event_tx.send(SessionEvent::HostAnnounce {
                    bind,
                    next_seq,
                    new_host_peer_id,
                });
            }
            Ok(Some(NetMessage::PeerList { peer_ids })) => {
                let _ = event_tx.send(SessionEvent::PeerList { peer_ids });
            }
            Ok(Some(NetMessage::Heartbeat { tick })) => {
                let _ = event_tx.send(SessionEvent::Heartbeat { tick });
            }
            Ok(Some(NetMessage::Reject { message })) => {
                let _ = event_tx.send(SessionEvent::CommandRejected { message });
            }
            Ok(Some(NetMessage::Desync {
                tick,
                expected_hash,
                actual_hash,
            })) => {
                let _ = event_tx.send(SessionEvent::Desync {
                    tick,
                    expected_hash,
                    actual_hash,
                });
            }
            Ok(Some(NetMessage::Error { message })) => {
                let _ = event_tx.send(SessionEvent::Disconnected { reason: message });
                return Ok(());
            }
            Ok(Some(other)) => {
                eprintln!("openttdrs-net: ignore msg {other:?}");
            }
            Err(NetError::Closed) => {
                let _ = event_tx.send(SessionEvent::Disconnected {
                    reason: "server closed".into(),
                });
                return Ok(());
            }
            Err(e) => return Err(e),
        }
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
    use super::{apply_command_as_company, elect_new_host};
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
}
