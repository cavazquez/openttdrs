//! Contenedor exterior del `.sav`: magic + versión + payload comprimido.

use std::error::Error as _;

use super::SavError;

const MAGIC_OTTN: &[u8; 4] = b"OTTN";
const MAGIC_OTTZ: &[u8; 4] = b"OTTZ";
const MAGIC_OTTX: &[u8; 4] = b"OTTX";
const MAGIC_OTTD: &[u8; 4] = b"OTTD";

/// Límite de bytes para payload descomprimido de `.sav`.
///
/// Mapas 4096×4096 con muchas entidades: ~50–100 MB descomprimidos.
/// Este límite (200 MB) cubre casos reales y previene agotamiento de memoria.
const MAX_SAV_DECOMPRESSED_BYTES: u64 = 200 * 1024 * 1024;

/// Reader que limita bytes escritos y rechaza contenido excesivo.
struct BoundedWriter {
    inner: Vec<u8>,
    limit: u64,
    written: u64,
}

impl BoundedWriter {
    fn new(limit: u64) -> Self {
        Self {
            inner: Vec::new(),
            limit,
            written: 0,
        }
    }

    fn into_vec(self) -> Vec<u8> {
        self.inner
    }
}

impl std::io::Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let new_written = self.written.saturating_add(buf.len() as u64);
        if new_written > self.limit {
            return Err(std::io::Error::other(format!(
                "límite de descompresión excedido: {} bytes > {} bytes",
                new_written, self.limit
            )));
        }
        self.written = new_written;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Descomprime el `.sav` y devuelve `(payload, versión_del_save)`.
pub(crate) fn decompress(raw: &[u8]) -> Result<(Vec<u8>, u16), SavError> {
    let (Some(magic), Some(&v_hi), Some(&v_lo), true) =
        (raw.get(0..4), raw.get(4), raw.get(5), raw.len() >= 8)
    else {
        return Err(SavError::BadFormat("archivo demasiado corto".into()));
    };
    let magic: [u8; 4] = [magic[0], magic[1], magic[2], magic[3]];
    let version = u16::from_be_bytes([v_hi, v_lo]);
    let payload = &raw[8..];

    let data = match &magic {
        MAGIC_OTTN => {
            if payload.len() as u64 > MAX_SAV_DECOMPRESSED_BYTES {
                return Err(SavError::DecompressedSizeExceeded {
                    actual: payload.len() as u64,
                    limit: MAX_SAV_DECOMPRESSED_BYTES,
                });
            }
            payload.to_vec()
        }
        MAGIC_OTTZ => {
            let mut writer = BoundedWriter::new(MAX_SAV_DECOMPRESSED_BYTES);
            let mut dec = flate2::read::ZlibDecoder::new(payload);
            std::io::copy(&mut dec, &mut writer).map_err(|e| {
                if e.kind() == std::io::ErrorKind::Other
                    && e.to_string().contains("límite de descompresión excedido")
                {
                    SavError::DecompressedSizeExceeded {
                        actual: writer.written,
                        limit: MAX_SAV_DECOMPRESSED_BYTES,
                    }
                } else {
                    SavError::Decompress(format!("zlib: {e}"))
                }
            })?;
            writer.into_vec()
        }
        MAGIC_OTTX => {
            let mut writer = BoundedWriter::new(MAX_SAV_DECOMPRESSED_BYTES);
            let mut input = std::io::BufReader::new(payload);
            lzma_rs::xz_decompress(&mut input, &mut writer).map_err(|e| {
                if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<std::io::Error>())
                    && io_err.kind() == std::io::ErrorKind::Other
                    && io_err
                        .to_string()
                        .contains("límite de descompresión excedido")
                {
                    return SavError::DecompressedSizeExceeded {
                        actual: writer.written,
                        limit: MAX_SAV_DECOMPRESSED_BYTES,
                    };
                }
                SavError::Decompress(format!("xz: {e:?}"))
            })?;
            writer.into_vec()
        }
        MAGIC_OTTD => {
            return Err(SavError::UnsupportedCompression(
                "compresión LZO (saves muy antiguos); abrí el save en un OpenTTD moderno y \
                 volvé a guardarlo"
                    .into(),
            ));
        }
        other => {
            return Err(SavError::BadFormat(format!(
                "magic desconocido: {:?}",
                String::from_utf8_lossy(other)
            )));
        }
    };
    Ok((data, version))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ottn_returns_raw_payload_and_version() {
        let mut raw = b"OTTN".to_vec();
        raw.extend_from_slice(&300u16.to_be_bytes());
        raw.extend_from_slice(&[0, 0]);
        raw.extend_from_slice(b"payload");
        let (data, version) = decompress(&raw).expect("ottn");
        assert_eq!(version, 300);
        assert_eq!(data, b"payload");
    }

    #[test]
    fn ottz_roundtrip() {
        let body = b"hola zlib".repeat(10);
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut enc, &body).expect("write");
        let compressed = enc.finish().expect("finish");
        let mut raw = b"OTTZ".to_vec();
        raw.extend_from_slice(&310u16.to_be_bytes());
        raw.extend_from_slice(&[0, 0]);
        raw.extend_from_slice(&compressed);
        let (data, version) = decompress(&raw).expect("ottz");
        assert_eq!(version, 310);
        assert_eq!(data, body);
    }

    #[test]
    fn ottd_lzo_gives_clear_error() {
        let mut raw = b"OTTD".to_vec();
        raw.extend_from_slice(&[0, 50, 0, 0, 1, 2, 3]);
        let err = decompress(&raw).expect_err("lzo no soportado");
        assert!(matches!(err, SavError::UnsupportedCompression(_)));
    }

    #[test]
    fn unknown_magic_is_error() {
        let err = decompress(b"XXXX\x00\x01\x00\x00data").expect_err("magic");
        assert!(matches!(err, SavError::BadFormat(_)));
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn ottn_excessive_payload_is_rejected() {
        let mut raw = b"OTTN".to_vec();
        raw.extend_from_slice(&300u16.to_be_bytes());
        raw.extend_from_slice(&[0, 0]);
        // Payload sintético > 200 MB
        let huge_size = (MAX_SAV_DECOMPRESSED_BYTES + 1) as usize;
        raw.resize(8 + huge_size, 0xFF);
        let err = decompress(&raw).expect_err("debe rechazar payload excesivo");
        assert!(matches!(err, SavError::DecompressedSizeExceeded { .. }));
    }

    #[test]
    fn ottz_decompression_bomb_is_rejected() {
        // Bomba zlib: payload pequeño que expande > 200 MB
        let bomb_bytes = vec![0u8; 1024 * 1024]; // 1 MB de ceros comprime muy bien
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        for _ in 0..210 {
            // Repetir para superar el límite al descomprimir
            std::io::Write::write_all(&mut enc, &bomb_bytes).expect("write bomb");
        }
        let compressed = enc.finish().expect("finish");
        let mut raw = b"OTTZ".to_vec();
        raw.extend_from_slice(&310u16.to_be_bytes());
        raw.extend_from_slice(&[0, 0]);
        raw.extend_from_slice(&compressed);
        let err = decompress(&raw).expect_err("debe rechazar bomba zlib");
        assert!(matches!(err, SavError::DecompressedSizeExceeded { .. }));
    }

    #[test]
    fn ottx_decompression_bomb_is_rejected() {
        // Bomba XZ: similar al test zlib
        let bomb_bytes = vec![0u8; 1024 * 1024];
        let mut compressed = Vec::new();
        {
            let mut writer = std::io::BufWriter::new(&mut compressed);
            for _ in 0..210 {
                lzma_rs::xz_compress(&mut std::io::Cursor::new(&bomb_bytes), &mut writer)
                    .expect("xz compress bomb chunk");
            }
        }
        let mut raw = b"OTTX".to_vec();
        raw.extend_from_slice(&320u16.to_be_bytes());
        raw.extend_from_slice(&[0, 0]);
        raw.extend_from_slice(&compressed);
        let err = decompress(&raw).expect_err("debe rechazar bomba xz");
        assert!(
            matches!(
                err,
                SavError::DecompressedSizeExceeded { .. } | SavError::Decompress(_)
            ),
            "esperado error de límite o descompresión, obtenido: {err:?}"
        );
    }
}
