//! Construcción aérea: helipuerto y aeropuertos por spec.

use crate::airport::{
    AirportPiece, airport_m6_airport, airport_spec_footprint, airport_spec_tiles,
    newgrf_airport_layout_selection_with_index, newgrf_airport_tile_gfx_with_layout,
};
use crate::airport_class::{AirportSpecId, NewgrfAirportSpecDef, newgrf_airport_spec_def};
use crate::economy::station_build_cost;
use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::{station_site_tile_allows_build, station_site_tile_needs_clear};
use crate::town::authority_allows_new_station;
use crate::{DEPOT_BUILD_COST, GameState, Station, StopKind};

use super::super::CommandError;
use super::shared::check_in_bounds;
use super::station::clear_station_site_tile;

/// Spec clonado y selector `(índice Action0, rotación)` de una construcción.
type SelectedNewgrfAirportLayout = (NewgrfAirportSpecDef, (u8, u8));

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

/// Catálogo y layout `NewGRF` activos para una construcción.
///
/// El comando histórico sólo conserva el eje, de modo que `layout` es `None`;
/// el picker moderno entrega el índice Action0 exacto. Al pedir un índice no
/// existente se rechaza en vez de sustituir silenciosamente otra rotación.
fn selected_newgrf_airport_layout(
    state: &GameState,
    axis_y: bool,
    layout: Option<u8>,
    newgrf_spec_id: Option<u16>,
) -> Result<Option<SelectedNewgrfAirportLayout>, CommandError> {
    let Some(id) = newgrf_spec_id.or(state.current_airport_newgrf_id) else {
        return if layout.is_some() {
            Err(CommandError::InvalidAirportLayout)
        } else {
            Ok(None)
        };
    };
    let Some(def) = newgrf_airport_spec_def(&state.airport_spec_catalog, id).cloned() else {
        return if layout.is_some() {
            Err(CommandError::InvalidAirportLayout)
        } else {
            Ok(None)
        };
    };
    let selection = newgrf_airport_layout_selection_with_index(&def, layout, axis_y)
        .ok_or(CommandError::InvalidAirportLayout)?;
    Ok(Some((def, selection)))
}

fn check_airport_tiles(
    state: &GameState,
    tiles: impl IntoIterator<Item = TileCoord>,
) -> Result<(), CommandError> {
    let mut flat_height = None;
    let mut saw_tile = false;
    for c in tiles {
        saw_tile = true;
        check_airport_placement(&state.map, &state.stations, c)?;
        let height = state.map.get(c).map_or(0, |tile| tile.height);
        if let Some(expected) = flat_height {
            if height != expected {
                return Err(CommandError::CannotPlaceStationOnOccupiedTile);
            }
        } else {
            // `CheckFlatLandAirport` toma la primera tesela del iterator
            // Action0 como referencia, no necesariamente el origen del área.
            flat_height = Some(height);
        }
    }
    if saw_tile {
        Ok(())
    } else {
        Err(CommandError::InvalidAirportLayout)
    }
}

fn check_airport_area_with_layout(
    state: &GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
    layout: Option<u8>,
    newgrf_spec_id: Option<u16>,
) -> Result<(), CommandError> {
    if let Some((def, (layout_index, rotation))) =
        selected_newgrf_airport_layout(state, axis_y, layout, newgrf_spec_id)?
    {
        // Igual que `AirportTileTableIterator`, validar sólo las teselas que
        // Action0 declara. El rectángulo tamaño X×Y sirve para selección y
        // station spread, pero no convierte los huecos de un layout en suelo
        // requerido.
        let tiles = newgrf_airport_tile_gfx_with_layout(
            origin,
            &def,
            &[],
            axis_y,
            Some(layout_index),
            Some(rotation),
        )
        .into_iter()
        .map(|(coord, _)| coord);
        return check_airport_tiles(state, tiles);
    }

    let (w, h) = airport_spec_footprint(spec, axis_y);
    check_airport_tiles(
        state,
        (0..h).flat_map(|dy| (0..w).map(move |dx| TileCoord::new(origin.x + dx, origin.y + dy))),
    )
}

pub(crate) fn check_airport_area(
    state: &GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    check_airport_area_with_layout(state, origin, axis_y, spec, None, None)
}

pub(crate) fn check_airport_area_with_explicit_layout(
    state: &GameState,
    origin: TileCoord,
    newgrf_spec_id: u16,
    layout: u8,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    check_airport_area_with_layout(
        state,
        origin,
        false,
        spec,
        Some(layout),
        Some(newgrf_spec_id),
    )
}

/// Gfx y piezas de un layout `NewGRF` ya orientado por Action0.
struct NewgrfAirportTileMapping {
    gfx: Vec<(TileCoord, u16)>,
    pieces: Vec<(TileCoord, AirportPiece)>,
}

fn newgrf_airport_tile_mapping(
    origin: TileCoord,
    axis_y: bool,
    def: &NewgrfAirportSpecDef,
    tile_catalog: &[crate::airport_tile_spec::AirportTileSpecDef],
    layout: Option<(u8, u8)>,
) -> NewgrfAirportTileMapping {
    let (layout_index, rotation) = layout.map_or((None, None), |(index, rotation)| {
        (Some(index), Some(rotation))
    });
    let gfx = newgrf_airport_tile_gfx_with_layout(
        origin,
        def,
        tile_catalog,
        axis_y,
        layout_index,
        rotation,
    );
    let pieces = gfx
        .iter()
        .map(|(coord, gfx)| {
            let piece = AirportPiece::from_station_gfx(
                crate::airport_tile_spec::resolve_airport_tile_piece_gfx(*gfx, tile_catalog),
            );
            (*coord, piece)
        })
        .collect();
    NewgrfAirportTileMapping { gfx, pieces }
}

/// Aeropuerto según [`AirportSpecId`] o layout `NewGRF` activo.
pub(in crate::command) fn place_airport_area(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    place_airport_area_with_layout(state, origin, axis_y, spec, None, None)
}

/// Construye un aeropuerto `NewGRF` usando un índice Action0 explícito.
pub(in crate::command) fn place_airport_area_with_explicit_layout(
    state: &mut GameState,
    origin: TileCoord,
    newgrf_spec_id: u16,
    layout: u8,
    spec: AirportSpecId,
) -> Result<(), CommandError> {
    place_airport_area_with_layout(
        state,
        origin,
        false,
        spec,
        Some(layout),
        Some(newgrf_spec_id),
    )
}

fn place_airport_area_with_layout(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    spec: AirportSpecId,
    layout: Option<u8>,
    newgrf_spec_id: Option<u16>,
) -> Result<(), CommandError> {
    check_airport_area_with_layout(state, origin, axis_y, spec, layout, newgrf_spec_id)?;

    let newgrf_id = newgrf_spec_id.or(state.current_airport_newgrf_id);
    let newgrf = selected_newgrf_airport_layout(state, axis_y, layout, newgrf_spec_id)?;
    let (newgrf_def, newgrf_layout) = newgrf.map_or((None, None), |(def, selection)| {
        (Some(def), Some(selection))
    });
    let place_spec = newgrf_def.as_ref().map_or(spec, |d| d.subst_id);
    // Cada layout Action0 ya contiene sus offsets para su rotación. Conservar
    // el selector elegido evita tanto transponer el footprint como guardar en
    // STNN una rotación distinta de los gfx realmente materializados.
    let (airport_tile_gfx, placed) = if let Some(def) = newgrf_def.as_ref() {
        let mapping = newgrf_airport_tile_mapping(
            origin,
            axis_y,
            def,
            &state.airport_tile_spec_catalog,
            newgrf_layout,
        );
        (mapping.gfx, mapping.pieces)
    } else {
        (
            Vec::new(),
            airport_spec_tiles(origin, place_spec, axis_y).collect(),
        )
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
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    st.airport_tiles = tiles;
    st.airport_tile_gfx = airport_tile_gfx;
    st.airport_spec = place_spec;
    st.airport_newgrf_spec_id = newgrf_id.filter(|_| newgrf_def.is_some());
    st.airport_ttd_type = newgrf_def.as_ref().map(|def| def.ttd_airport_type);
    let (layout_index, rotation) = newgrf_layout.unwrap_or((0, if axis_y { 2 } else { 0 }));
    st.airport_layout = layout_index;
    st.airport_rotation = rotation;
    st.airport_blocks = 0;
    // Catchment: `station_catchment_radius` lee `airport_spec` en cobertura.
    state.stations.push(st);
    if newgrf_def.is_some() {
        let dirty =
            crate::map::trigger_newgrf_airport_animation_for_station_with_towns_and_cargo_catalog_and_airport_catalog(
                &mut state.map,
                state.tick.get(),
                &mut state.stations,
                &state.towns,
                &state.cargo_spec_catalog,
                state.climate,
                &state.airport_tile_spec_catalog,
                &state.airport_spec_catalog,
                &mut state.newgrf_animated_airport_tiles,
                &state.newgrf_stack,
                station_anchor,
                crate::AirportAnimationTrigger::Built,
                None,
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
