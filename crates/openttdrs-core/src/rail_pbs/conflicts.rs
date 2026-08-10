//! Detección de conflictos de reserva y ocupación.

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind, rail_traversal_bits};
use crate::vehicle::{Vehicle, VehicleKind};

use super::model::{
    MAX_TRAIN_RESERVATION_LEN, ReservedRailStep, decode_rail_reservation_m2_hi,
    track_on_departure_tile,
};

pub(super) fn tracks_overlap(a: u8, b: u8) -> bool {
    a & b != 0
}

/// Ocupación física de una tesela por la cabeza o la cola de un consist.
#[derive(Debug, Clone, Copy)]
struct OccupiedTrainTile {
    vehicle_id: u32,
    /// `None` equivale a una cola: bloquea toda la tesela. La cabeza conserva
    /// su `TrackBit` para permitir el cruce de dos pistas independientes.
    track: Option<u8>,
}

/// Índice efímero de ocupación de trenes para una pasada PBS.
///
/// `tile_occupied_by_other_train` es correcto para consultas aisladas, pero
/// recomponer la topología del consist por cada arista explorada por PBS hace
/// que un save grande escale cuadráticamente. El índice conserva la distinción
/// cabeza/pista frente a cola/tesela completa y se construye una vez por pase.
#[derive(Debug, Default)]
pub(super) struct TrainOccupancyIndex {
    by_tile: HashMap<TileCoord, Vec<OccupiedTrainTile>>,
}

impl TrainOccupancyIndex {
    pub(super) fn from_vehicles(
        map: &Map,
        vehicles: &[Vehicle],
        fleet: &crate::fleet_index::FleetIndex,
    ) -> Self {
        let mut index = Self::default();
        for vehicle in vehicles
            .iter()
            .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
        {
            let head_track = track_on_departure_tile(
                map,
                vehicle.pos,
                vehicle.movement_target().unwrap_or(vehicle.pos),
            )
            .or_else(|| {
                vehicle
                    .path
                    .front()
                    .and_then(|&next| track_on_departure_tile(map, vehicle.pos, next))
            });
            for tile in
                crate::train_consist::consist_occupied_tiles_indexed(vehicles, fleet, vehicle.id)
            {
                index.insert(
                    tile,
                    OccupiedTrainTile {
                        vehicle_id: vehicle.id,
                        track: (tile == vehicle.pos).then_some(head_track).flatten(),
                    },
                );
            }
        }
        index
    }

    fn insert(&mut self, tile: TileCoord, entry: OccupiedTrainTile) {
        let entries = self.by_tile.entry(tile).or_default();
        if !entries
            .iter()
            .any(|existing| existing.vehicle_id == entry.vehicle_id)
        {
            entries.push(entry);
        }
    }

    pub(super) fn tile_occupied_by_other_train(
        &self,
        map: &Map,
        self_id: u32,
        tile: TileCoord,
        track: u8,
    ) -> bool {
        if map.get_kind(tile) == Some(TileKind::RailDepot) {
            return false;
        }
        self.by_tile.get(&tile).is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry.vehicle_id != self_id
                    && entry
                        .track
                        .is_none_or(|other_track| tracks_overlap(other_track, track))
            })
        })
    }

    pub(super) fn platform_occupied_by_other(&self, platform: &[TileCoord], self_id: u32) -> bool {
        platform.iter().any(|tile| {
            self.by_tile
                .get(tile)
                .is_some_and(|entries| entries.iter().any(|entry| entry.vehicle_id != self_id))
        })
    }
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
    if !matches!(t.kind, TileKind::Rail | TileKind::RailBridge) {
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

/// ¿Algún tren ajeno tiene reserva o cola sobre el andén concreto que contiene
/// `stop_tile`?
///
/// A diferencia de [`platform_reserved_or_occupied`], no mezcla vías paralelas
/// de la misma estación.
#[must_use]
pub fn platform_track_reserved_or_occupied(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    station_anchor: TileCoord,
    stop_tile: TileCoord,
    already_reserved: &HashSet<ReservedRailStep>,
) -> bool {
    let platform =
        crate::station::rail_station_platform_track_tiles(map, station_anchor, stop_tile);
    platform.into_iter().any(|tile| {
        already_reserved.iter().any(|s| s.tile == tile)
            || vehicles.iter().any(|v| {
                v.id != self_id
                    && v.kind == VehicleKind::Train
                    && v.is_consist_head()
                    && (v.reserved_steps.iter().any(|s| s.tile == tile)
                        || crate::train_consist::consist_occupied_tiles(vehicles, v.id)
                            .contains(&tile))
            })
    })
}

/// Variante de [`append_platform_reservation`] para la pasada PBS indexada.
///
/// `already_reserved` contiene todas las reservas ajenas activas durante la
/// pasada; el índice aporta únicamente la ocupación física de los consists.
pub(super) fn append_platform_reservation_indexed(
    map: &Map,
    occupancy: &TrainOccupancyIndex,
    vehicle: &Vehicle,
    already_reserved: &HashSet<ReservedRailStep>,
    out: &mut Vec<ReservedRailStep>,
) {
    let Some(crate::vehicle::VehicleOrder::Station { station, .. }) =
        vehicle.orders.get(vehicle.current_order)
    else {
        return;
    };
    let platform = crate::station::rail_station_platform_track_tiles(map, *station, vehicle.dest);
    if platform.is_empty()
        || platform
            .iter()
            .any(|tile| already_reserved.iter().any(|step| step.tile == *tile))
        || occupancy.platform_occupied_by_other(&platform, vehicle.id)
    {
        return;
    }
    append_platform_steps(map, &platform, already_reserved, out);
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
    let platform = crate::station::rail_station_platform_track_tiles(map, *station, vehicle.dest);
    if platform.is_empty()
        || platform_track_reserved_or_occupied(
            map,
            vehicles,
            vehicle.id,
            *station,
            vehicle.dest,
            already_reserved,
        )
    {
        return;
    }
    append_platform_steps(map, &platform, already_reserved, out);
}

fn append_platform_steps(
    map: &Map,
    platform: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
    out: &mut Vec<ReservedRailStep>,
) {
    for &tile in platform {
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
