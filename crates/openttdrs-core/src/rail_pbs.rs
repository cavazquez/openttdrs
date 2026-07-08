//! Reserva de ruta ferroviaria (`PBS` fase 2).
//!
//! Cada tren reserva **pistas** (`TrackBits`) a lo largo de su `path` hasta el primer
//! conflicto (otra reserva en la misma pista, ocupación o señal cerrada). Las vías
//! paralelas en la misma tesela (`Horz`/`Vert`) pueden reservarse de forma independiente.

#![allow(clippy::implicit_hasher)]

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::rail_signals::{dir_from_to, rail_traversal_bits};
use crate::train_movement::track_bit_for_movement;
use crate::vehicle::{Vehicle, VehicleKind};

/// Máscara de reserva PBS en el byte alto de `m2` (`m2_hi`: bits 8–11 del `m2()` de 16 bits).
pub const RAIL_RESERVATION_M2_HI_MASK: u8 = 0x0F;

/// Vía doble horizontal / vertical.
const RAIL_TB_HORZ: u8 = 0x0C;
const RAIL_TB_VERT: u8 = 0x30;

/// Tope de pasos reservados por tren (paridad con límites PBS del original).
pub const MAX_TRAIN_RESERVATION_LEN: usize = 64;

/// Penalización YAPF por cruzar una pista ya reservada por otro tren.
pub const YAPF_RESERVATION_CROSS_PENALTY: u32 = 80;

/// Un paso de reserva: tesela + un único `TrackBit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ReservedRailStep {
    pub tile: TileCoord,
    pub track: u8,
}

impl ReservedRailStep {
    #[must_use]
    pub const fn new(tile: TileCoord, track: u8) -> Self {
        Self { tile, track }
    }
}

#[must_use]
const fn opposite_dir(d: u8) -> u8 {
    (d + 2) & 3
}

/// Decodifica `m2_hi` → `TrackBits` reservados (`GetRailReservationTrackBits`).
#[must_use]
pub fn decode_rail_reservation_m2_hi(m2_hi: u8) -> u8 {
    let encoded = m2_hi & RAIL_RESERVATION_M2_HI_MASK;
    let track_idx = (encoded & 0x07).wrapping_sub(1);
    if track_idx > 5 {
        return 0;
    }
    let primary = 1_u8 << track_idx;
    if encoded & (1 << 3) != 0 {
        return primary | opposite_parallel_track(primary);
    }
    primary
}

#[must_use]
const fn opposite_parallel_track(track: u8) -> u8 {
    match track {
        0x04 => 0x08,
        0x08 => 0x04,
        0x10 => 0x20,
        0x20 => 0x10,
        _ => 0,
    }
}

/// Codifica `TrackBits` reservados en `m2_hi` (sin tocar el byte bajo de `m2`).
#[must_use]
pub fn encode_rail_reservation_to_m2_hi(track_bits: u8) -> u8 {
    if track_bits == 0 {
        return 0;
    }
    let Some(first_track) = (0..6u8).find(|i| track_bits & (1 << i) != 0) else {
        return 0;
    };
    let mut out = first_track + 1;
    if track_bits == RAIL_TB_HORZ || track_bits == RAIL_TB_VERT {
        out |= 1 << 3;
    }
    out
}

/// `true` si la tesela tiene alguna pista reservada en `m2_hi`.
#[must_use]
pub fn rail_tile_has_pbs_reservation(m2_hi: u8) -> bool {
    decode_rail_reservation_m2_hi(m2_hi) != 0
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

/// Pista usada en `tile` al avanzar `from` → `to`.
#[must_use]
pub fn track_for_rail_step(map: &Map, from: TileCoord, to: TileCoord) -> Option<u8> {
    let exit_dir = dir_from_to(from, to)?;
    let entry = opposite_dir(exit_dir);
    let tb = rail_traversal_bits(map, to);
    track_bit_for_movement(entry, tb)
}

/// Pista usada en `from` al salir hacia `to`.
#[must_use]
pub fn track_on_departure_tile(map: &Map, from: TileCoord, to: TileCoord) -> Option<u8> {
    let exit_dir = dir_from_to(from, to)?;
    let entry = opposite_dir(exit_dir);
    let tb = rail_traversal_bits(map, from);
    track_bit_for_movement(entry, tb)
}

fn tracks_overlap(a: u8, b: u8) -> bool {
    a & b != 0
}

/// `true` si otro tren ocupa `tile` en una pista que solapa con `track`.
#[must_use]
fn tile_occupied_by_other_train(
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
        if v.id == self_id || v.kind != VehicleKind::Train {
            return false;
        }
        if v.pos != tile {
            return false;
        }
        track_on_departure_tile(map, tile, v.movement_target().unwrap_or(tile))
            .or_else(|| {
                v.path
                    .front()
                    .and_then(|&next| track_on_departure_tile(map, tile, next))
            })
            .is_some_and(|other| tracks_overlap(other, track))
    })
}

/// `true` si algún paso reservado cae en `block`.
#[must_use]
pub fn pbs_block_has_reservation(vehicles: &[Vehicle], block: &[TileCoord]) -> bool {
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    vehicles.iter().any(|v| {
        v.kind == VehicleKind::Train
            && v.running
            && v.reserved_steps.iter().any(|s| block_set.contains(&s.tile))
    })
}

/// Calcula la reserva de un tren sin mutar el mapa global de reservas.
#[must_use]
pub fn compute_train_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    let vehicle = &vehicles[vehicle_idx];
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return Vec::new();
    }

    let path: Vec<TileCoord> = vehicle.path.iter().copied().collect();
    let mut out = Vec::new();
    let mut cur = vehicle.pos;

    let Some(pos_track) = path
        .first()
        .and_then(|&next| track_on_departure_tile(map, cur, next))
    else {
        let tb = rail_traversal_bits(map, cur);
        let track = (0..6u8)
            .find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
            .unwrap_or(tb & 0x3F);
        if track != 0 {
            out.push(ReservedRailStep::new(cur, track));
        }
        return out;
    };
    out.push(ReservedRailStep::new(cur, pos_track));

    for (i, &next) in path.iter().enumerate() {
        if out.len() >= MAX_TRAIN_RESERVATION_LEN {
            break;
        }
        let beyond = path.get(i + 1).copied();
        if !crate::rail_signals::rail_step_signal_allows(map, vehicles, cur, next, beyond) {
            break;
        }
        let Some(track) =
            track_on_departure_tile(map, cur, next).or_else(|| track_for_rail_step(map, cur, next))
        else {
            break;
        };
        let step = ReservedRailStep::new(next, track);
        if already_reserved.contains(&step) {
            break;
        }
        if tile_occupied_by_other_train(map, vehicles, vehicle.id, next, track) {
            break;
        }
        out.push(step);
        cur = next;
    }

    out
}

/// Recalcula `reserved_steps` de todos los trenes (orden por índice = prioridad).
pub fn update_train_reservations(map: &Map, vehicles: &mut [Vehicle]) {
    let mut global = HashSet::new();
    for i in 0..vehicles.len() {
        if vehicles[i].kind != VehicleKind::Train {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        let reserved = compute_train_reservation(map, vehicles, i, &global);
        for step in &reserved {
            global.insert(*step);
        }
        vehicles[i].reserved_steps = reserved;
    }
}

/// `true` si el tren no puede avanzar al `movement_target` (sin reserva en esa pista).
#[must_use]
pub fn train_blocked_by_reservation(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    if map.get_kind(vehicle.pos) == Some(TileKind::RailDepot) {
        return false;
    }
    if vehicle.reserved_steps.is_empty() {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    if vehicle.path.front() != Some(&next) {
        return false;
    }
    let Some(track) = track_on_departure_tile(map, vehicle.pos, next)
        .or_else(|| track_for_rail_step(map, vehicle.pos, next))
    else {
        return false;
    };
    if vehicle
        .reserved_steps
        .iter()
        .any(|s| s.tile == next && (s.track == track || tracks_overlap(s.track, track)))
    {
        return false;
    }
    if vehicle.reserved_steps.iter().any(|s| s.tile == vehicle.pos) {
        return false;
    }
    true
}

/// Escribe reservas PBS en `m2_hi` y marca `dirty` al cambiar.
pub fn sync_reservations_to_map(
    map: &mut Map,
    vehicles: &[Vehicle],
    prev_active: &mut HashSet<TileCoord>,
    dirty: &mut Vec<TileCoord>,
) {
    let mut next_tracks: HashMap<TileCoord, u8> = HashMap::new();
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        for step in &v.reserved_steps {
            if map.get_kind(step.tile) == Some(TileKind::Rail) {
                next_tracks
                    .entry(step.tile)
                    .and_modify(|bits| *bits |= step.track)
                    .or_insert(step.track);
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
        if tile.kind != TileKind::Rail {
            continue;
        }
        let had = decode_rail_reservation_m2_hi(tile.m2_hi);
        let want = next_tracks.get(&c).copied().unwrap_or(0);
        tile.m2_hi =
            (tile.m2_hi & !RAIL_RESERVATION_M2_HI_MASK) | encode_rail_reservation_to_m2_hi(want);
        if had != want {
            let _ = map.set_tile(c, tile);
            dirty.push(c);
        }
    }

    *prev_active = next_tracks.keys().copied().collect();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::parity::{
        TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y, TRAIN_DUAL_VEHICLE_2_ID,
        TRAIN_DUAL_VEHICLE_ID, build_train_supply_dual,
    };

    #[test]
    fn encode_decode_roundtrip_horz_and_single() {
        assert_eq!(decode_rail_reservation_m2_hi(0), 0);
        assert_eq!(
            decode_rail_reservation_m2_hi(encode_rail_reservation_to_m2_hi(0x04)),
            0x04
        );
        assert_eq!(
            decode_rail_reservation_m2_hi(encode_rail_reservation_to_m2_hi(RAIL_TB_HORZ)),
            RAIL_TB_HORZ
        );
    }

    #[test]
    fn parallel_tracks_get_disjoint_reservations() {
        let mut state = build_train_supply_dual();
        {
            let t2 = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            t2.pos = TileCoord::new(7, TRAIN_DUAL_TRACK_RET_Y);
            t2.path = VecDeque::from([
                TileCoord::new(6, TRAIN_DUAL_TRACK_RET_Y),
                TileCoord::new(5, TRAIN_DUAL_TRACK_RET_Y),
            ]);
            t2.running = true;
        }
        {
            let t1 = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            t1.pos = TileCoord::new(5, TRAIN_DUAL_TRACK_OUT_Y);
            t1.path = VecDeque::from([
                TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y),
            ]);
            t1.running = true;
        }

        let mut dirty = Vec::new();
        crate::rail_signals::update_rail_signal_states(
            &mut state.map,
            &state.vehicles,
            &mut dirty,
            true,
        );
        update_train_reservations(&state.map, &mut state.vehicles);
        let t1 = state.vehicles.iter().find(|v| v.id == 1).expect("tren 1");
        let t2 = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            t1.reserved_steps
                .iter()
                .all(|s| s.tile.y == TRAIN_DUAL_TRACK_OUT_Y)
        );
        assert!(
            t2.reserved_steps
                .iter()
                .all(|s| s.tile.y == TRAIN_DUAL_TRACK_RET_Y)
        );
        assert!(
            t1.reserved_steps.len() >= 3,
            "tren 1 reserva ida: {:?}",
            t1.reserved_steps
        );
        assert!(
            t2.reserved_steps.len() >= 3,
            "tren 2 reserva vuelta: {:?}",
            t2.reserved_steps
        );
    }

    #[test]
    fn disjoint_tracks_on_same_tile_do_not_conflict() {
        let tile = TileCoord::new(5, 4);
        let upper = 0x04;
        let lower = 0x08;
        let mut reserved = HashSet::from([ReservedRailStep::new(tile, upper)]);
        let lower_step = ReservedRailStep::new(tile, lower);
        assert!(!reserved.contains(&lower_step));
        reserved.insert(lower_step);
        assert_eq!(reserved.len(), 2);
    }

    #[test]
    fn follower_reservation_stops_before_leader_on_same_track() {
        let mut state = build_train_supply_dual();
        let leader_pos = TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y);
        let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);
        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            leader.pos = leader_pos;
            leader.path.clear();
            leader.running = true;
        }
        let mut follower = crate::vehicle::Vehicle::new(
            2,
            VehicleKind::Train,
            follower_pos,
            TileCoord::new(13, TRAIN_DUAL_TRACK_OUT_Y),
        );
        follower.path = VecDeque::from([
            TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
            leader_pos,
            TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y),
        ]);
        follower.running = true;
        state.vehicles.push(follower);

        update_train_reservations(&state.map, &mut state.vehicles);
        let follower = state.vehicles.iter().find(|v| v.id == 2).expect("tren 2");
        assert!(
            follower
                .reserved_steps
                .iter()
                .all(|s| s.tile.x <= follower_pos.x),
            "no debe reservar más allá del líder: {:?}",
            follower.reserved_steps
        );
        assert!(
            !follower
                .reserved_steps
                .iter()
                .any(|s| s.tile.x > follower_pos.x),
            "reserva cortada antes del líder: {:?}",
            follower.reserved_steps
        );
    }

    #[test]
    fn connector_tile_stays_reserved_while_train_turns() {
        let mut state = build_train_supply_dual();
        let connector = TileCoord::new(10, 5);
        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            leader.pos = connector;
            leader.path = VecDeque::from([TileCoord::new(10, TRAIN_DUAL_TRACK_RET_Y)]);
            leader.running = true;
        }

        update_train_reservations(&state.map, &mut state.vehicles);
        let leader = state.vehicles.iter().find(|v| v.id == 1).expect("tren 1");
        assert!(
            leader.reserved_steps.iter().any(|s| s.tile == connector),
            "conector ocupado: {:?}",
            leader.reserved_steps
        );
    }

    #[test]
    fn sync_sets_m2_reservation_bits_on_rail() {
        let mut state = build_train_supply_dual();
        let tile = TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y);
        let rails_before = state.map.get(tile).expect("vía").m5 & 0x3F;
        let track =
            track_on_departure_tile(&state.map, tile, TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y))
                .expect("pista");
        state.vehicles[0].reserved_steps = vec![ReservedRailStep::new(tile, track)];
        let mut prev = HashSet::new();
        let mut dirty = Vec::new();
        sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
        let t = state.map.get(tile).expect("vía");
        assert_eq!(
            t.m5 & 0x3F,
            rails_before,
            "reserva no debe alterar TrackBits"
        );
        assert_ne!(decode_rail_reservation_m2_hi(t.m2_hi), 0);
        assert!(!dirty.is_empty());

        state.vehicles[0].reserved_steps.clear();
        sync_reservations_to_map(&mut state.map, &state.vehicles, &mut prev, &mut dirty);
        let t = state.map.get(tile).expect("vía");
        assert_eq!(decode_rail_reservation_m2_hi(t.m2_hi), 0);
    }
}
