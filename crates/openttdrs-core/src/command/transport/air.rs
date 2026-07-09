//! Construcción aérea: helipuerto / aeropuerto 1×1.

use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::station_site_tile_allows_build;
use crate::{DEPOT_BUILD_COST, GameState, Station, StopKind};

use super::super::CommandError;
use super::shared::check_in_bounds;

pub(crate) fn check_airport_placement(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
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
        _ => Ok(()),
    }
}

/// Helipuerto 1×1: tesela `Airport` + estación `StopKind::Airport` (compra + carga).
pub(in crate::command) fn place_airport(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_airport_placement(&state.map, &state.stations, c)?;
    state
        .map
        .set_kind(c, TileKind::Airport)
        .map_err(|_| CommandError::OutOfBounds)?;
    // mapt estación-like; m5=0 heliport; m6 tipo Airport (=1).
    state
        .map
        .set_mapt_m5(c, 0x50, 0)
        .map_err(|_| CommandError::OutOfBounds)?;
    if let Some(mut tile) = state.map.get(c) {
        tile.m6 = (tile.m6 & !0x78) | (1 << 3);
        let _ = state.map.set_tile(c, tile);
    }
    state
        .stations
        .push(Station::new_with_kind(c, StopKind::Airport));
    state.economy.money -= DEPOT_BUILD_COST;
    Ok(())
}
