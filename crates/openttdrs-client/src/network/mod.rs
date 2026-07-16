//! Multijugador mínimo (#21): flags `--server` / `--client` sobre `openttdrs-net`.

mod cli;
mod dispatch;
mod failover;
mod plugin;

pub use cli::{NetCli, parse_net_cli};
pub use dispatch::apply_player_command;
pub use plugin::{NetworkPlugin, NetworkRole, NetworkRuntime, NetworkStatus};
