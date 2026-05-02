use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::station_site_adjacent_to_transport;
use crate::{
    CLEAR_TILE_COST, DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST, ROAD_BUILD_COST,
    STATION_BUILD_COST, Station, StopKind,
};

use super::{CommandError, in_bounds};

pub(super) fn place_road(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    place_road_bits(state, c, 0x05)
}

pub(super) fn transport_tile_is_buildable(kind: TileKind) -> bool {
    !matches!(kind, TileKind::Water | TileKind::Void)
}

pub(super) fn build_error_for_kind(kind: TileKind) -> CommandError {
    match kind {
        TileKind::Water => CommandError::CannotPlaceRoadOnWater,
        TileKind::Void => CommandError::CannotPlaceRoadOnVoid,
        _ => CommandError::OutOfBounds,
    }
}

pub(super) fn place_single_transport_tile(
    state: &mut GameState,
    c: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost: i64,
) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    if !transport_tile_is_buildable(kind) {
        return Err(build_error_for_kind(kind));
    }
    state
        .map
        .set_kind(c, kind_to_place)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, mapt, m5)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= cost;
    Ok(())
}

pub(super) fn place_road_depot_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    place_single_transport_tile(
        state,
        c,
        TileKind::RoadDepot,
        0x20,
        0x20 | dir,
        DEPOT_BUILD_COST,
    )?;
    if let Some((exit, road_bits)) = road_depot_exit_for_dir(&state.map, c, dir) {
        let _ = place_road_bits(state, exit, road_bits);
    }
    Ok(())
}

pub(super) fn road_depot_exit_for_dir(
    map: &Map,
    depot_pos: TileCoord,
    dir: u8,
) -> Option<(TileCoord, u8)> {
    let ((dx, dy), road_bits) = match dir & 0x03 {
        0 => ((-1_i32, 0_i32), 0x02),
        1 => ((0_i32, 1_i32), 0x01),
        2 => ((1_i32, 0_i32), 0x08),
        _ => ((0_i32, -1_i32), 0x04),
    };
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some((c, road_bits))
}

fn axis_line(a: TileCoord, b: TileCoord) -> Vec<TileCoord> {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        let step = if b.x >= a.x { 1 } else { -1 };
        let mut out = Vec::new();
        let mut x = a.x;
        loop {
            out.push(TileCoord::new(x, a.y));
            if x == b.x {
                break;
            }
            x += step;
        }
        out
    } else {
        let step = if b.y >= a.y { 1 } else { -1 };
        let mut out = Vec::new();
        let mut y = a.y;
        loop {
            out.push(TileCoord::new(a.x, y));
            if y == b.y {
                break;
            }
            y += step;
        }
        out
    }
}

pub(super) fn place_tunnel_or_bridge(
    state: &mut GameState,
    a: TileCoord,
    b: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost_per_tile: i64,
) -> Result<(), CommandError> {
    let line = axis_line(a, b);
    if line.len() < 2 {
        return Err(CommandError::OutOfBounds);
    }
    for c in &line {
        in_bounds(&state.map, *c)?;
        let kind = state.map.get_kind(*c).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(kind) {
            return Err(build_error_for_kind(kind));
        }
    }
    let cost = cost_per_tile * i64::try_from(line.len()).unwrap_or(i64::MAX);
    for c in line {
        state
            .map
            .set_kind(c, kind_to_place)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(c, mapt, m5)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= cost;
    Ok(())
}

pub(super) fn place_road_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => {
            let existing = state.map.get(c).map_or(0, |t| {
                if t.kind == TileKind::Road {
                    t.m5 & 0x0F
                } else {
                    0
                }
            });
            let road_bits = (existing | (bits & 0x0F)).max(0x01);
            write_normal_road_tile(state, c, road_bits)?;
            state.economy.money -= ROAD_BUILD_COST;
            Ok(())
        }
    }
}

pub(super) fn set_road_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => {
            let road_bits = (bits & 0x0F).max(0x01);
            write_normal_road_tile(state, c, road_bits)?;
            state.economy.money -= ROAD_BUILD_COST;
            Ok(())
        }
    }
}

fn write_normal_road_tile(
    state: &mut GameState,
    c: TileCoord,
    road_bits: u8,
) -> Result<(), CommandError> {
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.kind = TileKind::Road;
    // MP_ROAD normal tile: low nibble stores road bits, high bits subtype=0.
    tile.mapt = 0x20;
    tile.m5 = road_bits & 0x0F;
    tile.m1 = 0;
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)
}

pub(super) fn place_station(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    place_stop_kind(state, c, 0, StopKind::TruckStop)
}

pub(super) fn place_station_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    place_stop_kind(state, c, dir, StopKind::TruckStop)
}

pub(super) fn place_stop_kind(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    if state.stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceStationOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => {
            if !station_site_adjacent_to_transport(&state.map, c) {
                return Err(CommandError::StationNotAdjacentToTransport);
            }
            state
                .map
                .set_kind(c, TileKind::Station)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(c, 0x50, dir & 0x03)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.stations.push(Station::new_with_kind(c, stop_kind));
            state.economy.money -= STATION_BUILD_COST;
            Ok(())
        }
    }
}

pub(super) fn place_rail(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        _ => {
            state
                .map
                .set_kind(c, TileKind::Rail)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.economy.money -= RAIL_BUILD_COST;
            Ok(())
        }
    }
}

pub(super) fn clear_tile(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    if let Some(industry_idx) = state.industries.iter().position(|i| i.contains_tile(c)) {
        let industry_tiles = state.industries[industry_idx].tiles.clone();
        for tile in industry_tiles {
            state
                .map
                .set_kind(tile, TileKind::Grass)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(tile, 0x00, 0x00)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        state.industries.remove(industry_idx);
        state.economy.money -= CLEAR_TILE_COST;
        return Ok(());
    }
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.stations.retain(|s| s.pos != c);
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    state.economy.money -= CLEAR_TILE_COST;
    Ok(())
}
