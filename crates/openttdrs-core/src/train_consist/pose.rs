//! Poses derivadas de las unidades de un consist sobre el recorrido reciente
//! de su cabeza.

use crate::map::TileCoord;
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
/// La cabeza conserva su cinemática autoritativa. Las restantes se proyectan
/// hacia atrás por el historial de teselas usando la longitud acumulada.
#[must_use]
pub fn consist_unit_poses(vehicles: &[Vehicle], head_id: u32) -> Vec<TrainUnitPose> {
    let ids = consist_unit_ids(vehicles, head_id);
    let Some(head) = vehicles.iter().find(|v| v.id == head_id) else {
        return Vec::new();
    };
    let mut poses = Vec::with_capacity(ids.len());
    let mut back_pixels = 0_u16;
    for (index, id) in ids.iter().copied().enumerate() {
        let Some(unit) = vehicles.iter().find(|v| v.id == id) else {
            continue;
        };
        if index != 0 {
            back_pixels = back_pixels.saturating_add(u16::from(unit.unit_length.max(1)));
        }
        poses.push(project_behind_head(head, back_pixels, index));
    }
    poses
}

fn project_behind_head(head: &Vehicle, back_pixels: u16, index: usize) -> TrainUnitPose {
    const PIXELS_PER_TILE: u16 = 16;
    let head_pixel = u16::from(head.rail_pixel.min(15));
    let (tile, rail_pixel) = if back_pixels == 0 {
        (head.pos, head.rail_pixel.min(15))
    } else if back_pixels <= head_pixel {
        (
            head.pos,
            u8::try_from(head_pixel - back_pixels).unwrap_or(0),
        )
    } else {
        // Cruzamos el borde trasero de la tesela de la cabeza: `into` píxeles
        // dentro del historial (1 = recién entrada a la tesela previa, rail_pixel 15).
        let into = back_pixels - head_pixel;
        let hist = usize::from((into - 1) / PIXELS_PER_TILE);
        let pixel_from_exit = (into - 1) % PIXELS_PER_TILE;
        let rail_pixel = 15_u8.saturating_sub(u8::try_from(pixel_from_exit).unwrap_or(15));
        let tile = head
            .rail_tile_history
            .get(hist)
            .copied()
            .unwrap_or_else(|| fallback_tile(head.pos, head.direction, hist + 1));
        (tile, rail_pixel)
    };
    TrainUnitPose {
        tile,
        rail_pixel,
        direction: if index == 0 {
            head.direction
        } else {
            head.curve_prev_direction
        },
        curve_prev_direction: head.curve_prev_direction,
    }
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
        // Oráculo multi-vagón: cabeza (46,37) px5 DIR_NE; primer vagón a 8 px → (47,37) px13.
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
    fn follower_uses_head_history_after_crossing_tile() {
        let mut head = unit(1, TileCoord::new(4, 4));
        head.rail_pixel = 2;
        head.rail_tile_history.push_back(TileCoord::new(5, 4));
        head.next_unit = Some(2);
        let mut wagon = unit(2, TileCoord::new(4, 4));
        wagon.unit_length = 24;
        wagon.prev_unit = Some(1);
        let poses = consist_unit_poses(&[head, wagon], 1);
        // into = 24-2 = 22 → hist=1; sin segunda entrada, fallback 2 pasos detrás.
        assert_eq!(poses[1].tile, TileCoord::new(6, 4));
        assert_eq!(poses[1].rail_pixel, 10);
    }
}
