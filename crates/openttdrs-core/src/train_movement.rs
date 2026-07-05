//! Tablas y constantes ferroviarias portadas de `OpenTTD` (golden en
//! `tests/golden_rail.rs` contra `tests/fixtures/parity/train_movement_golden.json`).

/// Parámetros de frenado en curva y cambio de altura (`train_cmd.cpp:3147`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccelSlowdownParams {
    /// Penalización en giro corto (`cur_speed -= cur_speed >> 8` con 64).
    pub small_turn: u8,
    /// Penalización en giro largo (128 → −50 %).
    pub large_turn: u8,
    /// Fracción al subir (`cur_speed -= cur_speed * z_up >> 8`).
    pub z_up: u8,
    /// Bonificación al bajar (suma directa en unidades de velocidad).
    pub z_down: u8,
}

/// `_accel_slowdown[]`: normal, monorail, maglev (`train_cmd.cpp:3147-3152`).
pub const ACCEL_SLOWDOWN: [AccelSlowdownParams; 3] = [
    AccelSlowdownParams {
        small_turn: 64,
        large_turn: 128,
        z_up: 64,
        z_down: 2,
    },
    AccelSlowdownParams {
        small_turn: 64,
        large_turn: 128,
        z_up: 64,
        z_down: 2,
    },
    AccelSlowdownParams {
        small_turn: 0,
        large_turn: 128,
        z_up: 64,
        z_down: 2,
    },
];

/// `_vehicle_initial_x_fract` por `DiagDirection` NE, SE, SW, NW.
pub const VEHICLE_INITIAL_X_FRACT: [u8; 4] = [10, 8, 4, 8];
/// `_vehicle_initial_y_fract` por `DiagDirection` NE, SE, SW, NW.
pub const VEHICLE_INITIAL_Y_FRACT: [u8; 4] = [8, 4, 8, 10];

/// `_fractcoords_enter` NE, SE, SW, NW (`rail_cmd.cpp:2975`).
pub const FRACTCOORDS_ENTER: [(u8, u8); 4] = [(10, 8), (8, 4), (4, 8), (8, 10)];
/// `_fractcoords_behind` NE, SE, SW, NW (`rail_cmd.cpp:2967`).
pub const FRACTCOORDS_BEHIND: [(u8, u8); 4] = [(15, 8), (8, 0), (0, 8), (8, 15)];
/// `_deltacoord_leaveoffset` NE, SE, SW, NW (`rail_cmd.cpp:2986`).
pub const DELTACOORD_LEAVE_OFFSET: [(i8, i8); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];

/// `_tunnel_visibility_frame` NE, SE, SW, NW (`tunnelbridge_cmd.cpp:1956`).
pub const TUNNEL_VISIBILITY_FRAME: [u8; 4] = [12, 8, 8, 12];

/// Multiplicadores de `Train::UpdateSpeed` con `AM_ORIGINAL` (`train_cmd.cpp:3085`).
pub const TRAIN_UPDATE_SPEED_ACCEL_MUL: i32 = 2;
pub const TRAIN_UPDATE_SPEED_BRAKE_MUL: i32 = 4;

/// Diferencia angular entre dos direcciones (`DirDifference`, `direction_func.h:68`).
#[must_use]
pub const fn dir_difference(d0: u8, d1: u8) -> u8 {
    d0.wrapping_sub(d1) % 8
}

/// Giro de 45° (`DirDiff::Right45` / `DirDiff::Left45`).
#[must_use]
pub const fn is_45_degree_turn(d0: u8, d1: u8) -> bool {
    matches!(dir_difference(d0, d1), 1 | 7)
}

/// Sub-tesela y dirección al entrar a una tesela (`vehicle.cpp:3359`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleSubcoord {
    pub x: u8,
    pub y: u8,
    pub dir: u8,
}

/// `_vehicle_subcoord[enterdir][track]` — `None` = combinación inválida (`{}`).
/// Tracks: X, Y, UPPER, LOWER, LEFT, RIGHT.
pub const VEHICLE_SUBCOORD: [[Option<VehicleSubcoord>; 6]; 4] = [
    // NE
    [
        Some(VehicleSubcoord {
            x: 15,
            y: 8,
            dir: crate::DIR_NE,
        }),
        None,
        None,
        Some(VehicleSubcoord {
            x: 15,
            y: 8,
            dir: crate::DIR_E,
        }),
        Some(VehicleSubcoord {
            x: 15,
            y: 7,
            dir: crate::DIR_N,
        }),
        None,
    ],
    // SE
    [
        None,
        Some(VehicleSubcoord {
            x: 8,
            y: 0,
            dir: crate::DIR_SE,
        }),
        Some(VehicleSubcoord {
            x: 7,
            y: 0,
            dir: crate::DIR_E,
        }),
        None,
        Some(VehicleSubcoord {
            x: 8,
            y: 0,
            dir: crate::DIR_S,
        }),
        None,
    ],
    // SW
    [
        Some(VehicleSubcoord {
            x: 0,
            y: 8,
            dir: crate::DIR_SW,
        }),
        None,
        Some(VehicleSubcoord {
            x: 0,
            y: 7,
            dir: crate::DIR_W,
        }),
        None,
        None,
        Some(VehicleSubcoord {
            x: 0,
            y: 8,
            dir: crate::DIR_S,
        }),
    ],
    // NW
    [
        None,
        Some(VehicleSubcoord {
            x: 8,
            y: 15,
            dir: crate::DIR_NW,
        }),
        None,
        Some(VehicleSubcoord {
            x: 8,
            y: 15,
            dir: crate::DIR_W,
        }),
        None,
        Some(VehicleSubcoord {
            x: 7,
            y: 15,
            dir: crate::DIR_N,
        }),
    ],
];

/// Máscara de trackbits que tocan un lado diagonal (`_track_bits_by_diagdir`).
pub const RAIL_TOUCHING_SIDE_NE: u8 = 0x25;
pub const RAIL_TOUCHING_SIDE_SE: u8 = 0x2A;
pub const RAIL_TOUCHING_SIDE_SW: u8 = 0x19;
pub const RAIL_TOUCHING_SIDE_NW: u8 = 0x16;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn accel_slowdown_normal_row_matches_openttd_formula() {
        let row = &ACCEL_SLOWDOWN[0];
        assert_eq!(row.small_turn, 64);
        assert_eq!(row.large_turn, 128);
        assert_eq!(row.z_up, 64);
    }
}
