//! Fixture `fixtures/sp3_visual_checklist.ottdmap` (20×12): escenas SP3.0/SP3.1 separadas.
//!
//! Regenerar: `python3 scripts/gen_sp3_visual_checklist_ottdmap.py`

#![allow(clippy::expect_used)]

use openttdrs_core::{Map, TileCoord, TileKind, tile_slope_and_z};

const FIXTURE: &[u8] = include_bytes!("fixtures/sp3_visual_checklist.ottdmap");

fn tile(map: &Map, x: i32, y: i32) -> openttdrs_core::Tile {
    map.get(TileCoord::new(x, y))
        .unwrap_or_else(|| panic!("tile ({x},{y})"))
}

#[test]
fn loads_sp3_visual_checklist_layout() {
    let (map, ex) = Map::from_ottd_binary_with_extras(FIXTURE).expect("fixture MAP1");
    assert_eq!(map.dimensions(), (20, 12));
    assert_eq!(ex.station_xy.len(), 3);
    assert!(ex.station_xy.contains(&(3, 9)));
    assert!(ex.station_xy.contains(&(5, 9)));
    assert!(ex.station_xy.contains(&(7, 9)));

    // Carretera plana (y=3), separada 2 teselas en x
    assert_eq!(tile(&map, 1, 3).kind, TileKind::Road);
    assert_eq!(tile(&map, 1, 3).m5, 0x05);
    assert_eq!(tile(&map, 3, 3).m5, 0x0A);
    assert_eq!(tile(&map, 5, 3).m5, 0x07);
    assert_eq!(tile(&map, 7, 3).m5, 0x0F);
    assert_eq!(tile(&map, 9, 3).m5, 0x40);
    assert_eq!(tile(&map, 11, 3).m5, 0x41);
    let tram = tile(&map, 15, 3);
    assert_eq!(tram.m5, 0x0A);
    assert_eq!(tram.m3, 0x0A);

    // Vía plana (y=5)
    assert_eq!(tile(&map, 1, 5).kind, TileKind::Rail);
    assert_eq!(tile(&map, 1, 5).m5 & 0x3F, 0x02);
    assert_eq!(tile(&map, 3, 5).m5 & 0x3F, 0x01);
    assert_eq!(tile(&map, 5, 5).m5 & 0x3F, 0x07);
    assert_eq!(tile(&map, 7, 5).m5 & 0x3F, 0x03);

    let sig = tile(&map, 9, 5);
    assert_eq!(sig.kind, TileKind::Rail);
    assert_eq!((sig.m5 >> 6) & 0x3, 1);
    assert_eq!(sig.m3, 0xC0);
    let snow = tile(&map, 11, 5);
    assert_eq!(snow.m3 & 0x0F, 0x0C);

    // SP3.1: carretera en pendiente (y=7)
    assert_eq!(tile(&map, 1, 7).kind, TileKind::Road);
    assert_eq!(
        tile_slope_and_z(&map, TileCoord::new(1, 7)).map(|(h, _)| h),
        Some(12) // SLOPE_NE
    );
    assert_eq!(
        tile_slope_and_z(&map, TileCoord::new(4, 7)).map(|(h, _)| h),
        Some(6) // SLOPE_SE
    );
    assert_eq!(
        tile_slope_and_z(&map, TileCoord::new(7, 7)).map(|(h, _)| h),
        Some(3) // SLOPE_SW
    );
    assert_eq!(
        tile_slope_and_z(&map, TileCoord::new(10, 7)).map(|(h, _)| h),
        Some(9) // SLOPE_NW
    );
    let tram_slope = tile(&map, 13, 7);
    assert_eq!(tram_slope.kind, TileKind::Road);
    assert_eq!(tram_slope.m5, 0x05);
    assert_eq!(tram_slope.m3, 0x05);
    assert_eq!(
        tile_slope_and_z(&map, TileCoord::new(13, 7)).map(|(h, _)| h),
        Some(12)
    );

    // Objetos (y=9)
    let house = tile(&map, 1, 9);
    assert_eq!(house.kind, TileKind::House);
    assert_eq!(house.m8 & 0xFFF, 0);
    let truck_st = tile(&map, 3, 9);
    assert_eq!(truck_st.kind, TileKind::Station);
    assert_eq!((truck_st.m6 >> 3) & 0x0F, 2);
    let bus_st = tile(&map, 5, 9);
    assert_eq!(bus_st.kind, TileKind::Station);
    assert_eq!((bus_st.m6 >> 3) & 0x0F, 3);
    let rail_st = tile(&map, 7, 9);
    assert_eq!(rail_st.kind, TileKind::Station);
    assert_eq!((rail_st.m6 >> 3) & 0x0F, 0);
    assert_eq!(rail_st.m5 & 1, 1);
    assert_eq!(tile(&map, 9, 9).kind, TileKind::Industry);

    // Agua y costa (y=11)
    assert_eq!(tile(&map, 3, 11).kind, TileKind::Water);
    assert_eq!(tile(&map, 3, 11).m5, 0);
    assert_eq!(tile(&map, 5, 11).kind, TileKind::Water);
    assert_eq!(tile(&map, 5, 11).m5, 0x10);
    assert_eq!((tile(&map, 5, 11).m5 >> 4) & 0x0F, 1);

    // Buffer hierba entre escenas (cruce vs vía)
    assert_eq!(tile(&map, 2, 3).kind, TileKind::Grass);
    assert_eq!(tile(&map, 2, 5).kind, TileKind::Grass);
    assert_eq!(tile(&map, 0, 0).kind, TileKind::Grass);
}
