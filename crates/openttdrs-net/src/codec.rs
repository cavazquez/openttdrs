//! Framing length-prefixed (`u32` LE) + JSON.

use std::io::{Read, Write};

use crate::protocol::{NetError, NetMessage};

/// Escribe un mensaje (máx ~64 MiB de payload JSON).
pub fn write_message(stream: &mut impl Write, msg: &NetMessage) -> Result<(), NetError> {
    let payload = serde_json::to_vec(msg)?;
    let len = u32::try_from(payload.len())
        .map_err(|_| NetError::Protocol(format!("payload too large: {} bytes", payload.len())))?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()?;
    Ok(())
}

/// Lee un mensaje. `Ok(None)` no se usa; EOF → [`NetError::Closed`].
pub fn read_message(stream: &mut impl Read) -> Result<NetMessage, NetError> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Err(NetError::Closed),
        Err(e) => return Err(NetError::Io(e)),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 64 * 1024 * 1024 {
        return Err(NetError::Protocol(format!("frame too large: {len}")));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}
