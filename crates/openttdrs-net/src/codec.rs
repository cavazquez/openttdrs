//! Framing length-prefixed (`u32` LE) + JSON.

use std::io::{Read, Write};

use crate::protocol::{NetError, NetMessage};

/// Límite de un payload JSON individual en el wire protocol.
const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;

/// Decodificador incremental de frames `u32` LE + JSON.
///
/// TCP conserva el orden de bytes, pero no conserva los límites de las
/// escrituras. En un socket non-blocking, un `WouldBlock` puede llegar después
/// de consumir una parte del header o del payload; por eso el estado pertenece
/// a la conexión y no a una única llamada de lectura.
#[derive(Debug, Default)]
pub(crate) struct FrameDecoder {
    header: [u8; 4],
    header_filled: usize,
    payload: Vec<u8>,
    payload_len: Option<usize>,
    payload_filled: usize,
}

impl FrameDecoder {
    /// Intenta producir exactamente un mensaje sin perder bytes parciales.
    ///
    /// Devuelve `Ok(None)` únicamente cuando la fuente temporalmente no tiene
    /// más datos (`WouldBlock`). Un EOF entre bytes de un frame es un error de
    /// protocolo explícito: el siguiente frame ya no puede reconstruirse de
    /// manera fiable.
    pub(crate) fn try_read(
        &mut self,
        reader: &mut impl Read,
    ) -> Result<Option<NetMessage>, NetError> {
        loop {
            if self.payload_len.is_none() {
                match reader.read(&mut self.header[self.header_filled..]) {
                    Ok(0) => return self.eof(),
                    Ok(read) => {
                        self.header_filled += read;
                        if self.header_filled == self.header.len() {
                            let len = u32::from_le_bytes(self.header) as usize;
                            if len > MAX_FRAME_LEN {
                                self.reset_frame();
                                return Err(NetError::Protocol(format!("frame too large: {len}")));
                            }
                            self.payload.clear();
                            self.payload.resize(len, 0);
                            self.payload_len = Some(len);
                            self.payload_filled = 0;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return Ok(None);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(NetError::Io(error)),
                }
            }

            let Some(len) = self.payload_len else {
                continue;
            };
            if self.payload_filled == len {
                return self.finish_frame();
            }

            match reader.read(&mut self.payload[self.payload_filled..]) {
                Ok(0) => return self.eof(),
                Ok(read) => self.payload_filled += read,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(NetError::Io(error)),
            }
        }
    }

    fn eof(&mut self) -> Result<Option<NetMessage>, NetError> {
        if self.header_filled == 0 && self.payload_len.is_none() {
            return Err(NetError::Closed);
        }
        self.reset_frame();
        Err(NetError::Protocol(
            "connection closed in the middle of a frame".into(),
        ))
    }

    fn finish_frame(&mut self) -> Result<Option<NetMessage>, NetError> {
        let decoded = serde_json::from_slice(&self.payload).map_err(NetError::Json);
        self.reset_frame();
        decoded.map(Some)
    }

    fn reset_frame(&mut self) {
        self.header = [0; 4];
        self.header_filled = 0;
        self.payload.clear();
        self.payload_len = None;
        self.payload_filled = 0;
    }
}

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
    if len > MAX_FRAME_LEN {
        return Err(NetError::Protocol(format!("frame too large: {len}")));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::collections::VecDeque;
    use std::io::{self, Read};

    use super::{FrameDecoder, MAX_FRAME_LEN};
    use crate::protocol::{NetError, NetMessage};

    enum ReadStep {
        Bytes(Vec<u8>),
        WouldBlock,
        Eof,
    }

    struct ScriptedReader {
        steps: VecDeque<ReadStep>,
    }

    impl ScriptedReader {
        fn new(steps: impl IntoIterator<Item = ReadStep>) -> Self {
            Self {
                steps: steps.into_iter().collect(),
            }
        }
    }

    impl Read for ScriptedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let Some(step) = self.steps.pop_front() else {
                return Ok(0);
            };
            match step {
                ReadStep::Bytes(mut bytes) => {
                    let count = buffer.len().min(bytes.len());
                    buffer[..count].copy_from_slice(&bytes[..count]);
                    if count < bytes.len() {
                        self.steps
                            .push_front(ReadStep::Bytes(bytes.split_off(count)));
                    }
                    Ok(count)
                }
                ReadStep::WouldBlock => Err(io::Error::from(io::ErrorKind::WouldBlock)),
                ReadStep::Eof => Ok(0),
            }
        }
    }

    fn framed(message: &NetMessage) -> Vec<u8> {
        let payload = serde_json::to_vec(message).expect("serializa mensaje de prueba");
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        frame.extend_from_slice(&payload);
        frame
    }

    #[test]
    fn decoder_preserves_every_header_partition_and_byte_by_byte_input() {
        let message = NetMessage::Heartbeat { tick: 42 };
        let frame = framed(&message);

        for split in 1..4 {
            let mut decoder = FrameDecoder::default();
            let mut reader = ScriptedReader::new([
                ReadStep::Bytes(frame[..split].to_vec()),
                ReadStep::WouldBlock,
                ReadStep::Bytes(frame[split..].to_vec()),
            ]);
            assert_eq!(decoder.try_read(&mut reader).unwrap(), None);
            assert_eq!(
                decoder.try_read(&mut reader).unwrap(),
                Some(message.clone())
            );
        }

        let mut byte_steps = Vec::new();
        for byte in &frame {
            byte_steps.push(ReadStep::Bytes(vec![*byte]));
            byte_steps.push(ReadStep::WouldBlock);
        }
        let mut decoder = FrameDecoder::default();
        let mut reader = ScriptedReader::new(byte_steps);
        for _ in 0..frame.len() - 1 {
            assert_eq!(decoder.try_read(&mut reader).unwrap(), None);
        }
        assert_eq!(decoder.try_read(&mut reader).unwrap(), Some(message));
    }

    #[test]
    fn decoder_preserves_fragmented_payload_and_concatenated_frames() {
        let first = NetMessage::Reject {
            message: "payload fragmentado".into(),
        };
        let second = NetMessage::Heartbeat { tick: 9 };
        let first_frame = framed(&first);
        let second_frame = framed(&second);
        let payload_split = 4 + 3;
        let mut combined = first_frame[payload_split..].to_vec();
        combined.extend_from_slice(&second_frame);

        let mut decoder = FrameDecoder::default();
        let mut reader = ScriptedReader::new([
            ReadStep::Bytes(first_frame[..payload_split].to_vec()),
            ReadStep::WouldBlock,
            ReadStep::Bytes(combined),
        ]);
        assert_eq!(decoder.try_read(&mut reader).unwrap(), None);
        assert_eq!(decoder.try_read(&mut reader).unwrap(), Some(first));
        assert_eq!(decoder.try_read(&mut reader).unwrap(), Some(second));
    }

    #[test]
    fn decoder_rejects_eof_inside_a_header_or_payload() {
        let mut header_decoder = FrameDecoder::default();
        let mut header_reader = ScriptedReader::new([ReadStep::Bytes(vec![1, 0]), ReadStep::Eof]);
        assert!(matches!(
            header_decoder.try_read(&mut header_reader),
            Err(NetError::Protocol(message)) if message.contains("middle of a frame")
        ));

        let mut payload_decoder = FrameDecoder::default();
        let mut payload_reader =
            ScriptedReader::new([ReadStep::Bytes(vec![4, 0, 0, 0, b'{', b'}']), ReadStep::Eof]);
        assert!(matches!(
            payload_decoder.try_read(&mut payload_reader),
            Err(NetError::Protocol(message)) if message.contains("middle of a frame")
        ));
    }

    #[test]
    fn decoder_rejects_oversized_frames_before_allocating_a_payload() {
        let too_large = u32::try_from(MAX_FRAME_LEN + 1).unwrap();
        let mut decoder = FrameDecoder::default();
        let mut reader = ScriptedReader::new([ReadStep::Bytes(too_large.to_le_bytes().to_vec())]);

        assert!(matches!(
            decoder.try_read(&mut reader),
            Err(NetError::Protocol(message)) if message.contains("frame too large")
        ));
        assert!(decoder.payload.is_empty());
        assert!(decoder.payload_len.is_none());
    }
}
