//! Tipos de datos y helpers de codificación PBS.

use crate::map::{Map, RAIL_TB_X, RAIL_TB_Y, TileCoord};
use crate::map::{opposite_diag_dir as opposite_dir, rail_traversal_bits};
use crate::rail_signals::dir_from_to;
use crate::train_movement::track_bit_for_movement;

/// Máscara de reserva PBS en el byte alto de `m2` (`m2_hi`: bits 8–11 del `m2()` de 16 bits).
pub const RAIL_RESERVATION_M2_HI_MASK: u8 = 0x0F;

/// Vía doble horizontal / vertical.
pub(super) const RAIL_TB_HORZ: u8 = 0x0C;
pub(super) const RAIL_TB_VERT: u8 = 0x30;

/// Tope de pasos reservados por tren (paridad con límites PBS del original).
pub const MAX_TRAIN_RESERVATION_LEN: usize = 64;

/// Coste base por tesela diagonal YAPF (`pathfinder_type.h` `YAPF_TILE_LENGTH`).
pub const YAPF_TILE_LENGTH: u32 = 100;
/// Coste de tesela en esquina / vía no diagonal (`YAPF_TILE_CORNER_LENGTH`).
pub const YAPF_TILE_CORNER_LENGTH: u32 = 71;
/// Penalización YAPF por cruzar una pista ya reservada (`rail_pbs_cross_penalty` = 3×tesela).
pub const YAPF_RESERVATION_CROSS_PENALTY: u32 = 300;

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

/// Pista usada en `tile` al avanzar `from` → `to`.
#[must_use]
pub fn track_for_rail_step(map: &Map, from: TileCoord, to: TileCoord) -> Option<u8> {
    if crate::rail_bridge_other_end(map, from) == Some(to)
        || crate::rail_bridge_other_end(map, to) == Some(from)
    {
        return Some(if from.y == to.y { RAIL_TB_X } else { RAIL_TB_Y });
    }
    let exit_dir = dir_from_to(from, to)?;
    let entry = opposite_dir(exit_dir);
    let tb = rail_traversal_bits(map, to);
    track_bit_for_movement(entry, tb)
}

/// Pista usada en `from` al salir hacia `to`.
#[must_use]
pub fn track_on_departure_tile(map: &Map, from: TileCoord, to: TileCoord) -> Option<u8> {
    if crate::rail_bridge_other_end(map, from) == Some(to) {
        return Some(if from.y == to.y { RAIL_TB_X } else { RAIL_TB_Y });
    }
    let exit_dir = dir_from_to(from, to)?;
    let entry = opposite_dir(exit_dir);
    let tb = rail_traversal_bits(map, from);
    track_bit_for_movement(entry, tb)
}
