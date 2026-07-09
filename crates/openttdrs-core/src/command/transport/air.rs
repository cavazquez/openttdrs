//! Construcción aérea: helipuerto 1×1 y aeropuerto small 4×3.

use crate::airport::{
    AirportPiece, airport_m6_airport, airport_small_footprint, airport_small_tiles,
};
use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::{station_site_tile_allows_build, station_site_tile_needs_clear};
use crate::town::authority_allows_new_station;
use crate::{DEPOT_BUILD_COST, GameState, STATION_BUILD_COST, Station, StopKind};

use super::super::CommandError;
use super::shared::check_in_bounds;
use super::station::clear_station_site_tile;

pub(crate) fn check_airport_placement(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if stations.iter().any(|s| s.covers_tile(c)) {
        return Err(CommandError::StationAlreadyExists);
    }
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceStationOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        k if !station_site_tile_allows_build(k) => {
            Err(CommandError::CannotPlaceStationOnOccupiedTile)
        }
        _ => Ok(()),
    }
}

/// Helipuerto 1×1: tesela `Airport` + estación `StopKind::Airport` (compra + carga).
pub(in crate::command) fn place_airport(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_airport_placement(&state.map, &state.stations, c)?;
    if !authority_allows_new_station(&state.towns, c) {
        return Err(CommandError::AuthorityRatingTooLow);
    }
    write_airport_tile(state, c, AirportPiece::Heliport)?;
    let mut st = Station::new_with_kind(c, StopKind::Airport);
    st.airport_tiles = vec![c];
    state.stations.push(st);
    state.economy.money -= DEPOT_BUILD_COST;
    Ok(())
}

pub(crate) fn check_airport_area(
    state: &GameState,
    origin: TileCoord,
    axis_y: bool,
) -> Result<(), CommandError> {
    let (w, h) = airport_small_footprint(axis_y);
    let h0 = state.map.get(origin).map_or(0, |t| t.height);
    for dy in 0..h {
        for dx in 0..w {
            let c = TileCoord::new(origin.x + dx, origin.y + dy);
            check_airport_placement(&state.map, &state.stations, c)?;
            let hc = state.map.get(c).map_or(0, |t| t.height);
            if hc != h0 {
                return Err(CommandError::CannotPlaceStationOnOccupiedTile);
            }
        }
    }
    Ok(())
}

/// Aeropuerto small 4×3: hangar + pista + apron.
pub(in crate::command) fn place_airport_area(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
) -> Result<(), CommandError> {
    check_airport_area(state, origin, axis_y)?;
    let hangar = airport_small_tiles(origin, axis_y)
        .find(|(_, p)| *p == AirportPiece::Hangar)
        .map(|(c, _)| c)
        .ok_or(CommandError::OutOfBounds)?;
    if !authority_allows_new_station(&state.towns, hangar) {
        return Err(CommandError::AuthorityRatingTooLow);
    }

    let mut tiles = Vec::with_capacity(12);
    for (c, piece) in airport_small_tiles(origin, axis_y) {
        if station_site_tile_needs_clear(state.map.get_kind(c).unwrap_or(TileKind::Grass)) {
            clear_station_site_tile(state, c)?;
        }
        write_airport_tile(state, c, piece)?;
        tiles.push(c);
        state.economy.money -= STATION_BUILD_COST;
    }
    let mut st = Station::new_with_kind(hangar, StopKind::Airport);
    st.airport_tiles = tiles;
    state.stations.push(st);
    Ok(())
}

fn write_airport_tile(
    state: &mut GameState,
    c: TileCoord,
    piece: AirportPiece,
) -> Result<(), CommandError> {
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.kind = TileKind::Airport;
    tile.mapt = 0x50;
    tile.m5 = piece as u8;
    tile.m6 = airport_m6_airport(tile.m6);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    Ok(())
}
