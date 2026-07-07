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

/// Índice 0..3 en tablas diagonales `OpenTTD` (NE, SE, SW, NW).
#[must_use]
pub const fn diag_dir_index(diag: u8) -> usize {
    match diag {
        crate::DIR_NE => 0,
        crate::DIR_SE => 1,
        crate::DIR_SW => 2,
        _ => 3, // NW u otras → fila NW de la tabla
    }
}

/// `true` si el tren aún estaría oculto al entrar al túnel (frames de visibilidad
/// del original; la sim aún no aplica ocultamiento en render).
#[must_use]
pub const fn tunnel_hides_train_at_progress(enter_diag: u8, progress: u8) -> bool {
    progress < TUNNEL_VISIBILITY_FRAME[diag_dir_index(enter_diag)]
}

/// Índice de pieza en [`VEHICLE_SUBCOORD`] para un único track bit (`X`..`RIGHT`).
#[must_use]
pub const fn rail_track_index(track_bit: u8) -> Option<usize> {
    match track_bit {
        0x01 => Some(0),
        0x02 => Some(1),
        0x04 => Some(2),
        0x08 => Some(3),
        0x10 => Some(4),
        0x20 => Some(5),
        _ => None,
    }
}

/// Sub-tesela de entrada según `_vehicle_subcoord` (solo piezas válidas).
#[must_use]
pub fn openttd_subcoord_at_entry(enter_diag: u8, track_bits: u8) -> Option<(f32, f32)> {
    let bit = if track_bits.is_power_of_two() {
        track_bits
    } else if track_bits & 0x01 != 0 {
        0x01
    } else if track_bits & 0x02 != 0 {
        0x02
    } else {
        return None;
    };
    let ti = rail_track_index(bit)?;
    let sub = VEHICLE_SUBCOORD[diag_dir_index(enter_diag)][ti]?;
    Some((f32::from(sub.x), f32::from(sub.y)))
}

/// Track bits que son piezas diagonales puras (no `X`/`Y`); el render usa centro
/// de vía en lugar de la tabla por pieza.
#[must_use]
pub const fn is_diagonal_rail_piece(bits: u8) -> bool {
    bits != 0 && bits.trailing_zeros() >= 2 && bits.is_power_of_two()
}

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

#[must_use]
const fn rail_bits_touching_side(enter_diag: u8) -> u8 {
    match diag_dir_index(enter_diag) {
        0 => RAIL_TOUCHING_SIDE_NE,
        1 => RAIL_TOUCHING_SIDE_SE,
        2 => RAIL_TOUCHING_SIDE_SW,
        _ => RAIL_TOUCHING_SIDE_NW,
    }
}

/// Trackbit usado al entrar en la tesela en dirección `enter_diag` (NE/SE/SW/NW).
#[must_use]
pub fn track_bit_for_movement(enter_diag: u8, track_bits: u8) -> Option<u8> {
    let tb = track_bits & 0x3F;
    if tb == 0 {
        return None;
    }
    let mask = rail_bits_touching_side(enter_diag);
    let bits = tb & mask;
    let pick = if bits.is_power_of_two() {
        bits
    } else if bits != 0 {
        // Rectas X/Y antes que curvas cuando varias piezas son válidas (empalmes).
        const ORDER: [u8; 6] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20];
        ORDER.into_iter().find(|b| {
            bits & b != 0
                && rail_track_index(*b)
                    .is_some_and(|ti| VEHICLE_SUBCOORD[diag_dir_index(enter_diag)][ti].is_some())
        })?
    } else if tb.is_power_of_two() {
        tb
    } else {
        return None;
    };
    Some(pick)
}

/// Sub-tesela en vía según `_vehicle_subcoord` + `_fractcoords_behind`.
#[must_use]
pub fn train_subtile_on_rail(enter_diag: u8, track_bits: u8, progress: f32) -> Option<(f32, f32)> {
    let bit = track_bit_for_movement(enter_diag, track_bits)?;
    let ti = rail_track_index(bit)?;
    let sub = VEHICLE_SUBCOORD[diag_dir_index(enter_diag)][ti]?;
    let (bx, by) = FRACTCOORDS_BEHIND[diag_dir_index(enter_diag)];
    let t = (progress / 255.0).clamp(0.0, 1.0);
    let x0 = f32::from(sub.x);
    let y0 = f32::from(sub.y);
    Some((x0 + (f32::from(bx) - x0) * t, y0 + (f32::from(by) - y0) * t))
}

/// Dirección de sprite en vía (entrada en curva vs salida recta).
#[must_use]
pub fn train_render_dir_on_rail(enter_diag: u8, track_bits: u8, progress: f32) -> Option<u8> {
    let bit = track_bit_for_movement(enter_diag, track_bits)?;
    let ti = rail_track_index(bit)?;
    let sub = VEHICLE_SUBCOORD[diag_dir_index(enter_diag)][ti]?;
    if progress < 128.0 {
        Some(sub.dir)
    } else {
        Some(enter_diag)
    }
}

/// Dirección diagonal de salida/entrada según la orientación del depósito (`m5 & 3`).
#[must_use]
pub const fn train_depot_facing(mouth: u8) -> u8 {
    match mouth & 0x03 {
        0 => crate::DIR_NE,
        1 => crate::DIR_SE,
        2 => crate::DIR_SW,
        _ => crate::DIR_NW,
    }
}

/// Sub-tesela dentro del depósito: de `_fractcoords_behind` a `_fractcoords_enter`.
#[must_use]
pub fn train_depot_subtile(mouth: u8, progress: f32) -> (f32, f32) {
    let idx = diag_dir_index(train_depot_facing(mouth));
    let (bx, by) = FRACTCOORDS_BEHIND[idx];
    let (ex, ey) = FRACTCOORDS_ENTER[idx];
    let t = (progress / 255.0).clamp(0.0, 1.0);
    (
        f32::from(bx) + (f32::from(ex) - f32::from(bx)) * t,
        f32::from(by) + (f32::from(ey) - f32::from(by)) * t,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{DIR_NE, DIR_SE};

    #[test]
    fn tunnel_hides_train_matches_visibility_frame() {
        assert!(tunnel_hides_train_at_progress(DIR_NE, 11));
        assert!(!tunnel_hides_train_at_progress(DIR_NE, 12));
        assert_eq!(TUNNEL_VISIBILITY_FRAME[diag_dir_index(DIR_SE)], 8);
    }

    #[test]
    fn train_subtile_on_upper_curve_moves_diagonally() {
        use super::train_subtile_on_rail;
        // NE + LEFT: entrada válida en `_vehicle_subcoord` (15,7) → (15,8).
        let (x0, y0) = train_subtile_on_rail(DIR_NE, 0x10, 0.0).expect("NE left");
        let (_x1, y1) = train_subtile_on_rail(DIR_NE, 0x10, 255.0).expect("NE left end");
        assert!((x0 - 15.0).abs() < 0.1 && (y0 - 7.0).abs() < 0.1);
        assert!((y1 - y0).abs() > 0.5, "debe avanzar en Y por la curva");
    }

    #[test]
    fn track_bit_on_depot_junction_picks_valid_subcoord() {
        use crate::DIR_SW;
        assert_eq!(track_bit_for_movement(DIR_SW, 0x29), Some(0x01));
        let sub = train_subtile_on_rail(DIR_SW, 0x29, 128.0).expect("SW on X|LOWER|RIGHT");
        assert!((sub.0 - 0.0).abs() < 0.1);
    }

    #[test]
    fn train_depot_subtile_ends_at_enter_fractcoord() {
        use super::{FRACTCOORDS_ENTER, train_depot_subtile};
        let mouth = 3; // NW
        let (x, y) = train_depot_subtile(mouth, 255.0);
        assert_eq!(
            (x, y),
            (
                f32::from(FRACTCOORDS_ENTER[3].0),
                f32::from(FRACTCOORDS_ENTER[3].1)
            )
        );
        let (x0, y0) = train_depot_subtile(mouth, 0.0);
        assert_eq!((x0, y0), (8.0, 15.0));
    }

    #[test]
    fn openttd_subcoord_entry_for_x_track() {
        let (x, y) = openttd_subcoord_at_entry(DIR_NE, 0x01).unwrap();
        assert_eq!((x, y), (15.0, 8.0));
    }
}
