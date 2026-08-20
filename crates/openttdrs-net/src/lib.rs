//! Transporte TCP mínimo para multijugador lockstep ([#21](https://github.com/cavazquez/openttdrs/issues/21)).
//!
//! Protocolo v3: frames `u32` LE + JSON [`NetMessage`]. El servidor es autoritativo:
//! asigna `seq`, retransmite [`NetMessage::Commit`] y avanza ticks.
//! Host migration listen-server: [`elect_new_host`] / ADR 0004.
//! Ver `docs/adr/0001-multiplayer-v1.md` y `docs/adr/0004-host-migration-post-v1.md`.

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]

mod codec;
mod peer;
mod protocol;
mod session;

pub use codec::{read_message, write_message};
pub use peer::{DEFAULT_PORT, connect, failover_connect_addr, failover_listen_bind, listen};
pub use protocol::{NetError, NetMessage, PROTOCOL_VERSION};
pub use session::{
    ClientSession, ClientSessionHandle, ListenServer, ListenServerHandle, SessionEvent,
    apply_command_as_company, apply_session_event, elect_new_host,
};
