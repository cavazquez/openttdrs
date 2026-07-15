//! Topología de vía: conectividad por lados diagonales (`track_type.h`).

use super::rail_bits::{RAIL_TB_CROSS, RAIL_TB_X, RAIL_TB_Y};
use super::{Map, Tile, TileCoord, TileKind};
use crate::station::is_rail_waypoint_tile;

/// Máscara de trackbits que tocan un lado diagonal (`_track_bits_by_diagdir`).
pub const RAIL_TOUCHING_SIDE_NE: u8 = 0x25;
pub const RAIL_TOUCHING_SIDE_SE: u8 = 0x2A;
pub const RAIL_TOUCHING_SIDE_SW: u8 = 0x19;
pub const RAIL_TOUCHING_SIDE_NW: u8 = 0x16;

/// Máscara de trackbits que tocan un lado (`DiagDir` 0..3: NE/SE/SW/NW).
#[must_use]
pub const fn rail_bits_touching_side(side: u8) -> u8 {
    match side & 3 {
        0 => RAIL_TOUCHING_SIDE_NE,
        1 => RAIL_TOUCHING_SIDE_SE,
        2 => RAIL_TOUCHING_SIDE_SW,
        _ => RAIL_TOUCHING_SIDE_NW,
    }
}

/// Trackbit que conecta dos lados (`DiagDir`) de una tesela (`track_type.h`):
/// X = NE↔SW, Y = SE↔NW, UPPER = NE↔NW, LOWER = SE↔SW, LEFT = SW↔NW, RIGHT = NE↔SE.
#[must_use]
pub const fn rail_bit_for_sides(a: u8, b: u8) -> u8 {
    let (lo, hi) = if (a & 3) < (b & 3) {
        (a & 3, b & 3)
    } else {
        (b & 3, a & 3)
    };
    match (lo, hi) {
        (0, 2) => 0x01, // X
        (1, 3) => 0x02, // Y
        (0, 3) => 0x04, // UPPER
        (1, 2) => 0x08, // LOWER
        (2, 3) => 0x10, // LEFT
        (0, 1) => 0x20, // RIGHT
        _ => 0,
    }
}

/// Dirección diagonal opuesta (`DiagDir` 0..3).
#[must_use]
pub const fn opposite_diag_dir(d: u8) -> u8 {
    (d + 2) & 3
}

/// Offset de tesela para bloques de señal / YAPF.
///
/// NE/SW invertidos respecto a `TileOffsByDiagDir` ([`super::diag_dir_offset`]).
#[must_use]
pub const fn rail_signal_diag_dir_offset(dir: u8) -> (i32, i32) {
    match dir & 3 {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    }
}

#[must_use]
fn is_rail_station_tile(tile: &Tile) -> bool {
    tile.kind == TileKind::Station && (tile.m6 >> 3).trailing_zeros() >= 4
}

/// Trackbits transitables de una tesela de la red ferroviaria (sin depósitos,
/// que se tratan aparte porque solo conectan por su boca).
#[must_use]
pub fn rail_traversal_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Rail => {
            let tb = t.m5 & 0x3F;
            if tb == 0 { RAIL_TB_X } else { tb }
        }
        TileKind::RailTunnel | TileKind::RailBridge => RAIL_TB_CROSS,
        // Andenes y waypoints: vía a lo largo del eje en `m5` bit 0.
        TileKind::Station if is_rail_station_tile(&t) || is_rail_waypoint_tile(&t) => {
            if t.m5 & 1 != 0 { RAIL_TB_Y } else { RAIL_TB_X }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_side_masks_match_openttd() {
        assert_eq!(rail_bits_touching_side(0), RAIL_TOUCHING_SIDE_NE);
        assert_eq!(rail_bits_touching_side(1), RAIL_TOUCHING_SIDE_SE);
        assert_eq!(rail_bits_touching_side(2), RAIL_TOUCHING_SIDE_SW);
        assert_eq!(rail_bits_touching_side(3), RAIL_TOUCHING_SIDE_NW);
    }

    #[test]
    fn bit_for_sides_axis_and_curve() {
        assert_eq!(rail_bit_for_sides(0, 2), RAIL_TB_X);
        assert_eq!(rail_bit_for_sides(1, 3), RAIL_TB_Y);
        assert_eq!(rail_bit_for_sides(0, 3), 0x04); // UPPER
        assert_eq!(rail_bit_for_sides(2, 0), RAIL_TB_X); // simetría
    }

    #[test]
    fn opposite_and_signal_offset() {
        assert_eq!(opposite_diag_dir(0), 2);
        assert_eq!(opposite_diag_dir(1), 3);
        assert_eq!(rail_signal_diag_dir_offset(0), (1, 0));
        assert_eq!(rail_signal_diag_dir_offset(2), (-1, 0));
    }

    #[test]
    fn traversal_rail_station_tunnel_and_grass() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);

        map.set_kind(c, TileKind::Rail).unwrap();
        let mut t = map.get(c).unwrap();
        t.m5 = 0x03;
        map.set_tile(c, t).unwrap();
        assert_eq!(rail_traversal_bits(&map, c), 0x03);

        t.m5 = 0;
        map.set_tile(c, t).unwrap();
        assert_eq!(rail_traversal_bits(&map, c), RAIL_TB_X);

        map.set_kind(c, TileKind::RailTunnel).unwrap();
        assert_eq!(rail_traversal_bits(&map, c), RAIL_TB_CROSS);

        map.set_kind(c, TileKind::Station).unwrap();
        let mut t = map.get(c).unwrap();
        t.m6 = 0; // StationType::Rail
        t.m5 = 0x01; // eje Y
        map.set_tile(c, t).unwrap();
        assert_eq!(rail_traversal_bits(&map, c), RAIL_TB_Y);

        map.set_kind(c, TileKind::Grass).unwrap();
        assert_eq!(rail_traversal_bits(&map, c), 0);
    }
}
