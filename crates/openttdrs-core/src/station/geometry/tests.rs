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

fn previous_station_at_tile<'a>(
    map: &Map,
    stations: &'a [Station],
    tile: TileCoord,
) -> Option<&'a Station> {
    if let Some(station) = stations.iter().find(|station| station.covers_tile(tile)) {
        return Some(station);
    }
    if map.get_kind(tile) != Some(TileKind::Station) {
        return None;
    }
    stations
        .iter()
        .filter(|station| {
            matches!(
                station.stop_kind,
                StopKind::RailStation | StopKind::RailWaypoint
            ) && previous_footprint(map, station.pos).contains(&tile)
        })
        .min_by_key(|station| manhattan(station.pos, tile))
}

fn assert_lookup_matches(
    map: &Map,
    stations: &[Station],
    index: &crate::TerminalSpatialIndex,
    tile: TileCoord,
) {
    let slot = |result: Option<&Station>| {
        result.and_then(|station| {
            stations
                .iter()
                .position(|candidate| std::ptr::eq(candidate, station))
        })
    };
    let expected = slot(previous_station_at_tile(map, stations, tile));
    assert_eq!(
        slot(station_at_tile(map, stations, tile)),
        expected,
        "live tile={tile:?}"
    );
    assert_eq!(
        slot(station_at_tile_indexed(map, stations, tile, index)),
        expected,
        "indexed tile={tile:?}"
    );
}

fn rail_station(pos: TileCoord, kind: StopKind) -> Station {
    let mut station = Station::new(pos);
    station.stop_kind = kind;
    station.ottd_station_id = Some(77);
    station
}

#[test]
fn indexed_lookup_keeps_all_small_topologies_and_direct_coverage_priority() {
    let mut queries = 0;
    for mask in 0_u16..512 {
        let mut map = Map::new_flat(5, 5, 0);
        for bit in 0..9 {
            if mask & (1 << bit) != 0 {
                map.set_kind(TileCoord::new(1 + bit % 3, 1 + bit / 3), TileKind::Station)
                    .expect("fixture");
            }
        }
        let mut direct = rail_station(TileCoord::new(4, 4), StopKind::Dock);
        direct.joined_tiles.push(TileCoord::new(2, 2));
        direct.airport_tiles.push(TileCoord::new(1, 3));
        let stations = vec![
            rail_station(TileCoord::new(1, 1), StopKind::RailStation),
            rail_station(TileCoord::new(3, 3), StopKind::RailWaypoint),
            direct,
        ];
        let mut index = crate::TerminalSpatialIndex::default();
        index.ensure_current(&map, &stations);
        for y in -1..6 {
            for x in -1..6 {
                assert_lookup_matches(&map, &stations, &index, TileCoord::new(x, y));
                queries += 1;
            }
        }
        assert!(std::ptr::eq(
            station_at_tile_indexed(&map, &stations, TileCoord::new(2, 2), &index).expect("direct"),
            &raw const stations[2]
        ));
    }
    assert_eq!(queries, 25_088);
}

#[test]
fn indexed_lookup_reads_current_station_order_roles_and_new_anchors() {
    let mut map = Map::new_flat(9, 3, 0);
    for x in 0..9 {
        map.set_kind(TileCoord::new(x, 1), TileKind::Station)
            .expect("fixture");
    }
    let left = rail_station(TileCoord::new(2, 1), StopKind::RailStation);
    let right = rail_station(TileCoord::new(6, 1), StopKind::RailWaypoint);
    let mut stations = vec![left.clone(), right.clone()];
    let mut index = crate::TerminalSpatialIndex::default();
    index.ensure_current(&map, &stations);
    let middle = TileCoord::new(4, 1);
    assert!(std::ptr::eq(
        station_at_tile_indexed(&map, &stations, middle, &index).expect("tie"),
        &raw const stations[0]
    ));
    let mut scenarios = Vec::new();
    scenarios.push(stations.clone());
    stations.reverse();
    scenarios.push(stations.clone());
    stations[0].stop_kind = StopKind::BusStop;
    scenarios.push(stations.clone());
    stations[0].joined_tiles.push(middle);
    scenarios.push(stations.clone());
    stations[0].joined_tiles.clear();
    stations[0].airport_tiles.push(middle);
    scenarios.push(stations.clone());
    scenarios.push(vec![left.clone(), left.clone(), right.clone()]);
    scenarios.push(vec![right]);
    scenarios.push(vec![rail_station(
        TileCoord::new(5, 1),
        StopKind::RailStation,
    )]);
    scenarios.push(Vec::new());
    for stations in scenarios {
        for y in -1..4 {
            for x in -1..10 {
                assert_lookup_matches(&map, &stations, &index, TileCoord::new(x, y));
            }
        }
    }
    assert_eq!(index.rebuilds(), 1);
}

#[test]
fn indexed_lookup_falls_back_after_demolition_map_clone_and_bounded_geometry_changes() {
    let mut map = Map::new_flat(24, 24, 0);
    for y in 0..24 {
        for x in 0..24 {
            map.set_kind(TileCoord::new(x, y), TileKind::Station)
                .expect("fixture");
        }
    }
    let stations = vec![
        rail_station(TileCoord::new(12, 12), StopKind::RailStation),
        rail_station(TileCoord::new(0, 0), StopKind::RailWaypoint),
    ];
    let mut index = crate::TerminalSpatialIndex::default();
    index.ensure_current(&map, &stations);
    let check = |map: &Map, index: &crate::TerminalSpatialIndex| {
        for y in 0..24 {
            for x in 0..24 {
                assert_lookup_matches(map, &stations, index, TileCoord::new(x, y));
            }
        }
    };
    check(&map, &index);
    let clone = map.clone();
    assert!(!index.is_bound_to_map(&clone));
    check(&clone, &index);
    for x in 0..24 {
        map.set_kind(TileCoord::new(x, 12), TileKind::Grass)
            .expect("demolition");
    }
    assert!(!index.is_bound_to_map(&map));
    check(&map, &index);
    index.ensure_current(&map, &stations);
    assert!(index.is_bound_to_map(&map));
    check(&map, &index);
    let mut tile = map.get(TileCoord::new(1, 1)).expect("tile");
    tile.m2 = 99;
    map.set_tile(TileCoord::new(1, 1), tile)
        .expect("different native id");
    assert!(!index.is_bound_to_map(&map));
    check(&map, &index);
    let replacement = Map::new_flat(24, 24, 0);
    assert!(!index.is_bound_to_map(&replacement));
    check(&replacement, &index);
}
