//! Sincronización de reservas PBS al mapa (`m2_hi`, `m5`).

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::{Vehicle, VehicleKind};

use super::model::{
    RAIL_RESERVATION_M2_HI_MASK, decode_rail_reservation_m2_hi, encode_rail_reservation_to_m2_hi,
};

/// Bit de reserva PBS en cruces a nivel (`HasCrossingReservation` / `m5` bit 4).
pub const CROSSING_RESERVATION_M5_BIT: u8 = 1 << 4;

/// Escribe reservas PBS en `m2_hi` (vía plana) y `m5` bit 4 (cruces); marca `dirty`.
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
                if tile.kind == TileKind::Rail && decode_rail_reservation_m2_hi(tile.m2_hi) != 0 {
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
                Some(TileKind::Rail) => {
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
        let changed = if tile.kind == TileKind::Rail {
            let had = decode_rail_reservation_m2_hi(tile.m2_hi);
            tile.m2_hi = (tile.m2_hi & !RAIL_RESERVATION_M2_HI_MASK)
                | encode_rail_reservation_to_m2_hi(want);
            had != want
        } else if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
            let had = tile.m5 & CROSSING_RESERVATION_M5_BIT != 0;
            let want_flag = want != 0;
            if want_flag {
                tile.m5 |= CROSSING_RESERVATION_M5_BIT;
            } else {
                tile.m5 &= !CROSSING_RESERVATION_M5_BIT;
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
