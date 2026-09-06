use crate::economy::{road_stop_build_cost_factored, station_build_cost, waypoint_build_cost};
use crate::map::{Map, TileCoord, TileKind, tile_slope_and_z};
use crate::pathfinder::{
    station_entrance_faces_rail, station_entrance_faces_road, station_site_tile_allows_build,
    station_site_tile_needs_clear,
};
use crate::station::is_rail_waypoint_tile;
use crate::{CLEAR_TILE_COST, GameState, Station, StopKind};

use super::super::{CommandError, require_tile_owned_by_active};
use crate::town::{self, authority_allows_new_station};

#[allow(unused_imports)]
use crate::command::transport::internal::{
    RAIL_TB_X, RAIL_TB_Y, check_in_bounds, connect_road_stop, rail_axis_y_from_trackbits,
    rail_axis_y_unambiguous, road_stop_m5,
};

/// Acceso a red de carretera para bahía (`dir` 0..3) o drive-through (`4`/`5`).
fn road_stop_entrance_ok(map: &Map, c: TileCoord, dir: u8) -> bool {
    if crate::road_stop_spec::is_drive_through_orientation(dir) {
        let ends = if crate::road_stop_spec::drive_through_axis_y(dir) {
            [(0i32, -1i32), (0, 1)]
        } else {
            [(-1i32, 0i32), (1, 0)]
        };
        ends.into_iter().any(|(dx, dy)| {
            let n = TileCoord::new(c.x + dx, c.y + dy);
            map.get_kind(n).is_some_and(|k| {
                matches!(
                    k,
                    TileKind::Road
                        | TileKind::RoadDepot
                        | TileKind::RoadTunnel
                        | TileKind::RoadBridge
                )
            })
        })
    } else {
        station_entrance_faces_road(map, c, dir)
    }
}

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
                road_stop_entrance_ok(map, c, dir)
            };
            if entrance_ok {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
    }
}

/// Restricciones del `RoadStopSpec` activo (query + execute).
pub(crate) fn check_road_stop_spec_restrictions(
    state: &GameState,
    orientation: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    let Some(id) = state.current_road_stop_spec else {
        return Ok(());
    };
    let Some(def) = crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, id)
    else {
        return Err(CommandError::RoadStopSpecTypeMismatch);
    };
    if !def.matches_stop_kind(stop_kind) {
        return Err(CommandError::RoadStopSpecTypeMismatch);
    }
    let is_dt = crate::road_stop_spec::is_drive_through_orientation(orientation);
    if def.drive_through_only() && !is_dt {
        return Err(CommandError::RoadStopDriveThroughRequired);
    }
    let rt_class =
        crate::road_type::road_type_def(&state.road_type_catalog, state.current_road_type)
            .map_or_else(|| state.current_road_type.road_tram_type(), |d| d.class);
    if def.road_only() && rt_class != crate::road_type::RoadTramType::Road {
        return Err(CommandError::RoadStopRoadTypeMismatch);
    }
    if def.tram_only() && rt_class != crate::road_type::RoadTramType::Tram {
        return Err(CommandError::RoadStopRoadTypeMismatch);
    }
    let owner_colour = state
        .companies
        .iter()
        .find(|company| company.id == state.active_company)
        .map_or(state.company_colour, |company| company.colour);
    if !crate::newgrf_callback::apply_road_stop_availability_callback_with_context(
        def,
        stop_kind,
        state.current_road_type,
        &state.road_type_catalog,
        state.active_company,
        owner_colour,
        &state.companies,
        crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date),
    ) {
        return Err(CommandError::NewGrfCallbackDenied);
    }
    Ok(())
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
        StopKind::Dock => crate::station::STATION_TYPE_DOCK,
        StopKind::Buoy => 6,
        StopKind::RailWaypoint => 7,
        StopKind::RoadWaypoint => 8,
    }
}

pub(in crate::command::transport) fn apply_station_m6(m6: u8, stop_kind: StopKind) -> u8 {
    (m6 & !0x78) | (ottd_station_type_bits(stop_kind) << 3)
}

pub(in crate::command::transport) fn rail_station_gfx_from_axis(axis_y: bool) -> u8 {
    if axis_y { 3 } else { 2 }
}

pub(in crate::command) fn rail_station_m5(map: &Map, c: TileCoord, dir: u8) -> u8 {
    // Preferir vecino con un solo eje; CROSS no impone andén (cae a `dir`).
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if let Some(t) = map.get(n)
            && t.kind == TileKind::Rail
            && let Some(axis_y) = rail_axis_y_unambiguous(t.m5)
        {
            return rail_station_gfx_from_axis(axis_y);
        }
    }
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
    check_rail_station_spec_restrictions(state, 1, 1)?;
    let axis_y = rail_station_m5(&state.map, c, dir) & 1 != 0;
    check_rail_station_slope_callbacks_with_diagnostic(state, c, axis_y, 1, 1)?;
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

/// Restricciones Action0 del spec ferroviario activo (query + execute).
///
/// El callback CB13 se evalúa antes de alterar el mapa, sin `Station` creada,
/// como hace `OpenTTD`. Esto también cubre el comando 1×1, que antes ignoraba
/// los límites de plataformas/longitud del spec seleccionado.
pub(in crate::command) fn check_rail_station_spec_restrictions(
    state: &GameState,
    platforms: u8,
    length: u8,
) -> Result<(), CommandError> {
    let spec_id = state.current_station_spec;
    let Some(spec) = crate::station_class::station_spec_def(&state.station_spec_catalog, spec_id)
    else {
        return Ok(());
    };
    if !spec.allows_platforms(platforms) || !spec.allows_length(length) {
        return Err(CommandError::StationSizeNotAllowed);
    }
    let owner_colour = state
        .companies
        .iter()
        .find(|company| company.id == state.active_company)
        .map_or(state.company_colour, |company| company.colour);
    if !crate::newgrf_callback::apply_station_availability_callback_for_build_with_context(
        spec,
        state.active_company,
        owner_colour,
        &state.companies,
        crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date),
    ) {
        return Err(CommandError::NewGrfCallbackDenied);
    }
    Ok(())
}

/// Ejecuta CB149 para cada tesela de la estación ferroviaria antes de mutar.
///
/// La posición relativa siempre es `platform << 8 | position`: la conversión
/// a coordenadas de mapa depende del eje, pero no la codificación del callback.
pub(in crate::command) fn check_rail_station_slope_callbacks(
    state: &GameState,
    origin: TileCoord,
    axis_y: bool,
    platforms: u8,
    length: u8,
) -> Result<(), CommandError> {
    check_rail_station_slope_callbacks_impl(state, origin, axis_y, platforms, length)
        .map_err(|(error, _)| error)
}

pub(in crate::command) fn check_rail_station_slope_callbacks_with_diagnostic(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    platforms: u8,
    length: u8,
) -> Result<(), CommandError> {
    state.runtime.last_station_slope_diagnostic = None;
    match check_rail_station_slope_callbacks_impl(&*state, origin, axis_y, platforms, length) {
        Ok(()) => Ok(()),
        Err((error, diagnostic)) => {
            state.runtime.last_station_slope_diagnostic = diagnostic;
            Err(error)
        }
    }
}

fn check_rail_station_slope_callbacks_impl(
    state: &GameState,
    origin: TileCoord,
    axis_y: bool,
    platforms: u8,
    length: u8,
) -> Result<
    (),
    (
        CommandError,
        Option<crate::newgrf_callback::StationSlopeCallbackDiagnostic>,
    ),
> {
    let Some(spec) = crate::station_class::station_spec_def(
        &state.station_spec_catalog,
        state.current_station_spec,
    ) else {
        return Ok(());
    };
    if !spec.has_slope_check_callback() {
        return Ok(());
    }

    for platform in 0..platforms {
        for position in 0..length {
            let c = if axis_y {
                TileCoord::new(
                    origin.x + i32::from(platform),
                    origin.y + i32::from(position),
                )
            } else {
                TileCoord::new(
                    origin.x + i32::from(position),
                    origin.y + i32::from(platform),
                )
            };
            let (slope, _) =
                tile_slope_and_z(&state.map, c).ok_or((CommandError::OutOfBounds, None))?;
            let outcome = crate::newgrf_callback::resolve_station_slope_callback_for_build_with_map(
                spec,
                &state.map,
                c,
                state.climate,
                slope,
                axis_y,
                platforms,
                length,
                platform,
                position,
            );
            if !matches!(
                outcome,
                crate::newgrf_callback::StationSlopeCallbackOutcome::Allow
            ) {
                return Err((
                    CommandError::NewGrfCallbackDenied,
                    Some(crate::newgrf_callback::StationSlopeCallbackDiagnostic {
                        grfid: spec.newgrf_grfid,
                        outcome,
                    }),
                ));
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
    let spec_id = state.current_station_spec;
    check_rail_station_spec_restrictions(state, platforms, length)?;
    let (w, h) = rail_station_footprint(axis_y, platforms, length);
    check_rail_station_area(state, origin, w, h)?;
    check_rail_station_slope_callbacks_with_diagnostic(state, origin, axis_y, platforms, length)?;
    let anchor = TileCoord::new(origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
    if !authority_allows_new_station(&state.towns, anchor, state.active_company) {
        return Err(CommandError::AuthorityRatingTooLow);
    }

    let layout = crate::station_class::station_spec_layout(
        &state.station_spec_catalog,
        spec_id,
        usize::from(platforms),
        usize::from(length),
    );
    // Precalcular m5 (layout 0x0E + callback 24) antes de mutar el mapa.
    let mut tile_gfx: Vec<u8> = Vec::with_capacity(layout.len());
    for n in 0..platforms {
        for l in 0..length {
            let idx = usize::from(n) * usize::from(length) + usize::from(l);
            let base = layout[idx] + u8::from(axis_y);
            let gfx = crate::station_class::station_spec_def(&state.station_spec_catalog, spec_id)
                .map_or(base, |def| {
                    crate::station_class::apply_station_build_tile_layout_callback(
                        def, base, platforms, length, n, l, axis_y,
                    )
                });
            tile_gfx.push(gfx);
        }
    }
    for n in 0..platforms {
        for l in 0..length {
            let c = if axis_y {
                TileCoord::new(origin.x + i32::from(n), origin.y + i32::from(l))
            } else {
                TileCoord::new(origin.x + i32::from(l), origin.y + i32::from(n))
            };
            let idx = usize::from(n) * usize::from(length) + usize::from(l);
            let gfx = tile_gfx[idx];
            if station_site_tile_needs_clear(state.map.get_kind(c).unwrap_or(TileKind::Grass)) {
                clear_station_site_tile(state, c)?;
            }
            let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            tile.kind = TileKind::Station;
            tile.mapt = 0x50;
            tile.m5 = gfx;
            tile.m3 = (tile.m3 & !0x06) | crate::default_station_catenary_flags(gfx);
            tile.m8 = crate::set_rail_type_on_tile(tile, state.current_rail_type).m8;
            tile.m6 = apply_station_m6(tile.m6, StopKind::RailStation);
            // `GetAnimationFrame` usa MAP7: una estación nueva empieza en 0.
            tile.m7 = 0;
            state
                .map
                .set_tile(c, tile)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.economy.money -= station_build_cost(&state.global_economy);
        }
    }

    let anchor = TileCoord::new(origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
    let mut st = Station::new_with_kind(anchor, StopKind::RailStation);
    st.owner = state.active_company;
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    st.station_spec = spec_id;
    state.stations.push(st);
    let tick = state.tick.get();
    let climate = state.climate;
    for n in 0..platforms {
        for l in 0..length {
            let c = if axis_y {
                TileCoord::new(origin.x + i32::from(n), origin.y + i32::from(l))
            } else {
                TileCoord::new(origin.x + i32::from(l), origin.y + i32::from(n))
            };
            if crate::map::trigger_newgrf_station_animation_with_towns_and_world_and_cargo_catalog(
                &mut state.map,
                tick,
                &mut state.stations,
                &state.companies,
                &state.towns,
                &state.industries,
                &state.cargo_spec_catalog,
                climate,
                &state.station_spec_catalog,
                &mut state.newgrf_animated_station_tiles,
                c,
                crate::station_class::StationAnimationTrigger::Built,
            ) {
                state.runtime.industry_tile_dirty.push(c);
            }
        }
    }
    if let Some((town_id, delta)) =
        town::apply_station_build_rating_penalty(&mut state.towns, anchor, state.active_company)
    {
        state
            .runtime
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

/// Spec `NewGRF` a persistir (ya validado por `check_road_stop_spec_restrictions`).
fn resolve_road_stop_spec_for_placement(state: &GameState) -> Option<u16> {
    state.current_road_stop_spec.filter(|&id| {
        crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, id).is_some()
    })
}

fn road_stop_build_cost_for_state(state: &GameState, stop_kind: StopKind) -> i64 {
    state
        .current_road_stop_spec
        .and_then(|id| crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, id))
        .map_or_else(
            || station_build_cost(&state.global_economy),
            |def| {
                road_stop_build_cost_factored(
                    &state.global_economy,
                    stop_kind,
                    def.build_cost_multiplier,
                )
            },
        )
}

pub(in crate::command::transport) fn station_placement_on_tile(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    if !authority_allows_new_station(&state.towns, c, state.active_company) {
        return Err(CommandError::AuthorityRatingTooLow);
    }
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    let build_cost = if matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        road_stop_build_cost_for_state(state, stop_kind)
    } else {
        station_build_cost(&state.global_economy)
    };
    // Snapshot para rollback si `connect_road_stop` falla (antes dejaba Station huérfana
    // y RoadHaul no podía reintentar otra boca).
    let prev_tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
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
    if stop_kind == StopKind::RailStation {
        tile.m3 = (tile.m3 & !0x06) | crate::default_station_catenary_flags(tile.m5);
        tile.m8 = crate::set_rail_type_on_tile(tile, state.current_rail_type).m8;
        tile.m7 = 0;
    }
    tile.m6 = apply_station_m6(tile.m6, stop_kind);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    if matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop)
        && let Err(e) = connect_road_stop(state, c, dir)
    {
        let _ = state.map.set_tile(c, prev_tile);
        return Err(e);
    }
    let mut st = Station::new_with_kind(c, stop_kind);
    st.owner = state.active_company;
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    if stop_kind == StopKind::RailStation {
        st.station_spec = state.current_station_spec;
    }
    if matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        st.road_stop_spec = resolve_road_stop_spec_for_placement(state);
        // El estado de una parada custom pertenece a su tesela desde el
        // primer frame; esto permite conservarlo si más adelante se une a una
        // estación compuesta con otro spec.
        let _ = st.ensure_road_stop_tile_state(c);
        st.sync_legacy_road_stop_anchor();
    }
    if let Some(spec_id) = st.road_stop_spec
        && let Some(def) =
            crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, spec_id)
    {
        let view = state.map.get(c).map_or(dir, |station_tile| station_tile.m5);
        if crate::newgrf_callback::trigger_road_stop_animation(
            def,
            &mut st,
            view,
            crate::StationAnimationTrigger::Built,
            None,
            state.tick.get(),
        ) {
            state.runtime.industry_tile_dirty.push(c);
        }
    }
    state.stations.push(st);
    if stop_kind == StopKind::RailStation {
        let tick = state.tick.get();
        let climate = state.climate;
        if crate::map::trigger_newgrf_station_animation_with_towns_and_world_and_cargo_catalog(
            &mut state.map,
            tick,
            &mut state.stations,
            &state.companies,
            &state.towns,
            &state.industries,
            &state.cargo_spec_catalog,
            climate,
            &state.station_spec_catalog,
            &mut state.newgrf_animated_station_tiles,
            c,
            crate::station_class::StationAnimationTrigger::Built,
        ) {
            state.runtime.industry_tile_dirty.push(c);
        }
    }
    state.economy.money -= build_cost;
    if let Some((town_id, delta)) =
        town::apply_station_build_rating_penalty(&mut state.towns, c, state.active_company)
    {
        state
            .runtime
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
    if matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        check_road_stop_spec_restrictions(state, dir, stop_kind)?;
    }
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
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let axis_y = rail_waypoint_axis_from_trackbits(tile.m5).unwrap_or(false);
    let mut out = tile;
    out.kind = TileKind::Station;
    out.mapt = 0x50;
    out.m5 = u8::from(axis_y);
    out.m3 = (out.m3 & !0x06) | crate::default_station_catenary_flags(out.m5);
    out.m6 = apply_station_m6(out.m6, StopKind::RailWaypoint);
    out.m7 = 0;
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    let mut st = Station::new_with_kind(c, StopKind::RailWaypoint);
    st.owner = state.active_company;
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    st.station_spec = state.current_station_spec;
    state.stations.push(st);
    let tick = state.tick.get();
    let climate = state.climate;
    if crate::map::trigger_newgrf_station_animation_with_towns_and_world_and_cargo_catalog(
        &mut state.map,
        tick,
        &mut state.stations,
        &state.companies,
        &state.towns,
        &state.industries,
        &state.cargo_spec_catalog,
        climate,
        &state.station_spec_catalog,
        &mut state.newgrf_animated_station_tiles,
        c,
        crate::station_class::StationAnimationTrigger::Built,
    ) {
        state.runtime.industry_tile_dirty.push(c);
    }
    state.economy.money -= waypoint_build_cost(&state.global_economy);
    Ok(())
}

/// Eje de waypoint road: solo carretera recta X (`0x0A`) o Y (`0x05`).
pub(in crate::command::transport) fn road_waypoint_axis_bits(bits: u8) -> Option<u8> {
    match bits & 0x0F {
        0x0A => Some(0x0A),
        0x05 => Some(0x05),
        _ => None,
    }
}

pub(crate) fn check_place_road_waypoint(
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
        TileKind::Road => {
            road_waypoint_axis_bits(tile.m5).ok_or(CommandError::CannotPlaceWaypointOnTrack)?;
            Ok(())
        }
        TileKind::Station
            if crate::station::station_type_from_m6(tile.m6)
                == crate::station::STATION_TYPE_ROAD_WAYPOINT =>
        {
            Err(CommandError::StationAlreadyExists)
        }
        _ => Err(CommandError::CannotPlaceWaypointOnTrack),
    }
}

pub(in crate::command) fn place_road_waypoint(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_road_waypoint(&state.map, c, &state.stations)?;
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let axis = road_waypoint_axis_bits(tile.m5).unwrap_or(0x0A);
    let mut out = tile;
    out.kind = TileKind::Station;
    out.mapt = 0x50;
    // Eje en m5 (0 = X, 1 = Y), bits de carretera en m3 para pathfinding.
    out.m5 = u8::from(axis == 0x05);
    out.m3 = (out.m3 & !0x0F) | axis;
    out.m6 = apply_station_m6(out.m6, StopKind::RoadWaypoint);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    let mut st = Station::new_with_kind(c, StopKind::RoadWaypoint);
    st.owner = state.active_company;
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    state.stations.push(st);
    state.economy.money -= waypoint_build_cost(&state.global_economy);
    Ok(())
}

/// Nombre personalizado de estación (`OpenTTD` ~32 chars).
pub const MAX_STATION_NAME_CHARS: usize = 32;

pub(crate) fn rename_station(
    state: &mut GameState,
    station_pos: TileCoord,
    name: Option<String>,
) -> Result<(), CommandError> {
    let Some(station) = state.stations.iter_mut().find(|s| s.pos == station_pos) else {
        return Err(CommandError::StationNotFound);
    };
    let normalized = name.and_then(|n| {
        let trimmed = n.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    if normalized
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_STATION_NAME_CHARS)
    {
        return Err(CommandError::StationNameTooLong);
    }
    station.name = normalized;
    station.newgrf_string_id = if station.name.is_some() {
        crate::station::STATION_STRING_ID_FALLBACK
    } else {
        crate::station::STATION_STRING_ID_DEFAULT
    };
    Ok(())
}

fn rewrite_order_station(order: &mut crate::VehicleOrder, from: TileCoord, to: TileCoord) {
    use crate::VehicleOrder;
    match order {
        VehicleOrder::Station { station, .. } if *station == from => *station = to,
        VehicleOrder::Waypoint { waypoint, .. } if *waypoint == from => *waypoint = to,
        _ => {}
    }
}

/// Une dos paradas road 1×1 o dos estaciones rail (misma compañía, mismo
/// tipo; rail: mismo eje). Cuando `station.distant_join_stations` está
/// desactivado, exige que sus huellas sean adyacentes.
pub(crate) fn join_stations(
    state: &mut GameState,
    keep: TileCoord,
    merge: TileCoord,
) -> Result<(), CommandError> {
    if keep == merge {
        return Err(CommandError::CannotJoinStations);
    }
    let keep_idx = state
        .stations
        .iter()
        .position(|s| s.pos == keep)
        .ok_or(CommandError::StationNotFound)?;
    let merge_idx = state
        .stations
        .iter()
        .position(|s| s.pos == merge)
        .ok_or(CommandError::StationNotFound)?;
    let adjacent = {
        let keep_st = &state.stations[keep_idx];
        let merge_st = &state.stations[merge_idx];
        if keep_st.owner != merge_st.owner || keep_st.stop_kind != merge_st.stop_kind {
            return Err(CommandError::CannotJoinStations);
        }
        match keep_st.stop_kind {
            StopKind::BusStop | StopKind::TruckStop => {
                let dist = (keep.x - merge.x).abs() + (keep.y - merge.y).abs();
                dist == 1
            }
            StopKind::RailStation => {
                let keep_tiles =
                    crate::station::rail_station_owned_tiles(&state.map, &state.stations, keep_st);
                let merge_tiles =
                    crate::station::rail_station_owned_tiles(&state.map, &state.stations, merge_st);
                let keep_axis =
                    crate::station::rail_station_axis_y(&state.map, &state.stations, keep_st);
                let merge_axis =
                    crate::station::rail_station_axis_y(&state.map, &state.stations, merge_st);
                if keep_axis != merge_axis {
                    return Err(CommandError::CannotJoinStations);
                }
                crate::station::station_tile_sets_adjacent(&keep_tiles, &merge_tiles)
            }
            _ => return Err(CommandError::CannotJoinStations),
        }
    };
    if !state.construction.distant_join_stations && !adjacent {
        return Err(CommandError::CannotJoinStations);
    }
    // Materializar los campos legacy antes de extraer la estación. Así un join
    // posterior conserva spec/frame/random distintos por tesela en vez de
    // quedarse con el estado del ancla `keep`.
    state.stations[keep_idx].normalize_road_stop_tile_states();
    state.stations[merge_idx].normalize_road_stop_tile_states();
    let mut merge_st = state.stations.remove(merge_idx);
    // Tras remove, keep_idx puede haber cambiado.
    let keep_idx = state
        .stations
        .iter()
        .position(|s| s.pos == keep)
        .ok_or(CommandError::StationNotFound)?;

    merge_st.ensure_packets_from_stock();
    let merge_packets = merge_st.cargo_packets.drain_all();
    let merge_income = merge_st.income;
    let merge_tsp = merge_st.time_since_pickup;
    let merge_joined = std::mem::take(&mut merge_st.joined_tiles);
    let merge_road_stop_tile_states = std::mem::take(&mut merge_st.road_stop_tile_states);

    let keep_st = &mut state.stations[keep_idx];
    keep_st.ensure_packets_from_stock();
    for packet in merge_packets {
        keep_st.cargo_packets.push(packet);
    }
    keep_st.sync_stock_from_packets();
    keep_st.income = keep_st.income.saturating_add(merge_income);
    for cargo in crate::ALL_CARGO_TYPES {
        let a = keep_st.time_since_pickup.get(cargo);
        let b = merge_tsp.get(cargo);
        keep_st.time_since_pickup.set(cargo, a.max(b));
    }
    if !keep_st.joined_tiles.contains(&merge) {
        keep_st.joined_tiles.push(merge);
    }
    for t in merge_joined {
        if t != keep && !keep_st.joined_tiles.contains(&t) {
            keep_st.joined_tiles.push(t);
        }
    }
    for (tile, tile_state) in merge_road_stop_tile_states {
        *keep_st.ensure_road_stop_tile_state(tile) = tile_state;
    }
    keep_st.normalize_road_stop_tile_states();

    for vehicle in &mut state.vehicles {
        for order in &mut vehicle.orders {
            rewrite_order_station(order, merge, keep);
        }
    }
    for list in &mut state.shared_order_lists {
        for order in &mut list.orders {
            rewrite_order_station(order, merge, keep);
        }
    }
    for sub in &mut state.subsidies {
        if sub.dest_station_pos == merge {
            sub.dest_station_pos = keep;
        }
    }
    Ok(())
}
