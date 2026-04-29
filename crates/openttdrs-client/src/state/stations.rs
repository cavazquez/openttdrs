//! Helpers de estaciones para bootstrap de mapas.

use openttdrs_core::{GameState, OttdmapExtras, Station, TileCoord, TileKind};
use std::collections::HashSet;

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
            state.stations.push(Station::new(c));
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
                state.stations.push(Station::new(c));
            }
        }
    }
}
