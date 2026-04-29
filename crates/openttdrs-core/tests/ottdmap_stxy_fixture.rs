//! Fixture binario versionado en `tests/fixtures/v5p12_stxy.ottdmap` (v5+12 + footer STXY).

use openttdrs_core::{Map, TileCoord, TileKind};

const FIXTURE: &[u8] = include_bytes!("fixtures/v5p12_stxy.ottdmap");

#[test]
fn loads_v5p12_fixture_with_stxy_and_m2_hi() {
    let (map, ex) = Map::from_ottd_binary_with_extras(FIXTURE).expect("fixture válido");
    assert_eq!(map.dimensions(), (2, 2));
    assert_eq!(ex.station_xy, vec![(0, 0)]);
    let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
    assert_eq!(t0.kind, TileKind::Station);
    assert_eq!(t0.m2_hi, 0);
    let t11 = map.get(TileCoord::new(1, 1)).expect("tile");
    assert_eq!(t11.m2_hi, 3);
}
