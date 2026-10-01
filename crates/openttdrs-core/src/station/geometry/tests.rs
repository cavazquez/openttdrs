//! The pre-optimization `std::HashSet` traversal is the ordered output oracle.

use super::*;

#[test]
fn connected_footprints_keep_order_for_every_three_by_three_topology() {
    for mask in 0_u16..512 {
        let mut map = Map::new_flat(5, 5, 0);
        for bit in 0..9 {
            let coord = TileCoord::new(1 + bit % 3, 1 + bit / 3);
            if mask & (1 << bit) != 0 {
                map.set_kind(coord, TileKind::Station).expect("fixture");
            }
        }
        for anchor in [
            TileCoord::new(2, 2),
            TileCoord::new(1, 1),
            TileCoord::new(0, 2),
            TileCoord::new(-1, 2),
        ] {
            assert_eq!(
                station_footprint_tiles(&map, anchor),
                previous_footprint(&map, anchor),
                "mask={mask}, anchor={anchor:?}"
            );
        }
    }
}

#[test]
fn budget_boundaries_kinds_and_live_map_changes_keep_previous_footprint() {
    let mut map = Map::new_flat(24, 24, 0);
    for y in 0..24 {
        for x in 0..24 {
            map.set_kind(TileCoord::new(x, y), TileKind::Station)
                .expect("fixture");
        }
    }
    let center = TileCoord::new(12, 12);
    let actual = station_footprint_tiles(&map, center);
    assert_eq!(actual, previous_footprint(&map, center));
    assert!(actual.len() >= 64);
    assert_eq!(
        &actual[..5],
        &[
            center,
            TileCoord::new(11, 12),
            TileCoord::new(12, 13),
            TileCoord::new(13, 12),
            TileCoord::new(12, 11),
        ]
    );
    for kind in [
        TileKind::Grass,
        TileKind::Airport,
        TileKind::Road,
        TileKind::Rail,
        TileKind::Station,
    ] {
        for x in 0..24 {
            map.set_kind(TileCoord::new(x, 12), kind).expect("fixture");
        }
        for anchor in [center, TileCoord::new(0, 0), TileCoord::new(24, 12)] {
            assert_eq!(
                station_footprint_tiles(&map, anchor),
                previous_footprint(&map, anchor),
                "kind={kind:?}, anchor={anchor:?}"
            );
        }
    }
    let mut clone = map.clone();
    clone.set_kind(center, TileKind::Grass).expect("fixture");
    assert_eq!(
        station_footprint_tiles(&clone, center),
        previous_footprint(&clone, center)
    );
    assert_eq!(
        station_footprint_tiles(&map, center),
        previous_footprint(&map, center)
    );
}

fn previous_footprint(map: &Map, anchor: TileCoord) -> Vec<TileCoord> {
    const MAX_FOOTPRINT: usize = 64;
    let mut tiles = vec![anchor];
    let mut seen = std::collections::HashSet::from([anchor]);
    let mut i = 0;
    while i < tiles.len() && tiles.len() < MAX_FOOTPRINT {
        let c = tiles[i];
        i += 1;
        for dir in 0..4u8 {
            let (dx, dy) = diag_dir_offset(dir);
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if map.get_kind(n) == Some(TileKind::Station) && seen.insert(n) {
                tiles.push(n);
            }
        }
    }
    tiles
}
