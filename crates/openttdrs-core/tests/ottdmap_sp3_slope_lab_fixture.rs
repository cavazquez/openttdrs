//! Fixture `fixtures/sp3_slope_lab.ottdmap` (16×16): laboratorio agua + vía en pendiente.
//!
//! Regenerar: `python3 scripts/gen_sp3_slope_lab_ottdmap.py`

#![allow(clippy::expect_used)]

use openttdrs_core::{Map, TileCoord, TileKind, tile_slope_and_z};

const FIXTURE: &[u8] = include_bytes!("fixtures/sp3_slope_lab.ottdmap");

fn tile(map: &Map, x: i32, y: i32) -> openttdrs_core::Tile {
    map.get(TileCoord::new(x, y))
        .unwrap_or_else(|| panic!("tile ({x},{y})"))
}

#[test]
fn loads_sp3_slope_lab_layout() {
    let (map, _ex) = Map::from_ottd_binary_with_extras(FIXTURE).expect("fixture MAP1");
    assert_eq!(map.dimensions(), (16, 16));

    // Referencia plana (y=1)
    assert_eq!(tile(&map, 1, 1).kind, TileKind::Rail);
    assert_eq!(tile(&map, 1, 1).m5 & 0x3F, 0x02);
    assert_eq!(tile(&map, 4, 1).m5 & 0x3F, 0x01);
    assert_eq!(tile(&map, 7, 1).m5 & 0x3F, 0x07);
    assert_eq!(tile(&map, 10, 1).m5 & 0x3F, 0x03);

    // Lago Clear 3×3 (centro sin vecinos de tierra → mar animado)
    for tx in 2..=4 {
        for ty in 3..=5 {
            let w = tile(&map, tx, ty);
            assert_eq!(w.kind, TileKind::Water);
            assert_eq!(w.m5, 0);
            assert_eq!(w.height, 4);
        }
    }
    let coast = tile(&map, 8, 4);
    assert_eq!(coast.kind, TileKind::Water);
    assert_eq!(coast.m5, 0x10);
    assert_eq!(coast.height, 4);
    assert_eq!((coast.m5 >> 4) & 0x0F, 1);

    // Recta Y en pendiente (y=8)
    for (x, tileh) in [(1, 12), (4, 6), (7, 3), (10, 9)] {
        let r = tile(&map, x, 8);
        assert_eq!(r.kind, TileKind::Rail);
        assert_eq!(r.m5 & 0x3F, 0x02);
        assert_eq!(
            tile_slope_and_z(&map, TileCoord::new(x, 8)).map(|(h, _)| h),
            Some(tileh)
        );
    }

    // Cruce en pendiente (y=11)
    for (x, tileh) in [(1, 12), (4, 6), (7, 3), (10, 9)] {
        let r = tile(&map, x, 11);
        assert_eq!(r.kind, TileKind::Rail);
        assert_eq!(r.m5 & 0x3F, 0x03);
        assert_eq!(
            tile_slope_and_z(&map, TileCoord::new(x, 11)).map(|(h, _)| h),
            Some(tileh)
        );
    }

    // T en pendiente (y=14)
    for (x, tileh) in [(1, 12), (4, 6), (7, 3), (10, 9)] {
        let r = tile(&map, x, 14);
        assert_eq!(r.kind, TileKind::Rail);
        assert_eq!(r.m5 & 0x3F, 0x07);
        assert_eq!(
            tile_slope_and_z(&map, TileCoord::new(x, 14)).map(|(h, _)| h),
            Some(tileh)
        );
    }

    // Separación: hierba entre escenas
    assert_eq!(tile(&map, 2, 1).kind, TileKind::Grass);
    assert_eq!(tile(&map, 1, 7).kind, TileKind::Grass);
    assert_eq!(tile(&map, 1, 10).kind, TileKind::Grass);
    assert_eq!(tile(&map, 1, 13).kind, TileKind::Grass);
}
