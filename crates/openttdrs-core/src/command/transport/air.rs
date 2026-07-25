//! Construcción aérea: helipuerto y aeropuertos por spec.

use crate::airport::{
    AirportPiece, airport_m6_airport, airport_spec_footprint, airport_spec_tiles,
};
use crate::airport_class::AirportSpecId;
use crate::economy::station_build_cost;
use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::{station_site_tile_allows_build, station_site_tile_needs_clear};
use crate::town::authority_allows_new_station;
use crate::{DEPOT_BUILD_COST, GameState, Station, StopKind};

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
    place_airport_area(state, c, false, AirportSpecId::Heliport)
}

pub(crate) fn check_airport_area(
    state: &GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    let (w, h) = airport_spec_footprint(spec, axis_y);
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

/// Aeropuerto según [`AirportSpecId`]: hangar/helipuerto + footprint.
pub(in crate::command) fn place_airport_area(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    check_airport_area(state, origin, axis_y, spec)?;
    let station_anchor = airport_spec_tiles(origin, spec, axis_y)
        .find(|(_, p)| p.is_hangar())
        .map_or(origin, |(c, _)| c);
    if !authority_allows_new_station(&state.towns, station_anchor, state.active_company) {
        return Err(CommandError::AuthorityRatingTooLow);
    }

    let noise_add = airport_noise_contribution(state, station_anchor, spec)?;

    let tile_count = airport_spec_tiles(origin, spec, axis_y).count();
    let mut tiles = Vec::with_capacity(tile_count);
    for (c, piece) in airport_spec_tiles(origin, spec, axis_y) {
        if station_site_tile_needs_clear(state.map.get_kind(c).unwrap_or(TileKind::Grass)) {
            clear_station_site_tile(state, c)?;
        }
        write_airport_tile(state, c, piece)?;
        tiles.push(c);
    }
    if matches!(spec, AirportSpecId::Heliport | AirportSpecId::Oilrig) {
        state.economy.money -= DEPOT_BUILD_COST;
    } else {
        let cost = station_build_cost(&state.global_economy)
            .saturating_mul(i64::try_from(tile_count).unwrap_or(1));
        state.economy.money -= cost;
    }
    if let Some((town_idx, add)) = noise_add {
        state.towns[town_idx].noise_reached = state.towns[town_idx]
            .noise_reached
            .saturating_add(u16::from(add));
    }
    let mut st = Station::new_with_kind(station_anchor, StopKind::Airport);
    st.owner = state.active_company;
    st.airport_tiles = tiles;
    st.airport_spec = spec;
    st.airport_blocks = 0;
    // Catchment: `station_catchment_radius` lee `airport_spec` en cobertura.
    state.stations.push(st);
    Ok(())
}

/// Contribución de ruido al pueblo más cercano (`GetAirportNoiseLevelForDistance`).
///
/// Con `station_noise_level` activo, rechaza si supera `MaxTownNoise`.
fn airport_noise_contribution(
    state: &GameState,
    airport_pos: TileCoord,
    spec: AirportSpecId,
) -> Result<Option<(usize, u8)>, CommandError> {
    use crate::airport_class::{
        TOWN_NOISE_POPULATION_DEFAULT, airport_noise_for_distance, airport_spec_def, max_town_noise,
    };
    use crate::town::nearest_town_index;

    let Some((town_idx, dist)) = nearest_town_index(&state.towns, airport_pos) else {
        return Ok(None);
    };
    let noise_level = airport_spec_def(spec).map_or(0, |d| d.noise_level);
    // Tolerancia permisiva: 8 + 0×4 (sin setting de council tolerance).
    let effective = airport_noise_for_distance(noise_level, dist, 8);
    if state.station_noise_level {
        let town = &state.towns[town_idx];
        let max = max_town_noise(town.population, TOWN_NOISE_POPULATION_DEFAULT);
        let next = u32::from(town.noise_reached).saturating_add(u32::from(effective));
        if next > u32::from(max) {
            return Err(CommandError::AirportNoiseTooHigh);
        }
    }
    Ok(Some((town_idx, effective)))
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
