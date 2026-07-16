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
    if v < (1 << 7) {
        buf.push(v as u8);
    } else {
        buf.push(0x80 | ((v >> 8) as u8));
        buf.push((v & 0xFF) as u8);
    }
    Ok(())
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
