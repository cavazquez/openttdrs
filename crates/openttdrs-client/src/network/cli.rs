//! Args de red del binario cliente.

use bevy::prelude::Resource;
use openttdrs_net::DEFAULT_PORT;

/// Modo de red pedido por CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default, Resource)]
pub enum NetCli {
    #[default]
    Offline,
    /// Listen-server (host + UI). Default bind `0.0.0.0:3979`.
    Server { bind: String },
    /// Cliente-only hacia `addr`.
    Client { addr: String },
}

/// Parsea `--server [bind]` y `--client <addr>` (el resto se ignora).
#[must_use]
pub fn parse_net_cli(args: impl IntoIterator<Item = String>) -> NetCli {
    let mut mode = NetCli::Offline;
    let mut args = args.into_iter().peekable();
    // Skip argv[0]
    let _ = args.next();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server" => {
                let bind = match args.peek() {
                    Some(next) if !next.starts_with('-') => args.next().unwrap_or_default(),
                    _ => format!("0.0.0.0:{DEFAULT_PORT}"),
                };
                let bind = if bind.is_empty() {
                    format!("0.0.0.0:{DEFAULT_PORT}")
                } else if !bind.contains(':') {
                    format!("{bind}:{DEFAULT_PORT}")
                } else {
                    bind
                };
                mode = NetCli::Server { bind };
            }
            "--client" => {
                let Some(addr) = args.next() else {
                    eprintln!("error: --client requiere <addr>");
                    std::process::exit(2);
                };
                let addr = if addr.contains(':') {
                    addr
                } else {
                    format!("{addr}:{DEFAULT_PORT}")
                };
                mode = NetCli::Client { addr };
            }
            "--help-net" => {
                eprintln!(
                    "Red (#21):\n  --server [HOST:PORT]   listen-server (default 0.0.0.0:{DEFAULT_PORT})\n  --client <HOST[:PORT]> cliente-only (sin mapa local; espera Welcome)\n  dedicated: cargo run -p openttdrs-net --bin openttdrs-dedicated -- [--bind …] [--seed N]"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    mode
}
