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
    /// Índice en `Vehicle::path` del siguiente paso desde `pos`.
    pub path_index: usize,
}

impl VehiclePose {
    #[must_use]
    pub fn from_vehicle(v: &Vehicle) -> Self {
        Self {
            pos: v.pos,
            progress: v.progress,
            progress_f: f32::from(v.progress),
            depart_turn: v.depart_turn,
            depart_turn_f: f32::from(v.depart_turn),
            path_index: 0,
        }
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

/// Extrapola posición sub-tesela entre ticks de sim (atraviesa límites de tesela sin saltos).
#[must_use]
pub fn extrapolate_vehicle_pose(v: &Vehicle, alpha: f32) -> VehiclePose {
    let mut pose = VehiclePose::from_vehicle(v);
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 || !v.running {
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
