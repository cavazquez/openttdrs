//! Logica de carreteras para sprites.

use openttdrs_core::{Map, TileCoord, TileKind};

#[must_use]
pub fn road_tile_has_tram_track(m8: u16) -> bool {
    let t = (m8 >> 6) & 0x3F;
    t != 0 && t != 0x3F
}

/// Decodifica los road bits efectivos desde m5 segun el tipo de tesela.
pub fn effective_road_bits(
    mapt: u8,
    m5: u8,
    kind: TileKind,
    mp_road: u8,
    mp_tunnelbridge: u8,
) -> Option<u8> {
    let tt = (mapt >> 4) & 0xF;
    match tt {
        t if t == mp_road => {
            let subtype = (m5 >> 6) & 0x3;
            match subtype {
                0 => {
                    let rb = m5 & 0x0F;
                    if rb == 0 { None } else { Some(rb) }
                }
                1 => {
                    let axis = m5 & 1;
                    Some(if axis == 0 { 0x0A } else { 0x05 })
                }
                2 => {
                    let d = m5 & 0x3;
                    Some((1u8 << (3 ^ d)) & 0x0F)
                }
                _ => None,
            }
        }
        t if t == mp_tunnelbridge && kind == TileKind::Road => {
            let d = m5 & 0x3;
            Some((1u8 << (3 ^ d)) & 0x0F)
        }
        _ => None,
    }
}

#[inline]
pub fn road_flat_index(road_bits: u8, flat_offset_tbl: &[u8; 16]) -> usize {
    usize::from(flat_offset_tbl[usize::from(road_bits & 0x0F)])
}

#[must_use]
pub fn road_flat_sprite_index(tileh: u8, road_bits: u8, flat_offset_tbl: &[u8; 16]) -> usize {
    match tileh.min(14) {
        0 => road_flat_index(road_bits, flat_offset_tbl),
        12 => 11,
        6 => 12,
        3 => 13,
        9 => 14,
        _ => road_flat_index(road_bits, flat_offset_tbl),
    }
}

pub fn road_bits_for_render(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_road: u8,
    mp_tunnelbridge: u8,
) -> u8 {
    if let Some(t) = map.get(pos)
        && let Some(rb) = effective_road_bits(t.mapt, t.m5, t.kind, mp_road, mp_tunnelbridge)
        && rb != 0
    {
        return rb & 0x0F;
    }
    let is_road_or_station = |c: TileCoord| -> bool {
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
            return false;
        }
        matches!(
            map.get_kind(c),
            Some(TileKind::Road | TileKind::Station | TileKind::Industry | TileKind::House)
        )
    };
    let mut bits = 0u8;
    if is_road_or_station(TileCoord::new(pos.x - 1, pos.y)) {
        bits |= 8; // NE
    }
    if is_road_or_station(TileCoord::new(pos.x, pos.y - 1)) {
        bits |= 1; // NW
    }
    if is_road_or_station(TileCoord::new(pos.x + 1, pos.y)) {
        bits |= 2; // SW
    }
    if is_road_or_station(TileCoord::new(pos.x, pos.y + 1)) {
        bits |= 4; // SE
    }
    if bits == 0 {
        bits = 0x05;
    }
    bits
}
