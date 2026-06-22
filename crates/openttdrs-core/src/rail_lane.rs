//! Carril paralelo dentro de una tesela plana (paridad `OpenTTD` `viewport.cpp` / `rail_gui.cpp`).

const RAIL_TB_UPPER: u8 = 0x04;
const RAIL_TB_LOWER: u8 = 0x08;
const RAIL_TB_LEFT: u8 = 0x10;
const RAIL_TB_RIGHT: u8 = 0x20;

/// Vía E-O en pantalla: `UPPER` si `fract_x + fract_y <= 256`, si no `LOWER`.
/// Usa coordenadas fraccionarias 0–255 como `_tile_fract_coords` de `OpenTTD`.
#[must_use]
pub fn rail_horz_lane_bit(fract_x: u8, fract_y: u8) -> u8 {
    if u16::from(fract_x) + u16::from(fract_y) <= 256 {
        RAIL_TB_UPPER
    } else {
        RAIL_TB_LOWER
    }
}

/// Vía N-S en pantalla: `LEFT` si `fract_x > fract_y`, si no `RIGHT`.
#[must_use]
pub fn rail_vert_lane_bit(fract_x: u8, fract_y: u8) -> u8 {
    if fract_x > fract_y {
        RAIL_TB_LEFT
    } else {
        RAIL_TB_RIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horz_lane_splits_by_fract_sum() {
        assert_eq!(rail_horz_lane_bit(0, 0), RAIL_TB_UPPER);
        assert_eq!(rail_horz_lane_bit(128, 128), RAIL_TB_UPPER);
        assert_eq!(rail_horz_lane_bit(200, 100), RAIL_TB_LOWER);
    }

    #[test]
    fn vert_lane_splits_by_fract_compare() {
        assert_eq!(rail_vert_lane_bit(200, 100), RAIL_TB_LEFT);
        assert_eq!(rail_vert_lane_bit(50, 150), RAIL_TB_RIGHT);
        assert_eq!(rail_vert_lane_bit(100, 100), RAIL_TB_RIGHT);
    }
}
