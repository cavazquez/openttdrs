//! Estructura de pose y extrapolación entre ticks.

use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleKind, direction_from_tile_step, reverse_direction};

/// Posición sub-tesela usada para dibujo (puede diferir del estado de sim tras extrapolar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehiclePose {
    pub pos: TileCoord,
    pub progress: u8,
    /// Progreso sub-tesela continuo (0–255) para interpolación de render.
    pub progress_f: f32,
    pub depart_turn: u8,
    pub depart_turn_f: f32,
    /// Frame continuo de `_road_drive_data`: `Vehicle::frame` más el remanente
    /// físico normalizado por `GetAdvanceDistance`.
    pub road_frame_f: f32,
    /// Índice en `Vehicle::path` del siguiente paso desde `pos`.
    pub path_index: usize,
    /// `vehicle.road_side` / conducción por la derecha (`_rv_station_right_*`).
    pub drive_on_right: bool,
}

impl VehiclePose {
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn from_vehicle(v: &Vehicle) -> Self {
        let progress_f = if v.kind == VehicleKind::Train {
            crate::engine::train_visual_progress_from_motion(
                v.rail_pixel,
                v.progress,
                crate::engine::get_advance_distance(v.direction),
            )
        } else {
            f32::from(v.progress)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let progress = progress_f.round() as u8;
        let road_frame_f = if matches!(
            v.kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
        ) {
            let in_bay_before_stop = crate::road_movement::rvsb::is_bay_road_state(v.road_state)
                && v.road_state & crate::road_movement::rvsb::RVSB_ENTERED_STOP == 0;
            if v.cur_speed == 0 || v.movement_target().is_none() && !in_bay_before_stop {
                f32::from(v.frame)
            } else {
                f32::from(v.frame)
                    + f32::from(v.progress)
                        / crate::engine::get_advance_distance(v.direction) as f32
            }
        } else {
            0.0
        };
        Self {
            pos: v.pos,
            progress,
            progress_f,
            depart_turn: v.depart_turn,
            depart_turn_f: f32::from(v.depart_turn),
            road_frame_f,
            path_index: 0,
            drive_on_right: false,
        }
    }

    /// Marca el lado de circulación para tablas `_rv_station_*` / `_road_drive_data`.
    #[must_use]
    pub const fn with_drive_on_right(mut self, drive_on_right: bool) -> Self {
        self.drive_on_right = drive_on_right;
        self
    }

    #[must_use]
    pub(super) fn movement_progress_f(self) -> f32 {
        if self.depart_turn_f > 0.0 {
            self.depart_turn_f
        } else {
            self.progress_f
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn sync_discrete_fields(&mut self) {
        self.progress_f = self.progress_f.clamp(0.0, 255.0);
        self.depart_turn_f = self.depart_turn_f.clamp(0.0, 255.0);
        self.progress = self.progress_f.round() as u8;
        self.depart_turn = self.depart_turn_f.round() as u8;
    }
}

pub(super) fn movement_target_at(
    v: &Vehicle,
    pos: TileCoord,
    path_index: usize,
) -> Option<TileCoord> {
    if !v.running {
        return None;
    }
    if let Some(&next) = v.path.get(path_index) {
        return Some(next);
    }
    if pos == v.dest {
        return None;
    }
    if v.kind == VehicleKind::Train {
        return None;
    }
    if !v.orders.is_empty() && v.no_network_route_to_order {
        return None;
    }
    let dx = v.dest.x - pos.x;
    let dy = v.dest.y - pos.y;
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(if dx != 0 {
        TileCoord::new(pos.x + dx.signum(), pos.y)
    } else {
        TileCoord::new(pos.x, pos.y + dy.signum())
    })
}

fn needs_depart_turnaround_at(v: &Vehicle, pos: TileCoord, path_index: usize) -> bool {
    if v.kind == VehicleKind::Train {
        return false;
    }
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return false;
    };
    let outbound = direction_from_tile_step(pos, next);
    outbound == reverse_direction(v.direction)
}

pub(super) fn virtual_advance_tile(
    v: &Vehicle,
    pos: TileCoord,
    path_index: usize,
) -> Option<(TileCoord, usize)> {
    if let Some(&next) = v.path.get(path_index) {
        return Some((next, path_index + 1));
    }
    if pos == v.dest {
        return None;
    }
    let dx = v.dest.x - pos.x;
    let dy = v.dest.y - pos.y;
    if dx == 0 && dy == 0 {
        return None;
    }
    Some((
        if dx != 0 {
            TileCoord::new(pos.x + dx.signum(), pos.y)
        } else {
            TileCoord::new(pos.x, pos.y + dy.signum())
        },
        path_index,
    ))
}

/// Pose un poco detrás del vehículo para emitir humo/chispas (cola en la vía).
#[must_use]
pub fn retreat_vehicle_pose(v: &Vehicle, pose: VehiclePose, back: u8) -> VehiclePose {
    if back == 0 || pose.depart_turn_f > 0.0 {
        return pose;
    }
    let back_f = f32::from(back);
    let mut p = pose;
    if p.progress_f >= back_f {
        p.progress_f -= back_f;
        p.sync_discrete_fields();
        return p;
    }
    let deficit = back_f - p.progress_f;
    p.progress_f = 0.0;
    if let Some((prev, idx)) = previous_tile_on_route(v, p.pos, p.path_index) {
        p.pos = prev;
        p.path_index = idx;
        p.progress_f = 255.0 - deficit;
        p.sync_discrete_fields();
    }
    p
}

fn previous_tile_on_route(
    v: &Vehicle,
    pos: TileCoord,
    path_index: usize,
) -> Option<(TileCoord, usize)> {
    if path_index > 0 {
        return v.path.get(path_index - 1).map(|&c| (c, path_index - 1));
    }
    for (dx, dy) in [
        (-1_i32, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (-1, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
    ] {
        let prev = TileCoord::new(pos.x - dx, pos.y - dy);
        if movement_target_at(v, prev, path_index) == Some(pos) {
            return Some((prev, path_index));
        }
    }
    None
}

/// Extrapola la pose entre ticks de sim. En carretera avanza el frame continuo
/// de la tabla; los demás vehículos conservan la extrapolación por tesela.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn extrapolate_vehicle_pose(v: &Vehicle, alpha: f32) -> VehiclePose {
    let mut pose = VehiclePose::from_vehicle(v);
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 || !v.running || (v.kind == VehicleKind::Aircraft && v.airport_fta_active) {
        return pose;
    }
    if matches!(
        v.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        let entering_bay = crate::road_movement::rvsb::is_bay_road_state(v.road_state)
            && v.road_state & crate::road_movement::rvsb::RVSB_ENTERED_STOP == 0;
        if v.cur_speed > 0 && (v.movement_target().is_some() || entering_bay) {
            pose.road_frame_f += crate::engine::get_advance_speed(v.cur_speed) as f32 * alpha
                / crate::engine::get_advance_distance(v.direction) as f32;
        }
        return pose;
    }
    if v.kind == VehicleKind::Train {
        if v.cur_speed == 0 || movement_target_at(v, pose.pos, pose.path_index).is_none() {
            return pose;
        }
        let physical_step =
            crate::engine::get_advance_speed(v.effective_speed()).saturating_mul(2) as f32;
        let advance_distance = crate::engine::get_advance_distance(v.movement_direction()) as f32;
        let delta = physical_step / advance_distance.max(1.0) * (255.0 / 16.0) * alpha;
        pose.progress_f += delta;
        let mut path_index = pose.path_index;
        while pose.progress_f >= 255.0 {
            pose.progress_f -= 255.0;
            let Some((next, next_index)) = virtual_advance_tile(v, pose.pos, path_index) else {
                pose.progress_f = 255.0;
                break;
            };
            pose.pos = next;
            path_index = next_index;
        }
        pose.path_index = path_index;
        pose.sync_discrete_fields();
        return pose;
    }
    let mut step = f32::from(v.progress_step());
    if step <= f32::EPSILON {
        if v.cur_speed == 0 {
            return pose;
        }
        step = 1.0;
    }
    let mut delta = step * alpha;
    if delta <= f32::EPSILON {
        return pose;
    }

    let mut path_index = pose.path_index;

    if pose.depart_turn_f > 0.0 {
        pose.depart_turn_f += delta;
        if pose.depart_turn_f < 255.0 {
            pose.sync_discrete_fields();
            pose.path_index = path_index;
            return pose;
        }
        delta = pose.depart_turn_f - 255.0;
        pose.depart_turn_f = 0.0;
        pose.progress_f = 0.0;
        pose.progress = 0;
    }

    if pose.progress_f >= 255.0 && needs_depart_turnaround_at(v, pose.pos, path_index) {
        pose.depart_turn_f = delta;
        pose.path_index = path_index;
        pose.sync_discrete_fields();
        return pose;
    }

    pose.progress_f += delta;
    while pose.progress_f >= 255.0 {
        pose.progress_f -= 255.0;
        let Some((next, next_index)) = virtual_advance_tile(v, pose.pos, path_index) else {
            pose.progress_f = 255.0;
            break;
        };
        pose.pos = next;
        path_index = next_index;
    }

    pose.path_index = path_index;
    pose.sync_discrete_fields();
    pose
}

/// Progreso sub-tesela para dibujo (permite extrapolación visual entre ticks de sim).
#[must_use]
pub fn vehicle_render_progress(v: &Vehicle, tick_alpha: f32) -> u8 {
    extrapolate_vehicle_pose(v, tick_alpha).progress
}
