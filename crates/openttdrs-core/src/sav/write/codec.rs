//! Codificadores de bajo nivel (gamma, strings).

use super::super::SavError;

/// Codifica un valor gamma (< 2^14) para chunks de tabla.
///
/// # Errors
///
/// Devuelve error si `v >= 2^14`.
pub(crate) fn write_gamma(v: u32, buf: &mut Vec<u8>) -> Result<(), SavError> {
    if v >= (1 << 14) {
        return Err(SavError::ValueOutOfRange {
            field: "gamma",
            value: v,
        });
    }
    write_full_gamma(v, buf);
    Ok(())
}

/// `SlWriteSimpleGamma` nativo, hasta u32. Una fila importada puede contener
/// columnas opacas grandes aunque los campos que escribe el MVP sean cortos.
pub(super) fn write_full_gamma(value: u32, buf: &mut Vec<u8>) {
    match value {
        0..0x80 => buf.push(value as u8),
        0x80..0x4000 => {
            buf.push(0x80 | (value >> 8) as u8);
            buf.push(value as u8);
        }
        0x4000..0x20_0000 => {
            buf.push(0xC0 | (value >> 16) as u8);
            buf.extend_from_slice(&value.to_be_bytes()[2..]);
        }
        0x20_0000..0x1000_0000 => {
            buf.push(0xE0 | (value >> 24) as u8);
            buf.extend_from_slice(&value.to_be_bytes()[1..]);
        }
        _ => {
            buf.push(0xF0);
            buf.extend_from_slice(&value.to_be_bytes());
        }
    }
}

/// Escribe string prefijado con su longitud gamma.
///
/// # Errors
///
/// Devuelve error si la longitud del string `>= 2^14`.
pub(crate) fn write_str(s: &str, buf: &mut Vec<u8>) -> Result<(), SavError> {
    write_gamma(s.len() as u32, buf)?;
    buf.extend_from_slice(s.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_gamma_boundaries_match_openttd_encoding() {
        let cases: &[(u32, &[u8])] = &[
            (0x7F, &[0x7F]),
            (0x80, &[0x80, 0x80]),
            (0x3FFF, &[0xBF, 0xFF]),
            (0x4000, &[0xC0, 0x40, 0x00]),
            (0x1F_FFFF, &[0xDF, 0xFF, 0xFF]),
            (0x20_0000, &[0xE0, 0x20, 0x00, 0x00]),
            (0x0FFF_FFFF, &[0xEF, 0xFF, 0xFF, 0xFF]),
            (0x1000_0000, &[0xF0, 0x10, 0, 0, 0]),
            (u32::MAX, &[0xF0, 0xFF, 0xFF, 0xFF, 0xFF]),
        ];
        for &(value, expected) in cases {
            let mut bytes = Vec::new();
            write_full_gamma(value, &mut bytes);
            assert_eq!(bytes, expected);
            let mut offset = 0;
            assert_eq!(
                crate::tnbp_decode::read_sl_gamma(&bytes, &mut offset),
                Ok(value)
            );
            assert_eq!(offset, bytes.len());
        }
    }

    #[test]
    fn full_gamma_rejects_truncation_and_unsupported_prefix() {
        for bytes in [
            &[0x80, 0x80][..],
            &[0xC0, 0x40, 0],
            &[0xE0, 0x20, 0, 0],
            &[0xF0, 0x10, 0, 0, 0],
        ] {
            for length in 0..bytes.len() {
                assert!(crate::tnbp_decode::read_sl_gamma(&bytes[..length], &mut 0).is_err());
            }
        }
        assert!(crate::tnbp_decode::read_sl_gamma(&[0xF8, 0, 0, 0, 0], &mut 0).is_err());
        // OpenTTD ignora los tres bits no usados del prefijo 11110---.
        assert_eq!(
            crate::tnbp_decode::read_sl_gamma(&[0xF7, 0x12, 0x34, 0x56, 0x78], &mut 0),
            Ok(0x1234_5678)
        );
    }

    #[test]
    fn write_gamma_127_succeeds() {
        let mut buf = Vec::new();
        assert!(write_gamma(127, &mut buf).is_ok());
        assert_eq!(buf, vec![127]);
    }

    #[test]
    fn write_gamma_128_succeeds() {
        let mut buf = Vec::new();
        assert!(write_gamma(128, &mut buf).is_ok());
        assert_eq!(buf, vec![0x80, 0x80]);
    }

    #[test]
    fn write_gamma_16383_succeeds() {
        let mut buf = Vec::new();
        assert!(write_gamma(16383, &mut buf).is_ok());
        assert_eq!(buf, vec![0xBF, 0xFF]);
    }

    #[test]
    fn write_gamma_16384_fails() {
        let mut buf = Vec::new();
        let result = write_gamma(16384, &mut buf);
        assert!(result.is_err());
        if let Err(SavError::ValueOutOfRange { field, value }) = result {
            assert_eq!(field, "gamma");
            assert_eq!(value, 16384);
        } else {
            panic!("Expected ValueOutOfRange error");
        }
    }

    #[test]
    fn write_str_normal_succeeds() {
        let mut buf = Vec::new();
        assert!(write_str("test", &mut buf).is_ok());
        assert_eq!(buf, vec![4, b't', b'e', b's', b't']);
    }

    #[test]
    fn write_str_empty_succeeds() {
        let mut buf = Vec::new();
        assert!(write_str("", &mut buf).is_ok());
        assert_eq!(buf, vec![0]);
    }

    #[test]
    fn write_str_too_long_fails() {
        let mut buf = Vec::new();
        let long_str = "a".repeat(16384);
        let result = write_str(&long_str, &mut buf);
        assert!(result.is_err());
    }
}
