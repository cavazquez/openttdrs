//! Plugin Bevy: sesión TCP + aplicación de eventos de red.

use std::sync::Mutex;

use bevy::prelude::*;
use openttdrs_core::{GameState, apply_command};
use openttdrs_net::{ClientSession, ListenServer, SessionEvent};

use crate::bevy_app::UpdateSet;
use crate::network::cli::NetCli;
use crate::network::dispatch::{install_client, install_offline, install_server};
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
    pub const fn role(&self) -> NetworkRole {
        self.role
    }

    fn try_recv_event(&self) -> Option<SessionEvent> {
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

    fn broadcast_advance(&self, count: u32) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            let _ = g.broadcast_advance(count);
        }
    }

    fn publish_snapshot(&self, snapshot_json: String) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            g.update_snapshot(snapshot_json);
        }
    }

    fn broadcast_hash(&self, tick: u64, hash: u64) {
        if let Some(server) = &self.server
            && let Ok(g) = server.lock()
        {
            let _ = g.broadcast_hash(tick, hash);
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
                    enter_ingame_after_network_welcome
                        .run_if(network_pending_enter)
                        .after(poll_network),
                )
                    .in_set(UpdateSet::Sim),
            )
            .add_systems(
                FixedUpdate,
                broadcast_tick_after_step.run_if(
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
                    *net = NetworkRuntime {
                        role: NetworkRole::ListenServer,
                        server: Some(Mutex::new(server)),
                        client: None,
                    };
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
                *net = NetworkRuntime {
                    role: NetworkRole::Client,
                    server: None,
                    client: Some(Mutex::new(client)),
                };
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
    net: Res<NetworkRuntime>,
    mut status: ResMut<NetworkStatus>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
) {
    while let Some(event) = net.try_recv_event() {
        if let Err(msg) = handle_event(
            &mut sim,
            &mut vehicle_index,
            &mut status,
            net.role(),
            &event,
        ) {
            status.desync = Some(msg.clone());
            error!("network: {msg}");
        }
    }
}

fn handle_event(
    sim: &mut SimWorld,
    vehicle_index: &mut VehicleIndex,
    status: &mut NetworkStatus,
    role: NetworkRole,
    event: &SessionEvent,
) -> Result<(), String> {
    match event {
        SessionEvent::Welcome { snapshot_json, .. } => {
            sim.state = GameState::load_json(snapshot_json).map_err(|e| e.to_string())?;
            vehicle_index.rebuild(&sim.state.vehicles);
            info!(
                "network: welcome applied tick={} hash={:#x}",
                sim.state.tick.get(),
                sim.state.canonical_hash()
            );
            if role == NetworkRole::Client {
                status.pending_enter_ingame = true;
            }
            Ok(())
        }
        SessionEvent::Commit { command, seq } => {
            apply_command(&mut sim.state, command).map_err(|e| e.to_string())?;
            vehicle_index.rebuild(&sim.state.vehicles);
            debug!("network: commit seq={seq}");
            Ok(())
        }
        SessionEvent::AdvanceTicks { count } => {
            for _ in 0..*count {
                sim.state.step();
            }
            vehicle_index.rebuild(&sim.state.vehicles);
            Ok(())
        }
        SessionEvent::HashCheck { tick, hash } => {
            let local_tick = sim.state.tick.get();
            if local_tick != *tick {
                // Snapshot de late-join o cola de AdvanceTicks aún no al día.
                debug!("network: skip hash check server_tick={tick} local_tick={local_tick}");
                return Ok(());
            }
            let actual = sim.state.canonical_hash();
            if actual != *hash {
                let msg = format!("desync tick={tick} expected={hash:#x} actual={actual:#x}");
                status.desync = Some(msg.clone());
                return Err(msg);
            }
            Ok(())
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
        SessionEvent::Disconnected { reason } => {
            warn!("network disconnected: {reason}");
            status.label = format!("disconnected: {reason}");
            Ok(())
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
    if tick > 0 && tick.is_multiple_of(37) {
        net.broadcast_hash(tick, sim.state.canonical_hash());
    }
}
