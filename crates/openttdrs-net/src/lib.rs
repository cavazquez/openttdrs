//! Transporte TCP mínimo para multijugador lockstep (I8 / [#21](https://github.com/cavazquez/openttdrs/issues/21)).
//!
//! Protocolo: frames `u32` LE + JSON [`NetMessage`]. El servidor es autoritativo:
//! valida propuestas, asigna `seq`, retransmite [`NetMessage::Commit`] y avanza ticks.
//! Ver `docs/adr/0001-multiplayer-v1.md`.

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
pub use peer::{DEFAULT_PORT, connect, listen};
pub use protocol::{NetError, NetMessage, PROTOCOL_VERSION};
pub use session::{
    ClientSession, ClientSessionHandle, ListenServer, ListenServerHandle, SessionEvent,
    apply_session_event,
};
