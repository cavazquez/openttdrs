//! Validación vía + pendiente según `GetRailFoundation` / `CheckRailSlope` de OpenTTD
//! (`rail_cmd.cpp`, tablas `_valid_tracks_without_foundation` y
//! `_valid_tracks_on_leveled_foundation`).

use super::slope::{SLOPE_STEEP, complement_slope};

/// Esquinas individuales (`slope_type.h`).
const SLOPE_W: u8 = 0x01;
const SLOPE_S: u8 = 0x02;
const SLOPE_E: u8 = 0x04;
const SLOPE_N: u8 = 0x08;

/// `TrackBits` (`track_type.h`).
const TRACK_BIT_X: u8 = 1;
const TRACK_BIT_Y: u8 = 2;
const TRACK_BIT_UPPER: u8 = 4;
const TRACK_BIT_LOWER: u8 = 8;
const TRACK_BIT_LEFT: u8 = 16;
const TRACK_BIT_RIGHT: u8 = 32;
const TRACK_BIT_HORZ: u8 = TRACK_BIT_UPPER | TRACK_BIT_LOWER;
const TRACK_BIT_VERT: u8 = TRACK_BIT_LEFT | TRACK_BIT_RIGHT;

const FOUNDATION_INVALID: u8 = 0xFF;

/// Esquinas de tesela (`Corner` en `slope_type.h`).
const CORNER_W: u8 = 0;
const CORNER_S: u8 = 1;
const CORNER_E: u8 = 2;
const CORNER_N: u8 = 3;

const SLOPE_NWS: u8 = SLOPE_N | SLOPE_W | SLOPE_S;
const SLOPE_WSE: u8 = SLOPE_W | SLOPE_S | SLOPE_E;
const SLOPE_SEN: u8 = SLOPE_S | SLOPE_E | SLOPE_N;
const SLOPE_STEEP_W: u8 = SLOPE_STEEP | SLOPE_NWS;
const SLOPE_STEEP_S: u8 = SLOPE_STEEP | SLOPE_WSE;
const SLOPE_STEEP_E: u8 = SLOPE_STEEP | SLOPE_SEN;
const VALID_TRACKS_WITHOUT_FOUNDATION: [u8; 15] = [
    0x3F, 0x20, 0x04, 0x01, 0x10, 0x00, 0x02, 0x08, 0x08, 0x02, 0x00, 0x10, 0x01, 0x04, 0x20,
];

/// `_valid_tracks_on_leveled_foundation` en `rail_cmd.cpp`.
const VALID_TRACKS_ON_LEVELED_FOUNDATION: [u8; 15] = [
    0x00, 0x10, 0x08, 0x1A, 0x20, 0x3F, 0x29, 0x3F, 0x04, 0x15, 0x3F, 0x3F, 0x26, 0x3F, 0x3F,
];

#[inline]
const fn is_steep_slope(tileh: u8) -> bool {
    tileh & SLOPE_STEEP != 0
}

#[inline]
const fn is_slope_with_one_corner_raised(tileh: u8) -> bool {
    matches!(tileh, SLOPE_W | SLOPE_S | SLOPE_E | SLOPE_N)
}

#[inline]
const fn slope_with_one_corner_raised(corner: u8) -> u8 {
    1 << corner
}

#[inline]
const fn opposite_corner(corner: u8) -> u8 {
    corner ^ 2
}

#[inline]
const fn slope_with_three_corners_raised(corner: u8) -> u8 {
    complement_slope(slope_with_one_corner_raised(corner))
}

#[inline]
const fn is_slope_with_three_corners_raised(tileh: u8) -> bool {
    !is_steep_slope(tileh) && is_slope_with_one_corner_raised(complement_slope(tileh))
}

#[inline]
const fn corner_to_track_bits(corner: u8) -> u8 {
    match corner {
        CORNER_W => TRACK_BIT_LEFT,
        CORNER_S => TRACK_BIT_LOWER,
        CORNER_E => TRACK_BIT_RIGHT,
        _ => TRACK_BIT_UPPER,
    }
}

/// `TracksOverlap` en `track_func.h`.
#[inline]
const fn tracks_overlap(bits: u8) -> bool {
    if bits == 0 {
        return false;
    }
    let without_first = bits & (bits - 1);
    if without_first == 0 {
        return false;
    }
    bits != TRACK_BIT_HORZ && bits != TRACK_BIT_VERT
}

#[inline]
const fn highest_slope_corner(tileh: u8) -> u8 {
    match tileh & !0xE0 {
        SLOPE_W | SLOPE_STEEP_W => CORNER_W,
        SLOPE_S | SLOPE_STEEP_S => CORNER_S,
        SLOPE_E | SLOPE_STEEP_E => CORNER_E,
        _ => CORNER_N,
    }
}

#[inline]
const fn halftile_foundation(corner: u8) -> u8 {
    6 + corner
}

#[inline]
const fn special_rail_foundation(corner: u8) -> u8 {
    10 + corner
}

/// Réplica de `GetRailFoundation` (`rail_cmd.cpp`). Devuelve `FOUNDATION_INVALID` (0xFF)
/// si la combinación pendiente + `TrackBits` no es construible.
#[must_use]
pub fn rail_foundation_for_trackbits(tileh: u8, bits: u8) -> u8 {
    let bits = bits & 0x3F;
    if bits == 0 {
        return 0;
    }

    if is_steep_slope(tileh) {
        if bits == TRACK_BIT_X {
            return 2;
        }
        if bits == TRACK_BIT_Y {
            return 3;
        }
        let highest = highest_slope_corner(tileh);
        let higher_track = corner_to_track_bits(highest);
        if bits == higher_track {
            return halftile_foundation(highest);
        }
        if tracks_overlap(bits | higher_track) {
            return FOUNDATION_INVALID;
        }
        return if bits & higher_track != 0 { 5 } else { 4 };
    }

    let tileh_idx = usize::from(tileh.min(14));
    if bits & !VALID_TRACKS_WITHOUT_FOUNDATION[tileh_idx] == 0 {
        return 0;
    }

    let valid_on_leveled = bits & !VALID_TRACKS_ON_LEVELED_FOUNDATION[tileh_idx] == 0;

    let track_corner = match bits {
        TRACK_BIT_LEFT => CORNER_W,
        TRACK_BIT_LOWER => CORNER_S,
        TRACK_BIT_RIGHT => CORNER_E,
        TRACK_BIT_UPPER => CORNER_N,
        TRACK_BIT_HORZ => {
            if tileh == SLOPE_N {
                return halftile_foundation(CORNER_N);
            }
            if tileh == SLOPE_S {
                return halftile_foundation(CORNER_S);
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        TRACK_BIT_VERT => {
            if tileh == SLOPE_W {
                return halftile_foundation(CORNER_W);
            }
            if tileh == SLOPE_E {
                return halftile_foundation(CORNER_E);
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        TRACK_BIT_X => {
            if is_slope_with_one_corner_raised(tileh) {
                return 2;
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        TRACK_BIT_Y => {
            if is_slope_with_one_corner_raised(tileh) {
                return 3;
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        _ => {
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
    };

    if !valid_on_leveled {
        return FOUNDATION_INVALID;
    }
    if is_slope_with_three_corners_raised(tileh) {
        return 1;
    }
    if (tileh & slope_with_three_corners_raised(opposite_corner(track_corner)))
        == slope_with_one_corner_raised(track_corner)
    {
        return halftile_foundation(track_corner);
    }
    special_rail_foundation(track_corner)
}

/// `true` si los `TrackBits` pueden colocarse en `tileh` (fundación distinta de inválida).
#[must_use]
pub fn rail_trackbits_valid_on_slope(tileh: u8, bits: u8) -> bool {
    rail_foundation_for_trackbits(tileh, bits) != FOUNDATION_INVALID
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, TileCoord, tile_slope_and_z};

    #[test]
    fn flat_allows_all_trackbits() {
        assert!(rail_trackbits_valid_on_slope(0, 0x3F));
        assert!(rail_trackbits_valid_on_slope(0, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(0, TRACK_BIT_X | TRACK_BIT_Y));
    }

    #[test]
    fn ew_ridge_allows_diagonal_with_leveled_foundation() {
        // `SLOPE_EW` (5): sin fundación no cabe nada; con fundación nivelada sí (`GetRailFoundation`).
        assert!(rail_trackbits_valid_on_slope(5, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(5, TRACK_BIT_HORZ));
    }

    #[test]
    fn sw_slope_allows_x_and_y_with_foundation() {
        assert!(rail_trackbits_valid_on_slope(3, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(3, TRACK_BIT_Y));
    }

    #[test]
    fn w_corner_rejects_horz_track() {
        // `SLOPE_W` (1): solo RIGHT sin fundación; HORZ no es válido ni con fundación nivelada.
        assert!(!rail_trackbits_valid_on_slope(1, TRACK_BIT_HORZ));
        assert!(rail_trackbits_valid_on_slope(1, TRACK_BIT_RIGHT));
    }

    #[test]
    fn computed_tileh_matches_openrtd_sw() {
        let mut map = Map::new_flat(4, 4, 1);
        let c = TileCoord::new(1, 1);
        map.set_height(c, 1).unwrap();
        map.set_height(TileCoord::new(2, 1), 2).unwrap();
        map.set_height(TileCoord::new(1, 2), 1).unwrap();
        map.set_height(TileCoord::new(2, 2), 2).unwrap();
        let (tileh, _) = tile_slope_and_z(&map, c).unwrap();
        assert_eq!(tileh, 3);
        assert!(rail_trackbits_valid_on_slope(tileh, TRACK_BIT_X));
    }

    #[test]
    fn steep_slope_only_inclined_diagonals_or_halftile_corner() {
        assert!(rail_trackbits_valid_on_slope(SLOPE_STEEP_W, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(SLOPE_STEEP_W, TRACK_BIT_Y));
        assert!(!rail_trackbits_valid_on_slope(
            SLOPE_STEEP_W,
            TRACK_BIT_HORZ
        ));
        assert!(rail_trackbits_valid_on_slope(SLOPE_STEEP_W, TRACK_BIT_LEFT));
    }
}
