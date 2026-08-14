//! Decodificación de trackbits ferroviarios desde bytes MAPT/M5 (`rail_map.h`).

use super::TileKind;

/// Tipo de tesela `MP_RAILWAY` (nibble alto de MAPT).
pub const OTTD_MP_RAILWAY: u8 = 1;

/// Subtipo de tesela ferroviaria en bits 6–7 de `m5` (`RailTileType`).
pub const RAIL_TILE_NORMAL: u8 = 0;
pub const RAIL_TILE_SIGNALS: u8 = 1;
pub const RAIL_TILE_DEPOT: u8 = 3;

/// `TrackBits` en vía clásica (`track_type.h`).
pub const RAIL_TB_X: u8 = 0x01;
pub const RAIL_TB_Y: u8 = 0x02;
pub const RAIL_TB_UPPER: u8 = 0x04;
pub const RAIL_TB_LOWER: u8 = 0x08;
pub const RAIL_TB_LEFT: u8 = 0x10;
pub const RAIL_TB_RIGHT: u8 = 0x20;
pub const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;
pub const RAIL_TB_HORZ: u8 = RAIL_TB_UPPER | RAIL_TB_LOWER;
pub const RAIL_TB_VERT: u8 = RAIL_TB_LEFT | RAIL_TB_RIGHT;

/// Vía con señales (`RailTileType::Signals`, bits 6–7 de `m5`).
#[must_use]
pub fn rail_tile_is_signals(m5: u8) -> bool {
    (m5 >> 6) & 0x3 == RAIL_TILE_SIGNALS
}

/// Trackbits efectivos desde `mapt`/`m5` según el subtipo de tesela ferroviaria.
#[must_use]
pub fn effective_rail_trackbits(mapt: u8, m5: u8, kind: TileKind, mp_rail: u8) -> Option<u8> {
    if kind != TileKind::Rail {
        return None;
    }
    let tt = (mapt >> 4) & 0xF;
    if tt != mp_rail {
        return None;
    }
    let subtype = (m5 >> 6) & 0x3;
    match subtype {
        RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS => Some(m5 & 0x3F),
        RAIL_TILE_DEPOT => {
            let d = m5 & 0x3;
            // `DiagDirToDiagTrack(d)` = `d & 1`: NE/SW usan X y SE/NW
            // usan Y. Invertirlo hacía que un depósito vecino pareciera no
            // conectar con su rama de catenaria (`MaskWireBits`, Kale).
            Some(if d & 1 == 0 { RAIL_TB_X } else { RAIL_TB_Y })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_and_signals_return_low_six_bits() {
        let mapt = OTTD_MP_RAILWAY << 4;
        assert_eq!(
            effective_rail_trackbits(mapt, 0x15, TileKind::Rail, OTTD_MP_RAILWAY),
            Some(0x15)
        );
        let m5_sig = (RAIL_TILE_SIGNALS << 6) | 0x03;
        assert_eq!(
            effective_rail_trackbits(mapt, m5_sig, TileKind::Rail, OTTD_MP_RAILWAY),
            Some(0x03)
        );
        assert!(rail_tile_is_signals(m5_sig));
        assert!(!rail_tile_is_signals(0x15));
    }

    #[test]
    fn depot_maps_direction_to_axis() {
        let mapt = OTTD_MP_RAILWAY << 4;
        let m5_x = RAIL_TILE_DEPOT << 6;
        let m5_y = (RAIL_TILE_DEPOT << 6) | 1;
        assert_eq!(
            effective_rail_trackbits(mapt, m5_x, TileKind::Rail, OTTD_MP_RAILWAY),
            Some(RAIL_TB_X)
        );
        assert_eq!(
            effective_rail_trackbits(mapt, m5_y, TileKind::Rail, OTTD_MP_RAILWAY),
            Some(RAIL_TB_Y)
        );
    }

    #[test]
    fn rejects_wrong_kind_or_mapt() {
        let mapt = OTTD_MP_RAILWAY << 4;
        assert_eq!(
            effective_rail_trackbits(mapt, 0x01, TileKind::Road, OTTD_MP_RAILWAY),
            None
        );
        assert_eq!(
            effective_rail_trackbits(0x20, 0x01, TileKind::Rail, OTTD_MP_RAILWAY),
            None
        );
    }
}
