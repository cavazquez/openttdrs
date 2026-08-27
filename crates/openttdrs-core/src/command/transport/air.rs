//! Construcción aérea: helipuerto y aeropuertos por spec.

use crate::airport::{
    AirportPiece, airport_m6_airport, airport_spec_footprint, airport_spec_tiles,
    newgrf_airport_footprint, newgrf_airport_tiles,
};
use crate::airport_class::{AirportSpecId, newgrf_airport_spec_def};
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
    let (w, h) = if let Some(id) = state.current_airport_newgrf_id
        && let Some(def) = newgrf_airport_spec_def(&state.airport_spec_catalog, id)
    {
        newgrf_airport_footprint(def, axis_y)
    } else {
        airport_spec_footprint(spec, axis_y)
    };
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

/// Aeropuerto según [`AirportSpecId`] o layout `NewGRF` activo.
pub(in crate::command) fn place_airport_area(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    check_airport_area(state, origin, axis_y, spec)?;

    let newgrf_id = state.current_airport_newgrf_id;
    let newgrf_def = newgrf_id.and_then(|id| {
        state
            .airport_spec_catalog
            .iter()
            .find(|d| d.id == id && d.enabled)
            .cloned()
    });
    let place_spec = newgrf_def.as_ref().map_or(spec, |d| d.subst_id);

    let airport_tile_gfx = newgrf_def.as_ref().map_or_else(Vec::new, |def| {
        crate::airport::newgrf_airport_tile_gfx(
            origin,
            def,
            &state.airport_tile_spec_catalog,
            axis_y,
        )
    });
    let placed: Vec<(TileCoord, AirportPiece)> = if let Some(ref def) = newgrf_def {
        newgrf_airport_tiles(origin, def, &state.airport_tile_spec_catalog, axis_y)
    } else {
        airport_spec_tiles(origin, place_spec, axis_y).collect()
    };

    let station_anchor = placed
        .iter()
        .find(|(_, p)| p.is_hangar())
        .map_or(origin, |(c, _)| *c);
    if !authority_allows_new_station(&state.towns, station_anchor, state.active_company) {
        return Err(CommandError::AuthorityRatingTooLow);
    }

    let noise_spec = place_spec;
    let noise_level_override = newgrf_def.as_ref().map(|d| d.noise_level);
    let noise_add = airport_noise_contribution_with_level(
        state,
        station_anchor,
        noise_spec,
        noise_level_override,
    )?;

    let tile_count = placed.len();
    let mut tiles = Vec::with_capacity(tile_count);
    for (c, piece) in placed {
        if station_site_tile_needs_clear(state.map.get_kind(c).unwrap_or(TileKind::Grass)) {
            clear_station_site_tile(state, c)?;
        }
        write_airport_tile(state, c, piece)?;
        tiles.push(c);
    }
    if matches!(place_spec, AirportSpecId::Heliport | AirportSpecId::Oilrig) && newgrf_def.is_none()
    {
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
    st.airport_tile_gfx = airport_tile_gfx;
    st.airport_spec = place_spec;
    st.airport_newgrf_spec_id = newgrf_id.filter(|_| newgrf_def.is_some());
    // La API de construcción conserva el eje como orientación geométrica.
    // Usar las rotaciones cardinales canónicas de OpenTTD mantiene la
    // distinción X/Y en SAV aunque todavía no se exponga el cuarto giro en UI.
    st.airport_rotation = if axis_y { 2 } else { 0 };
    st.airport_blocks = 0;
    // Catchment: `station_catchment_radius` lee `airport_spec` en cobertura.
    state.stations.push(st);
    if newgrf_def.is_some() {
        let dirty = crate::map::trigger_newgrf_airport_animation_for_station(
            &mut state.map,
            state.tick.get(),
            &mut state.stations,
            state.climate,
            &state.airport_tile_spec_catalog,
            &mut state.newgrf_animated_airport_tiles,
            &state.newgrf_stack,
            station_anchor,
            crate::AirportAnimationTrigger::Built,
            0,
        );
        state.runtime.industry_tile_dirty.extend(dirty);
    }
    Ok(())
}

/// Contribución de ruido al pueblo más cercano (`GetAirportNoiseLevelForDistance`).
///
/// Con `station_noise_level` activo, rechaza si supera `MaxTownNoise`.
fn airport_noise_contribution_with_level(
    state: &GameState,
    airport_pos: TileCoord,
    spec: AirportSpecId,
    noise_override: Option<u8>,
) -> Result<Option<(usize, u8)>, CommandError> {
    use crate::airport_class::{
        TOWN_NOISE_POPULATION_DEFAULT, airport_noise_for_distance, airport_spec_def, max_town_noise,
    };
    use crate::town::nearest_town_index;

    let Some((town_idx, dist)) = nearest_town_index(&state.towns, airport_pos) else {
        return Ok(None);
    };
    let noise_level =
        noise_override.unwrap_or_else(|| airport_spec_def(spec).map_or(0, |d| d.noise_level));
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
