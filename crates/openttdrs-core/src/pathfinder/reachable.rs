//! BFS de tesela más lejana alcanzable (destinos iniciales road/tram desde depósito).

use std::collections::{HashSet, VecDeque};

use crate::map::{Map, TileCoord, TileKind};

use super::network::{PathNetwork, is_network_tile, tiles_connected};

/// Tesela de red road/tram más lejana (Manhattan) desde `start`.
///
/// - [`PathNetwork::Road`]: solo kind `Road|RoadBridge|RoadTunnel` (sin bits). `None` si `start` inválido.
/// - [`PathNetwork::Tram`]: conectividad vía `tiles_connected`; siempre `Some` (mínimo `start`).
/// - Otras redes: `None`.
#[must_use]
pub fn farthest_reachable_tile(
    map: &Map,
    start: TileCoord,
    network: PathNetwork,
) -> Option<TileCoord> {
    match network {
        PathNetwork::Road => farthest_reachable_road(map, start),
        PathNetwork::Tram => Some(farthest_reachable_tram(map, start)),
        PathNetwork::Rail | PathNetwork::Water | PathNetwork::Air => None,
    }
}

fn traversable_road_kind(kind: Option<TileKind>) -> bool {
    matches!(
        kind,
        Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel)
    )
}

fn tile_distance(a: TileCoord, b: TileCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

fn farthest_reachable_road(map: &Map, start: TileCoord) -> Option<TileCoord> {
    let (mw, mh) = map.dimensions();
    if !traversable_road_kind(map.get_kind(start)) {
        return None;
    }
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([start]);
    let mut farthest = start;
    seen.insert(start);

    while let Some(cur) = queue.pop_front() {
        if tile_distance(cur, start) > tile_distance(farthest, start) {
            farthest = cur;
        }
        for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw.cast_signed() || next.y >= mh.cast_signed()
            {
                continue;
            }
            if seen.insert(next) && traversable_road_kind(map.get_kind(next)) {
                queue.push_back(next);
            }
        }
    }

    Some(farthest)
}

fn farthest_reachable_tram(map: &Map, start: TileCoord) -> TileCoord {
    let (mw, mh) = map.dimensions();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([start]);
    let mut farthest = start;
    seen.insert(start);

    while let Some(cur) = queue.pop_front() {
        if tile_distance(cur, start) > tile_distance(farthest, start) {
            farthest = cur;
        }
        for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw.cast_signed() || next.y >= mh.cast_signed()
            {
                continue;
            }
            if !seen.insert(next) {
                continue;
            }
            let next_kind = map.get_kind(next).unwrap_or(TileKind::Grass);
            if is_network_tile(map, next, next_kind, PathNetwork::Tram)
                && tiles_connected(map, cur, next, PathNetwork::Tram)
            {
                queue.push_back(next);
            }
        }
    }

    farthest
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::road_type::{RoadType, set_tram_road_type_on_tile, set_tram_track_bits_on_tile};

    fn write_road(m: &mut Map, c: TileCoord, bits: u8) {
        m.set_kind(c, TileKind::Road).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = bits & 0x0F;
        m.set_tile(c, t).unwrap();
    }

    fn write_tram(m: &mut Map, c: TileCoord, bits: u8) {
        m.set_kind(c, TileKind::Road).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = 0;
        t = set_tram_track_bits_on_tile(t, bits);
        t = set_tram_road_type_on_tile(t, Some(RoadType::Tram));
        m.set_tile(c, t).unwrap();
    }

    #[test]
    fn road_farthest_on_straight_line() {
        let mut m = Map::new_flat(8, 8, 0);
        for x in 1..=5_i32 {
            write_road(&mut m, TileCoord::new(x, 2), 0x0A);
        }
        let far = farthest_reachable_tile(&m, TileCoord::new(1, 2), PathNetwork::Road);
        assert_eq!(far, Some(TileCoord::new(5, 2)));
    }

    #[test]
    fn road_none_on_grass_start() {
        let m = Map::new_flat(4, 4, 0);
        assert!(farthest_reachable_tile(&m, TileCoord::new(1, 1), PathNetwork::Road).is_none());
    }

    #[test]
    fn tram_does_not_cross_road_without_m3() {
        let mut m = Map::new_flat(6, 6, 0);
        write_tram(&mut m, TileCoord::new(1, 1), 0x0A);
        write_tram(&mut m, TileCoord::new(2, 1), 0x0A);
        write_road(&mut m, TileCoord::new(3, 1), 0x0A); // solo road, sin m3
        let far = farthest_reachable_tile(&m, TileCoord::new(1, 1), PathNetwork::Tram)
            .expect("tram start");
        assert_eq!(far, TileCoord::new(2, 1));
    }
}
