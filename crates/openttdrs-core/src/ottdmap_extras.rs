//! Footers opcionales tras los planos densos de `.ottdmap` v5 (`INDP`, `OBTY`, `STNN`, `TNBP`).
//!
//! Ver [`crate::tnbp_decode`] para interpretar el blob `TNBP` (tabla Sl / segmentos gamma).
use crate::map::{OTTDMAP_HEADER_LEN_VERSIONED, OTTDMAP_MAGIC_VERSIONED};

/// Offset del primer byte **después** de los planos densos (`MAP1`).
///
/// El layout actual usa 12 planos fijos por tesela.
#[must_use]
pub fn dense_payload_end(data: &[u8], n: usize) -> usize {
    let header_len = OTTDMAP_HEADER_LEN_VERSIONED;
    if data.len() < 4 || &data[0..4] != OTTDMAP_MAGIC_VERSIONED {
        return header_len.saturating_add(n.saturating_mul(12));
    }
    let end12 = header_len.saturating_add(n.saturating_mul(12));
    if data.len() >= end12 {
        return end12;
    }
    end12
}

/// Datos parseados de footers (best-effort; se detiene ante magic desconocido).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OttdmapExtras {
    /// Pares `(industry_index, industry_type)` del footer `INDP`.
    pub industry_types: Vec<(u16, u8)>,
    /// Pool autoritativo `(ObjectID, ObjectType)` del footer `OBTY`.
    ///
    /// `None` significa que el export es anterior al footer y conserva la
    /// compatibilidad histórica que codificaba el tipo directamente en `m5`.
    /// `Some(vec![])` representa un save moderno sin objetos: desde ese punto
    /// `MAP5` vuelve a ser siempre el byte alto crudo de `ObjectID`.
    pub object_types: Option<Vec<(u32, u16)>>,
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
                b"OBTY" => {
                    if off + 4 > data.len() {
                        break;
                    }
                    let count = usize::try_from(u32::from_le_bytes(
                        data[off..off + 4].try_into().unwrap_or([0; 4]),
                    ))
                    .unwrap_or(0);
                    off += 4;
                    let need = count.saturating_mul(6);
                    if off + need > data.len() {
                        break;
                    }
                    let mut object_types = Vec::with_capacity(count.min(65_536));
                    for _ in 0..count {
                        let object_id = u32::from_le_bytes([
                            data[off],
                            data[off + 1],
                            data[off + 2],
                            data[off + 3],
                        ]);
                        let object_type = u16::from_le_bytes([data[off + 4], data[off + 5]]);
                        object_types.push((object_id, object_type));
                        off += 6;
                    }
                    out.object_types = Some(object_types);
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

    /// Tamaño del blob **TNBP** (túneles/puentes) si el `.ottdmap` lo incluyó.
    #[must_use]
    pub fn tnbp_blob_len(&self) -> usize {
        self.tnbp_blob.as_ref().map_or(0, Vec::len)
    }

    /// Intenta decodificar el footer **TNBP** (formato saveload gamma / tabla Sl).
    #[must_use]
    pub fn decode_tnbp(
        &self,
    ) -> Option<Result<crate::tnbp_decode::TnbpDecoded, crate::tnbp_decode::TnbpDecodeError>> {
        self.tnbp_blob
            .as_deref()
            .map(crate::tnbp_decode::decode_tnbp_blob)
    }

    /// Túneles al estilo JGR (`tile_n` / `tile_s` en tabla) si el decode lo permite.
    #[must_use]
    pub fn jgr_tunnels_from_tnbp(&self) -> Vec<crate::tnbp_decode::JgrTunnelRecord> {
        self.decode_tnbp()
            .and_then(Result::ok)
            .map(|d| crate::tnbp_decode::jgr_tunnels_from_decoded(&d))
            .unwrap_or_default()
    }

    /// Resumen JSON del blob TNBP (decode + conteos); `None` si no hay footer.
    #[must_use]
    pub fn tnbp_json_summary(&self) -> Option<serde_json::Value> {
        self.tnbp_blob
            .as_deref()
            .map(crate::tnbp_decode::tnbp_blob_to_json_value)
    }

    /// Busca el tipo `OpenTTD` en footer `INDP` por `IndustryID` (`m2` en tesela `MP_INDUSTRY`).
    #[must_use]
    pub fn industry_type_for_instance(&self, instance: u8) -> Option<u8> {
        let idx = u16::from(instance);
        self.industry_types
            .iter()
            .find(|(i, _)| *i == idx)
            .map(|(_, t)| *t)
    }

    /// Compat: antes se pasaba `m1`; ahora preferir [`Self::industry_type_for_instance`].
    #[must_use]
    pub fn industry_type_for_tile_index(&self, m1: u8) -> Option<u8> {
        self.industry_type_for_instance(m1 & 0x7F)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_map1_header(v: &mut Vec<u8>, w: u32, h: u32) {
        v.extend_from_slice(OTTDMAP_MAGIC_VERSIONED);
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // format_version
        v.extend_from_slice(&0u16.to_le_bytes()); // flags
    }

    #[test]
    fn parses_indp_after_v5_dense() {
        let w = 1u32;
        let h = 1u32;
        let n = 1usize;
        let mut v = Vec::new();
        push_map1_header(&mut v, w, h);
        v.push(0x80); // MAPT industry
        v.push(0); // MAPH
        v.push(0); // m1
        v.push(0); // m2
        v.push(0); // m2_hi
        v.push(0); // m3
        v.push(0); // m3hi
        v.push(0); // m5
        v.push(0); // m6
        v.push(0); // m7
        v.extend_from_slice(&0u16.to_le_bytes()); // m8
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
        push_map1_header(&mut v, w, h);
        v.extend_from_slice(&[0x50, 0, 0, 0]); // MAPT: una MP_STATION en (0,0)
        v.extend_from_slice(&[1, 1, 1, 1]); // heights
        v.extend_from_slice(&[0; 4]); // m1
        v.extend_from_slice(&[0; 4]); // m2
        v.extend_from_slice(&[0; 4]); // m2_hi
        v.extend_from_slice(&[0; 4]); // m3
        v.extend_from_slice(&[0; 4]); // m3hi
        v.extend_from_slice(&[0; 4]); // m5
        v.extend_from_slice(&[0; 4]); // m6
        v.extend_from_slice(&[0; 4]); // m7
        v.extend_from_slice(&[0u8; 8]); // m8 ×2
        v.extend_from_slice(b"STXY");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        let end = dense_payload_end(&v, n);
        assert_eq!(end, 16 + 12 * n);
        let ex = OttdmapExtras::parse_footers(&v, end);
        assert_eq!(ex.station_xy, vec![(0, 0)]);
    }

    #[test]
    fn parses_obty_footer_without_rewriting_map5() {
        let w = 1u32;
        let h = 1u32;
        let n = 1usize;
        let mut v = Vec::new();
        push_map1_header(&mut v, w, h);
        v.push(0xA0); // MAPT: MP_OBJECT
        v.push(0); // MAPH
        v.push(0); // m1
        v.push(17); // m2 low: ObjectID 17
        v.push(0); // m2 high
        v.push(0); // m3
        v.push(0); // m3hi
        v.push(0); // m5: byte alto crudo de ObjectID
        v.push(0); // m6
        v.push(0); // m7
        v.extend_from_slice(&0u16.to_le_bytes()); // m8
        v.extend_from_slice(b"OBTY");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&17u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // lighthouse

        let ex = OttdmapExtras::parse_footers(&v, dense_payload_end(&v, n));
        assert_eq!(ex.object_types, Some(vec![(17, 1)]));
    }

    #[test]
    fn dense_end_12_planes_before_indp() {
        let w = 1u32;
        let h = 1u32;
        let n = 1usize;
        let mut v = Vec::new();
        push_map1_header(&mut v, w, h);
        v.push(0x10);
        v.push(0);
        v.push(0); // m1
        v.push(0); // m2
        v.push(0xCD); // m2_hi
        v.push(0); // m3
        v.push(0); // m3hi
        v.push(0); // m5
        v.push(0); // m6
        v.push(0); // m7
        v.extend_from_slice(&0u16.to_le_bytes()); // m8
        v.extend_from_slice(b"INDP");
        v.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(dense_payload_end(&v, n), 28);
        let ex = OttdmapExtras::parse_footers(&v, 28);
        assert!(ex.industry_types.is_empty());
    }
}
