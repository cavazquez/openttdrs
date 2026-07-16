//! Curvas y trayectorias sub-tesela.

use crate::vehicle::VehicleDirection;

type SubTile = (f32, f32);

/// Carriles rectos (índices 0/1/8/9 de `_road_road_drive_data`, carril izquierdo).
const STRAIGHT: [(f32, f32, f32, f32); 8] = [
    (8.0, 8.0, 8.0, 8.0),
    (15.0, 5.0, 0.0, 5.0),
    (8.0, 8.0, 8.0, 8.0),
    (5.0, 0.0, 5.0, 15.0),
    (8.0, 8.0, 8.0, 8.0),
    (0.0, 9.0, 15.0, 9.0),
    (8.0, 8.0, 8.0, 8.0),
    (9.0, 15.0, 9.0, 0.0),
];

// Giros 90° — `_roadveh_drive_data_{2,3,4,5,10,11,12,13}` (sin flags NEXT/TURNED).
const CURVE_NE_SE: &[SubTile] = &[
    (15.0, 5.0),
    (14.0, 5.0),
    (13.0, 5.0),
    (12.0, 5.0),
    (11.0, 5.0),
    (10.0, 5.0),
    (9.0, 6.0),
    (8.0, 7.0),
    (7.0, 8.0),
    (6.0, 9.0),
    (5.0, 10.0),
    (5.0, 11.0),
    (5.0, 12.0),
    (5.0, 13.0),
    (5.0, 14.0),
    (5.0, 15.0),
];
const CURVE_SE_SW: &[SubTile] = &[
    (5.0, 0.0),
    (5.0, 1.0),
    (5.0, 2.0),
    (5.0, 3.0),
    (5.0, 4.0),
    (5.0, 5.0),
    (6.0, 6.0),
    (7.0, 7.0),
    (8.0, 8.0),
    (9.0, 9.0),
    (10.0, 9.0),
    (11.0, 9.0),
    (12.0, 9.0),
    (13.0, 9.0),
    (14.0, 9.0),
    (15.0, 9.0),
];
const CURVE_SW_NW: &[SubTile] = &[
    (0.0, 9.0),
    (1.0, 9.0),
    (2.0, 9.0),
    (3.0, 9.0),
    (4.0, 9.0),
    (5.0, 9.0),
    (6.0, 8.0),
    (7.0, 7.0),
    (8.0, 6.0),
    (9.0, 5.0),
    (9.0, 4.0),
    (9.0, 3.0),
    (9.0, 2.0),
    (9.0, 1.0),
    (9.0, 0.0),
];
const CURVE_NW_NE: &[SubTile] = &[
    (5.0, 0.0),
    (5.0, 1.0),
    (5.0, 2.0),
    (4.0, 3.0),
    (3.0, 4.0),
    (2.0, 5.0),
    (1.0, 5.0),
    (0.0, 5.0),
];
const CURVE_NE_NW: &[SubTile] = &[
    (9.0, 15.0),
    (9.0, 14.0),
    (9.0, 13.0),
    (9.0, 12.0),
    (9.0, 11.0),
    (9.0, 10.0),
    (8.0, 9.0),
    (7.0, 8.0),
    (6.0, 7.0),
    (5.0, 6.0),
    (4.0, 5.0),
    (3.0, 5.0),
    (2.0, 5.0),
    (1.0, 5.0),
    (0.0, 5.0),
];
const CURVE_NW_SW: &[SubTile] = &[
    (9.0, 15.0),
    (9.0, 14.0),
    (9.0, 13.0),
    (10.0, 12.0),
    (11.0, 11.0),
    (12.0, 10.0),
    (13.0, 9.0),
    (14.0, 9.0),
    (15.0, 9.0),
];
const CURVE_SW_SE: &[SubTile] = &[
    (0.0, 9.0),
    (1.0, 9.0),
    (2.0, 9.0),
    (3.0, 10.0),
    (4.0, 11.0),
    (5.0, 12.0),
    (5.0, 13.0),
    (5.0, 14.0),
    (5.0, 15.0),
];
const CURVE_SE_NE: &[SubTile] = &[
    (15.0, 5.0),
    (14.0, 5.0),
    (13.0, 5.0),
    (12.0, 4.0),
    (11.0, 3.0),
    (10.0, 2.0),
    (9.0, 1.0),
    (9.0, 0.0),
];

// Media vuelta en parada/estación (fin de carril de entrada → inicio del opuesto).
const U_TURN_NW_SE: &[SubTile] = &[
    (9.0, 0.0),
    (8.0, 1.0),
    (7.0, 2.0),
    (6.0, 3.0),
    (5.0, 4.0),
    (5.0, 5.0),
    (5.0, 6.0),
    (5.0, 7.0),
    (5.0, 8.0),
    (5.0, 9.0),
    (5.0, 10.0),
    (5.0, 11.0),
    (5.0, 12.0),
    (5.0, 13.0),
    (5.0, 14.0),
    (5.0, 15.0),
];
const U_TURN_NE_SW: &[SubTile] = &[
    (0.0, 5.0),
    (1.0, 5.0),
    (2.0, 6.0),
    (3.0, 7.0),
    (4.0, 8.0),
    (5.0, 9.0),
    (6.0, 9.0),
    (7.0, 9.0),
    (8.0, 9.0),
    (9.0, 9.0),
    (10.0, 9.0),
    (11.0, 9.0),
    (12.0, 9.0),
    (13.0, 9.0),
    (14.0, 9.0),
    (15.0, 9.0),
];

use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW};

pub(super) const fn turn_curve(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> Option<&'static [SubTile]> {
    match (entry, exit) {
        (DIR_NE, DIR_SE) => Some(CURVE_NE_SE),
        (DIR_SE, DIR_SW) => Some(CURVE_SE_SW),
        (DIR_SW, DIR_NW) => Some(CURVE_SW_NW),
        (DIR_NW, DIR_NE) => Some(CURVE_NW_NE),
        (DIR_NE, DIR_NW) => Some(CURVE_NE_NW),
        (DIR_NW, DIR_SW) => Some(CURVE_NW_SW),
        (DIR_SW, DIR_SE) => Some(CURVE_SW_SE),
        (DIR_SE, DIR_NE) => Some(CURVE_SE_NE),
        _ => None,
    }
}

/// Puntos sub-tesela de la curva de giro (copias de `_roadveh_drive_data_*`;
/// expuesto para los tests golden contra las tablas C++ upstream).
#[must_use]
pub const fn turn_curve_points(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> Option<&'static [(f32, f32)]> {
    turn_curve(entry, exit)
}

pub(super) fn depart_u_turn_curve(inbound: VehicleDirection) -> Option<&'static [SubTile]> {
    match inbound {
        DIR_NW | DIR_SE => Some(U_TURN_NW_SE),
        DIR_NE | DIR_SW => Some(U_TURN_NE_SW),
        _ => None,
    }
}

#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(super) fn sample_curve(points: &[SubTile], progress: f32) -> (f32, f32) {
    let n = points.len();
    if n == 0 {
        return (8.0, 8.0);
    }
    if n == 1 {
        return points[0];
    }
    // Curvas OpenTTD: ≤16 puntos; `progress` 0–255 recorre índice 0..=n-1.
    let prog = progress.clamp(0.0, 255.0);
    let last = (n - 1) as f32;
    let scaled = prog / 255.0 * last;
    let i = scaled.floor() as usize;
    let j = (i + 1).min(n - 1);
    let frac = scaled - i as f32;
    let (x0, y0) = points[i];
    let (x1, y1) = points[j];
    (x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
}

#[must_use]
pub fn straight_subtile(dir: VehicleDirection, progress: f32) -> (f32, f32) {
    let i = dir.min(7) as usize;
    let (x0, y0, x1, y1) = STRAIGHT[i];
    let t = (progress / 255.0).clamp(0.0, 1.0);
    (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
}

use crate::vehicle::{DIR_E, DIR_N, DIR_S, DIR_W};

/// Centro de vía (`OpenTTD` ~8; eje horizontal alinea con el centro visual de `TRACK_X`).
const TRAIN_TRACK_CENTER: f32 = 8.0;

#[must_use]
pub fn train_straight_subtile(dir: VehicleDirection, progress: f32) -> (f32, f32) {
    let t = (progress / 255.0).clamp(0.0, 1.0);
    match dir {
        DIR_SW | DIR_E => (15.0 * t, TRAIN_TRACK_CENTER),
        DIR_NE | DIR_W => (15.0 * (1.0 - t), TRAIN_TRACK_CENTER),
        DIR_SE | DIR_S => (TRAIN_TRACK_CENTER, 15.0 * t),
        DIR_NW | DIR_N => (TRAIN_TRACK_CENTER, 15.0 * (1.0 - t)),
        _ => (TRAIN_TRACK_CENTER, TRAIN_TRACK_CENTER),
    }
}
