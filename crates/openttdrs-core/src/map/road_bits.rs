//! Decodificación de road bits desde bytes MAPT/M5 (`road_map.h`).

use super::TileKind;

/// Tipo de tesela `MP_ROAD` (nibble alto de MAPT).
pub const OTTD_MP_ROAD: u8 = 2;
/// Tipo de tesela `MP_TUNNELBRIDGE`.
pub const OTTD_MP_TUNNELBRIDGE: u8 = 9;

/// Decodifica los road bits efectivos desde `mapt`/`m5` según el subtipo de tesela.
#[must_use]
pub fn effective_road_bits(
    mapt: u8,
    m5: u8,
    kind: TileKind,
    mp_road: u8,
    mp_tunnelbridge: u8,
) -> Option<u8> {
    let tt = (mapt >> 4) & 0xF;
    match tt {
        t if t == mp_road => {
            let subtype = (m5 >> 6) & 0x3;
            match subtype {
                0 => {
                    let rb = m5 & 0x0F;
                    if rb == 0 { None } else { Some(rb) }
                }
                1 => {
                    let axis = m5 & 1;
                    Some(if axis == 0 { 0x0A } else { 0x05 })
                }
                2 => {
                    let d = m5 & 0x3;
                    Some((1u8 << (3 ^ d)) & 0x0F)
                }
                _ => None,
            }
        }
        t if t == mp_tunnelbridge && kind == TileKind::Road => {
            let d = m5 & 0x3;
            Some((1u8 << (3 ^ d)) & 0x0F)
        }
        _ => None,
    }
}
