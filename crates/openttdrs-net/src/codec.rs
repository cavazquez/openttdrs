//! Framing length-prefixed (`u32` LE) + JSON.

use std::collections::VecDeque;
use std::io::{Read, Write};

use crate::protocol::{NetError, NetMessage};

/// Límite de un payload JSON individual en el wire protocol.
const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;
/// Máximo de bytes de salida pendientes por peer. Permite un frame máximo y
/// evita que un receptor que dejó de leer haga crecer la memoria sin límite.
const MAX_QUEUED_OUTPUT_BYTES: usize = MAX_FRAME_LEN + size_of::<u32>();
/// Presupuesto de escritura por sondeo para que un peer rápido no monopolice
/// el hilo de sesión mientras otros esperan progreso.
const MAX_WRITE_BYTES_PER_POLL: usize = 64 * 1024;

/// Resultado de un sondeo de entrada non-blocking.
#[derive(Debug)]
pub(crate) struct FrameRead {
    pub(crate) message: Option<NetMessage>,
    pub(crate) made_progress: bool,
}

/// Resultado de un sondeo de salida non-blocking.
#[derive(Debug)]
pub(crate) struct FrameWrite {
    pub(crate) made_progress: bool,
    pub(crate) is_empty: bool,
}

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
    #[cfg(test)]
    pub(crate) fn try_read(
        &mut self,
        reader: &mut impl Read,
    ) -> Result<Option<NetMessage>, NetError> {
        self.try_read_with_progress(reader)
            .map(|result| result.message)
    }

    /// Igual que [`Self::try_read`], informando si se consumió algún byte.
    ///
    /// Los loops de sesión usan esta señal para distinguir un peer inactivo de
    /// uno que está transfiriendo un frame grande lentamente.
    pub(crate) fn try_read_with_progress(
        &mut self,
        reader: &mut impl Read,
    ) -> Result<FrameRead, NetError> {
        let mut made_progress = false;
        loop {
            if self.payload_len.is_none() {
                match reader.read(&mut self.header[self.header_filled..]) {
                    Ok(0) => return self.eof(),
                    Ok(read) => {
                        made_progress = true;
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
                        return Ok(FrameRead {
                            message: None,
                            made_progress,
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(NetError::Io(error)),
                }
            }

            let Some(len) = self.payload_len else {
                continue;
            };
            if self.payload_filled == len {
                return self.finish_frame(made_progress);
            }

            match reader.read(&mut self.payload[self.payload_filled..]) {
                Ok(0) => return self.eof(),
                Ok(read) => {
                    made_progress = true;
                    self.payload_filled += read;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Ok(FrameRead {
                        message: None,
                        made_progress,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(NetError::Io(error)),
            }
        }
    }

    /// Indica si un frame ya consumió bytes pero todavía no fue entregado.
    pub(crate) fn has_partial_frame(&self) -> bool {
        self.header_filled != 0 || self.payload_len.is_some()
    }

    fn eof(&mut self) -> Result<FrameRead, NetError> {
        if self.header_filled == 0 && self.payload_len.is_none() {
            return Err(NetError::Closed);
        }
        self.reset_frame();
        Err(NetError::Protocol(
            "connection closed in the middle of a frame".into(),
        ))
    }

    fn finish_frame(&mut self, made_progress: bool) -> Result<FrameRead, NetError> {
        let decoded = serde_json::from_slice(&self.payload).map_err(NetError::Json);
        self.reset_frame();
        decoded.map(|message| FrameRead {
            message: Some(message),
            made_progress,
        })
    }

    fn reset_frame(&mut self) {
        self.header = [0; 4];
        self.header_filled = 0;
        self.payload.clear();
        self.payload_len = None;
        self.payload_filled = 0;
    }
}

/// Cola y emisor incremental de frames de salida.
///
/// Un `write_all` sobre un socket bloqueante permite que un receptor lento
/// detenga a todos los peers. Esta cola conserva fronteras y offsets de cada
/// frame, limita la memoria por conexión y escribe sólo un presupuesto fijo en
/// cada sondeo.
#[derive(Debug)]
pub(crate) struct FrameWriter {
    frames: VecDeque<Vec<u8>>,
    front_offset: usize,
    queued_bytes: usize,
    max_queued_bytes: usize,
}

impl Default for FrameWriter {
    fn default() -> Self {
        Self::with_max_queued_bytes(MAX_QUEUED_OUTPUT_BYTES)
    }
}

impl FrameWriter {
    pub(crate) fn with_max_queued_bytes(max_queued_bytes: usize) -> Self {
        Self {
            frames: VecDeque::new(),
            front_offset: 0,
            queued_bytes: 0,
            max_queued_bytes,
        }
    }

    /// Encola un frame completo sin modificar la cola cuando supera el límite.
    pub(crate) fn queue(&mut self, message: &NetMessage) -> Result<(), NetError> {
        let frame = encode_message(message)?;
        let queued = self
            .queued_bytes
            .checked_add(frame.len())
            .ok_or_else(|| NetError::Protocol("outbound queue length overflow".into()))?;
        if queued > self.max_queued_bytes {
            return Err(NetError::Protocol(format!(
                "outbound queue limit exceeded: {queued} > {} bytes",
                self.max_queued_bytes
            )));
        }
        self.queued_bytes = queued;
        self.frames.push_back(frame);
        Ok(())
    }

    /// Escribe hasta el presupuesto del sondeo y conserva el resto para la
    /// próxima vuelta. `WouldBlock` no es un error: el deadline de la sesión
    /// decide cuándo expulsar al peer que no vuelve a progresar.
    pub(crate) fn try_flush(&mut self, writer: &mut impl Write) -> Result<FrameWrite, NetError> {
        let mut made_progress = false;
        let mut written_this_poll = 0;

        while written_this_poll < MAX_WRITE_BYTES_PER_POLL {
            let (frame_len, write_result) = {
                let Some(frame) = self.frames.front() else {
                    break;
                };
                let available = MAX_WRITE_BYTES_PER_POLL - written_this_poll;
                let end = (self.front_offset + available).min(frame.len());
                (frame.len(), writer.write(&frame[self.front_offset..end]))
            };
            match write_result {
                Ok(0) => {
                    return Err(NetError::Io(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "connection closed while writing frame",
                    )));
                }
                Ok(written) => {
                    made_progress = true;
                    written_this_poll += written;
                    self.front_offset += written;
                    self.queued_bytes = self.queued_bytes.saturating_sub(written);
                    if self.front_offset == frame_len {
                        let _ = self.frames.pop_front();
                        self.front_offset = 0;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(NetError::Io(error)),
            }
        }

        Ok(FrameWrite {
            made_progress,
            is_empty: self.frames.is_empty(),
        })
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.frames.is_empty()
    }
}

/// Escribe un mensaje (máx ~64 MiB de payload JSON).
pub fn write_message(stream: &mut impl Write, msg: &NetMessage) -> Result<(), NetError> {
    let frame = encode_message(msg)?;
    stream.write_all(&frame)?;
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

fn encode_message(msg: &NetMessage) -> Result<Vec<u8>, NetError> {
    let payload = serde_json::to_vec(msg)?;
    let len = u32::try_from(payload.len())
        .map_err(|_| NetError::Protocol(format!("payload too large: {} bytes", payload.len())))?;
    let mut frame = Vec::with_capacity(size_of::<u32>() + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::collections::VecDeque;
    use std::io::{self, Cursor, Read, Write};

    use super::{FrameDecoder, FrameWriter, MAX_FRAME_LEN, read_message};
    use crate::protocol::{NetError, NetMessage};

    enum ReadStep {
        Bytes(Vec<u8>),
        WouldBlock,
        Eof,
    }

    struct ScriptedReader {
        steps: VecDeque<ReadStep>,
    }

    enum WriteStep {
        Bytes(usize),
        WouldBlock,
    }

    struct ScriptedWriter {
        steps: VecDeque<WriteStep>,
        bytes: Vec<u8>,
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

    impl ScriptedWriter {
        fn new(steps: impl IntoIterator<Item = WriteStep>) -> Self {
            Self {
                steps: steps.into_iter().collect(),
                bytes: Vec::new(),
            }
        }
    }

    impl Write for ScriptedWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let Some(step) = self.steps.pop_front() else {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            };
            match step {
                WriteStep::Bytes(limit) => {
                    let count = limit.min(buffer.len());
                    self.bytes.extend_from_slice(&buffer[..count]);
                    Ok(count)
                }
                WriteStep::WouldBlock => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
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

    #[test]
    fn writer_preserves_partial_output_and_frame_order() {
        let first = NetMessage::Heartbeat { tick: 17 };
        let second = NetMessage::AdvanceTicks { count: 3 };
        let mut writer = FrameWriter::with_max_queued_bytes(1024);
        writer.queue(&first).unwrap();
        writer.queue(&second).unwrap();
        let mut sink = ScriptedWriter::new([
            WriteStep::Bytes(2),
            WriteStep::WouldBlock,
            WriteStep::Bytes(usize::MAX),
            WriteStep::Bytes(usize::MAX),
        ]);

        let first_flush = writer.try_flush(&mut sink).unwrap();
        assert!(first_flush.made_progress);
        assert!(writer.has_pending());
        let second_flush = writer.try_flush(&mut sink).unwrap();
        assert!(second_flush.made_progress);
        assert!(!writer.has_pending());

        let mut decoded = Cursor::new(sink.bytes);
        assert_eq!(read_message(&mut decoded).unwrap(), first);
        assert_eq!(read_message(&mut decoded).unwrap(), second);
    }

    #[test]
    fn writer_rejects_a_frame_when_the_bounded_queue_is_full() {
        let mut writer = FrameWriter::with_max_queued_bytes(0);
        assert!(matches!(
            writer.queue(&NetMessage::Heartbeat { tick: 1 }),
            Err(NetError::Protocol(message)) if message.contains("queue limit")
        ));
        assert!(!writer.has_pending());
    }
}
