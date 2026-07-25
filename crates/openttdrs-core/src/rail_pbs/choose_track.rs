//! `ChooseTrainTrack`: elección de vía con YAPF y reserva atómica al entrar.

use crate::map::{Map, TileCoord, opposite_diag_dir as opposite_dir, rail_traversal_bits};
use crate::pathfinder::TunnelWormholes;
use crate::pathfinder::yapf::next_rail_trackdir_yapf;
use crate::rail_signals::dir_from_to;
use crate::train_movement::track_bit_for_movement;
use crate::vehicle::{Vehicle, VehicleKind};

use super::model::{ReservedRailStep, track_for_rail_step};

/// Resultado de elegir vía al entrar en una tesela (`ChooseTrainTrack`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChosenTrainTrack {
    pub next: TileCoord,
    pub track: u8,
    pub exit_dir: u8,
}

/// `true` si la tesela de entrada tiene más de una pista alcanzable desde `enter_dir`.
#[must_use]
pub fn tile_is_track_choice(map: &Map, enter_tile: TileCoord, enter_dir: u8) -> bool {
    let tb = rail_traversal_bits(map, enter_tile);
    if tb.count_ones() <= 1 {
        return false;
    }
    let mask = crate::map::rail_bits_touching_side(enter_dir);
    let reachable = tb & mask;
    reachable.count_ones() > 1
}

/// Elige el siguiente paso con YAPF cuando hay cruce; si no, conserva el `path` actual.
///
/// Paridad simplificada de `ChooseTrainTrack` (`train_cmd.cpp`): preferir pista ya
/// reservada; si hay elección, `next_rail_trackdir_yapf` hacia `dest`; reservar el
/// `TrackBit` de la tesela de entrada en `reserved_steps`.
pub fn choose_train_track_on_enter(
    map: &Map,
    vehicle: &mut Vehicle,
    wormholes: Option<&TunnelWormholes>,
) -> Option<ChosenTrainTrack> {
    if vehicle.kind != VehicleKind::Train || !vehicle.is_consist_head() {
        return None;
    }
    let from = vehicle.pos;
    let path_next = vehicle.path.front().copied();
    let enter_tile = path_next?;
    let enter_dir = dir_from_to(from, enter_tile)?;

    // Pista ya reservada en la tesela de entrada (sigue la reserva PBS).
    if let Some(step) = vehicle.reserved_steps.iter().find(|s| s.tile == enter_tile) {
        let track = step.track;
        return Some(ChosenTrainTrack {
            next: enter_tile,
            track,
            exit_dir: enter_dir,
        });
    }

    let choice = tile_is_track_choice(map, enter_tile, enter_dir)
        || rail_exit_choices(map, from, enter_dir).len() > 1;

    let (next, track, exit_dir) = if choice {
        if let Some((yapf_next, yapf_track, yapf_dir)) =
            next_rail_trackdir_yapf(map, from, vehicle.dest, wormholes)
        {
            // Reescribir el frente del path si YAPF elige otro ramal.
            if vehicle.path.front() != Some(&yapf_next) {
                if let Some(pos) = vehicle.path.iter().position(|&t| t == yapf_next) {
                    for _ in 0..pos {
                        vehicle.path.pop_front();
                    }
                } else {
                    vehicle.path.push_front(yapf_next);
                }
            }
            let track = track_for_rail_step(map, from, yapf_next)
                .or(Some(yapf_track))
                .unwrap_or(yapf_track);
            (yapf_next, track, yapf_dir)
        } else {
            let track = track_for_rail_step(map, from, enter_tile).or_else(|| {
                track_bit_for_movement(
                    opposite_dir(enter_dir),
                    rail_traversal_bits(map, enter_tile),
                )
            })?;
            (enter_tile, track, enter_dir)
        }
    } else {
        let track = track_for_rail_step(map, from, enter_tile).or_else(|| {
            track_bit_for_movement(
                opposite_dir(enter_dir),
                rail_traversal_bits(map, enter_tile),
            )
        })?;
        (enter_tile, track, enter_dir)
    };

    // Reserva atómica al entrar: anotar el TrackBit de la tesela destino.
    let enter_track = track_for_rail_step(map, from, next).unwrap_or(track);
    let step = ReservedRailStep::new(next, enter_track);
    if !vehicle
        .reserved_steps
        .iter()
        .any(|s| s.tile == step.tile && s.track == step.track)
    {
        vehicle.reserved_steps.push(step);
    }
    // También la pista de salida de la tesela actual (OpenTTD TryReserveRailTrack en moving_front).
    if let Some(dep) = super::model::track_on_departure_tile(map, from, next) {
        let dep_step = ReservedRailStep::new(from, dep);
        if !vehicle
            .reserved_steps
            .iter()
            .any(|s| s.tile == dep_step.tile && s.track == dep_step.track)
        {
            vehicle.reserved_steps.insert(0, dep_step);
        }
    }

    Some(ChosenTrainTrack {
        next,
        track: enter_track,
        exit_dir,
    })
}

fn rail_exit_choices(map: &Map, from: TileCoord, preferred_dir: u8) -> Vec<TileCoord> {
    let tb = rail_traversal_bits(map, from);
    let mut out = Vec::new();
    for dir in 0..4u8 {
        if dir == opposite_dir(preferred_dir) {
            continue;
        }
        if tb & crate::map::rail_bits_touching_side(dir) == 0 {
            continue;
        }
        let (dx, dy) = crate::map::diag_dir_offset(dir);
        let next = TileCoord::new(from.x + dx, from.y + dy);
        let entry = opposite_dir(dir);
        if rail_traversal_bits(map, next) & crate::map::rail_bits_touching_side(entry) != 0 {
            out.push(next);
        }
    }
    out
}
