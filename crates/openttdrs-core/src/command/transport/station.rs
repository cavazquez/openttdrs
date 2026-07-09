use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::{
    station_entrance_faces_rail, station_entrance_faces_road, station_site_tile_allows_build,
    station_site_tile_needs_clear,
};
use crate::station::is_rail_waypoint_tile;
use crate::{
    CLEAR_TILE_COST, GameState, STATION_BUILD_COST, Station, StopKind, WAYPOINT_BUILD_COST,
};

use super::super::CommandError;
use crate::town::{self, authority_allows_new_station};

#[allow(unused_imports)]
use crate::command::transport::internal::{
    RAIL_TB_X, RAIL_TB_Y, check_in_bounds, connect_road_stop, rail_axis_y_from_trackbits,
    road_stop_m5,
};

pub(crate) fn check_station_placement(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceStationOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        k if !station_site_tile_allows_build(k) => {
            Err(CommandError::CannotPlaceStationOnOccupiedTile)
        }
        _ => {
            let entrance_ok = if stop_kind == StopKind::RailStation {
                station_entrance_faces_rail(map, c, dir)
            } else {
                station_entrance_faces_road(map, c, dir)
            };
            if entrance_ok {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
    }
}

pub(in crate::command) fn place_station(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    if state.stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    let dir = (0..4).find(|&d| {
        check_station_placement(&state.map, &state.stations, c, d, StopKind::TruckStop).is_ok()
    });
    let Some(dir) = dir else {
        return Err(CommandError::StationNotAdjacentToTransport);
    };
    station_placement_on_tile(state, c, dir, StopKind::TruckStop)
}

pub(in crate::command) fn place_station_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    place_stop_kind(state, c, dir, StopKind::TruckStop)
}

pub(in crate::command::transport) fn ottd_station_type_bits(stop_kind: StopKind) -> u8 {
    match stop_kind {
        StopKind::RailStation => 0,
        StopKind::Airport => 1,
        StopKind::TruckStop => 2,
        StopKind::BusStop => 3,
        StopKind::Dock => 4,
        StopKind::RailWaypoint => 7,
    }
}

pub(in crate::command::transport) fn apply_station_m6(m6: u8, stop_kind: StopKind) -> u8 {
    (m6 & !0x78) | (ottd_station_type_bits(stop_kind) << 3)
}

pub(in crate::command::transport) fn rail_station_gfx_from_axis(axis_y: bool) -> u8 {
    if axis_y { 3 } else { 2 }
}

pub(in crate::command::transport) fn rail_station_m5(map: &Map, c: TileCoord, dir: u8) -> u8 {
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if let Some(t) = map.get(n)
            && t.kind == TileKind::Rail
        {
            return rail_station_gfx_from_axis(rail_axis_y_from_trackbits(t.m5));
        }
    }
    rail_station_gfx_from_axis(dir.is_multiple_of(2))
}

pub(in crate::command) fn place_rail_station(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_station_placement(&state.map, &state.stations, c, dir, StopKind::RailStation)?;
    station_placement_on_tile(state, c, dir, StopKind::RailStation)
}

#[must_use]
pub const fn rail_station_footprint(axis_y: bool, platforms: u8, length: u8) -> (i32, i32) {
    let p = platforms as i32;
    let l = length as i32;
    if axis_y { (p, l) } else { (l, p) }
}

/// Layout gfx base (sin bit de eje) por andén×longitud — `station_cmd` de `OpenTTD`.
#[must_use]
pub fn rail_station_layout(platforms: usize, length: usize) -> Vec<u8> {
    fn single(row: &mut [u8]) {
        row.fill(0);
        row[(row.len() - 1) / 2] = 2;
    }
    fn multi(row: &mut [u8], b: u8) {
        row.fill(b);
        if row.len() > 4 {
            row[0] = 0;
            row[row.len() - 1] = 0;
        }
    }
    let mut layout = vec![0u8; platforms * length];
    if length == 1 {
        single(&mut layout);
        return layout;
    }
    let mut start = 0;
    let mut remaining = platforms;
    if remaining % 2 == 1 {
        single(&mut layout[start..start + length]);
        start += length;
        remaining -= 1;
    }
    while remaining > 0 {
        multi(&mut layout[start..start + length], 4);
        multi(&mut layout[start + length..start + 2 * length], 6);
        start += 2 * length;
        remaining -= 2;
    }
    layout
}

pub(in crate::command) fn check_rail_station_area(
    state: &GameState,
    origin: TileCoord,
    w: i32,
    h: i32,
) -> Result<(), CommandError> {
    for dy in 0..h {
        for dx in 0..w {
            let c = TileCoord::new(origin.x + dx, origin.y + dy);
            check_in_bounds(&state.map, c)?;
            if state.stations.iter().any(|s| s.pos == c) {
                return Err(CommandError::StationAlreadyExists);
            }
            match state.map.get_kind(c).unwrap_or(TileKind::Grass) {
                TileKind::Water => return Err(CommandError::CannotPlaceStationOnWater),
                TileKind::Void => return Err(CommandError::CannotPlaceStationOnVoid),
                k if !station_site_tile_allows_build(k) => {
                    return Err(CommandError::CannotPlaceStationOnOccupiedTile);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(in crate::command) fn place_rail_station_area(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    platforms: u8,
    length: u8,
) -> Result<(), CommandError> {
    let platforms = platforms.clamp(1, 7);
    let length = length.clamp(1, 7);
    let (w, h) = rail_station_footprint(axis_y, platforms, length);
    check_rail_station_area(state, origin, w, h)?;
    let anchor = TileCoord::new(origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
    if !authority_allows_new_station(&state.towns, anchor) {
        return Err(CommandError::AuthorityRatingTooLow);
    }

    let layout = rail_station_layout(usize::from(platforms), usize::from(length));
    for n in 0..platforms {
        for l in 0..length {
            let c = if axis_y {
                TileCoord::new(origin.x + i32::from(n), origin.y + i32::from(l))
            } else {
                TileCoord::new(origin.x + i32::from(l), origin.y + i32::from(n))
            };
            let idx = usize::from(n) * usize::from(length) + usize::from(l);
            let gfx = layout[idx] + u8::from(axis_y);
            if station_site_tile_needs_clear(state.map.get_kind(c).unwrap_or(TileKind::Grass)) {
                clear_station_site_tile(state, c)?;
            }
            let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            tile.kind = TileKind::Station;
            tile.mapt = 0x50;
            tile.m5 = gfx;
            tile.m6 = apply_station_m6(tile.m6, StopKind::RailStation);
            state
                .map
                .set_tile(c, tile)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.economy.money -= STATION_BUILD_COST;
        }
    }

    let anchor = TileCoord::new(origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
    let mut st = Station::new_with_kind(anchor, StopKind::RailStation);
    st.owner = state.active_company;
    state.stations.push(st);
    if let Some((town_id, delta)) =
        town::apply_station_build_rating_penalty(&mut state.towns, anchor)
    {
        state
            .pending_sim_events
            .push(crate::sim_events::SimEvent::TownRatingChanged { town_id, delta });
    }
    Ok(())
}

pub(in crate::command::transport) fn clear_station_site_tile(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= CLEAR_TILE_COST;
    Ok(())
}

pub(in crate::command::transport) fn station_placement_on_tile(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    if !authority_allows_new_station(&state.towns, c) {
        return Err(CommandError::AuthorityRatingTooLow);
    }
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    if station_site_tile_needs_clear(kind) {
        clear_station_site_tile(state, c)?;
    }
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.kind = TileKind::Station;
    tile.mapt = 0x50;
    tile.m5 = if stop_kind == StopKind::RailStation {
        rail_station_m5(&state.map, c, dir)
    } else {
        road_stop_m5(dir)
    };
    tile.m6 = apply_station_m6(tile.m6, stop_kind);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    if matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        connect_road_stop(state, c, dir)?;
    }
    let mut st = Station::new_with_kind(c, stop_kind);
    st.owner = state.active_company;
    state.stations.push(st);
    state.economy.money -= STATION_BUILD_COST;
    if let Some((town_id, delta)) = town::apply_station_build_rating_penalty(&mut state.towns, c) {
        state
            .pending_sim_events
            .push(crate::sim_events::SimEvent::TownRatingChanged { town_id, delta });
    }
    Ok(())
}

pub(in crate::command) fn place_stop_kind(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    check_station_placement(&state.map, &state.stations, c, dir, stop_kind)?;
    station_placement_on_tile(state, c, dir, stop_kind)
}

pub(in crate::command::transport) fn rail_waypoint_axis_from_trackbits(tb: u8) -> Option<bool> {
    match tb & 0x3F {
        RAIL_TB_X => Some(false),
        RAIL_TB_Y => Some(true),
        _ => None,
    }
}

pub(crate) fn check_place_rail_waypoint(
    map: &Map,
    c: TileCoord,
    stations: &[Station],
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    match tile.kind {
        TileKind::Rail => {
            rail_waypoint_axis_from_trackbits(tile.m5)
                .ok_or(CommandError::CannotPlaceWaypointOnTrack)?;
            Ok(())
        }
        TileKind::Station if is_rail_waypoint_tile(&tile) => {
            Err(CommandError::StationAlreadyExists)
        }
        _ => Err(CommandError::CannotPlaceWaypointOnTrack),
    }
}

pub(in crate::command) fn place_rail_waypoint(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_rail_waypoint(&state.map, c, &state.stations)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let axis_y = rail_waypoint_axis_from_trackbits(tile.m5).unwrap_or(false);
    let mut out = tile;
    out.kind = TileKind::Station;
    out.mapt = 0x50;
    out.m5 = u8::from(axis_y);
    out.m6 = apply_station_m6(out.m6, StopKind::RailWaypoint);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    let mut st = Station::new_with_kind(c, StopKind::RailWaypoint);
    st.owner = state.active_company;
    state.stations.push(st);
    state.economy.money -= WAYPOINT_BUILD_COST;
    Ok(())
}
