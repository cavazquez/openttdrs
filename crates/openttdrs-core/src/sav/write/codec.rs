//! Codificadores de bajo nivel (gamma, strings).

/// Codifica un valor gamma (< 2^14) para chunks de tabla.
pub(super) fn write_gamma(v: u32, buf: &mut Vec<u8>) {
    assert!(v < (1 << 14), "export usa gammas < 2^14");
    if v < (1 << 7) {
        buf.push(v as u8);
    } else {
        buf.push(0x80 | ((v >> 8) as u8));
        buf.push((v & 0xFF) as u8);
    }
}

/// Escribe string prefijado con su longitud gamma.
pub(super) fn write_str(s: &str, buf: &mut Vec<u8>) {
    write_gamma(s.len() as u32, buf);
    buf.extend_from_slice(s.as_bytes());
}
