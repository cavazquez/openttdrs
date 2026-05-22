//! Logica de carreteras para sprites.

use openttdrs_core::{Map, TileCoord, TileKind};

/// Tabla `offsets[]` de `GetRoadSpriteOffset` en `road_cmd.cpp` (tesela plana).
/// Sprite final = `SPR_ROAD_Y` (1332) + entrada; índices 11–14 son variantes en pendiente NE/SE/SW/NW.
pub const ROAD_FLAT_OFFSET_TBL: [u8; 16] = [0, 18, 17, 7, 16, 0, 10, 5, 15, 8, 1, 4, 9, 3, 6, 2];

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

/// Índice `road_flat_{idx:02}`; en pendientes diagonales OpenTTD ignora `road_bits`
/// y usa siempre los offsets 11–14 (`SPR_ROAD_Y`+11..+14, mismo rango que `road_flat_11..14`).
#[must_use]
pub fn road_flat_sprite_index(tileh: u8, road_bits: u8, flat_offset_tbl: &[u8; 16]) -> usize {
    match tileh.min(14) {
        12 => 11, // SLOPE_NE
        6 => 12,  // SLOPE_SE
        3 => 13,  // SLOPE_SW
        9 => 14,  // SLOPE_NW
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const MP_ROAD: u8 = 2;
    const MP_TB: u8 = 9;
    const M3_FIXTURE: &[u8] =
        include_bytes!("../../../openttdrs-core/tests/fixtures/m3_road_tram_2x2.ottdmap");
    const SP3_VISUAL_FIXTURE: &[u8] =
        include_bytes!("../../../openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap");

    /// Golden: `GetRoadSpriteOffset(SLOPE_FLAT, bits)` → índice PNG `road_flat_*`.
    const EXPECTED_FLAT_INDICES_1_TO_15: [(u8, usize); 15] = [
        (0x01, 18),
        (0x02, 17),
        (0x03, 7),
        (0x04, 16),
        (0x05, 0),
        (0x06, 10),
        (0x07, 5),
        (0x08, 15),
        (0x09, 8),
        (0x0A, 1),
        (0x0B, 4),
        (0x0C, 9),
        (0x0D, 3),
        (0x0E, 6),
        (0x0F, 2),
    ];

    #[test]
    fn flat_road_bits_1_to_15_match_openttd_offset_table() {
        for (bits, expected) in EXPECTED_FLAT_INDICES_1_TO_15 {
            assert_eq!(
                road_flat_sprite_index(0, bits, &ROAD_FLAT_OFFSET_TBL),
                expected,
                "road_bits 0x{bits:02X}"
            );
            assert_eq!(
                road_flat_index(bits, &ROAD_FLAT_OFFSET_TBL),
                expected,
                "road_flat_index 0x{bits:02X}"
            );
        }
    }

    #[test]
    fn sloped_ne_se_sw_nw_ignore_road_bits() {
        assert_eq!(road_flat_sprite_index(12, 0x05, &ROAD_FLAT_OFFSET_TBL), 11);
        assert_eq!(road_flat_sprite_index(6, 0x0A, &ROAD_FLAT_OFFSET_TBL), 12);
        assert_eq!(road_flat_sprite_index(3, 0x03, &ROAD_FLAT_OFFSET_TBL), 13);
        assert_eq!(road_flat_sprite_index(9, 0x0F, &ROAD_FLAT_OFFSET_TBL), 14);
    }

    #[test]
    fn m3_fixture_effective_bits_and_tram_overlay_index() {
        let map = Map::from_ottd_binary(M3_FIXTURE).expect("fixture MAP1");
        let t = map
            .get(TileCoord::new(0, 0))
            .expect("tesela carretera con tranvía");
        assert_eq!(
            effective_road_bits(t.mapt, t.m5, t.kind, MP_ROAD, MP_TB),
            Some(0x03)
        );
        assert_eq!(t.m3, 0x0A);
        assert_eq!(
            tram_flat_sprite_index(0, t.m3, &ROAD_FLAT_OFFSET_TBL),
            Some(1)
        );
    }

    #[test]
    fn sp3_visual_fixture_crossings_decode_road_axis() {
        let map = Map::from_ottd_binary(SP3_VISUAL_FIXTURE).expect("checklist MAP1");
        let cx = map.get(TileCoord::new(5, 2)).expect("cruce X");
        let cy = map.get(TileCoord::new(6, 2)).expect("cruce Y");
        assert_eq!(
            effective_road_bits(cx.mapt, cx.m5, cx.kind, MP_ROAD, MP_TB),
            Some(0x0A)
        );
        assert_eq!(
            effective_road_bits(cy.mapt, cy.m5, cy.kind, MP_ROAD, MP_TB),
            Some(0x05)
        );
    }

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
        assert_eq!(road_flat_index(bits, &ROAD_FLAT_OFFSET_TBL), 2);
        assert!(tram_flat_sprite_index(0, 0x03, &ROAD_FLAT_OFFSET_TBL).is_some());
        assert_eq!(road_flat_sprite_index(12, bits, &ROAD_FLAT_OFFSET_TBL), 11);
        assert!(road_tile_has_tram_track(0x80));
    }
}
