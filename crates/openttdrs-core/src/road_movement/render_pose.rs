//! Funciones de rendering: dirección y posición sub-tesela para sprites.

use super::bay::{bay_render_direction, bay_subtile, parked_inside_bay};
use super::curves::{
    depart_u_turn_curve, sample_curve, straight_subtile, train_straight_subtile, turn_curve,
};
use super::pose::{VehiclePose, movement_target_at};
use crate::depot::rail_depot_mouth_dir;
use crate::map::{Map, TileKind};
use crate::refit::vehicle_in_depot;
use crate::train_movement::{
    train_depot_facing, train_depot_subtile, train_render_dir_on_rail, train_subtile_on_rail,
};
use crate::vehicle::{Vehicle, VehicleDirection, VehicleKind, direction_from_tile_step};

/// Sub-tesela `OpenTTD` para dibujo (recto, curva de giro o media vuelta en parada).
#[must_use]
pub fn vehicle_subtile(v: &Vehicle) -> (f32, f32) {
    vehicle_subtile_with_progress(v, v.progress)
}

/// Como [`vehicle_subtile`] con progreso explícito (p. ej. interpolación de render).
#[must_use]
pub fn vehicle_subtile_with_progress(v: &Vehicle, progress: u8) -> (f32, f32) {
    vehicle_subtile_at(
        v,
        VehiclePose {
            pos: v.pos,
            progress,
            progress_f: f32::from(progress),
            depart_turn: v.depart_turn,
            depart_turn_f: f32::from(v.depart_turn),
            path_index: 0,
        },
    )
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
    if matches!(v.kind, VehicleKind::Train) {
        return train_subtile_with_map(v, pose, map);
    }
    if parked_inside_bay(v, pose.pos)
        && let Some(subtile) = bay_subtile(v, pose)
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
    if v.movement_target().is_some() && (v.progress < 255 || v.cur_speed > 0) {
        return v.movement_direction();
    }
    v.direction
}

fn train_rail_subtile(map: &Map, v: &Vehicle, pose: VehiclePose) -> (f32, f32) {
    let enter = train_subtile_direction(v);
    let progress = pose.movement_progress_f();
    if let Some(tile) = map.get(pose.pos)
        && tile.kind == TileKind::Rail
        && let Some(sub) = train_subtile_on_rail(enter, tile.m5, progress)
    {
        return sub;
    }
    train_straight_subtile(enter, progress)
}

fn train_render_direction_with_map(map: &Map, v: &Vehicle, pose: VehiclePose) -> VehicleDirection {
    let enter = train_subtile_direction(v);
    let progress = pose.movement_progress_f();
    if let Some(tile) = map.get(pose.pos)
        && tile.kind == TileKind::Rail
        && let Some(dir) = train_render_dir_on_rail(enter, tile.m5, progress)
    {
        return dir;
    }
    enter
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
pub fn vehicle_render_direction(v: &Vehicle, progress: u8) -> VehicleDirection {
    vehicle_render_direction_at(
        v,
        VehiclePose {
            pos: v.pos,
            progress,
            progress_f: f32::from(progress),
            depart_turn: v.depart_turn,
            depart_turn_f: f32::from(v.depart_turn),
            path_index: 0,
        },
    )
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
    if matches!(v.kind, VehicleKind::Train)
        && let Some(map) = map
        && vehicle_in_depot(map, pose.pos)
        && let Some(mouth) = rail_depot_mouth_dir(map, pose.pos)
    {
        return train_depot_facing(mouth);
    }
    if matches!(v.kind, VehicleKind::Train) {
        if let Some(map) = map {
            return train_render_direction_with_map(map, v, pose);
        }
        return train_subtile_direction(v);
    }
    if parked_inside_bay(v, pose.pos)
        && pose.depart_turn_f <= 0.0
        && pose.progress_f < 255.0
        && let Some(dir) = bay_render_direction(v, pose)
    {
        return dir;
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
