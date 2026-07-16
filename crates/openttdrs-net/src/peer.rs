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

/// Dirección de reconnect tras failover: mismo host, `puerto + 1`.
#[must_use]
pub fn failover_connect_addr(server_addr: &str) -> Option<String> {
    let (host, port) = split_host_port(server_addr)?;
    Some(format!("{host}:{}", port.saturating_add(1)))
}

/// Bind del nuevo listen-server tras failover: `0.0.0.0:(puerto + 1)`.
#[must_use]
pub fn failover_listen_bind(server_addr: &str) -> Option<String> {
    let (_host, port) = split_host_port(server_addr)?;
    Some(format!("0.0.0.0:{}", port.saturating_add(1)))
}

fn split_host_port(addr: &str) -> Option<(String, u16)> {
    let (host, port_s) = addr.rsplit_once(':')?;
    let port: u16 = port_s.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::{failover_connect_addr, failover_listen_bind};

    #[test]
    fn failover_addrs_bump_port() {
        assert_eq!(
            failover_connect_addr("127.0.0.1:3979").as_deref(),
            Some("127.0.0.1:3980")
        );
        assert_eq!(
            failover_listen_bind("192.168.1.10:4000").as_deref(),
            Some("0.0.0.0:4001")
        );
        assert!(failover_connect_addr("no-port").is_none());
    }
}
