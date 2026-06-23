//! Fixture `fixtures/m3_road_tram_2x2.ottdmap`: MAP1 v1, 2×2, tesela (0,0) `MP_ROAD` con `m3` ≠ 0 (bits tranvía M3LO).

#![allow(clippy::expect_used)]

use openttdrs_core::{
    Map, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE, TileCoord, TileKind, effective_road_bits,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/m3_road_tram_2x2.ottdmap");

#[test]
fn loads_m3_on_road_tile_from_fixture() {
    let map = Map::from_ottd_binary(FIXTURE).expect("fixture MAP1 válido");
    assert_eq!(map.dimensions(), (2, 2));
    let road = map.get(TileCoord::new(0, 0)).expect("tile 0,0");
    assert_eq!(road.kind, TileKind::Road);
    assert_eq!(
        road.m3, 0x0A,
        "M3LO persistido en .ottdmap (tranvía / m3 bajo)"
    );
    assert_eq!(road.m5, 0x03, "road bits NW+NE en M5LO");
    assert_eq!(road.m3 & 0x0F, 0x0A, "tram bits en M3LO");
    let grass = map.get(TileCoord::new(1, 0)).expect("tile 1,0");
    assert_eq!(grass.kind, TileKind::Grass);
    assert_eq!(grass.m3, 0);
}

#[test]
fn effective_road_bits_on_m3_fixture_matches_imported_m5() {
    let map = Map::from_ottd_binary(FIXTURE).expect("fixture MAP1 válido");
    let road = map.get(TileCoord::new(0, 0)).expect("tile 0,0");
    assert_eq!(
        effective_road_bits(
            road.mapt,
            road.m5,
            road.kind,
            OTTD_MP_ROAD,
            OTTD_MP_TUNNELBRIDGE
        ),
        Some(0x03),
        "NW+NE desde subtipo Normal (m5 & 0x0F)"
    );
}
