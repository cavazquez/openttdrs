//! Funciones de rendering: dirección y posición sub-tesela para sprites.

use super::bay::{
    bay_direction_at_frame_side, bay_render_direction, bay_subtile, bay_subtile_at_frame_side,
    direction_from_subtile_delta, parked_inside_bay,
};
use super::curves::{
    depart_u_turn_curve, sample_curve, straight_subtile, train_straight_subtile, turn_curve,
};
use super::depot::{road_depot_direction, road_depot_subtile};
use super::drive_data::road_drive_entry;
use super::overtake::drive_state_with_overtake_and_side;
use super::pose::{VehiclePose, movement_target_at};
use super::rvsb::is_bay_road_state;
use crate::depot::rail_depot_mouth_dir;
use crate::map::{Map, TileKind};
use crate::refit::vehicle_in_depot;
use crate::train_movement::{
    diag_dir_side, train_depot_facing, train_depot_subtile, train_render_dir_on_track,
    train_subtile_on_track,
};
use crate::vehicle::{Vehicle, VehicleDirection, VehicleKind, direction_from_tile_step};

/// Sub-tesela `OpenTTD` para dibujo (recto, curva de giro o media vuelta en parada).
#[must_use]
pub fn vehicle_subtile(v: &Vehicle) -> (f32, f32) {
    vehicle_subtile_at(v, VehiclePose::from_vehicle(v))
}

/// Como [`vehicle_subtile`] con progreso explícito (p. ej. interpolación de render).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn vehicle_subtile_with_progress(v: &Vehicle, progress: u8) -> (f32, f32) {
    let mut pose = VehiclePose::from_vehicle(v);
    pose.progress = progress;
    pose.progress_f = f32::from(progress);
    pose.road_frame_f = f32::from(v.frame)
        + f32::from(progress) / crate::engine::get_advance_distance(v.direction) as f32;
    vehicle_subtile_at(v, pose)
}

/// Sub-tesela para una pose concreta (sim actual o extrapolada).
#[must_use]
pub fn vehicle_subtile_at(v: &Vehicle, pose: VehiclePose) -> (f32, f32) {
    vehicle_subtile_at_with_map(v, pose, None)
}

/// Como [`vehicle_subtile_at`] con mapa para depósito y entrada a vía.
#[must_use]
pub fn vehicle_subtile_at_with_map(
    v: &Vehicle,
    pose: VehiclePose,
    map: Option<&Map>,
) -> (f32, f32) {
    if v.kind == VehicleKind::Aircraft && v.airport_fta_active && v.airport_subpos_valid {
        return (
            f32::from(u8::try_from(v.airport_sub_x.rem_euclid(16)).unwrap_or(0)),
            f32::from(u8::try_from(v.airport_sub_y.rem_euclid(16)).unwrap_or(0)),
        );
    }
    if matches!(v.kind, VehicleKind::Train) {
        return train_subtile_with_map(v, pose, map);
    }
    if let Some(subtile) = road_depot_subtile(v.road_depot_phase) {
        return subtile;
    }
    if parked_inside_bay(v, pose.pos)
        && !is_bay_road_state(v.road_state)
        && let Some(subtile) = bay_subtile(v, pose)
    {
        return subtile;
    }
    if is_road_kind(v.kind)
        && let Some(subtile) = road_frame_subtile(v, pose.road_frame_f, pose.drive_on_right)
    {
        return subtile;
    }
    if pose.depart_turn_f > 0.0
        && let Some(curve) = depart_u_turn_curve(v.direction)
    {
        return sample_curve(curve, pose.depart_turn_f);
    }
    if pose.progress_f >= 255.0
        && movement_target_at(v, pose.pos, pose.path_index).is_none()
        && pose.pos == v.dest
    {
        return straight_subtile(v.direction, 255.0);
    }
    if let Some((entry, exit)) = road_turn_entry_exit_at(v, pose.pos, pose.path_index)
        && let Some(curve) = turn_curve(entry, exit)
    {
        return sample_curve(curve, pose.progress_f);
    }
    let dir = if pose.progress_f >= 255.0
        && movement_target_at(v, pose.pos, pose.path_index).is_some()
        && needs_depart_turnaround_at(v, pose.pos, pose.path_index)
    {
        v.direction
    } else {
        movement_direction_at(v, pose.pos, pose.path_index)
    };
    straight_subtile(dir, pose.progress_f)
}

#[must_use]
pub fn train_subtile_direction(v: &Vehicle) -> VehicleDirection {
    if v.movement_target().is_some() && (v.rail_pixel > 0 || v.cur_speed > 0 || v.progress > 0) {
        return v.movement_direction();
    }
    v.direction
}

fn train_rail_subtile(map: &Map, v: &Vehicle, pose: VehiclePose) -> (f32, f32) {
    let (enter, track) = train_route_on_tile(map, v, pose);
    let progress = pose.movement_progress_f();
    if let Some(track) = track
        && let Some(sub) = train_subtile_on_track(enter, track, progress)
    {
        return sub;
    }
    train_straight_subtile(enter, progress)
}

fn train_render_direction_with_map(map: &Map, v: &Vehicle, pose: VehiclePose) -> VehicleDirection {
    let (enter, track) = train_route_on_tile(map, v, pose);
    let progress = pose.movement_progress_f();
    if let Some(track) = track
        && let Some(dir) = train_render_dir_on_track(enter, track, progress)
    {
        return dir;
    }
    enter
}

/// Dirección de entrada y `TrackBit` exactos de la ruta para la pose dibujada.
///
/// La selección anterior miraba todos los bits del empalme y priorizaba una
/// recta aunque YAPF hubiese elegido una curva. Aquí se reconstruyen los dos
/// lados desde `anterior → actual → siguiente`; para followers, el controlador
/// conserva esos rumbos en `direction`/`curve_prev_direction`.
fn train_route_on_tile(
    map: &Map,
    v: &Vehicle,
    pose: VehiclePose,
) -> (VehicleDirection, Option<u8>) {
    let previous = train_previous_tile_at(v, pose);
    let enter = previous.map_or(v.direction, |prev| direction_from_tile_step(prev, pose.pos));
    let outbound = movement_target_at(v, pose.pos, pose.path_index).map_or_else(
        || {
            if v.prev_unit.is_some() {
                v.curve_prev_direction
            } else {
                enter
            }
        },
        |next| direction_from_tile_step(pose.pos, next),
    );

    let Some(tile) = map.get(pose.pos).filter(|tile| tile.kind == TileKind::Rail) else {
        return (enter, None);
    };
    let track_bits = tile.m5 & 0x3F;
    let entry_side = crate::map::opposite_diag_dir(diag_dir_side(enter));
    let exit_side = diag_dir_side(outbound);
    let route_track = crate::map::rail_bit_for_sides(entry_side, exit_side);
    if route_track != 0 && track_bits & route_track != 0 {
        return (enter, Some(route_track));
    }

    if let Some(step) = v.reserved_steps.iter().find(|step| {
        step.tile == pose.pos
            && track_bits & step.track != 0
            && crate::map::rail_bits_touching_side(entry_side) & step.track != 0
    }) {
        return (enter, Some(step.track));
    }
    (
        enter,
        crate::train_movement::track_bit_for_movement(enter, track_bits),
    )
}

fn train_previous_tile_at(v: &Vehicle, pose: VehiclePose) -> Option<crate::map::TileCoord> {
    match pose.path_index {
        0 if pose.pos == v.pos => v.rail_tile_history.front().copied(),
        0 => None,
        1 => Some(v.pos),
        n => v.path.get(n - 2).copied(),
    }
}

fn train_subtile_with_map(v: &Vehicle, pose: VehiclePose, map: Option<&Map>) -> (f32, f32) {
    if let Some(map) = map
        && vehicle_in_depot(map, pose.pos)
        && let Some(mouth) = rail_depot_mouth_dir(map, pose.pos)
    {
        return train_depot_subtile(mouth, pose.movement_progress_f());
    }
    if let Some(map) = map {
        return train_rail_subtile(map, v, pose);
    }
    train_straight_subtile(train_subtile_direction(v), pose.movement_progress_f())
}

/// Dirección de sprite con progreso de render (giros suaves entre ticks).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn vehicle_render_direction(v: &Vehicle, progress: u8) -> VehicleDirection {
    let mut pose = VehiclePose::from_vehicle(v);
    pose.progress = progress;
    pose.progress_f = f32::from(progress);
    pose.road_frame_f = f32::from(v.frame)
        + f32::from(progress) / crate::engine::get_advance_distance(v.direction) as f32;
    vehicle_render_direction_at(v, pose)
}

/// Dirección de sprite para una pose concreta.
#[must_use]
pub fn vehicle_render_direction_at(v: &Vehicle, pose: VehiclePose) -> VehicleDirection {
    vehicle_render_direction_at_with_map(v, pose, None)
}

/// Como [`vehicle_render_direction_at`] con mapa (orientación en depósito).
#[must_use]
pub fn vehicle_render_direction_at_with_map(
    v: &Vehicle,
    pose: VehiclePose,
    map: Option<&Map>,
) -> VehicleDirection {
    // La FTA mueve aeronaves en coordenadas sub-tesela y actualiza `direction`
    // con el desplazamiento real de cada tick. Su `path` permanece vacío; si
    // se reconstruye aquí un movimiento Manhattan hacia la orden global, el
    // sprite queda apuntando al aeropuerto aunque el avión esté girando.
    if v.kind == VehicleKind::Aircraft && v.airport_fta_active {
        return v.direction;
    }
    if matches!(v.kind, VehicleKind::Train)
        && let Some(map) = map
        && vehicle_in_depot(map, pose.pos)
        && let Some(mouth) = rail_depot_mouth_dir(map, pose.pos)
    {
        return train_depot_facing(mouth);
    }
    if let Some(direction) = road_depot_direction(v.road_depot_phase) {
        return direction;
    }
    if matches!(v.kind, VehicleKind::Train) {
        if let Some(map) = map {
            return train_render_direction_with_map(map, v, pose);
        }
        return train_subtile_direction(v);
    }
    if parked_inside_bay(v, pose.pos)
        && !is_bay_road_state(v.road_state)
        && pose.depart_turn_f <= 0.0
        && pose.progress_f < 255.0
        && let Some(dir) = bay_render_direction(v, pose)
    {
        return dir;
    }
    if is_road_kind(v.kind)
        && let Some(direction) = road_frame_direction(v, pose.road_frame_f, pose.drive_on_right)
    {
        return direction;
    }
    if pose.depart_turn_f > 0.0 {
        let outbound = movement_target_at(v, pose.pos, pose.path_index)
            .map_or(v.direction, |next| direction_from_tile_step(pose.pos, next));
        if pose.depart_turn_f < 128.0 {
            return v.direction;
        }
        return turn_cardinal_for_render(v.direction, outbound);
    }
    let Some(next) = movement_target_at(v, pose.pos, pose.path_index) else {
        return v.direction;
    };
    let entry = direction_from_tile_step(pose.pos, next);
    if pose.progress_f < 128.0 {
        return entry;
    }
    if let Some(after) = v.path.get(pose.path_index + 1) {
        let exit = direction_from_tile_step(next, *after);
        if exit != entry {
            return turn_cardinal_for_render(entry, exit);
        }
    }
    entry
}

#[must_use]
const fn is_road_kind(kind: VehicleKind) -> bool {
    matches!(
        kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn road_frame_subtile(v: &Vehicle, frame_f: f32, drive_on_right: bool) -> Option<(f32, f32)> {
    if is_bay_road_state(v.road_state) {
        return bay_subtile_at_frame_side(v.road_state, frame_f, drive_on_right);
    }
    let state =
        drive_state_with_overtake_and_side(v.road_state, v.overtaking, drive_on_right) & 0x1F;
    let frame_f = frame_f.max(0.0);
    let index = frame_f.floor().min(f32::from(u8::MAX)) as u8;
    let a = normal_road_point(state, index).or_else(|| {
        index
            .checked_sub(1)
            .and_then(|i| normal_road_point(state, i))
    })?;
    let b = index
        .checked_add(1)
        .and_then(|i| normal_road_point(state, i))
        .unwrap_or(a);
    let t = frame_f.fract();
    Some((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn road_frame_direction(
    v: &Vehicle,
    frame_f: f32,
    drive_on_right: bool,
) -> Option<VehicleDirection> {
    if is_bay_road_state(v.road_state) {
        return bay_direction_at_frame_side(v.road_state, frame_f, drive_on_right);
    }
    let state =
        drive_state_with_overtake_and_side(v.road_state, v.overtaking, drive_on_right) & 0x1F;
    let index = frame_f.floor().clamp(0.0, f32::from(u8::MAX)) as u8;
    let here = normal_road_point(state, index)?;
    if let Some(next) = index
        .checked_add(1)
        .and_then(|i| normal_road_point(state, i))
        && let Some(direction) = direction_from_subtile_delta(next.0 - here.0, next.1 - here.1)
    {
        return Some(direction);
    }
    let previous = index
        .checked_sub(1)
        .and_then(|i| normal_road_point(state, i))?;
    direction_from_subtile_delta(here.0 - previous.0, here.1 - previous.1)
}

fn normal_road_point(state: u8, frame: u8) -> Option<(f32, f32)> {
    let entry = road_drive_entry(state, frame)?;
    if entry.is_next_tile() || entry.is_turned() {
        return None;
    }
    Some((f32::from(entry.x), f32::from(entry.y)))
}

#[must_use]
const fn turn_cardinal_for_render(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> VehicleDirection {
    use crate::vehicle::{
        DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, reverse_direction,
    };
    match (entry, exit) {
        (DIR_NE, DIR_SE) | (DIR_SE, DIR_NE) => DIR_E,
        (DIR_SE, DIR_SW) | (DIR_SW, DIR_SE) => DIR_S,
        (DIR_SW, DIR_NW) | (DIR_NW, DIR_SW) => DIR_W,
        (DIR_NW, DIR_NE) | (DIR_NE, DIR_NW) => DIR_N,
        _ if exit == reverse_direction(entry) => entry,
        _ => entry,
    }
}

fn movement_direction_at(
    v: &Vehicle,
    pos: crate::map::TileCoord,
    path_index: usize,
) -> VehicleDirection {
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return v.direction;
    };
    direction_from_tile_step(pos, next)
}

fn road_turn_entry_exit_at(
    v: &Vehicle,
    pos: crate::map::TileCoord,
    path_index: usize,
) -> Option<(VehicleDirection, VehicleDirection)> {
    if !v.running {
        return None;
    }
    let next = movement_target_at(v, pos, path_index)?;
    let after = v.path.get(path_index + 1).copied()?;
    let entry = direction_from_tile_step(pos, next);
    let exit = direction_from_tile_step(next, after);
    if entry == exit || entry & 1 == 0 || exit & 1 == 0 {
        return None;
    }
    Some((entry, exit))
}

fn needs_depart_turnaround_at(v: &Vehicle, pos: crate::map::TileCoord, path_index: usize) -> bool {
    use crate::vehicle::reverse_direction;
    if v.kind == VehicleKind::Train {
        return false;
    }
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return false;
    };
    let outbound = direction_from_tile_step(pos, next);
    outbound == reverse_direction(v.direction)
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
