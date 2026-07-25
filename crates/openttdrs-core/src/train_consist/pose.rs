//! Poses de las unidades de un consist, unidad a unidad con
//! [`crate::train_movement::calc_next_vehicle_offset`].

use crate::map::TileCoord;
use crate::train_movement::calc_next_vehicle_offset;
use crate::vehicle::{
    DIR_E, DIR_N, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, Vehicle, VehicleDirection,
};

use super::topology::consist_unit_ids;

/// Pose ferroviaria de una unidad, expresada en tesela y píxel de vía.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainUnitPose {
    pub tile: TileCoord,
    pub rail_pixel: u8,
    pub direction: VehicleDirection,
    pub curve_prev_direction: VehicleDirection,
}

/// Poses de las unidades, ordenadas desde la cabeza hasta la cola.
///
/// La cabeza conserva su cinemática autoritativa. Cada unidad siguiente se
/// coloca `CalcNextVehicleOffset(prev, next)` píxeles detrás de la anterior
/// sobre el recorrido de la cabeza (historial de teselas).
#[must_use]
pub fn consist_unit_poses(vehicles: &[Vehicle], head_id: u32) -> Vec<TrainUnitPose> {
    let ids = consist_unit_ids(vehicles, head_id);
    let Some(head) = vehicles.iter().find(|v| v.id == head_id) else {
        return Vec::new();
    };
    let mut poses = Vec::with_capacity(ids.len());
    let (head_enter, head_exit) = route_directions_at(head, head.pos);
    let mut prev_pose = TrainUnitPose {
        tile: head.pos,
        rail_pixel: head.rail_pixel.min(15),
        direction: head_enter,
        curve_prev_direction: head_exit,
    };
    poses.push(prev_pose);

    let mut prev_length = head.unit_length.max(1);
    for id in ids.iter().copied().skip(1) {
        let Some(unit) = vehicles.iter().find(|v| v.id == id) else {
            continue;
        };
        let next_length = unit.unit_length.max(1);
        let offset = calc_next_vehicle_offset(prev_length, next_length, false);
        prev_pose = project_behind_unit(head, &prev_pose, offset);
        poses.push(prev_pose);
        prev_length = next_length;
    }
    poses
}

/// Retrocede `back_pixels` desde la pose de la unidad precedente sobre el
/// recorrido de la cabeza.
fn project_behind_unit(head: &Vehicle, from: &TrainUnitPose, back_pixels: u16) -> TrainUnitPose {
    const PIXELS_PER_TILE: u16 = 16;
    let start_pixel = u16::from(from.rail_pixel.min(15));
    if back_pixels == 0 {
        return *from;
    }
    if back_pixels <= start_pixel {
        let (enter, exit) = route_directions_at(head, from.tile);
        return TrainUnitPose {
            tile: from.tile,
            rail_pixel: u8::try_from(start_pixel - back_pixels).unwrap_or(0),
            direction: enter,
            curve_prev_direction: exit,
        };
    }

    // Cruzamos el borde trasero de la tesela de referencia: `into` píxeles
    // dentro del historial relativo a `from.tile`.
    let into = back_pixels - start_pixel;
    let hist_skip = history_index_after(head, from.tile);
    let hist = usize::from((into - 1) / PIXELS_PER_TILE);
    let pixel_from_exit = (into - 1) % PIXELS_PER_TILE;
    let rail_pixel = 15_u8.saturating_sub(u8::try_from(pixel_from_exit).unwrap_or(15));
    let abs_hist = hist_skip.saturating_add(hist);
    let tile = head
        .rail_tile_history
        .get(abs_hist)
        .copied()
        .unwrap_or_else(|| fallback_tile(head.pos, head.direction, abs_hist + 1));
    let (enter, exit) = route_directions_at(head, tile);
    TrainUnitPose {
        tile,
        rail_pixel,
        direction: enter,
        curve_prev_direction: exit,
    }
}

/// Rumbo al entrar y al salir de una tesela del historial de la cabeza.
/// El segundo se persiste en followers para que el render reconstruya el
/// `TrackBit` correcto aun cuando su `path` se mantiene vacío.
fn route_directions_at(head: &Vehicle, tile: TileCoord) -> (VehicleDirection, VehicleDirection) {
    if tile == head.pos {
        let enter = head.direction;
        let exit = head.path.front().copied().map_or(enter, |next| {
            crate::vehicle::direction_from_tile_step(tile, next)
        });
        return (enter, exit);
    }
    let Some(index) = head.rail_tile_history.iter().position(|&c| c == tile) else {
        return (head.curve_prev_direction, head.curve_prev_direction);
    };
    let newer = if index == 0 {
        head.pos
    } else {
        head.rail_tile_history[index - 1]
    };
    let exit = crate::vehicle::direction_from_tile_step(tile, newer);
    let enter = head
        .rail_tile_history
        .get(index + 1)
        .copied()
        .map_or(exit, |older| {
            crate::vehicle::direction_from_tile_step(older, tile)
        });
    (enter, exit)
}

/// Índice en `rail_tile_history` de la primera tesela *detrás* de `tile`.
fn history_index_after(head: &Vehicle, tile: TileCoord) -> usize {
    if tile == head.pos {
        return 0;
    }
    head.rail_tile_history
        .iter()
        .position(|&t| t == tile)
        .map_or(0, |i| i + 1)
}

/// Tesela `steps` detrás de `from` según el sentido de marcha de la cabeza.
fn fallback_tile(from: TileCoord, direction: VehicleDirection, steps: usize) -> TileCoord {
    let steps = i32::try_from(steps).unwrap_or(i32::MAX);
    // Offset de tesela en sentido contrario al avance (`_tileoffs_by_dir` invertido).
    let (dx, dy) = match direction {
        DIR_E => (1, 1),
        DIR_SE => (0, 1),
        DIR_S => (-1, 1),
        DIR_SW => (-1, 0),
        DIR_W => (-1, -1),
        DIR_NW => (0, -1),
        DIR_N => (1, -1),
        // `DIR_NE` y fallback: detrás = +X.
        _ => (1, 0),
    };
    TileCoord::new(from.x + dx * steps, from.y + dy * steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DIR_NE;
    use crate::vehicle::VehicleKind;

    fn unit(id: u32, pos: TileCoord) -> Vehicle {
        let mut v = Vehicle::new(id, VehicleKind::Train, pos, pos);
        v.unit_length = 8;
        v.direction = DIR_NE;
        v.curve_prev_direction = DIR_NE;
        v
    }

    #[test]
    fn follower_projects_half_tile_behind_head() {
        let mut head = unit(1, TileCoord::new(4, 4));
        head.rail_pixel = 12;
        head.next_unit = Some(2);
        let mut wagon = unit(2, TileCoord::new(4, 4));
        wagon.prev_unit = Some(1);
        let poses = consist_unit_poses(&[head, wagon], 1);
        assert_eq!(poses.len(), 2);
        assert_eq!(poses[1].tile, TileCoord::new(4, 4));
        assert_eq!(poses[1].rail_pixel, 4);
    }

    #[test]
    fn follower_crosses_into_previous_tile_when_back_exceeds_head_pixel() {
        // Oráculo multi-vagón: cabeza (46,37) px5 DIR_NE; primer vagón a offset 8 → (47,37) px13.
        let mut head = unit(1, TileCoord::new(46, 37));
        head.rail_pixel = 5;
        head.next_unit = Some(2);
        let mut wagon = unit(2, TileCoord::new(46, 37));
        wagon.prev_unit = Some(1);
        let poses = consist_unit_poses(&[head, wagon], 1);
        assert_eq!(poses[1].tile, TileCoord::new(47, 37));
        assert_eq!(poses[1].rail_pixel, 13);
    }

    #[test]
    fn follower_uses_calc_next_vehicle_offset_not_raw_length() {
        // Cabeza 8 + vagón 24 → offset centro-a-centro 16, no 24.
        let mut head = unit(1, TileCoord::new(4, 4));
        head.rail_pixel = 2;
        head.rail_tile_history.push_back(TileCoord::new(5, 4));
        head.next_unit = Some(2);
        let mut wagon = unit(2, TileCoord::new(4, 4));
        wagon.unit_length = 24;
        wagon.prev_unit = Some(1);
        let poses = consist_unit_poses(&[head, wagon], 1);
        // offset = calc_next_vehicle_offset(8, 24) = 16; into = 16-2 = 14 → hist=0, px=2.
        assert_eq!(poses[1].tile, TileCoord::new(5, 4));
        assert_eq!(poses[1].rail_pixel, 2);
        assert_eq!(calc_next_vehicle_offset(8, 24, false), 16);
    }

    #[test]
    fn second_wagon_offsets_from_first_not_only_head_length() {
        let mut head = unit(1, TileCoord::new(10, 10));
        head.rail_pixel = 15;
        head.next_unit = Some(2);
        let mut w1 = unit(2, TileCoord::new(10, 10));
        w1.prev_unit = Some(1);
        w1.next_unit = Some(3);
        let mut w2 = unit(3, TileCoord::new(10, 10));
        w2.prev_unit = Some(2);
        let poses = consist_unit_poses(&[head, w1, w2], 1);
        // Cada eslabón aporta offset 8 → cola a 16 px detrás de la cabeza.
        assert_eq!(poses[2].tile, TileCoord::new(11, 10));
        assert_eq!(poses[2].rail_pixel, 15);
    }
}
