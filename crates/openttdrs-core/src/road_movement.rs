//! Sub-tesela de vehículos en carretera/vía (`table/roadveh_movement.h`).

use crate::vehicle::{
    DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleDirection, direction_from_tile_step,
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

/// Sub-tesela `OpenTTD` para dibujo (recto o curva de giro).
#[must_use]
pub fn vehicle_subtile(v: &Vehicle) -> (f32, f32) {
    if let Some((entry, exit)) = road_turn_entry_exit(v)
        && let Some(curve) = turn_curve(entry, exit)
    {
        return sample_curve(curve, v.progress);
    }
    straight_subtile(v.movement_direction(), v.progress)
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
    fn turn_midpoint_differs_from_tile_center() {
        let mut v = ne_to_se_turn_vehicle();
        v.progress = 128;
        let (x, y) = vehicle_subtile(&v);
        // Punto medio de la curva NE→SE (~(7,8)), no centro del rombo.
        assert!(x > 5.0 && x < 10.0);
        assert!(y > 6.0 && y < 11.0);
    }
}
