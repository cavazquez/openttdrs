//! Sincronización de reservas PBS al mapa (`m2_hi`, `m5`) y liberación walk.

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::{
    is_pbs_signal_type, rail_signal_present_mask, rail_signal_state_mask, rail_tile_is_signals,
    signal_exit_dir, signal_track_for_bit, signal_type_for_track,
};
use crate::station::{
    STATION_TILE_RESERVATION, is_rail_station_type, station_tile_has_reservation,
    station_type_from_m6,
};
use crate::vehicle::{Vehicle, VehicleKind};

use super::model::{
    RAIL_RESERVATION_M2_HI_MASK, ReservedRailStep, decode_rail_reservation_m2_hi,
    encode_rail_reservation_to_m2_hi,
};

fn is_rail_reservation_tile(kind: TileKind) -> bool {
    matches!(kind, TileKind::Rail | TileKind::RailBridge)
}

fn is_rail_station_reservation_tile(tile: &crate::map::Tile) -> bool {
    tile.kind == TileKind::Station && is_rail_station_type(station_type_from_m6(tile.m6))
}

/// Bit de reserva PBS en cruces a nivel (`HasCrossingReservation` / `m5` bit 4).
pub const CROSSING_RESERVATION_M5_BIT: u8 = 1 << 4;

/// Escribe reservas PBS en `m2_hi` (vía plana) y `m5` bit 4 (cruces); marca `dirty`.
///
/// Al liberar teselas que dejan de estar reservadas, pone en rojo las señales PBS
/// de esa tesela (paridad de `FreeTrainTrackReservation` / `ClearPathReservation`).
pub fn sync_reservations_to_map(
    map: &mut Map,
    vehicles: &[Vehicle],
    prev_active: &mut HashSet<TileCoord>,
    dirty: &mut Vec<TileCoord>,
) {
    // Primera sync tras importar un `.sav`: las reservas `m2_hi` del save deben
    // entrar en `prev_active` para poder liberarse cuando el consist ya no las usa.
    if prev_active.is_empty() {
        let (w, h) = map.dimensions();
        for y in 0..h.cast_signed() {
            for x in 0..w.cast_signed() {
                let c = TileCoord::new(x, y);
                let Some(tile) = map.get(c) else {
                    continue;
                };
                if (is_rail_reservation_tile(tile.kind)
                    && decode_rail_reservation_m2_hi(tile.m2_hi) != 0)
                    || (is_rail_station_reservation_tile(&tile)
                        && station_tile_has_reservation(tile.m6))
                {
                    prev_active.insert(c);
                }
            }
        }
    }

    let mut next_tracks: HashMap<TileCoord, u8> = HashMap::new();
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        for step in &v.reserved_steps {
            match map.get_kind(step.tile) {
                Some(TileKind::Rail | TileKind::RailBridge) => {
                    next_tracks
                        .entry(step.tile)
                        .and_modify(|bits| *bits |= step.track)
                        .or_insert(step.track);
                }
                Some(TileKind::Station)
                    if {
                        map.get(step.tile)
                            .is_some_and(|tile| is_rail_station_reservation_tile(&tile))
                    } =>
                {
                    next_tracks
                        .entry(step.tile)
                        .and_modify(|bits| *bits |= step.track)
                        .or_insert(step.track);
                }
                Some(TileKind::Road) => {
                    let Some(tile) = map.get(step.tile) else {
                        continue;
                    };
                    if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
                        next_tracks.entry(step.tile).or_insert(step.track);
                    }
                }
                _ => {}
            }
        }
    }

    let mut touch = HashSet::new();
    for c in prev_active.iter().chain(next_tracks.keys()) {
        touch.insert(*c);
    }

    for c in touch {
        let Some(mut tile) = map.get(c) else {
            continue;
        };
        let want = next_tracks.get(&c).copied().unwrap_or(0);
        let changed = if is_rail_reservation_tile(tile.kind) {
            let had = decode_rail_reservation_m2_hi(tile.m2_hi);
            // Liberación: PBS a rojo al quitar reserva (FreeTrainTrackReservation).
            if had != 0 && want == 0 {
                set_pbs_signals_red_on_tile(&mut tile);
            }
            tile.m2_hi = (tile.m2_hi & !RAIL_RESERVATION_M2_HI_MASK)
                | encode_rail_reservation_to_m2_hi(want);
            had != want || (had != 0 && want == 0)
        } else if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
            let had = tile.m5 & CROSSING_RESERVATION_M5_BIT != 0;
            let want_flag = want != 0;
            if want_flag {
                tile.m5 |= CROSSING_RESERVATION_M5_BIT;
            } else {
                tile.m5 &= !CROSSING_RESERVATION_M5_BIT;
            }
            had != want_flag
        } else if is_rail_station_reservation_tile(&tile) {
            let had = station_tile_has_reservation(tile.m6);
            let want_flag = want != 0;
            if want_flag {
                tile.m6 |= STATION_TILE_RESERVATION;
            } else {
                tile.m6 &= !STATION_TILE_RESERVATION;
            }
            had != want_flag
        } else {
            false
        };
        if changed {
            let _ = map.set_tile(c, tile);
            dirty.push(c);
        }
    }

    *prev_active = next_tracks.keys().copied().collect();
}

/// `FreeTrainTrackReservation`: recorre la reserva tesela a tesela, pone PBS a rojo
/// y limpia `reserved_steps` del tren (`train_cmd.cpp:2476-2536`).
pub fn free_train_track_reservation(
    map: &mut Map,
    vehicle: &mut Vehicle,
    dirty: &mut Vec<TileCoord>,
) {
    if vehicle.kind != VehicleKind::Train || !vehicle.is_consist_head() {
        return;
    }
    let steps: Vec<ReservedRailStep> = std::mem::take(&mut vehicle.reserved_steps);
    let mut prev: Option<TileCoord> = Some(vehicle.pos);
    for step in steps {
        let Some(mut tile) = map.get(step.tile) else {
            prev = Some(step.tile);
            continue;
        };
        let mut changed = false;
        if is_rail_reservation_tile(tile.kind) {
            let before_m3hi = tile.m3hi;
            if rail_tile_is_signals(tile.m5) {
                let exit_dir = prev.and_then(|p| crate::rail_signals::dir_from_to(p, step.tile));
                set_pbs_signals_red_along(&mut tile, exit_dir);
            }
            changed |= tile.m3hi != before_m3hi;
            let had = decode_rail_reservation_m2_hi(tile.m2_hi);
            let next_bits = if had & step.track != 0 {
                had & !step.track
            } else {
                had
            };
            if next_bits != had {
                tile.m2_hi = (tile.m2_hi & !RAIL_RESERVATION_M2_HI_MASK)
                    | encode_rail_reservation_to_m2_hi(next_bits);
                changed = true;
            }
        } else if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind)
            && tile.m5 & CROSSING_RESERVATION_M5_BIT != 0
        {
            tile.m5 &= !CROSSING_RESERVATION_M5_BIT;
            changed = true;
        }
        if changed {
            let _ = map.set_tile(step.tile, tile);
            dirty.push(step.tile);
        }
        prev = Some(step.tile);
    }
}

fn set_pbs_signals_red_on_tile(tile: &mut crate::map::Tile) {
    set_pbs_signals_red_along(tile, None);
}

fn set_pbs_signals_red_along(tile: &mut crate::map::Tile, along_exit: Option<u8>) {
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return;
    }
    let present = rail_signal_present_mask(tile.m3);
    if present == 0 {
        return;
    }
    let rails = tile.m5 & 0x3F;
    let mut states = rail_signal_state_mask(tile.m3hi);
    let mut changed = false;
    for bit in 0..4u8 {
        if present & (1 << bit) == 0 {
            continue;
        }
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(0, |track| signal_type_for_track(tile.m2, track));
        if !is_pbs_signal_type(sig_type) {
            continue;
        }
        if let Some(dir) = along_exit {
            let exit = signal_exit_dir(rails, bit);
            // PBS a favor o en contra (OpenTTD también marca la opuesta para update).
            if exit != dir && exit != crate::map::opposite_diag_dir(dir) {
                continue;
            }
        }
        if states & (1 << bit) != 0 {
            states &= !(1 << bit);
            changed = true;
        }
    }
    if changed {
        tile.m3hi = (tile.m3hi & 0x0F) | (states << 4);
    }
}
