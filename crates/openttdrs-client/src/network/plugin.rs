//! Plugin Bevy: sesión TCP + aplicación de eventos de red.

use std::sync::Mutex;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_net::{ClientSession, ListenServer, SessionEvent};

use crate::bevy_app::{FixedUpdateSet, UpdateSet};
use crate::network::banner::sync_failover_banner;
use crate::network::cli::NetCli;
use crate::network::dispatch::{install_client, install_offline, install_server};
use crate::network::failover::{
    ClientFailoverState, FailoverUiPhase, HOST_SILENCE_TIMEOUT, PendingFailoverReconnect,
    tick_pending_reconnect, try_client_failover,
};
use crate::render::{MapVisualLayer, ShoreTile, VehicleIndex, WaterTile};
use crate::state::{ClientScreen, EditorSession, SimRunState, SimWorld};
use crate::ui::{MainMenuCamera, MainMenuUi, leave_main_menu};

/// Rol de la instancia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkRole {
    #[default]
    Offline,
    ListenServer,
    Client,
}

/// Estado visible en HUD / logs.
#[derive(Resource, Debug, Clone, Default)]
pub struct NetworkStatus {
    pub label: String,
    pub desync: Option<String>,
    /// Tras `Welcome` en modo cliente: salir del menú y mostrar el mapa.
    pub pending_enter_ingame: bool,
    /// Banner / pausa durante host migration (#171).
    pub failover_phase: FailoverUiPhase,
    /// Tras pausar por failover, reanudar sim cuando la fase vuelva a Idle.
    pub resume_sim_after_failover: bool,
}

/// Sesión de red. Los sockets viven bajo `Mutex` porque `mpsc::Receiver` no es `Sync`.
#[derive(Resource)]
pub struct NetworkRuntime {
    role: NetworkRole,
    server: Option<Mutex<ListenServer>>,
    client: Option<Mutex<ClientSession>>,
}

impl NetworkRuntime {
    #[must_use]
    pub fn offline() -> Self {
        Self {
            role: NetworkRole::Offline,
            server: None,
            client: None,
        }
    }

    #[must_use]
    pub fn listen_server(server: ListenServer) -> Self {
        Self {
            role: NetworkRole::ListenServer,
            server: Some(Mutex::new(server)),
            client: None,
        }
    }

    #[must_use]
    pub fn client(client: ClientSession) -> Self {
        Self {
            role: NetworkRole::Client,
            server: None,
            client: Some(Mutex::new(client)),
        }
    }

    #[must_use]
    pub const fn role(&self) -> NetworkRole {
        self.role
    }

    pub(crate) fn try_recv_event(&self) -> Option<SessionEvent> {
        match self.role {
            NetworkRole::ListenServer => self
                .server
                .as_ref()
                .and_then(|s| s.lock().ok().and_then(|g| g.try_recv())),
            NetworkRole::Client => self
                .client
                .as_ref()
                .and_then(|c| c.lock().ok().and_then(|g| g.try_recv())),
            NetworkRole::Offline => None,
        }
    }

    pub(crate) fn broadcast_advance(&self, count: u32) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            let _ = g.broadcast_advance(count);
        }
    }

    pub(crate) fn publish_snapshot(&self, snapshot_json: String) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            g.update_snapshot(snapshot_json);
        }
    }

    pub(crate) fn broadcast_hash(&self, tick: u64, hash: u64) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            let _ = g.broadcast_hash(tick, hash);
        }
    }

    pub(crate) fn broadcast_heartbeat(&self, tick: u64) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            let _ = g.broadcast_heartbeat(tick);
        }
    }

    pub(crate) fn report_desync(&self, tick: u64, expected_hash: u64, actual_hash: u64) {
        if let Some(client) = &self.client
            && let Ok(g) = client.lock()
        {
            let _ = g.report_desync(tick, expected_hash, actual_hash);
        }
    }
}

pub struct NetworkPlugin {
    pub cli: NetCli,
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.cli.clone())
            .insert_resource(NetworkRuntime::offline())
            .insert_resource(NetworkStatus::default())
            .insert_resource(PendingNetCli(self.cli.clone()))
            .add_systems(Startup, start_network_session)
            .add_systems(
                Update,
                (
                    poll_network.run_if(network_active),
                    check_host_silence_failover
                        .run_if(resource_exists::<ClientFailoverState>)
                        .after(poll_network),
                    tick_pending_reconnect
                        .run_if(resource_exists::<PendingFailoverReconnect>)
                        .after(check_host_silence_failover),
                    resume_sim_after_failover
                        .after(tick_pending_reconnect)
                        .after(check_host_silence_failover),
                    enter_ingame_after_network_welcome
                        .run_if(network_pending_enter)
                        .after(poll_network),
                    sync_failover_banner.after(resume_sim_after_failover),
                )
                    .in_set(UpdateSet::Sim),
            )
            .add_systems(
                FixedUpdate,
                broadcast_tick_after_step
                    .in_set(FixedUpdateSet::Events)
                    .run_if(
                        in_state(ClientScreen::InGame)
                            .and_then(in_state(SimRunState::Running))
                            .and_then(is_listen_server),
                    ),
            );
    }
}

#[derive(Resource, Clone)]
struct PendingNetCli(NetCli);

fn network_active(net: Res<NetworkRuntime>) -> bool {
    net.role() != NetworkRole::Offline
}

fn network_pending_enter(status: Res<NetworkStatus>) -> bool {
    status.pending_enter_ingame
}

fn is_listen_server(net: Res<NetworkRuntime>) -> bool {
    net.role() == NetworkRole::ListenServer
}

/// Al recibir el snapshot del servidor, pasar del menú a la vista de partida.
#[allow(clippy::too_many_arguments)] // sistema Bevy: queries + estados
fn enter_ingame_after_network_welcome(
    mut status: ResMut<NetworkStatus>,
    mut commands: Commands,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut next_sim: ResMut<NextState<SimRunState>>,
    screen: Res<State<ClientScreen>>,
) {
    if !status.pending_enter_ingame {
        return;
    }
    status.pending_enter_ingame = false;
    commands.insert_resource(EditorSession::inactive());
    if *screen.get() != ClientScreen::InGame {
        leave_main_menu(
            &mut commands,
            &q_menu,
            &q_menu_cam,
            &intro_layers,
            &mut next_screen,
        );
        info!("network: entrando a InGame (partida del servidor)");
    }
    next_sim.set(SimRunState::Running);
}

fn start_network_session(
    mut commands: Commands,
    pending: Res<PendingNetCli>,
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<NetworkStatus>,
    sim: Res<SimWorld>,
) {
    match &pending.0 {
        NetCli::Offline => {
            install_offline();
        }
        NetCli::Server { bind } => {
            let snapshot = match sim.state.save_json() {
                Ok(s) => s,
                Err(e) => {
                    error!("--server: no se pudo serializar snapshot: {e}");
                    eprintln!("error: --server snapshot: {e}");
                    std::process::exit(1);
                }
            };
            match ListenServer::start(bind, snapshot) {
                Ok(server) => {
                    install_server(server.handle());
                    info!(
                        "listen-server on {bind} tick={} hash={:#x}",
                        sim.state.tick.get(),
                        sim.state.canonical_hash()
                    );
                    status.label = format!("server {bind}");
                    *net = NetworkRuntime::listen_server(server);
                }
                Err(e) => {
                    error!("no se pudo iniciar --server {bind}: {e}");
                    eprintln!("error: --server {bind}: {e}");
                    std::process::exit(1);
                }
            }
        }
        NetCli::Client { addr } => match ClientSession::connect(addr) {
            Ok(client) => {
                install_client(client.handle());
                info!("client connecting to {addr}");
                status.label = format!("client {addr}");
                commands.insert_resource(ClientFailoverState::new(addr.clone()));
                *net = NetworkRuntime::client(client);
            }
            Err(e) => {
                error!("no se pudo conectar --client {addr}: {e}");
                eprintln!("error: --client {addr}: {e}");
                std::process::exit(1);
            }
        },
    }
}

fn poll_network(
    mut commands: Commands,
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<NetworkStatus>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut failover: Option<ResMut<ClientFailoverState>>,
    mut next_sim: ResMut<NextState<SimRunState>>,
) {
    let mut host_lost = false;
    while let Some(event) = net.try_recv_event() {
        if let Some(fo) = failover.as_mut() {
            fo.note_rx();
        }
        match handle_event(
            &mut sim,
            &mut vehicle_index,
            &mut status,
            failover.as_deref_mut(),
            net.role(),
            &event,
        ) {
            Ok(EventOutcome::HostLost) => host_lost = true,
            Ok(EventOutcome::Ok) => {}
            Err(msg) => {
                status.desync = Some(msg.clone());
                if let Some((tick, expected_hash, actual_hash)) = parse_desync_message(&msg) {
                    net.report_desync(tick, expected_hash, actual_hash);
                }
                error!("network: {msg}");
            }
        }
    }
    if host_lost && let Some(fo) = failover.as_mut() {
        pause_for_failover(&mut status, &mut next_sim);
        let _ = try_client_failover(&mut commands, &mut net, &mut status, fo, &sim);
    }
}

fn check_host_silence_failover(
    mut commands: Commands,
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<NetworkStatus>,
    mut failover: ResMut<ClientFailoverState>,
    sim: Res<SimWorld>,
    mut next_sim: ResMut<NextState<SimRunState>>,
) {
    if net.role() != NetworkRole::Client || failover.failover_attempted {
        return;
    }
    if failover.last_rx.elapsed() < HOST_SILENCE_TIMEOUT {
        return;
    }
    warn!(
        "network: silencio del host > {:?} — intentando failover",
        HOST_SILENCE_TIMEOUT
    );
    pause_for_failover(&mut status, &mut next_sim);
    let _ = try_client_failover(&mut commands, &mut net, &mut status, &mut failover, &sim);
}

fn pause_for_failover(status: &mut NetworkStatus, next_sim: &mut NextState<SimRunState>) {
    status.resume_sim_after_failover = true;
    next_sim.set(SimRunState::Paused);
}

/// Tras promote / reconnect exitoso, reanudar la simulación.
fn resume_sim_after_failover(
    mut status: ResMut<NetworkStatus>,
    mut next_sim: ResMut<NextState<SimRunState>>,
    pending: Option<Res<PendingFailoverReconnect>>,
) {
    if !status.resume_sim_after_failover || pending.is_some() {
        return;
    }
    match &status.failover_phase {
        FailoverUiPhase::Idle => {
            status.resume_sim_after_failover = false;
            next_sim.set(SimRunState::Running);
        }
        FailoverUiPhase::Failed { .. } => {
            // Se mantiene pausado; el banner muestra el error.
        }
        FailoverUiPhase::Promoting { .. } | FailoverUiPhase::Reconnecting { .. } => {}
    }
}

enum EventOutcome {
    Ok,
    HostLost,
}

fn handle_event(
    sim: &mut SimWorld,
    vehicle_index: &mut VehicleIndex,
    status: &mut NetworkStatus,
    failover: Option<&mut ClientFailoverState>,
    role: NetworkRole,
    event: &SessionEvent,
) -> Result<EventOutcome, String> {
    match event {
        SessionEvent::Welcome {
            snapshot_json,
            next_seq,
            peer_id,
        } => {
            sim.state = GameState::load_json(snapshot_json).map_err(|e| e.to_string())?;
            status.desync = None;
            vehicle_index.rebuild(&sim.state.vehicles);
            if let Some(fo) = failover {
                fo.peer_id = Some(*peer_id);
                fo.next_seq = *next_seq;
                if !fo.known_peers.contains(peer_id) {
                    fo.known_peers.push(*peer_id);
                }
            }
            info!(
                "network: welcome applied tick={} hash={:#x} peer_id={peer_id}",
                sim.state.tick.get(),
                sim.state.canonical_hash()
            );
            if role == NetworkRole::Client {
                status.pending_enter_ingame = true;
            }
            Ok(EventOutcome::Ok)
        }
        SessionEvent::Commit { command, seq } => {
            apply_command(&mut sim.state, command).map_err(|e| e.to_string())?;
            vehicle_index.rebuild(&sim.state.vehicles);
            if let Some(fo) = failover {
                fo.next_seq = seq.saturating_add(1);
            }
            debug!("network: commit seq={seq}");
            Ok(EventOutcome::Ok)
        }
        SessionEvent::AdvanceTicks { count } => {
            for _ in 0..*count {
                sim.state.step();
            }
            vehicle_index.rebuild(&sim.state.vehicles);
            Ok(EventOutcome::Ok)
        }
        SessionEvent::HashCheck { tick, hash } => {
            let local_tick = sim.state.tick.get();
            if local_tick != *tick {
                debug!("network: skip hash check server_tick={tick} local_tick={local_tick}");
                return Ok(EventOutcome::Ok);
            }
            let actual = sim.state.canonical_hash();
            if actual != *hash {
                let msg = format!("desync tick={tick} expected={hash:#x} actual={actual:#x}");
                status.desync = Some(msg.clone());
                return Err(msg);
            }
            status.desync = None;
            Ok(EventOutcome::Ok)
        }
        SessionEvent::Desync {
            tick,
            expected_hash,
            actual_hash,
        } => {
            let msg =
                format!("desync tick={tick} expected={expected_hash:#x} actual={actual_hash:#x}");
            status.desync = Some(msg.clone());
            Err(msg)
        }
        SessionEvent::PeerList { peer_ids } => {
            if let Some(fo) = failover {
                fo.known_peers = peer_ids.clone();
            }
            debug!("network: peer_list={peer_ids:?}");
            Ok(EventOutcome::Ok)
        }
        SessionEvent::Heartbeat { tick } => {
            debug!("network: heartbeat tick={tick}");
            Ok(EventOutcome::Ok)
        }
        SessionEvent::HostAnnounce {
            bind,
            next_seq,
            new_host_peer_id,
        } => {
            info!(
                "network: host_announce bind={bind} next_seq={next_seq} new_host={new_host_peer_id}"
            );
            if let Some(fo) = failover {
                fo.next_seq = *next_seq;
                // Loser: destino explícito del anuncio (sin volver a sumar puerto).
                if fo.peer_id != Some(*new_host_peer_id)
                    && let Some(addr) = connect_addr_from_announce(&fo.server_addr, bind)
                {
                    fo.reconnect_addr = Some(addr.clone());
                    fo.failover_attempted = false;
                    status.label = format!("host_announce:{bind}");
                    status.failover_phase = FailoverUiPhase::Reconnecting { addr, attempt: 1 };
                    return Ok(EventOutcome::HostLost);
                }
            }
            status.label = format!("host_announce:{bind}");
            Ok(EventOutcome::Ok)
        }
        SessionEvent::Disconnected { reason } => {
            warn!("network disconnected: {reason}");
            status.label = format!("disconnected: {reason}");
            if role == NetworkRole::Client {
                Ok(EventOutcome::HostLost)
            } else {
                Ok(EventOutcome::Ok)
            }
        }
    }
}

fn broadcast_tick_after_step(net: Res<NetworkRuntime>, sim: Res<SimWorld>) {
    net.broadcast_advance(1);
    match sim.state.save_json() {
        Ok(json) => net.publish_snapshot(json),
        Err(e) => warn!("network: snapshot update failed: {e}"),
    }
    let tick = sim.state.tick.get();
    // Heartbeat frecuente (~cada tick) para failover; hash cada segundo de juego.
    net.broadcast_heartbeat(tick);
    if tick > 0 && tick.is_multiple_of(37) {
        net.broadcast_hash(tick, sim.state.canonical_hash());
    }
}

/// Conserva el host de `--client` y toma el puerto del bind anunciado (`0.0.0.0:P`).
fn connect_addr_from_announce(server_addr: &str, announce_bind: &str) -> Option<String> {
    let (host, _) = server_addr.rsplit_once(':')?;
    let (_, port) = announce_bind.rsplit_once(':')?;
    if host.is_empty() || port.is_empty() {
        return None;
    }
    Some(format!("{host}:{port}"))
}

fn parse_desync_message(message: &str) -> Option<(u64, u64, u64)> {
    let mut fields = message.strip_prefix("desync tick=")?.split_whitespace();
    let tick = fields.next()?.parse().ok()?;
    let expected = fields.next()?.strip_prefix("expected=")?;
    let actual = fields.next()?.strip_prefix("actual=")?;
    Some((
        tick,
        u64::from_str_radix(expected.trim_start_matches("0x"), 16).ok()?,
        u64::from_str_radix(actual.trim_start_matches("0x"), 16).ok()?,
    ))
}

#[cfg(test)]
mod desync_message_tests {
    use super::parse_desync_message;

    #[test]
    fn parses_hash_mismatch_for_reporting() {
        assert_eq!(
            parse_desync_message("desync tick=37 expected=0x10 actual=0x2a"),
            Some((37, 0x10, 0x2a))
        );
    }

    #[test]
    fn ignores_non_desync_errors() {
        assert_eq!(parse_desync_message("network timeout"), None);
    }
}
