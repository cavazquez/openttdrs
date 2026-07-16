//! Detección de conflictos de reserva y ocupación.

use std::collections::HashSet;

use crate::map::{Map, TileCoord, TileKind, rail_traversal_bits};
use crate::vehicle::{Vehicle, VehicleKind};

use super::model::{track_on_departure_tile, ReservedRailStep, decode_rail_reservation_m2_hi, MAX_TRAIN_RESERVATION_LEN};

pub(super) fn tracks_overlap(a: u8, b: u8) -> bool {
    a & b != 0
}

/// `true` si otro tren (huella completa del consist) ocupa `tile` solapando `track`.
#[must_use]
pub(super) fn tile_occupied_by_other_train(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    tile: TileCoord,
    track: u8,
) -> bool {
    if map.get_kind(tile) == Some(TileKind::RailDepot) {
        return false;
    }
    vehicles.iter().any(|v| {
        if v.id == self_id || v.kind != VehicleKind::Train || !v.is_consist_head() {
            return false;
        }
        let occupied = crate::train_consist::consist_occupied_tiles(vehicles, v.id);
        if !occupied.contains(&tile) {
            return false;
        }
        // Cabeza en esta tesela: solape por pista; cola: ocupa toda la tesela.
        if v.pos != tile {
            return true;
        }
        let Some(other) = track_on_departure_tile(map, tile, v.movement_target().unwrap_or(tile))
            .or_else(|| {
                v.path
                    .front()
                    .and_then(|&next| track_on_departure_tile(map, tile, next))
            })
        else {
            return true;
        };
        tracks_overlap(other, track)
    })
}

/// `true` si `track` choca con la reserva ya escrita en el mapa.
#[must_use]
pub fn tile_track_reserved_by_map(map: &Map, tile: TileCoord, track: u8) -> bool {
    let Some(t) = map.get(tile) else {
        return false;
    };
    if t.kind != TileKind::Rail {
        return false;
    }
    let reserved = decode_rail_reservation_m2_hi(t.m2_hi);
    reserved != 0 && reserved & track != 0
}

/// ¿Algún tren ajeno tiene reserva o cola sobre la plataforma de `station_anchor`?
#[must_use]
pub fn platform_reserved_or_occupied(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    station_anchor: TileCoord,
    already_reserved: &HashSet<ReservedRailStep>,
) -> bool {
    let platforms = crate::station::rail_station_platform_tiles(map, station_anchor);
    if platforms.is_empty() {
        return false;
    }
    for &tile in &platforms {
        if already_reserved.iter().any(|s| s.tile == tile) {
            return true;
        }
        if vehicles.iter().any(|v| {
            v.id != self_id
                && v.kind == VehicleKind::Train
                && v.is_consist_head()
                && (v.reserved_steps.iter().any(|s| s.tile == tile)
                    || crate::train_consist::consist_occupied_tiles(vehicles, v.id).contains(&tile))
        }) {
            return true;
        }
    }
    false
}

/// Reserva las teselas de plataforma del destino de estación (si la orden actual es Station).
pub(super) fn append_platform_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    already_reserved: &HashSet<ReservedRailStep>,
    out: &mut Vec<ReservedRailStep>,
) {
    let Some(crate::vehicle::VehicleOrder::Station { station, .. }) =
        vehicle.orders.get(vehicle.current_order)
    else {
        return;
    };
    if platform_reserved_or_occupied(map, vehicles, vehicle.id, *station, already_reserved) {
        return;
    }
    for tile in crate::station::rail_station_platform_tiles(map, *station) {
        if out.iter().any(|s| s.tile == tile) {
            continue;
        }
        if out.len() >= MAX_TRAIN_RESERVATION_LEN {
            break;
        }
        let tb = rail_traversal_bits(map, tile);
        let track = (0..6u8)
            .find_map(|i| {
                let bit = 1_u8 << i;
                (tb & bit != 0).then_some(bit)
            })
            .unwrap_or(0x01);
        let step = ReservedRailStep::new(tile, track);
        if already_reserved.contains(&step) {
            continue;
        }
        out.push(step);
    }
}

/// `true` si algún tren tiene reserva que sale de `signal_tile` por `exit_dir` y
/// termina en posición segura (`TryReservePath` exitoso).
#[must_use]
pub fn pbs_exit_has_complete_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    signal_tile: TileCoord,
    exit_dir: u8,
    block: &[TileCoord],
) -> bool {
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    let (dx, dy) = crate::map::diag_dir_offset(exit_dir);
    let first_beyond = TileCoord::new(signal_tile.x + dx, signal_tile.y + dy);
    vehicles.iter().any(|v| {
        if v.kind != VehicleKind::Train || !v.running {
            return false;
        }
        if !v.reserved_steps.iter().any(|s| s.tile == first_beyond) {
            return false;
        }
        if !v
            .reserved_steps
            .iter()
            .any(|s| block_set.contains(&s.tile) || s.tile == first_beyond)
        {
            return false;
        }
        super::search::reservation_ends_at_safe_wait(map, v)
    })
}
