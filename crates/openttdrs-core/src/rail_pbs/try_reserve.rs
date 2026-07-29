//! `TryPathReserve` con semántica de depósito (`train_cmd.cpp`).

use std::collections::HashSet;

use crate::depot::{has_depot_reservation, rail_depot_entrance_tile, set_depot_reservation};
use crate::map::{Map, TileKind};
use crate::pathfinding_settings::PathfindingSettings;
use crate::vehicle::{Vehicle, VehicleKind};

use super::train_reservation::{
    compute_train_reservation_with_settings, follow_train_reservation,
    reservation_ends_at_safe_wait_steps, vehicle_segment_requires_path_reserve,
};

/// Intenta reservar camino PBS; con cabeza en depósito aplica bit tentativo + rollback.
///
/// Paridad de `TryPathReserve` cuando `moving_front->track == Track::Depot`
/// (`!depot_leave_cleared` es el proxy de `Track::Depot`).
pub fn try_path_reserve(
    map: &mut Map,
    vehicles: &mut [Vehicle],
    vehicle_idx: usize,
    mark_as_stuck: bool,
    settings: PathfindingSettings,
) -> bool {
    let Some(vehicle) = vehicles.get(vehicle_idx) else {
        return false;
    };
    if vehicle.kind != VehicleKind::Train || !vehicle.is_consist_head() {
        return false;
    }

    let depot_pos = vehicle.pos;
    let in_depot_track =
        map.get_kind(depot_pos) == Some(TileKind::RailDepot) && !vehicle.depot_leave_cleared;

    if in_depot_track {
        if has_depot_reservation(map, depot_pos) {
            if mark_as_stuck {
                vehicles[vehicle_idx].pbs_stuck = true;
            }
            return false;
        }
        if let Some(entrance) = rail_depot_entrance_tile(map, depot_pos)
            && entrance_reserved_by_other(vehicles, vehicle.id, entrance)
        {
            if mark_as_stuck {
                vehicles[vehicle_idx].pbs_stuck = true;
            }
            return false;
        }
    }

    let previous = vehicles[vehicle_idx].reserved_steps.clone();
    let path_hint: Vec<_> = vehicles[vehicle_idx].path.iter().copied().collect();
    if reservation_ends_at_safe_wait_steps(map, depot_pos, &path_hint, &previous)
        && previous
            .iter()
            .any(|s| s.tile == depot_pos || path_hint.contains(&s.tile))
    {
        vehicles[vehicle_idx].pbs_stuck = false;
        return true;
    }

    if in_depot_track {
        let _ = set_depot_reservation(map, depot_pos, true);
    }

    let mut global = HashSet::new();
    for (i, v) in vehicles.iter().enumerate() {
        if i == vehicle_idx || v.kind != VehicleKind::Train || !v.is_consist_head() {
            continue;
        }
        for step in &v.reserved_steps {
            global.insert(*step);
        }
    }

    let reserved =
        compute_train_reservation_with_settings(map, vehicles, vehicle_idx, &global, settings);
    let reserved = follow_train_reservation(&previous, reserved, &vehicles[vehicle_idx]);

    // Vanilla (`pf.reserve_paths=false`) sin path signal delante: no hay reserva PBS
    // que crear. Bloquear aquí dejaba trenes eternos en depósito (#200 + d2a0fdf).
    if reserved.is_empty()
        && !settings.reserve_paths
        && !vehicle_segment_requires_path_reserve(map, &vehicles[vehicle_idx])
    {
        vehicles[vehicle_idx].pbs_stuck = false;
        return true;
    }

    let reached_beyond = reserved.iter().any(|s| s.tile != depot_pos);
    let ends_safe = reservation_ends_at_safe_wait_steps(map, depot_pos, &path_hint, &reserved);
    let ok = !reserved.is_empty() && (reached_beyond || ends_safe);

    if !ok {
        if in_depot_track {
            let _ = set_depot_reservation(map, depot_pos, false);
        }
        if mark_as_stuck {
            vehicles[vehicle_idx].pbs_stuck = true;
        }
        return false;
    }

    vehicles[vehicle_idx].reserved_steps = reserved;
    vehicles[vehicle_idx].pbs_stuck = false;
    true
}

fn entrance_reserved_by_other(
    vehicles: &[Vehicle],
    self_id: u32,
    tile: crate::map::TileCoord,
) -> bool {
    vehicles.iter().any(|v| {
        v.id != self_id
            && v.kind == VehicleKind::Train
            && v.is_consist_head()
            && v.reserved_steps.iter().any(|s| s.tile == tile)
    })
}
