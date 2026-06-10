//! Contenedor exterior del `.sav`: magic + versión + payload comprimido.

use super::SavError;

const MAGIC_OTTN: &[u8; 4] = b"OTTN";
const MAGIC_OTTZ: &[u8; 4] = b"OTTZ";
const MAGIC_OTTX: &[u8; 4] = b"OTTX";
const MAGIC_OTTD: &[u8; 4] = b"OTTD";

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
        MAGIC_OTTN => payload.to_vec(),
        MAGIC_OTTZ => {
            let mut out = Vec::new();
            let mut dec = flate2::read::ZlibDecoder::new(payload);
            std::io::Read::read_to_end(&mut dec, &mut out)
                .map_err(|e| SavError::Decompress(format!("zlib: {e}")))?;
            out
        }
        MAGIC_OTTX => {
            let mut out = Vec::new();
            let mut input = std::io::BufReader::new(payload);
            lzma_rs::xz_decompress(&mut input, &mut out)
                .map_err(|e| SavError::Decompress(format!("xz: {e:?}")))?;
            out
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
}
