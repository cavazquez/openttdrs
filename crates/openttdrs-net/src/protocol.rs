//! Mensajes del protocolo lockstep v1.

use openttdrs_core::Command;
use serde::{Deserialize, Serialize};

/// Versión del framing JSON. Subir si cambia el esquema de [`NetMessage`].
pub const PROTOCOL_VERSION: u16 = 1;

/// Mensaje de red (serializado como JSON dentro de un frame length-prefixed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetMessage {
    /// Cliente → servidor al conectar.
    Hello { protocol: u16 },
    /// Servidor → cliente: snapshot JSON de `GameState` (`save_json`).
    Welcome {
        protocol: u16,
        snapshot_json: String,
        next_seq: u64,
    },
    /// Cliente → servidor: propone un comando.
    Propose { command: Command },
    /// Servidor → todos: comando autorizado con secuencia monótona.
    Commit { seq: u64, command: Command },
    /// Servidor → clientes: avanzar N ticks de simulación.
    AdvanceTicks { count: u32 },
    /// Servidor → clientes: hash canónico en un tick (desync check).
    HashCheck { tick: u64, hash: u64 },
    /// Cualquiera → peer: divergencia detectada.
    Desync {
        tick: u64,
        expected_hash: u64,
        actual_hash: u64,
    },
    /// Error de protocolo / aplicación.
    Error { message: String },
}

/// Error de transporte o protocolo.
#[derive(Debug)]
pub enum NetError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Protocol(String),
    Closed,
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Json(e) => write!(f, "json: {e}"),
            Self::Protocol(m) => write!(f, "protocol: {m}"),
            Self::Closed => write!(f, "connection closed"),
        }
    }
}

impl std::error::Error for NetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Protocol(_) | Self::Closed => None,
        }
    }
}

impl From<std::io::Error> for NetError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for NetError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
