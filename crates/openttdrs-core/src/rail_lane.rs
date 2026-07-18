//! Carril paralelo y autorail (paridad `OpenTTD` `viewport.cpp` / `rail_gui.cpp` / `autorail.h`).

const RAIL_TB_X: u8 = 0x01;
const RAIL_TB_Y: u8 = 0x02;
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

/// Índices de `_autorail_piece` (`HT_DIR_*`) → `TrackBits`.
const AUTORAIL_PIECE_BITS: [u8; 6] = [
    RAIL_TB_X,     // HT_DIR_X
    RAIL_TB_Y,     // HT_DIR_Y
    RAIL_TB_UPPER, // HT_DIR_HU
    RAIL_TB_LOWER, // HT_DIR_HL
    RAIL_TB_LEFT,  // HT_DIR_VL
    RAIL_TB_RIGHT, // HT_DIR_VR
];

/// Tabla 16×16 de `OpenTTD` `_autorail_piece` (fila = `fract_y >> 4`, col = `fract_x >> 4`).
/// Valores: 0=X 1=Y 2=HU 3=HL 4=VL 5=VR.
#[rustfmt::skip]
const AUTORAIL_PIECE: [[u8; 16]; 16] = [
    [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5],
    [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5],
    [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5],
    [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5],
    [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5],
    [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5],
    [1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1],
    [4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3],
    [4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3],
    [4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3],
    [4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3],
    [4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3],
    [4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3],
];

/// Pieza de autorail según posición del cursor en la tesela (`_autorail_piece`).
///
/// `fract_*` en 0–255 (como `_tile_fract_coords`); (0,0) es la esquina N de la tesela.
#[must_use]
pub fn autorail_trackbit_from_fract(fract_x: u8, fract_y: u8) -> u8 {
    let col = usize::from(fract_x >> 4).min(15);
    let row = usize::from(fract_y >> 4).min(15);
    let piece = AUTORAIL_PIECE[row][col] as usize;
    AUTORAIL_PIECE_BITS[piece.min(5)]
}

/// ¿El arrastre de esta pieza avanza en eje X de mapa (varía `x`, fija `y`)?
#[must_use]
pub fn autorail_drag_uses_x_axis(trackbit: u8) -> bool {
    let tb = trackbit & 0x3F;
    // X / UPPER / LOWER se extienden variando x; Y / LEFT / RIGHT variando y.
    matches!(tb, RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_LOWER)
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

    #[test]
    fn autorail_fract_picks_expected_pieces() {
        // Banda X superior (fila 0, col 8).
        assert_eq!(autorail_trackbit_from_fract(128, 0), RAIL_TB_X);
        // Centro de tesela (fila 8, col 8) → X en la tabla OpenTTD.
        assert_eq!(autorail_trackbit_from_fract(128, 128), RAIL_TB_X);
        // Lado Y (fila 7, col 0).
        assert_eq!(autorail_trackbit_from_fract(0, 112), RAIL_TB_Y);
    }

    #[test]
    fn autorail_top_band_is_upper() {
        assert_eq!(autorail_trackbit_from_fract(0, 0), RAIL_TB_UPPER);
        assert_eq!(autorail_trackbit_from_fract(40, 40), RAIL_TB_UPPER);
    }

    #[test]
    fn autorail_drag_axis_matches_piece() {
        assert!(autorail_drag_uses_x_axis(RAIL_TB_X));
        assert!(autorail_drag_uses_x_axis(RAIL_TB_UPPER));
        assert!(!autorail_drag_uses_x_axis(RAIL_TB_Y));
        assert!(!autorail_drag_uses_x_axis(RAIL_TB_LEFT));
    }
}
