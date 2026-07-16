//! Enruta comandos del jugador según el rol de red (handle global instalado por el plugin).

use std::sync::RwLock;

use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_net::{ClientSessionHandle, ListenServerHandle};

use super::plugin::NetworkRole;

struct DispatchState {
    role: NetworkRole,
    server: Option<ListenServerHandle>,
    client: Option<ClientSessionHandle>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            role: NetworkRole::Offline,
            server: None,
            client: None,
        }
    }
}

static DISPATCH: RwLock<DispatchState> = RwLock::new(DispatchState {
    role: NetworkRole::Offline,
    server: None,
    client: None,
});

pub(super) fn install_offline() {
    if let Ok(mut g) = DISPATCH.write() {
        *g = DispatchState::default();
    }
}

pub(super) fn install_server(handle: ListenServerHandle) {
    if let Ok(mut g) = DISPATCH.write() {
        *g = DispatchState {
            role: NetworkRole::ListenServer,
            server: Some(handle),
            client: None,
        };
    }
}

pub(super) fn install_client(handle: ClientSessionHandle) {
    if let Ok(mut g) = DISPATCH.write() {
        *g = DispatchState {
            role: NetworkRole::Client,
            server: None,
            client: Some(handle),
        };
    }
}

/// Aplica un comando del jugador respetando listen-server / cliente-only.
///
/// Sustituye a `apply_command` en la UI de partida. Los tests pueden seguir
/// llamando `openttdrs_core::apply_command` directamente.
pub fn apply_player_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    let Ok(guard) = DISPATCH.read() else {
        return apply_command(state, cmd);
    };
    match guard.role {
        NetworkRole::Offline => apply_command(state, cmd),
        NetworkRole::ListenServer => {
            apply_command(state, cmd)?;
            if let Some(server) = &guard.server
                && let Err(e) = server.broadcast_commit(cmd.clone())
            {
                bevy::log::warn!("network: broadcast commit failed: {e}");
            }
            Ok(())
        }
        NetworkRole::Client => {
            if let Some(client) = &guard.client
                && let Err(e) = client.propose(cmd.clone())
            {
                bevy::log::warn!("network: propose failed: {e}");
            }
            Ok(())
        }
    }
}
