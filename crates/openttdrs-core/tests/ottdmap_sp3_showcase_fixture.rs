//! Fixture `fixtures/sp3_showcase.ottdmap` (64×48): mapa grande de QA visual.
//!
//! Regenerar: `python3 scripts/gen_sp3_showcase_ottdmap.py`

#![allow(clippy::expect_used)]

use openttdrs_core::{Map, TileCoord, TileKind};

const FIXTURE: &[u8] = include_bytes!("fixtures/sp3_showcase.ottdmap");

fn tile(map: &Map, x: i32, y: i32) -> openttdrs_core::Tile {
    map.get(TileCoord::new(x, y))
        .unwrap_or_else(|| panic!("tile ({x},{y})"))
}

fn industry_gfx9(t: &openttdrs_core::Tile) -> u16 {
    u16::from(t.m5) | (u16::from((t.m6 >> 2) & 1) << 8)
}

#[test]
fn loads_sp3_showcase_layout() {
    let (map, ex) = Map::from_ottd_binary_with_extras(FIXTURE).expect("fixture MAP1");
    assert_eq!(map.dimensions(), (64, 48));
    assert!(ex.station_xy.len() >= 8, "paradas/estaciones en STXY");

    // Mina de carbón (torre animada gfx 1)
    assert_eq!(tile(&map, 27, 3).kind, TileKind::Industry);
    assert_eq!(industry_gfx9(&tile(&map, 27, 3)), 1);
    // Mina de carbón multi-tesela (m2=1): cabeza + torre
    assert_eq!(tile(&map, 26, 3).m2, 1);
    assert_eq!(tile(&map, 27, 3).m2, 1);

    // Central (chimenea gfx 8 → humo)
    assert_eq!(industry_gfx9(&tile(&map, 35, 3)), 8);

    // Lago
    assert_eq!(tile(&map, 55, 10).kind, TileKind::Water);
    assert_eq!(tile(&map, 51, 10).m5, 0x10);

    // Checklist embebido: carretera plana y=34 (=31+3)
    assert_eq!(tile(&map, 3, 34).kind, TileKind::Road);
    assert_eq!(tile(&map, 3, 34).m5, 0x05);

    // Autopista principal
    assert_eq!(tile(&map, 30, 15).kind, TileKind::Road);
    assert_eq!(tile(&map, 30, 15).m5 & 0x0F, 0x0A);

    // Vía doble
    assert_eq!(tile(&map, 30, 17).kind, TileKind::Rail);
    assert_eq!(tile(&map, 30, 19).kind, TileKind::Rail);
}
