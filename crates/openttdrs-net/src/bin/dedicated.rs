//! Servidor dedicado headless (ADR 0001 / #21).
//!
//! ```text
//! cargo run -p openttdrs-net --bin openttdrs-dedicated -- --bind 0.0.0.0:3979
//! ```

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::{Command, GameState, TileCoord, apply_command};
use openttdrs_net::{ListenServer, SessionEvent, DEFAULT_PORT};

fn main() {
    let bind = parse_bind(env::args().skip(1));
    let mut state = GameState::new(64, 64);
    // Partida mínima jugable: un par de vías para que el snapshot no sea vacío de contenido.
    let _ = apply_command(&mut state, &Command::PlaceRail(TileCoord::new(8, 8)));
    let _ = apply_command(&mut state, &Command::PlaceRail(TileCoord::new(9, 8)));

    let snapshot = match state.save_json() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: no se pudo serializar el estado: {e}");
            std::process::exit(1);
        }
    };

    let server = match ListenServer::start(&bind, snapshot) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: no se pudo escuchar en {bind}: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("openttdrs-dedicated: bound {bind} (Ctrl+C para salir)");
    let tick_period = Duration::from_millis(27); // ~37 Hz
    let mut last_tick = Instant::now();
    let mut ticks_since_hash = 0u32;

    loop {
        while let Some(event) = server.try_recv() {
            match event {
                SessionEvent::Commit { command, seq } => {
                    if let Err(e) = apply_command(&mut state, &command) {
                        eprintln!("dedicated: reject commit seq={seq}: {e}");
                    } else {
                        eprintln!("dedicated: applied commit seq={seq}");
                        publish_snapshot(&server, &state);
                    }
                }
                SessionEvent::Disconnected { reason } => {
                    eprintln!("dedicated: {reason}");
                }
                other => {
                    eprintln!("dedicated: ignore {other:?}");
                }
            }
        }

        if last_tick.elapsed() >= tick_period {
            state.step();
            // Primero avisar a peers ya conectados; luego publicar snapshot para late-join.
            let _ = server.broadcast_advance(1);
            publish_snapshot(&server, &state);
            ticks_since_hash += 1;
            if ticks_since_hash >= 37 {
                ticks_since_hash = 0;
                let tick = state.tick.get();
                let hash = state.canonical_hash();
                let _ = server.broadcast_hash(tick, hash);
            }
            last_tick = Instant::now();
        }

        thread::sleep(Duration::from_millis(1));
    }
}

fn publish_snapshot(server: &ListenServer, state: &GameState) {
    match state.save_json() {
        Ok(json) => server.update_snapshot(json),
        Err(e) => eprintln!("dedicated: snapshot failed: {e}"),
    }
}

fn parse_bind(mut args: impl Iterator<Item = String>) -> String {
    let mut bind = format!("0.0.0.0:{DEFAULT_PORT}");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                if let Some(v) = args.next() {
                    bind = v;
                }
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: openttdrs-dedicated [--bind HOST:PORT]\nDefault bind: 0.0.0.0:{DEFAULT_PORT}"
                );
                std::process::exit(0);
            }
            other if other.starts_with("--bind=") => {
                bind = other.trim_start_matches("--bind=").to_string();
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }
    bind
}
