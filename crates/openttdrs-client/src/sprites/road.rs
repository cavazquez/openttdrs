//! Logica de carreteras para sprites.

use openttdrs_core::{Map, TileCoord, TileKind};

#[must_use]
pub fn road_tile_has_tram_track(m8: u16) -> bool {
    let t = (m8 >> 6) & 0x3F;
    t != 0 && t != 0x3F
}

/// M3LO bits 0–3: trazado de tranvía en carretera normal (`road_map.h`), misma máscara que road bits.
#[inline]
#[must_use]
pub fn tram_track_bits_m3(m3: u8) -> u8 {
    m3 & 0x0F
}

/// Índice del PNG `tram_flat_*` (y misma tabla de desplazamiento que carretera) cuando `m3`
/// define geometría; los assets se generan desde SPR_TRAMWAY_OVERLAY (`descargar_graficos.sh`).
#[must_use]
pub fn tram_flat_sprite_index(tileh: u8, m3: u8, flat_offset_tbl: &[u8; 16]) -> Option<usize> {
    let tb = tram_track_bits_m3(m3);
    if tb == 0 {
        None
    } else {
        Some(road_flat_sprite_index(tileh, tb, flat_offset_tbl))
    }
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
    let flat = road_flat_index(road_bits, flat_offset_tbl);
    if road_bits & 0x0F != 0x0F {
        return flat;
    }
    match tileh.min(14) {
        12 => 11,
        6 => 12,
        3 => 13,
        9 => 14,
        _ => flat,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const MP_ROAD: u8 = 2;
    const MP_TB: u8 = 9;
    const FLAT_TBL: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

    #[test]
    fn effective_road_bits_subtypes_and_tunnelbridge() {
        assert_eq!(
            effective_road_bits(0x20, 0x0F, TileKind::Road, MP_ROAD, MP_TB),
            Some(0x0F)
        );
        assert_eq!(
            effective_road_bits(0x20, 0x40, TileKind::Road, MP_ROAD, MP_TB),
            Some(0x0A)
        );
        assert_eq!(
            effective_road_bits(0x20, 0x41, TileKind::Road, MP_ROAD, MP_TB),
            Some(0x05)
        );
        assert_eq!(
            effective_road_bits(0x20, 0x80, TileKind::Road, MP_ROAD, MP_TB),
            Some(0x08)
        );
        assert_eq!(
            effective_road_bits(0x90, 0x01, TileKind::Road, MP_ROAD, MP_TB),
            Some(0x04)
        );
    }

    #[test]
    fn fallback_neighbor_bits_and_indices_work() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        map.set_kind(center, TileKind::Road).unwrap();
        map.set_mapt_m5(center, 0x20, 0).unwrap();
        map.set_kind(TileCoord::new(0, 1), TileKind::Station)
            .unwrap();
        map.set_kind(TileCoord::new(1, 0), TileKind::Industry)
            .unwrap();
        map.set_kind(TileCoord::new(2, 1), TileKind::House).unwrap();
        map.set_kind(TileCoord::new(1, 2), TileKind::Road).unwrap();

        let bits = road_bits_for_render(&map, center, 3, 3, MP_ROAD, MP_TB);
        assert_eq!(bits, 0x0F);
        assert_eq!(road_flat_index(bits, &FLAT_TBL), 15);
        assert!(tram_flat_sprite_index(0, 0x03, &FLAT_TBL).is_some());
        assert_eq!(road_flat_sprite_index(12, bits, &FLAT_TBL), 11);
        assert!(road_tile_has_tram_track(0x80));
    }
}
