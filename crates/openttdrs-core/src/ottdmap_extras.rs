//! Footers opcionales tras los planos densos de `.ottdmap` v5 (`INDP`, `STNN`, `TNBP`).

#[inline]
fn looks_like_footer_magic(data: &[u8], off: usize) -> bool {
    data.get(off..off + 4)
        .is_some_and(|m| matches!(m, b"INDP" | b"STNN" | b"TNBP" | b"M2HI" | b"STXY"))
}

/// Offset del primer byte **después** de los planos densos (MAPO…`m3hi` o …`m2_hi` en v5+12).
///
/// - Si en `12+11·n` empieza un magic de footer conocido → fin del bloque denso **v5** (11 planos).
/// - Si no hay footer ahí pero el buffer alcanza `12+12·n` → incluye el plano `m2_hi` (MAP2 alto).
/// - Si el archivo está truncado, devuelve el máximo prefijo denso coherente (≤ 11 planos).
#[must_use]
pub fn dense_payload_end(data: &[u8], n: usize) -> usize {
    let end11 = 12usize.saturating_add(n.saturating_mul(11));
    let end12 = 12usize.saturating_add(n.saturating_mul(12));
    if data.len() >= end11 && looks_like_footer_magic(data, end11) {
        return end11;
    }
    if data.len() >= end12 {
        return end12;
    }

    let data_len = data.len();
    if data_len < 12usize.saturating_add(n.saturating_mul(3)) {
        return 12usize.saturating_add(n.saturating_mul(3));
    }
    let mut end = 12usize.saturating_add(n.saturating_mul(3));
    if data_len >= 12 + n * 4 {
        end = 12 + n * 4;
    }
    if data_len >= 12 + n * 5 {
        end = 12 + n * 5;
    }
    if data_len >= 12 + n * 7 {
        end = 12 + n * 7;
    }
    if data_len >= 12 + n * 8 {
        end = 12 + n * 8;
    }
    if data_len >= 12 + n * 11 {
        end = 12 + n * 11;
    }
    end.min(end11)
}

/// Datos parseados de footers (best-effort; se detiene ante magic desconocido).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OttdmapExtras {
    /// Pares `(industry_index, industry_type)` del footer `INDP`.
    pub industry_types: Vec<(u16, u8)>,
    pub stnn_blob: Option<Vec<u8>>,
    pub tnbp_blob: Option<Vec<u8>>,
    /// Teselas `MP_STATION` listadas por `parse_sav.py` (footer `STXY`); consumible sin decodificar `STNN`.
    pub station_xy: Vec<(u16, u16)>,
}

impl OttdmapExtras {
    /// Lee la cadena de footers a partir de `dense_end` (primer byte del primer magic).
    #[must_use]
    pub fn parse_footers(data: &[u8], dense_end: usize) -> Self {
        let mut out = Self::default();
        let mut off = dense_end;
        while off + 8 <= data.len() {
            let magic = &data[off..off + 4];
            off += 4;
            match magic {
                b"INDP" => {
                    if off + 4 > data.len() {
                        break;
                    }
                    let count = usize::try_from(u32::from_le_bytes(
                        data[off..off + 4].try_into().unwrap_or([0; 4]),
                    ))
                    .unwrap_or(0);
                    off += 4;
                    let need = count.saturating_mul(3);
                    if off + need > data.len() {
                        break;
                    }
                    out.industry_types.reserve(count.min(4096));
                    for _ in 0..count {
                        let idx = u16::from_le_bytes([data[off], data[off + 1]]);
                        let typ = data[off + 2];
                        out.industry_types.push((idx, typ));
                        off += 3;
                    }
                }
                b"STNN" | b"TNBP" => {
                    if off + 4 > data.len() {
                        break;
                    }
                    let bl = usize::try_from(u32::from_le_bytes(
                        data[off..off + 4].try_into().unwrap_or([0; 4]),
                    ))
                    .unwrap_or(0);
                    off += 4;
                    if off + bl > data.len() {
                        break;
                    }
                    let blob = data[off..off + bl].to_vec();
                    off += bl;
                    if magic == b"STNN" {
                        out.stnn_blob = Some(blob);
                    } else {
                        out.tnbp_blob = Some(blob);
                    }
                }
                b"STXY" => {
                    if off + 4 > data.len() {
                        break;
                    }
                    let count = usize::try_from(u32::from_le_bytes(
                        data[off..off + 4].try_into().unwrap_or([0; 4]),
                    ))
                    .unwrap_or(0);
                    off += 4;
                    let need = count.saturating_mul(4);
                    if off + need > data.len() {
                        break;
                    }
                    out.station_xy.reserve(count.min(65536));
                    for _ in 0..count {
                        let x = u16::from_le_bytes([data[off], data[off + 1]]);
                        let y = u16::from_le_bytes([data[off + 2], data[off + 3]]);
                        off += 4;
                        out.station_xy.push((x, y));
                    }
                }
                _ => break,
            }
        }
        out
    }

    /// Tamaño del blob **TNBP** (túneles/puentes) si el `.ottdmap` lo incluyó; el pool no se decodifica aún.
    #[must_use]
    pub fn tnbp_blob_len(&self) -> usize {
        self.tnbp_blob.as_ref().map_or(0, Vec::len)
    }

    /// Busca el tipo `OpenTTD` guardado para un índice de industria en tesela (`m1` bits 0–6).
    #[must_use]
    pub fn industry_type_for_tile_index(&self, m1: u8) -> Option<u8> {
        let idx = u16::from(m1 & 0x7F);
        self.industry_types
            .iter()
            .find(|(i, _)| *i == idx)
            .map(|(_, t)| *t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_indp_after_v5_dense() {
        let w = 1u32;
        let h = 1u32;
        let n = 1usize;
        let mut v = Vec::new();
        v.extend_from_slice(b"MAPO");
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.push(0x80); // MAPT industry
        v.push(0);
        v.push(0);
        v.push(0);
        v.push(0);
        v.extend_from_slice(&0u16.to_le_bytes()); // m6 + m8
        v.push(0); // m3
        v.extend_from_slice(&[0u8; 3]); // m2 m7 m3hi
        v.extend_from_slice(b"INDP");
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&5u16.to_le_bytes());
        v.push(42);
        v.extend_from_slice(&6u16.to_le_bytes());
        v.push(7);
        let end = dense_payload_end(&v, n);
        let ex = OttdmapExtras::parse_footers(&v, end);
        assert_eq!(ex.industry_types, vec![(5, 42), (6, 7)]);
    }

    #[test]
    fn tnbp_blob_len_default_zero() {
        let ex = OttdmapExtras::default();
        assert_eq!(ex.tnbp_blob_len(), 0);
    }

    #[test]
    fn parses_stxy_footer() {
        let w = 2u32;
        let h = 2u32;
        let n = 4usize;
        let mut v = Vec::new();
        v.extend_from_slice(b"MAPO");
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&[0x50, 0, 0, 0]); // MAPT: una MP_STATION en (0,0)
        v.extend_from_slice(&[1, 1, 1, 1]); // heights
        v.extend_from_slice(&[0; 4]); // m5
        v.extend_from_slice(&[0; 4]); // m1
        v.extend_from_slice(&[0; 4]); // m6
        v.extend_from_slice(&[0u8; 8]); // m8 ×2
        v.extend_from_slice(&[0; 4]); // m3
        v.extend_from_slice(&[0; 4]); // m2
        v.extend_from_slice(&[0; 4]); // m7
        v.extend_from_slice(&[0; 4]); // m3hi
        v.extend_from_slice(&[0, 0, 0, 0]); // m2_hi v5+12
        v.extend_from_slice(b"STXY");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        let end = dense_payload_end(&v, n);
        assert_eq!(end, 12 + 12 * n);
        let ex = OttdmapExtras::parse_footers(&v, end);
        assert_eq!(ex.station_xy, vec![(0, 0)]);
    }

    #[test]
    fn dense_end_12_planes_before_indp() {
        let w = 1u32;
        let h = 1u32;
        let n = 1usize;
        let mut v = Vec::new();
        v.extend_from_slice(b"MAPO");
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.push(0x10);
        v.push(0);
        v.push(0);
        v.extend_from_slice(&[0u8; 4]);
        v.extend_from_slice(&0u16.to_le_bytes());
        v.push(0);
        v.extend_from_slice(&[0u8; 3]); // m2 m7 m3hi
        v.push(0xCD); // m2_hi
        v.extend_from_slice(b"INDP");
        v.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(dense_payload_end(&v, n), 24);
        let ex = OttdmapExtras::parse_footers(&v, 24);
        assert!(ex.industry_types.is_empty());
    }
}
