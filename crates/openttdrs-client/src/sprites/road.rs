//! Logica de carreteras para sprites.

use openttdrs_core::{Map, TileCoord, TileKind};

/// Tabla `offsets[]` de `GetRoadSpriteOffset` en `road_cmd.cpp` (tesela plana).
/// Sprite final = `SPR_ROAD_Y` (1332) + entrada; índices 11–14 son variantes en pendiente NE/SE/SW/NW.
pub const ROAD_FLAT_OFFSET_TBL: [u8; 16] = [0, 18, 17, 7, 16, 0, 10, 5, 15, 8, 1, 4, 9, 3, 6, 2];

#[path = "road_depot_gfx_data_generated.rs"]
mod road_depot_gfx_data_generated;

pub use road_depot_gfx_data_generated::{
    ROAD_DEPOT_BUILD_LAYERS, ROAD_DEPOT_GROUND_PATH, RoadDepotLayerGfx,
};

/// Road bits en la tesela del depósito (`DiagDirToRoadBits` en `road_func.h`).
#[must_use]
pub fn road_depot_entrance_road_bits(dir: u8) -> u8 {
    1u8 << (3 ^ (dir & 3))
}

// ── Roadside (decoración del borde: pasto / acera / faroles) ─────────────────

/// `GetRoadside` (`road_map.h`): bits 3–5 de `m6` en carretera **normal**
/// (`RoadTileType::Normal`, bits 6–7 de `m5` = 0). En cruces y depósitos no aplica.
#[must_use]
pub fn road_tile_roadside(m5: u8, m6: u8) -> Option<u8> {
    if (m5 >> 6) & 0x3 == 0 {
        Some((m6 >> 3) & 0x7)
    } else {
        None
    }
}

/// `GetRoadGroundSprite` (`road_cmd.cpp`): Paved (2), StreetLights (3), Trees (5)
/// y PavedRoadWorks (7) usan el set pavimentado (`SPR_ROAD_Y - 19` = 1313..1331).
/// Barren (0), Grass (1) y GrassRoadWorks (6) usan el set sobre pasto.
#[must_use]
pub fn roadside_is_paved(roadside: u8) -> bool {
    matches!(roadside, 2 | 3 | 5 | 7)
}

/// Farol de `_roadside_lamps` (`table/road_land.h`): índice de PNG
/// (`road_streetlight_{i}.png`, 0 = 0x57E, 1 = 0x57F) y subcoordenadas de
/// mundo (0..15) dentro de la tesela.
pub type RoadsideLamp = (usize, f32, f32);

/// `_roadside_lamps[road_bits]` (`table/road_land.h`): faroles a dibujar cuando
/// `Roadside::StreetLights`. Indexado por los 4 road bits (NW=1, SW=2, SE=4, NE=8).
pub static ROADSIDE_LAMPS: [&[RoadsideLamp]; 16] = [
    &[],
    &[],
    &[],
    &[(1, 1.0, 8.0)],
    &[],
    &[(1, 1.0, 8.0), (0, 14.0, 8.0)],
    &[(0, 8.0, 1.0)],
    &[(1, 1.0, 8.0)],
    &[],
    &[(1, 8.0, 14.0)],
    &[(1, 8.0, 14.0), (0, 8.0, 1.0)],
    &[(1, 8.0, 14.0)],
    &[(0, 8.0, 1.0)],
    &[(0, 14.0, 8.0)],
    &[(0, 8.0, 1.0)],
    &[],
];

/// (w, h, xrel, yrel) NFO de `road_streetlight_{0,1}.png` (sprites 0x57E/0x57F).
pub static ROAD_STREETLIGHT_META: [(f32, f32, f32, f32); 2] =
    [(4.0, 14.0, 2.0, -13.0), (4.0, 14.0, -2.0, -13.0)];

#[must_use]
pub fn road_depot_build_layers(dir: usize) -> &'static [RoadDepotLayerGfx] {
    ROAD_DEPOT_BUILD_LAYERS[dir.min(3)]
}

#[must_use]
pub fn road_depot_seq_gfx(layer: &RoadDepotLayerGfx) -> crate::iso::RoadStopSeqGfx {
    crate::iso::RoadStopSeqGfx {
        dx: layer.dx,
        dy: layer.dy,
        dz: layer.dz,
        x_offs: layer.x_offs,
        y_offs: layer.y_offs,
        remap_x_adj: layer.remap_x_adj,
    }
}

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
    openttdrs_core::effective_road_bits(mapt, m5, kind, mp_road, mp_tunnelbridge)
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
    fn sp3_visual_fixture_tram_uses_aligned_track_mask() {
        let map = Map::from_ottd_binary(SP3_VISUAL_FIXTURE).expect("checklist MAP1");
        let t = map.get(TileCoord::new(15, 3)).expect("tranvía");
        assert_eq!(
            effective_road_bits(t.mapt, t.m5, t.kind, MP_ROAD, MP_TB),
            Some(0x0A)
        );
        assert_eq!(t.m3 & 0x0F, 0x0A);
        assert_eq!(
            tram_flat_sprite_index(0, t.m3, &ROAD_FLAT_OFFSET_TBL),
            Some(road_flat_sprite_index(0, 0x0A, &ROAD_FLAT_OFFSET_TBL))
        );
    }

    #[test]
    fn sp3_visual_fixture_tram_on_ne_slope_uses_slope_flat_index() {
        let map = Map::from_ottd_binary(SP3_VISUAL_FIXTURE).expect("checklist MAP1");
        let t = map
            .get(TileCoord::new(13, 7))
            .expect("tranvía pendiente NE");
        let tileh = openttdrs_core::tile_slope_and_z(&map, TileCoord::new(13, 7))
            .expect("slope")
            .0;
        assert_eq!(tileh, 12);
        assert_eq!(t.m3, 0x05);
        assert_eq!(
            road_flat_sprite_index(tileh, t.m5, &ROAD_FLAT_OFFSET_TBL),
            11
        );
        assert_eq!(
            tram_flat_sprite_index(tileh, t.m3, &ROAD_FLAT_OFFSET_TBL),
            Some(11)
        );
    }

    #[test]
    fn sp3_visual_fixture_crossings_decode_road_axis() {
        let map = Map::from_ottd_binary(SP3_VISUAL_FIXTURE).expect("checklist MAP1");
        let cx = map.get(TileCoord::new(9, 3)).expect("cruce X");
        let cy = map.get(TileCoord::new(11, 3)).expect("cruce Y");
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
    fn sp3_visual_fixture_road_depots_on_row_y6() {
        let map = Map::from_ottd_binary(SP3_VISUAL_FIXTURE).expect("checklist MAP1");
        for (x, dir) in [(3, 0), (6, 1), (10, 2), (14, 3)] {
            let t = map.get(TileCoord::new(x, 6)).expect("depot tile");
            assert_eq!(t.kind, TileKind::RoadDepot, "({x},6)");
            assert_eq!(t.m5 & 0x03, dir);
            assert_eq!((t.m5 >> 6) & 0x03, 2);
            assert_eq!(
                effective_road_bits(t.mapt, t.m5, t.kind, MP_ROAD, MP_TB),
                Some((1u8 << (3 ^ dir)) & 0x0F)
            );
        }
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

    #[test]
    fn road_depot_entrance_road_bits_match_diag_dir() {
        assert_eq!(road_depot_entrance_road_bits(0), 0x08);
        assert_eq!(road_depot_entrance_road_bits(1), 0x04);
        assert_eq!(road_depot_entrance_road_bits(2), 0x02);
        assert_eq!(road_depot_entrance_road_bits(3), 0x01);
    }

    #[test]
    fn road_depot_build_layer_counts_per_direction() {
        assert_eq!(road_depot_build_layers(0).len(), 1);
        assert_eq!(road_depot_build_layers(1).len(), 2);
        assert_eq!(road_depot_build_layers(2).len(), 2);
        assert_eq!(road_depot_build_layers(3).len(), 1);
    }

    #[test]
    fn road_depot_generated_offsets_from_nfo() {
        let ne = road_depot_build_layers(0)[0];
        assert_eq!(ne.path, "assets/opengfx/tiles/rail_1412.png");
        assert!((ne.x_offs - (-59.0)).abs() < 0.1);
        assert!((ne.y_offs - (-32.0)).abs() < 0.1);
        assert!((ne.remap_x_adj - (-13.0)).abs() < 0.1);

        let se_mouth = road_depot_build_layers(1)[0];
        assert!((se_mouth.x_offs - 18.0).abs() < 0.1);
        assert!((se_mouth.y_offs - 5.0).abs() < 0.1);
        assert!((se_mouth.remap_x_adj - (-3.0)).abs() < 0.1);

        let se_build = road_depot_build_layers(1)[1];
        assert!((se_build.x_offs - 1.0).abs() < 0.1);
        assert!((se_build.y_offs - (-38.0)).abs() < 0.1);

        let nw = road_depot_build_layers(3)[0];
        assert!((nw.x_offs - 1.0).abs() < 0.1);
        assert!((nw.y_offs - (-38.0)).abs() < 0.1);
        assert!((nw.remap_x_adj - 8.0).abs() < 0.1);
    }
}
