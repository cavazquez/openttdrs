//! Construcción de chunks RIFF y TABLE.

use super::super::chunks::{CH_RIFF, CH_TABLE};
use super::codec::{write_gamma, write_str};

/// Chunk RIFF: fourcc + tamaño 28-bit big-endian + payload.
pub(super) fn riff_chunk(name: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = payload.len();
    let mut out = Vec::with_capacity(8 + size);
    out.extend_from_slice(&name);
    out.push((((size >> 24) as u8) << 4) | CH_RIFF);
    out.push((size >> 16) as u8);
    out.push((size >> 8) as u8);
    out.push(size as u8);
    out.extend_from_slice(payload);
    out
}

/// Chunk TABLE simple: fourcc + header con campos + records gamma.
pub(super) fn table_chunk(name: [u8; 4], fields: &[(u8, &str)], records: &[Vec<u8>]) -> Vec<u8> {
    let mut header = Vec::new();
    for &(ftype, key) in fields {
        header.push(ftype);
        write_str(key, &mut header);
    }
    header.push(0);

    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(CH_TABLE);
    write_gamma(header.len() as u32 + 1, &mut out);
    out.extend_from_slice(&header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out);
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out);
    out
}

/// Chunk TABLE/SPARSE con header arbitrario + records gamma.
pub(super) fn raw_table_chunk(
    name: [u8; 4],
    header: &[u8],
    records: &[Vec<u8>],
    ch_type: u8,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(ch_type);
    write_gamma(header.len() as u32 + 1, &mut out);
    out.extend_from_slice(header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out);
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out);
    out
}
