//! Fixture `fixtures/sp3_visual_checklist.ottdmap` (12×8): escenas del checklist SP3.0.
//!
//! Regenerar: `python3 scripts/gen_sp3_visual_checklist_ottdmap.py`

#![allow(clippy::expect_used)]

use openttdrs_core::{Map, TileCoord, TileKind};

const FIXTURE: &[u8] = include_bytes!("fixtures/sp3_visual_checklist.ottdmap");

fn tile(map: &Map, x: i32, y: i32) -> openttdrs_core::Tile {
    map.get(TileCoord::new(x, y))
        .unwrap_or_else(|| panic!("tile ({x},{y})"))
}

#[test]
fn loads_sp3_visual_checklist_layout() {
    let (map, ex) = Map::from_ottd_binary_with_extras(FIXTURE).expect("fixture MAP1");
    assert_eq!(map.dimensions(), (12, 8));
    assert_eq!(ex.station_xy.len(), 2);
    assert!(ex.station_xy.contains(&(1, 5)));
    assert!(ex.station_xy.contains(&(4, 4)));

    // Carretera plana (fila y=2)
    assert_eq!(tile(&map, 1, 2).kind, TileKind::Road);
    assert_eq!(tile(&map, 1, 2).m5, 0x05);
    assert_eq!(tile(&map, 2, 2).m5, 0x0A);
    assert_eq!(tile(&map, 3, 2).m5, 0x07);
    assert_eq!(tile(&map, 4, 2).m5, 0x0F);
    assert_eq!(tile(&map, 5, 2).m5, 0x40);
    assert_eq!(tile(&map, 6, 2).m5, 0x41);
    assert_eq!(tile(&map, 7, 2).m3, 0x0A);

    // Vía (fila y=3)
    assert_eq!(tile(&map, 1, 3).kind, TileKind::Rail);
    assert_eq!(tile(&map, 1, 3).m5 & 0x3F, 0x02);
    assert_eq!(tile(&map, 2, 3).m5 & 0x3F, 0x01);
    assert_eq!(tile(&map, 3, 3).m5 & 0x3F, 0x07);
    assert_eq!(tile(&map, 4, 3).m5 & 0x3F, 0x03);

    let sig = tile(&map, 8, 3);
    assert_eq!(sig.kind, TileKind::Rail);
    assert_eq!((sig.m5 >> 6) & 0x3, 1);
    assert_eq!(sig.m3, 0xC0);
    let snow = tile(&map, 9, 3);
    assert_eq!(snow.m3 & 0x0F, 0x0C);

    // Casa, estación e industria (y=5)
    let house = tile(&map, 0, 5);
    assert_eq!(house.kind, TileKind::House);
    assert_eq!(house.m8 & 0xFFF, 0);
    let truck_st = tile(&map, 1, 5);
    assert_eq!(truck_st.kind, TileKind::Station);
    assert_eq!((truck_st.m6 >> 3) & 0x0F, 2);
    let rail_st = tile(&map, 4, 4);
    assert_eq!(rail_st.kind, TileKind::Station);
    assert_eq!((rail_st.m6 >> 3) & 0x0F, 0);
    assert_eq!(rail_st.m5 & 1, 1);
    assert_eq!(tile(&map, 4, 5).kind, TileKind::Industry);
    assert_eq!(tile(&map, 4, 5).m5, 0);

    // Agua y costa (y=7)
    assert_eq!(tile(&map, 2, 7).kind, TileKind::Water);
    assert_eq!(tile(&map, 2, 7).m5, 0);
    assert_eq!(tile(&map, 3, 7).kind, TileKind::Water);
    assert_eq!(tile(&map, 3, 7).m5, 0x10);
    assert_eq!((tile(&map, 3, 7).m5 >> 4) & 0x0F, 1);

    // Buffer hierba
    assert_eq!(tile(&map, 0, 0).kind, TileKind::Grass);
}
