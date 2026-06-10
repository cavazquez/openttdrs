//! Recorrido del stream de chunks del savegame descomprimido.

use crate::tnbp_decode::read_sl_gamma;

use super::SavError;

pub(crate) const CH_RIFF: u8 = 0;
pub(crate) const CH_ARRAY: u8 = 1;
pub(crate) const CH_SPARSE_ARRAY: u8 = 2;
pub(crate) const CH_TABLE: u8 = 3;
pub(crate) const CH_SPARSE_TABLE: u8 = 4;

/// Chunk crudo: para `CH_RIFF` el payload binario; para arrays/tablas el stream
/// gamma completo (registros + terminador), igual que `slurp_array_payload` de
/// `parse_sav.py`.
pub(crate) struct RawChunk {
    pub(crate) name: [u8; 4],
    pub(crate) ch_type: u8,
    pub(crate) body: Vec<u8>,
}

fn gamma(data: &[u8], off: &mut usize) -> Result<u32, SavError> {
    read_sl_gamma(data, off).map_err(|e| SavError::BadFormat(format!("gamma inválido: {e:?}")))
}

/// Salta registros gamma hasta el terminador 0 y devuelve el rango completo.
fn slurp_gamma_records(data: &[u8], off: &mut usize) -> Result<Vec<u8>, SavError> {
    let start = *off;
    loop {
        let n = gamma(data, off)?;
        if n == 0 {
            break;
        }
        let len = n as usize - 1;
        if *off + len > data.len() {
            return Err(SavError::BadFormat("registro gamma truncado".into()));
        }
        *off += len;
    }
    Ok(data[start..*off].to_vec())
}

/// Itera los chunks del save hasta el id 0 o un tipo no soportado (best-effort).
pub(crate) fn parse_chunks(data: &[u8]) -> Result<Vec<RawChunk>, SavError> {
    let mut out = Vec::new();
    let mut off = 0usize;

    while off + 4 <= data.len() {
        let id = [data[off], data[off + 1], data[off + 2], data[off + 3]];
        off += 4;
        if id == [0, 0, 0, 0] {
            break;
        }
        let Some(&m) = data.get(off) else { break };
        off += 1;
        let ch_type = m & 0x0F;

        let body = match ch_type {
            CH_RIFF => {
                if off + 3 > data.len() {
                    return Err(SavError::BadFormat("chunk RIFF truncado".into()));
                }
                let b2 = data[off] as usize;
                let low16 = ((data[off + 1] as usize) << 8) | data[off + 2] as usize;
                off += 3;
                let size = (b2 << 16) | (((m >> 4) as usize) << 24) | low16;
                if off + size > data.len() {
                    return Err(SavError::BadFormat(format!(
                        "RIFF {} truncado ({size} bytes)",
                        String::from_utf8_lossy(&id)
                    )));
                }
                let body = data[off..off + size].to_vec();
                off += size;
                body
            }
            CH_ARRAY | CH_SPARSE_ARRAY | CH_TABLE | CH_SPARSE_TABLE => {
                slurp_gamma_records(data, &mut off)?
            }
            // CH_READONLY u otros: sin formato conocido, se detiene el parseo.
            _ => break,
        };
        out.push(RawChunk {
            name: id,
            ch_type,
            body,
        });
    }
    Ok(out)
}

pub(crate) fn find_chunk<'a>(chunks: &'a [RawChunk], name: &str) -> Option<&'a RawChunk> {
    chunks.iter().find(|c| c.name == name.as_bytes())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    pub(crate) fn write_gamma(v: u32, buf: &mut Vec<u8>) {
        assert!(v < (1 << 14), "tests usan gammas pequeños");
        if v < (1 << 7) {
            buf.push(v as u8);
        } else {
            buf.push(0x80 | ((v >> 8) as u8));
            buf.push((v & 0xFF) as u8);
        }
    }

    fn riff_chunk(name: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = name.to_vec();
        let size = payload.len();
        out.push(((size >> 24) as u8) << 4); // tipo CH_RIFF en nibble bajo = 0
        out.push((size >> 16) as u8);
        out.push((size >> 8) as u8);
        out.push(size as u8);
        out.extend_from_slice(payload);
        out
    }

    fn array_chunk(name: [u8; 4], records: &[&[u8]]) -> Vec<u8> {
        let mut out = name.to_vec();
        out.push(CH_ARRAY);
        for r in records {
            write_gamma(r.len() as u32 + 1, &mut out);
            out.extend_from_slice(r);
        }
        write_gamma(0, &mut out);
        out
    }

    #[test]
    fn parses_riff_and_array_chunks() {
        let mut data = riff_chunk(*b"MAPT", &[1, 2, 3, 4]);
        data.extend_from_slice(&array_chunk(*b"INDY", &[&[9, 9], &[7]]));
        data.extend_from_slice(&[0, 0, 0, 0]);

        let chunks = parse_chunks(&data).expect("parse");
        assert_eq!(chunks.len(), 2);
        assert_eq!(&chunks[0].name, b"MAPT");
        assert_eq!(chunks[0].body, vec![1, 2, 3, 4]);
        assert_eq!(&chunks[1].name, b"INDY");
        assert_eq!(chunks[1].ch_type, CH_ARRAY);
        assert!(find_chunk(&chunks, "MAPT").is_some());
        assert!(find_chunk(&chunks, "ZZZZ").is_none());
    }

    #[test]
    fn stops_on_zero_terminator() {
        let mut data = vec![0, 0, 0, 0];
        data.extend_from_slice(b"basura");
        let chunks = parse_chunks(&data).expect("parse");
        assert!(chunks.is_empty());
    }
}
