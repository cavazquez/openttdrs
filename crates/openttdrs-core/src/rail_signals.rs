//! Señales ferroviarias de bloque (v1): colocación, bloques y simulación simple.

use std::collections::HashSet;

use crate::map::{Map, TileCoord, TileKind};
use crate::station::is_rail_waypoint_tile;

/// Subtipo de tesela ferroviaria en bits 6–7 de `m5` (`RailTileType`).
pub const RAIL_TILE_NORMAL: u8 = 0;
pub const RAIL_TILE_SIGNALS: u8 = 1;

const RAIL_TB_X: u8 = 0x01;
const RAIL_TB_Y: u8 = 0x02;

/// Coste de colocación de una señal de bloque (`Price::BuildSignal` aprox.).
pub const SIGNAL_BUILD_COST: i64 = 40;
/// Reembolso parcial al quitar vía (`Price::ClearRail` aprox.).
pub const RAIL_REMOVE_REFUND: i64 = 10;

#[must_use]
pub fn rail_tile_is_signals(m5: u8) -> bool {
    (m5 >> 6) & 0x3 == RAIL_TILE_SIGNALS
}

#[must_use]
pub fn rail_signal_present_mask(m3: u8) -> u8 {
    (m3 >> 4) & 0x0F
}

#[must_use]
pub fn rail_signal_state_mask(m3hi: u8) -> u8 {
    (m3hi >> 4) & 0x0F
}

#[must_use]
pub fn signal_is_green(m3hi: u8, sig_bit: u8) -> bool {
    (rail_signal_state_mask(m3hi) >> sig_bit) & 1 != 0
}

/// Trackbits transitables (misma lógica que el pathfinder).
#[must_use]
fn rail_traversal_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Rail => {
            let tb = t.m5 & 0x3F;
            if tb == 0 { RAIL_TB_X } else { tb }
        }
        TileKind::RailTunnel | TileKind::RailBridge => RAIL_TB_X | RAIL_TB_Y,
        TileKind::Station if is_rail_station_tile_kind(&t) || is_rail_waypoint_tile(&t) => {
            if t.m5 & 1 != 0 { RAIL_TB_Y } else { RAIL_TB_X }
        }
        _ => 0,
    }
}

#[must_use]
const fn opposite_dir(d: u8) -> u8 {
    (d + 2) & 3
}

#[must_use]
const fn diag_dir_offset(d: u8) -> (i32, i32) {
    match d & 3 {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    }
}

#[must_use]
const fn rail_bits_touching_side(side: u8) -> u8 {
    match side & 3 {
        0 => 0x25,
        1 => 0x2A,
        2 => 0x19,
        _ => 0x16,
    }
}

#[must_use]
fn rail_neighbors(map: &Map, cur: TileCoord, prev: Option<TileCoord>) -> Vec<TileCoord> {
    let tb = rail_traversal_bits(map, cur);
    if tb == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for dir in 0..4u8 {
        let (dx, dy) = diag_dir_offset(dir);
        let next = TileCoord::new(cur.x + dx, cur.y + dy);
        if prev == Some(next) {
            continue;
        }
        if tb & rail_bits_touching_side(dir) == 0 {
            continue;
        }
        let entry = opposite_dir(dir);
        if rail_traversal_bits(map, next) & rail_bits_touching_side(entry) != 0 {
            out.push(next);
        }
    }
    out
}

#[must_use]
fn dir_from_to(from: TileCoord, to: TileCoord) -> Option<u8> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    match (dx, dy) {
        (1, 0) => Some(0),
        (0, 1) => Some(1),
        (-1, 0) => Some(2),
        (0, -1) => Some(3),
        _ => None,
    }
}

/// Teselas del bloque protegido al salir de `signal_tile` hacia `exit_dir`.
#[must_use]
pub fn rail_block_ahead(map: &Map, signal_tile: TileCoord, exit_dir: u8) -> Vec<TileCoord> {
    let (dx, dy) = diag_dir_offset(exit_dir);
    let start = TileCoord::new(signal_tile.x + dx, signal_tile.y + dy);
    if map.get(start).is_none() {
        return Vec::new();
    }
    let mut block = vec![start];
    let mut cur = start;
    let mut prev = signal_tile;
    loop {
        let neighbors: Vec<_> = rail_neighbors(map, cur, Some(prev))
            .into_iter()
            .filter(|n| *n != prev)
            .collect();
        if neighbors.len() != 1 {
            break;
        }
        let next = neighbors[0];
        if map
            .get(next)
            .is_some_and(|t| t.kind == TileKind::Rail && rail_tile_is_signals(t.m5))
        {
            break;
        }
        block.push(next);
        prev = cur;
        cur = next;
    }
    block
}

#[must_use]
fn is_rail_station_tile_kind(tile: &crate::map::Tile) -> bool {
    tile.kind == TileKind::Station && (tile.m6 >> 3).trailing_zeros() >= 4
}

#[must_use]
fn block_is_clear(train_positions: &[TileCoord], block: &[TileCoord]) -> bool {
    let set: HashSet<_> = block.iter().copied().collect();
    !train_positions.iter().any(|p| set.contains(p))
}

/// Bits de señal presentes que controlan la salida `from` → `to`.
#[must_use]
fn signal_bits_for_exit(map: &Map, from: TileCoord, to: TileCoord) -> Vec<u8> {
    let Some(tile) = map.get(from) else {
        return Vec::new();
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return Vec::new();
    }
    let exit_dir = dir_from_to(from, to).unwrap_or(0);
    let rails = tile.m5 & 0x3F;
    let present = rail_signal_present_mask(tile.m3);
    let mut bits = Vec::new();
    let mut push_if = |bit: u8, dirs: &[u8]| {
        if present & (1 << bit) != 0 && dirs.contains(&exit_dir) {
            bits.push(bit);
        }
    };
    if rails & RAIL_TB_Y == 0 {
        if rails & RAIL_TB_X != 0 {
            push_if(2, &[0]);
            push_if(3, &[2]);
        } else {
            push_if(2, &[3]);
            push_if(3, &[1]);
            push_if(0, &[0]);
            push_if(1, &[2]);
        }
    } else {
        push_if(2, &[3]);
        push_if(3, &[1]);
    }
    bits
}

/// `true` si un tren no puede avanzar de `from` a `to` por señal en rojo.
#[must_use]
pub fn train_blocked_by_signal(
    map: &Map,
    train_positions: &[TileCoord],
    from: TileCoord,
    to: TileCoord,
) -> bool {
    let Some(tile) = map.get(from) else {
        return false;
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return false;
    }
    for bit in signal_bits_for_exit(map, from, to) {
        if !signal_is_green(tile.m3hi, bit) {
            return true;
        }
        let exit_dir = dir_from_to(from, to).unwrap_or(0);
        let block = rail_block_ahead(map, from, exit_dir);
        if !block_is_clear(train_positions, &block) {
            return true;
        }
    }
    false
}

fn refresh_signal_tile_states(
    map: &Map,
    train_positions: &[TileCoord],
    c: TileCoord,
    tile: crate::map::Tile,
) -> Option<crate::map::Tile> {
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return None;
    }
    let present = rail_signal_present_mask(tile.m3);
    if present == 0 {
        return None;
    }
    let mut states = 0u8;
    for bit in 0..4u8 {
        if present & (1 << bit) == 0 {
            continue;
        }
        let exit_dir = signal_exit_dir(tile.m5 & 0x3F, bit);
        let block = rail_block_ahead(map, c, exit_dir);
        if block_is_clear(train_positions, &block) {
            states |= 1 << bit;
        }
    }
    let mut out = tile;
    out.m3hi = (out.m3hi & 0x0F) | (states << 4);
    Some(out)
}

/// Recalcula verde/rojo en todas las teselas con señales.
pub fn update_rail_signal_states(map: &mut Map, train_positions: &[TileCoord]) {
    let (w, h) = map.dimensions();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            if let Some(out) = refresh_signal_tile_states(map, train_positions, c, tile) {
                let _ = map.set_tile(c, out);
            }
        }
    }
}

#[must_use]
fn signal_exit_dir(rails: u8, sig_bit: u8) -> u8 {
    if rails & RAIL_TB_Y != 0 {
        return if sig_bit == 2 { 3 } else { 1 };
    }
    if rails & RAIL_TB_X != 0 {
        return if sig_bit == 2 { 0 } else { 2 };
    }
    match sig_bit {
        0 => 0,
        1 => 2,
        2 => 3,
        _ => 1,
    }
}

/// Datos de codificación de una señal de bloque en una tesela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalPlacement {
    pub sig_bit: u8,
    pub m2: u8,
    pub m3: u8,
    pub m3hi: u8,
}

/// Direcciones válidas para colocar señal en vía recta (`DiagDir`).
#[must_use]
pub fn valid_signal_facings(trackbits: u8) -> &'static [u8] {
    match trackbits {
        RAIL_TB_X => &[0, 2], // NE, SW
        RAIL_TB_Y => &[3, 1], // NW, SE
        _ => &[],
    }
}

/// Elige la orientación de colocación más cercana a `orientation` (0..3).
#[must_use]
pub fn signal_facing_for_orientation(trackbits: u8, orientation: u8) -> u8 {
    let facings = valid_signal_facings(trackbits);
    let ori = orientation % 4;
    if let Some(f) = facings.iter().copied().find(|f| *f == ori) {
        return f;
    }
    facings.first().copied().unwrap_or(ori)
}

/// Siguiente orientación al rotar con RMB sobre vía recta.
#[must_use]
pub fn cycle_signal_facing(trackbits: u8, current: u8) -> u8 {
    let facings = valid_signal_facings(trackbits);
    if facings.is_empty() {
        return current % 4;
    }
    let cur = signal_facing_for_orientation(trackbits, current);
    let idx = facings.iter().position(|&f| f == cur).unwrap_or(0);
    facings[(idx + 1) % facings.len()]
}

#[must_use]
pub fn signal_bit_for_facing(trackbits: u8, face: u8) -> Option<u8> {
    match (trackbits, face % 4) {
        (RAIL_TB_X, 0) | (RAIL_TB_Y, 3) => Some(2), // NE / NW
        (RAIL_TB_X, 2) | (RAIL_TB_Y, 1) => Some(3), // SW / SE
        _ => None,
    }
}

const OTTD_TRACK_X: u8 = 0;
const OTTD_TRACK_Y: u8 = 1;
const OTTD_TRACK_LOWER: u8 = 3;
const OTTD_TRACK_RIGHT: u8 = 5;

fn m2_for_signal(sig_type: u8, variant: u8, track: u8) -> u8 {
    let base = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        4
    } else {
        0
    };
    let var_bit = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        7
    } else {
        3
    };
    ((sig_type & 7) << base) | ((variant & 1) << var_bit)
}

/// Codifica una señal de bloque eléctrica unidireccional (`SIGTYPE_BLOCK`, verde).
#[must_use]
pub fn signal_placement_for_facing(trackbits: u8, face: u8) -> Option<SignalPlacement> {
    let sig_bit = signal_bit_for_facing(trackbits, face)?;
    let ottd_track = if trackbits == RAIL_TB_X {
        OTTD_TRACK_X
    } else {
        OTTD_TRACK_Y
    };
    let present = 1 << sig_bit;
    Some(SignalPlacement {
        sig_bit,
        m2: m2_for_signal(0, 0, ottd_track),
        m3: present << 4,
        m3hi: present << 4,
    })
}

/// Compatibilidad con tests: señal por defecto en la primera dirección válida.
#[must_use]
pub fn encode_block_signal_on_track(trackbits: u8) -> (u8, u8, u8) {
    let face = valid_signal_facings(trackbits)
        .first()
        .copied()
        .unwrap_or(0);
    if let Some(p) = signal_placement_for_facing(trackbits, face) {
        (p.m2, p.m3, p.m3hi)
    } else {
        (0, 0, 0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::map::TileKind;

    fn write_rail(map: &mut Map, c: TileCoord, tb: u8) {
        map.set_kind(c, TileKind::Rail).expect("kind");
        let mut t = map.get(c).expect("tile");
        t.m5 = tb | (RAIL_TILE_NORMAL << 6);
        map.set_tile(c, t).expect("tile");
    }

    fn write_signal(map: &mut Map, c: TileCoord, tb: u8) {
        map.set_kind(c, TileKind::Rail).expect("kind");
        let (m2, m3, m3hi) = encode_block_signal_on_track(tb);
        let mut t = map.get(c).expect("tile");
        t.m5 = tb | (RAIL_TILE_SIGNALS << 6);
        t.m2 = m2;
        t.m3 = m3;
        t.m3hi = m3hi;
        map.set_tile(c, t).expect("tile");
    }

    #[test]
    fn signal_placement_is_single_bit() {
        let p = signal_placement_for_facing(RAIL_TB_X, 0).expect("NE on X");
        assert_eq!(p.m3 >> 4, 0b0100);
        let p2 = signal_placement_for_facing(RAIL_TB_X, 2).expect("SW on X");
        assert_eq!(p2.m3 >> 4, 0b1000);
    }

    #[test]
    fn block_ahead_stops_at_next_signal() {
        let mut map = Map::new_flat(8, 8, 0);
        write_rail(&mut map, TileCoord::new(0, 0), RAIL_TB_X);
        write_signal(&mut map, TileCoord::new(1, 0), RAIL_TB_X);
        write_rail(&mut map, TileCoord::new(2, 0), RAIL_TB_X);
        write_rail(&mut map, TileCoord::new(3, 0), RAIL_TB_X);
        let block = rail_block_ahead(&map, TileCoord::new(1, 0), 0);
        assert_eq!(
            block,
            vec![TileCoord::new(2, 0), TileCoord::new(3, 0)],
            "bloque hasta la siguiente señal o fin de vía"
        );
    }

    #[test]
    fn train_blocked_when_block_occupied() {
        let mut state = GameState::new(8, 8);
        write_rail(&mut state.map, TileCoord::new(0, 0), RAIL_TB_X);
        write_signal(&mut state.map, TileCoord::new(1, 0), RAIL_TB_X);
        write_rail(&mut state.map, TileCoord::new(2, 0), RAIL_TB_X);
        let train_pos = vec![TileCoord::new(2, 0)];
        update_rail_signal_states(&mut state.map, &train_pos);
        assert!(train_blocked_by_signal(
            &state.map,
            &train_pos,
            TileCoord::new(1, 0),
            TileCoord::new(2, 0)
        ));
    }
}
