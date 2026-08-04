//! Multijugador mínimo (#21): flags `--server` / `--client` sobre `openttdrs-net`.

mod banner;
mod cli;
mod dispatch;
mod failover;
mod plugin;
mod smoke;

pub use cli::{NetCli, parse_net_cli};
pub use dispatch::{apply_player_command, player_command_revision};
pub use plugin::{NetworkPlugin, NetworkRole, NetworkRuntime, NetworkStatus};
pub use smoke::{parse_handshake_smoke, run_handshake_smoke};
