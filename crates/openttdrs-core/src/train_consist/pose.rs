//! Poses de las unidades de un consist, unidad a unidad con
//! [`crate::train_movement::calc_next_vehicle_offset`].

use crate::map::TileCoord;
use crate::road_movement::VehiclePose;
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

/// An occurrence in the borrowed route, kept while walking the chain.
/// Coordinates alone cannot distinguish earlier visits to a junction.
#[derive(Clone, Copy)]
struct ProjectionPosition {
    pose: TrainUnitPose,
    history_index: Option<usize>,
}

/// Poses de las unidades, ordenadas desde la cabeza hasta la cola.
///
/// La cabeza conserva su cinemática autoritativa. Cada unidad siguiente se
/// coloca `CalcNextVehicleOffset(prev, next)` píxeles detrás de la anterior
/// sobre el recorrido de la cabeza (historial de teselas).
#[must_use]
pub fn consist_unit_poses(vehicles: &[Vehicle], head_id: u32) -> Vec<TrainUnitPose> {
    let ids = consist_unit_ids(vehicles, head_id);
    let slots: Vec<usize> = ids
        .iter()
        .filter_map(|id| vehicles.iter().position(|v| v.id == *id))
        .collect();
    consist_unit_poses_indexed(vehicles, &slots)
}

/// Variante para hot paths que ya disponen de slots del mismo consist.
#[must_use]
pub(crate) fn consist_unit_poses_indexed(
    vehicles: &[Vehicle],
    slots: &[usize],
) -> Vec<TrainUnitPose> {
    let Some(&head_slot) = slots.first() else {
        return Vec::new();
    };
    let Some(head) = vehicles.get(head_slot) else {
        return Vec::new();
    };
    let mut poses = Vec::with_capacity(slots.len());
    let route = ProjectionRoute {
        head,
        pos: head.pos,
        path_index: 0,
    };
    let (head_enter, head_exit) = route.directions_at(head.pos);
    let mut prev_pose = ProjectionPosition {
        pose: TrainUnitPose {
            tile: head.pos,
            rail_pixel: head.rail_pixel.min(15),
            direction: head_enter,
            curve_prev_direction: head_exit,
        },
        history_index: None,
    };
    poses.push(prev_pose.pose);

    let mut prev_length = head.unit_length.max(1);
    for &slot in slots.iter().skip(1) {
        let Some(unit) = vehicles.get(slot) else {
            continue;
        };
        let next_length = unit.unit_length.max(1);
        let offset = calc_next_vehicle_offset(prev_length, next_length, false);
        prev_pose = project_behind_unit(&route, &prev_pose, offset);
        poses.push(prev_pose.pose);
        prev_length = next_length;
    }
    poses
}

/// Presentation poses for an indexed train chain, sharing the already
/// extrapolated head's native pixel clock. Followers keep their own route
/// entry/exit metadata, even when they cross a tile between simulation ticks.
/// Depot stacks and unavailable route history keep their physical unit poses.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn consist_render_poses_indexed(
    vehicles: &[Vehicle],
    slots: &[usize],
    head_pose: VehiclePose,
) -> Vec<VehiclePose> {
    let Some(head) = slots.first().and_then(|&slot| vehicles.get(slot)) else {
        return Vec::new();
    };
    if slots.iter().any(|&slot| vehicles.get(slot).is_none()) {
        return Vec::new();
    }
    let mut poses: Vec<_> = slots
        .iter()
        .map(|&slot| VehiclePose::from_vehicle(&vehicles[slot]))
        .collect();
    poses[0] = head_pose;
    if head.kind != crate::vehicle::VehicleKind::Train
        || head.cur_speed == 0
        || head.rail_tile_history.is_empty()
        || head_pose.progress_f >= 255.0
    {
        return poses;
    }
    let route = ProjectionRoute {
        head,
        pos: head_pose.pos,
        path_index: head_pose.path_index,
    };
    let pixel = head_pose.progress_f * (16.0 / 255.0);
    // The public pose scale can round a whole pixel just below its integer.
    let nearest = pixel.round();
    let pixel = if (pixel - nearest).abs() <= pixel.abs().max(1.0) * f32::EPSILON * 2.0 {
        nearest
    } else {
        pixel
    };
    let whole = pixel.floor();
    let fraction = pixel - whole;
    let (enter, exit) = route.directions_at(head_pose.pos);
    let mut previous = ProjectionPosition {
        pose: TrainUnitPose {
            tile: head_pose.pos,
            rail_pixel: whole as u8,
            direction: enter,
            curve_prev_direction: exit,
        },
        history_index: None,
    };
    let mut previous_length = head.unit_length.max(1);
    for (index, &slot) in slots.iter().enumerate().skip(1) {
        let next_length = vehicles[slot].unit_length.max(1);
        previous = project_behind_unit(
            &route,
            &previous,
            calc_next_vehicle_offset(previous_length, next_length, false),
        );
        let pose = &mut poses[index];
        pose.pos = previous.pose.tile;
        pose.progress_f = (f32::from(previous.pose.rail_pixel) + fraction) * (255.0 / 16.0);
        pose.sync_discrete_fields();
        pose.train_route = Some((previous.pose.direction, previous.pose.curve_prev_direction));
        previous_length = next_length;
    }
    poses
}

/// Borrowed view of the head's route after virtual tile advances. It prepends
/// the traversed path to history without copying the vehicle or its orders.
struct ProjectionRoute<'a> {
    head: &'a Vehicle,
    pos: TileCoord,
    path_index: usize,
}

impl ProjectionRoute<'_> {
    fn history_tile(&self, index: usize) -> Option<TileCoord> {
        if index < self.path_index {
            if index + 1 == self.path_index {
                Some(self.head.pos)
            } else {
                self.head.path.get(self.path_index - index - 2).copied()
            }
        } else {
            self.head
                .rail_tile_history
                .get(index - self.path_index)
                .copied()
        }
    }

    fn history_index(&self, tile: TileCoord) -> Option<usize> {
        (0..self.path_index + self.head.rail_tile_history.len())
            .find(|&index| self.history_tile(index) == Some(tile))
    }

    fn next_tile(&self, index: usize) -> Option<TileCoord> {
        self.head.path.get(self.path_index + index).copied()
    }

    fn directions_at(&self, tile: TileCoord) -> (VehicleDirection, VehicleDirection) {
        route_directions_on_projection(self, tile)
    }
}

/// Retrocede `back_pixels` desde la pose de la unidad precedente sobre el
/// recorrido de la cabeza.
fn project_behind_unit(
    route: &ProjectionRoute<'_>,
    from: &ProjectionPosition,
    back_pixels: u16,
) -> ProjectionPosition {
    let head = route.head;
    let start_pixel = u16::from(from.pose.rail_pixel.min(15));
    if back_pixels == 0 {
        return *from;
    }
    if back_pixels <= start_pixel {
        let (enter, exit) = from.history_index.map_or_else(
            || route.directions_at(from.pose.tile),
            |index| route_directions_at_history(route, index),
        );
        return ProjectionPosition {
            pose: TrainUnitPose {
                tile: from.pose.tile,
                rail_pixel: u8::try_from(start_pixel - back_pixels).unwrap_or(0),
                direction: enter,
                curve_prev_direction: exit,
            },
            history_index: from.history_index,
        };
    }
    let mut into = back_pixels - start_pixel;
    let mut history = from.history_index.map_or_else(
        || {
            if from.pose.tile == route.pos {
                0
            } else {
                route.history_index(from.pose.tile).map_or(0, |i| i + 1)
            }
        },
        |index| index + 1,
    );
    loop {
        let Some(tile) = route.history_tile(history) else {
            // Keep the existing synthetic straight fallback when history is absent.
            let extra = usize::from((into - 1) / 16);
            let tile = fallback_tile(route.pos, head.direction, history + extra + 1);
            let (enter, exit) = route.directions_at(tile);
            return ProjectionPosition {
                pose: TrainUnitPose {
                    tile,
                    rail_pixel: u8::try_from(15 - (into - 1) % 16).unwrap_or(15),
                    direction: enter,
                    curve_prev_direction: exit,
                },
                history_index: None,
            };
        };
        let (enter, exit) = route_directions_at_history(route, history);
        let entry_side = crate::map::opposite_diag_dir(crate::train_movement::diag_dir_side(enter));
        let exit_side = crate::train_movement::diag_dir_side(exit);
        let track = crate::map::rail_bit_for_sides(entry_side, exit_side);
        let span = if enter & 1 == 0 || exit & 1 == 0 {
            16
        } else {
            crate::train_movement::train_render_dir_on_track(enter, track, 0.0)
                .map_or(16, crate::train_movement::rail_pixels_per_tile)
        };
        if into <= u16::from(span) {
            return ProjectionPosition {
                pose: TrainUnitPose {
                    tile,
                    rail_pixel: span - u8::try_from(into).unwrap_or(span),
                    direction: enter,
                    curve_prev_direction: exit,
                },
                history_index: Some(history),
            };
        }
        into -= u16::from(span);
        history += 1;
    }
}

/// Rumbo al entrar y al salir de una tesela del historial de la cabeza.
/// El segundo se persiste en followers para que el render reconstruya el
/// `TrackBit` correcto aun cuando su `path` se mantiene vacío.
fn route_directions_on_projection(
    route: &ProjectionRoute<'_>,
    tile: TileCoord,
) -> (VehicleDirection, VehicleDirection) {
    let head = route.head;
    if tile == route.pos {
        let enter = route.history_tile(0).map_or(head.direction, |previous| {
            crate::vehicle::direction_for_path_step(
                previous,
                tile,
                route.next_tile(0),
                head.direction,
            )
        });
        let exit = route.next_tile(0).map_or(enter, |next| {
            crate::vehicle::direction_for_path_step(tile, next, route.next_tile(1), enter)
        });
        return (enter, exit);
    }
    let Some(index) = route.history_index(tile) else {
        return (head.curve_prev_direction, head.curve_prev_direction);
    };
    route_directions_at_history(route, index)
}

/// Entrance/exit of this occurrence, including an older visit to the head tile.
fn route_directions_at_history(
    route: &ProjectionRoute<'_>,
    index: usize,
) -> (VehicleDirection, VehicleDirection) {
    let head = route.head;
    let Some(tile) = route.history_tile(index) else {
        return (head.curve_prev_direction, head.curve_prev_direction);
    };
    let newer = if index == 0 {
        route.pos
    } else {
        route.history_tile(index - 1).unwrap_or(route.pos)
    };
    let after_newer = match index {
        0 => route.next_tile(0),
        1 => Some(route.pos),
        _ => route.history_tile(index - 2),
    };
    let enter = route
        .history_tile(index + 1)
        .map_or(head.direction, |older| {
            crate::vehicle::direction_for_path_step(older, tile, Some(newer), head.direction)
        });
    let exit = crate::vehicle::direction_for_path_step(tile, newer, after_newer, enter);
    (enter, exit)
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
    use std::collections::VecDeque;

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

    #[test]
    fn tunnel_portal_keeps_consist_route_direction() {
        let west = TileCoord::new(0, 1);
        let west_mouth = TileCoord::new(1, 1);
        let east_mouth = TileCoord::new(5, 1);
        let east = TileCoord::new(6, 1);
        let mut head = unit(1, east_mouth);
        head.direction = crate::DIR_SW;
        head.path = VecDeque::from([east]);
        head.rail_tile_history = VecDeque::from([west_mouth, west]);

        assert_eq!(
            ProjectionRoute {
                head: &head,
                pos: head.pos,
                path_index: 0
            }
            .directions_at(east_mouth),
            (crate::DIR_SW, crate::DIR_SW)
        );
        assert_eq!(
            ProjectionRoute {
                head: &head,
                pos: head.pos,
                path_index: 0
            }
            .directions_at(west_mouth),
            (crate::DIR_SW, crate::DIR_SW)
        );
    }
}
