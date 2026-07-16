//! Accept / connect helpers.

use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::protocol::NetError;

/// Puerto por defecto (mismo que `OpenTTD` multiplayer).
pub const DEFAULT_PORT: u16 = 3979;

/// Escucha en `bind` (p.ej. `0.0.0.0:3979`).
pub fn listen(bind: impl ToSocketAddrs) -> Result<TcpListener, NetError> {
    let listener = TcpListener::bind(bind)?;
    listener.set_nonblocking(false)?;
    Ok(listener)
}

/// Conecta a `addr` (p.ej. `127.0.0.1:3979`).
pub fn connect(addr: impl ToSocketAddrs) -> Result<TcpStream, NetError> {
    let stream = TcpStream::connect(addr)?;
    stream.set_nodelay(true)?;
    Ok(stream)
}
