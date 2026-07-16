//! Helpers de estaciones para bootstrap de mapas.

use openttdrs_core::OttdmapExtras;
use openttdrs_core::prelude::*;
use std::collections::HashSet;

use openttdrs_core::stop_kind_from_m6;

fn push_station_from_tile(state: &mut GameState, c: TileCoord) {
    let kind = state
        .map
        .get(c)
        .map(|t| stop_kind_from_m6(t.m6))
        .unwrap_or_default();
    state.stations.push(Station::new_with_kind(c, kind));
}

/// Anade [`Station`] en coordenadas del footer `STXY` (export `parse_sav.py`), deduplicando.
pub(crate) fn place_stations_from_footer_stxy(
    state: &mut GameState,
    extras: Option<&OttdmapExtras>,
) {
    let Some(ex) = extras else {
        return;
    };
    if ex.station_xy.is_empty() {
        return;
    }
    let (mw, mh) = state.map.dimensions();
    let mut seen: HashSet<(i32, i32)> = state.stations.iter().map(|s| (s.pos.x, s.pos.y)).collect();
    for &(x, y) in &ex.station_xy {
        let xi = i32::from(x);
        let yi = i32::from(y);
        if xi < 0 || yi < 0 || xi >= mw as i32 || yi >= mh as i32 {
            continue;
        }
        let c = TileCoord::new(xi, yi);
        let key = (c.x, c.y);
        if seen.insert(key) {
            push_station_from_tile(state, c);
        }
    }
}

/// Anade [`Station`] por teselas `MP_STATION` del mapa (deduplica con estaciones ya creadas).
pub(crate) fn place_stations_from_map_tiles(state: &mut GameState) {
    let (mw, mh) = state.map.dimensions();
    let mut seen: HashSet<(i32, i32)> = state.stations.iter().map(|s| (s.pos.x, s.pos.y)).collect();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if state.map.get_kind(c) != Some(TileKind::Station) {
                continue;
            }
            let key = (c.x, c.y);
            if seen.insert(key) {
                push_station_from_tile(state, c);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod stations_coverage_tests {
    use super::{place_stations_from_footer_stxy, place_stations_from_map_tiles};
    use openttdrs_core::OttdmapExtras;
    use openttdrs_core::prelude::*;

    #[test]
    fn place_stations_from_footer_none_is_noop() {
        let mut state = GameState::new(4, 4);
        place_stations_from_footer_stxy(&mut state, None);
        assert!(state.stations.is_empty());
    }

    #[test]
    fn place_stations_from_footer_empty_stxy_is_noop() {
        let mut state = GameState::new(4, 4);
        let ex = OttdmapExtras::default();
        place_stations_from_footer_stxy(&mut state, Some(&ex));
    }

    #[test]
    fn place_stations_from_map_tiles_runs() {
        let mut state = GameState::new(4, 4);
        place_stations_from_map_tiles(&mut state);
    }

    #[test]
    fn place_stations_from_footer_dedups_and_skips_oob() {
        let mut state = GameState::new(4, 4);
        state.stations.push(Station::new(TileCoord::new(1, 1)));
        let ex = OttdmapExtras {
            station_xy: vec![(1, 1), (2, 2), (2, 2), (9, 9)],
            ..OttdmapExtras::default()
        };
        place_stations_from_footer_stxy(&mut state, Some(&ex));
        assert_eq!(state.stations.len(), 2);
        assert!(state.stations.iter().any(|s| s.pos == TileCoord::new(2, 2)));
    }

    #[test]
    fn place_stations_from_map_tiles_adds_once_per_tile() {
        let mut state = GameState::new(4, 4);
        assert!(
            state
                .map
                .set_kind(TileCoord::new(0, 0), TileKind::Station)
                .is_ok()
        );
        assert!(
            state
                .map
                .set_kind(TileCoord::new(1, 0), TileKind::Station)
                .is_ok()
        );
        state.stations.push(Station::new(TileCoord::new(0, 0)));

        place_stations_from_map_tiles(&mut state);
        assert_eq!(state.stations.len(), 2);
        assert!(state.stations.iter().any(|s| s.pos == TileCoord::new(1, 0)));
    }

    #[test]
    fn place_stations_from_map_tiles_uses_m6_for_rail_station() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(2, 2);
        state.map.set_kind(c, TileKind::Station).unwrap();
        let mut t = state.map.get(c).unwrap();
        t.m6 = 0;
        state.map.set_tile(c, t).unwrap();
        place_stations_from_map_tiles(&mut state);
        assert_eq!(state.stations.len(), 1);
        assert_eq!(
            state.stations[0].stop_kind,
            openttdrs_core::StopKind::RailStation
        );
    }
}
