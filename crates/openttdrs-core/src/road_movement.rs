//! Sub-tesela de vehículos en carretera/vía (`table/roadveh_movement.h`).

use crate::vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, Vehicle, VehicleDirection,
    VehicleKind, direction_from_tile_step, reverse_direction,
};

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

const fn turn_curve(entry: VehicleDirection, exit: VehicleDirection) -> Option<&'static [SubTile]> {
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

/// Giro de 90° en la tesela actual (`entry` → `exit` en el camino).
#[must_use]
pub fn road_turn_entry_exit(v: &Vehicle) -> Option<(VehicleDirection, VehicleDirection)> {
    if !v.running {
        return None;
    }
    let next = v.movement_target()?;
    let after = v.path.get(1).copied()?;
    let entry = direction_from_tile_step(v.pos, next);
    let exit = direction_from_tile_step(next, after);
    if entry == exit || entry & 1 == 0 || exit & 1 == 0 {
        return None;
    }
    turn_curve(entry, exit).map(|_| (entry, exit))
}

/// Centro de vía (`OpenTTD` ~8; eje horizontal alinea con el centro visual de `TRACK_X`).
const TRAIN_TRACK_CENTER: f32 = 8.0;

#[must_use]
pub fn train_subtile_direction(v: &Vehicle) -> VehicleDirection {
    if v.movement_target().is_some() && (v.progress < 255 || v.cur_speed > 0) {
        return v.movement_direction();
    }
    v.direction
}

#[must_use]
pub fn train_straight_subtile(dir: VehicleDirection, progress: u8) -> (f32, f32) {
    let t = f32::from(progress) / 255.0;
    match dir {
        DIR_SW | DIR_E => (15.0 * t, TRAIN_TRACK_CENTER),
        DIR_NE | DIR_W => (15.0 * (1.0 - t), TRAIN_TRACK_CENTER),
        DIR_SE | DIR_S => (TRAIN_TRACK_CENTER, 15.0 * t),
        DIR_NW | DIR_N => (TRAIN_TRACK_CENTER, 15.0 * (1.0 - t)),
        _ => (TRAIN_TRACK_CENTER, TRAIN_TRACK_CENTER),
    }
}

#[must_use]
pub fn straight_subtile(dir: VehicleDirection, progress: u8) -> (f32, f32) {
    let i = dir.min(7) as usize;
    let (x0, y0, x1, y1) = STRAIGHT[i];
    let t = f32::from(progress) / 255.0;
    (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
}

#[must_use]
fn sample_curve(points: &[SubTile], progress: u8) -> (f32, f32) {
    let n = points.len();
    if n == 0 {
        return (8.0, 8.0);
    }
    if n == 1 {
        return points[0];
    }
    // Curvas OpenTTD: ≤16 puntos; `progress` 0..=255 recorre índice 0..=n-1.
    let last = u8::try_from(n - 1).unwrap_or(u8::MAX);
    let scaled = u16::from(progress) * u16::from(last);
    let i = (scaled / 255).min(u16::from(last));
    let j = i.saturating_add(1).min(u16::from(last));
    let frac = f32::from(scaled % 255) / 255.0;
    let (x0, y0) = points[usize::from(i)];
    let (x1, y1) = points[usize::from(j)];
    (x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
}

fn depart_u_turn_curve(inbound: VehicleDirection) -> Option<&'static [SubTile]> {
    match inbound {
        DIR_NW => Some(U_TURN_NW_SE),
        DIR_SE => Some(U_TURN_NW_SE),
        DIR_NE => Some(U_TURN_NE_SW),
        DIR_SW => Some(U_TURN_NE_SW),
        _ => None,
    }
}

/// Progreso sub-tesela para dibujo (permite extrapolación visual entre ticks de sim).
#[must_use]
pub fn vehicle_render_progress(v: &Vehicle, tick_alpha: f32) -> u8 {
    if v.depart_turn > 0 {
        return v.depart_turn;
    }
    if !v.running || v.cur_speed == 0 || tick_alpha <= 0.0 {
        return v.progress;
    }
    let step = u16::from(v.progress_step());
    if step == 0 {
        return v.progress;
    }
    let extra = (f32::from(step) * tick_alpha.clamp(0.0, 1.0)) as u16;
    u8::try_from((u16::from(v.progress).saturating_add(extra)).min(255)).unwrap_or(255)
}

/// Sub-tesela `OpenTTD` para dibujo (recto, curva de giro o media vuelta en parada).
#[must_use]
pub fn vehicle_subtile(v: &Vehicle) -> (f32, f32) {
    vehicle_subtile_with_progress(v, v.progress)
}

/// Como [`vehicle_subtile`] con progreso explícito (p. ej. interpolación de render).
#[must_use]
pub fn vehicle_subtile_with_progress(v: &Vehicle, progress: u8) -> (f32, f32) {
    if matches!(v.kind, VehicleKind::Train) {
        return train_straight_subtile(train_subtile_direction(v), progress);
    }
    if v.depart_turn > 0 {
        if let Some(curve) = depart_u_turn_curve(v.direction) {
            return sample_curve(curve, v.depart_turn);
        }
    }
    if progress == 255 && v.movement_target().is_none() && v.pos == v.dest {
        return straight_subtile(v.direction, 255);
    }
    if let Some((entry, exit)) = road_turn_entry_exit(v)
        && let Some(curve) = turn_curve(entry, exit)
    {
        return sample_curve(curve, progress);
    }
    let dir = if progress == 255 && v.movement_target().is_some() && v.needs_depart_turnaround() {
        v.direction
    } else {
        v.movement_direction()
    };
    straight_subtile(dir, progress)
}

/// Dirección de sprite con progreso de render (giros suaves entre ticks).
#[must_use]
pub fn vehicle_render_direction(v: &Vehicle, progress: u8) -> VehicleDirection {
    if matches!(v.kind, VehicleKind::Train) {
        return train_subtile_direction(v);
    }
    if v.depart_turn > 0 {
        let outbound = v
            .movement_target()
            .map(|next| direction_from_tile_step(v.pos, next))
            .unwrap_or(v.direction);
        if progress < 128 {
            return v.direction;
        }
        return turn_cardinal_for_render(v.direction, outbound);
    }
    let Some(next) = v.movement_target() else {
        return v.direction;
    };
    let entry = direction_from_tile_step(v.pos, next);
    if progress < 128 {
        return entry;
    }
    if let Some(&after) = v.path.get(1) {
        let exit = direction_from_tile_step(next, after);
        if exit != entry {
            return turn_cardinal_for_render(entry, exit);
        }
    }
    entry
}

#[must_use]
const fn turn_cardinal_for_render(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> VehicleDirection {
    match (entry, exit) {
        (DIR_NE, DIR_SE) | (DIR_SE, DIR_NE) => crate::vehicle::DIR_E,
        (DIR_SE, DIR_SW) | (DIR_SW, DIR_SE) => crate::vehicle::DIR_S,
        (DIR_SW, DIR_NW) | (DIR_NW, DIR_SW) => crate::vehicle::DIR_W,
        (DIR_NW, DIR_NE) | (DIR_NE, DIR_NW) => crate::vehicle::DIR_N,
        _ if exit == reverse_direction(entry) => entry,
        _ => entry,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;

    use crate::map::TileCoord;
    use crate::vehicle::{Vehicle, VehicleKind};

    use super::*;

    fn ne_to_se_turn_vehicle() -> Vehicle {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(0, 2),
        );
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(0, 2)]);
        v
    }

    #[test]
    fn detects_ne_to_se_turn() {
        let v = ne_to_se_turn_vehicle();
        assert_eq!(road_turn_entry_exit(&v), Some((DIR_NE, DIR_SE)));
    }

    #[test]
    fn turn_curve_endpoints_match_openrtd_data() {
        let v = ne_to_se_turn_vehicle();
        let (entry, exit) = road_turn_entry_exit(&v).unwrap();
        let curve = turn_curve(entry, exit).unwrap();
        let start = sample_curve(curve, 0);
        let end = sample_curve(curve, 255);
        assert_eq!(start, curve[0]);
        assert_eq!(end, curve[curve.len() - 1]);
    }

    #[test]
    fn straight_tile_uses_movement_direction_not_cardinal_sprite() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        v.path = VecDeque::from([
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
        ]);
        v.progress = 200;
        assert!(road_turn_entry_exit(&v).is_none());
        let (x, y) = vehicle_subtile(&v);
        let (sx, sy) = straight_subtile(DIR_SW, 200);
        assert_eq!((x, y), (sx, sy));
    }

    #[test]
    fn train_uses_center_track_not_road_lanes() {
        let (tx, ty) = train_straight_subtile(DIR_SW, 128);
        let (rx, ry) = straight_subtile(DIR_SW, 128);
        assert!(
            (ty - TRAIN_TRACK_CENTER).abs() < 0.1,
            "eje horizontal por el centro de la vía"
        );
        assert!((ty - ry).abs() > 0.5, "no usa carril de carretera (y={ry})");
        assert!(tx > 0.0 && tx < 15.0, "avance a lo largo de x (tx={tx})");
        let _ = rx;
    }

    #[test]
    fn parked_at_station_uses_inbound_lane_end() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(15, 3),
            TileCoord::new(15, 3),
        );
        v.direction = DIR_NW;
        v.progress = 255;
        let parked = vehicle_subtile_with_progress(&v, 255);
        let inbound_end = straight_subtile(DIR_NW, 255);
        assert_eq!(parked, inbound_end);
    }

    #[test]
    fn turn_midpoint_differs_from_tile_center() {
        let mut v = ne_to_se_turn_vehicle();
        v.progress = 128;
        let (x, y) = vehicle_subtile(&v);
        // Punto medio de la curva NE→SE (~(7,8)), no centro del rombo.
        assert!(x > 5.0 && x < 10.0);
        assert!(y > 6.0 && y < 11.0);
    }
}
