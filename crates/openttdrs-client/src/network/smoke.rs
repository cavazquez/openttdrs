//! Smoke de handshake para el binario cliente empaquetado.

use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::GameState;
use openttdrs_net::{ClientSession, DEFAULT_PORT, SessionEvent};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(12);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Extrae `--network-smoke <HOST[:PORT]>` sin interferir con los flags normales.
pub fn parse_handshake_smoke(
    args: impl IntoIterator<Item = String>,
) -> Result<Option<String>, String> {
    let mut args = args.into_iter();
    let _ = args.next(); // argv[0]
    while let Some(arg) = args.next() {
        if arg != "--network-smoke" {
            continue;
        }
        let addr = args
            .next()
            .filter(|value| !value.starts_with('-') && !value.is_empty())
            .ok_or_else(|| "--network-smoke requiere <HOST[:PORT]>".to_string())?;
        return Ok(Some(if addr.contains(':') {
            addr
        } else {
            format!("{addr}:{DEFAULT_PORT}")
        }));
    }
    Ok(None)
}

/// Conecta el cliente empaquetado a un dedicated y exige un `Welcome` válido.
///
/// Reintenta la conexión para cubrir el intervalo entre lanzar el dedicated y que
/// el puerto quede escuchando. No inicia Bevy: el binario ya validó sus assets
/// antes de llamar a este smoke.
pub fn run_handshake_smoke(addr: &str) -> Result<(), String> {
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    let mut last_error = String::from("el dedicated no aceptó conexiones");

    while Instant::now() < deadline {
        match ClientSession::connect(addr) {
            Ok(session) => {
                while Instant::now() < deadline {
                    if let Some(event) = session.try_recv() {
                        match event {
                            SessionEvent::Welcome {
                                snapshot_json,
                                next_seq,
                                peer_id,
                            } => {
                                GameState::load_json(&snapshot_json).map_err(|error| {
                                    format!("Welcome con snapshot inválido: {error}")
                                })?;
                                println!(
                                    "Handshake OK: servidor={addr} peer_id={peer_id} next_seq={next_seq}"
                                );
                                return Ok(());
                            }
                            SessionEvent::Disconnected { reason } => {
                                last_error = reason;
                                break;
                            }
                            _ => {}
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            Err(error) => last_error = error.to_string(),
        }
        thread::sleep(RETRY_INTERVAL);
    }

    Err(format!(
        "handshake con {addr} no concluyó en {} s: {last_error}",
        HANDSHAKE_TIMEOUT.as_secs()
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::parse_handshake_smoke;

    #[test]
    fn smoke_addr_adds_default_port() {
        let args = vec!["client", "--network-smoke", "127.0.0.1"]
            .into_iter()
            .map(str::to_owned);
        assert_eq!(
            parse_handshake_smoke(args).expect("parse"),
            Some("127.0.0.1:3979".into())
        );
    }

    #[test]
    fn smoke_addr_requires_destination() {
        let args = vec!["client", "--network-smoke"]
            .into_iter()
            .map(str::to_owned);
        assert_eq!(
            parse_handshake_smoke(args).expect_err("missing addr"),
            "--network-smoke requiere <HOST[:PORT]>"
        );
    }
}
