//! Contexto Action2 para teselas de estación (vars de runtime).

use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::cargo_spec::CargoSpecDef;
use crate::industry::Industry;
use crate::map::{
    Map, TILE_PIXEL_HEIGHT, TileCoord, TileKind, rail_bits_touching_side, rail_traversal_bits,
    tile_slope_and_z,
};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{
    GrfTypeTranslationTables, cargo_from_local_id_with_catalog, local_cargo_id_with_catalog,
    reverse_rail_type,
};
use crate::rail_type::rail_type_from_tile;
use crate::station::{
    STATION_TILE_RESERVATION, STATION_TYPE_RAIL_WAYPOINT, Station, StationCoverage,
    station_at_tile, station_type_from_m6,
};
use crate::station_class::{StationSpecDef, station_spec_def};
use crate::world_gen::Climate;
use std::collections::BTreeSet;

/// Contexto Action2 para dibujar / resolver sprites de una tesela de estación.
///
/// MVP: `40` (plataforma), `42` (terreno+rail), `43` (owner), `44` (PBS),
/// `45` (continuación rail), `46` (posición centrada), `47` (spec centrado),
/// `49` (eje), `4A` (frame), `5F` (random), `10` (m5/tileh),
/// `67` (land info tesela actual, param 0).
#[must_use]
pub fn action2_eval_ctx_for_station_tile(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
) -> Action2EvalCtx {
    action2_eval_ctx_for_station_tile_with_grf(
        map,
        stations,
        coord,
        owner_colour,
        climate,
        type_tables,
        8,
    )
}

/// Variante que conserva la versión Action8 del GRF para traducir los
/// parámetros de cargo de las variables `60`–`65`/`69`.
#[must_use]
pub fn action2_eval_ctx_for_station_tile_with_grf(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
) -> Action2EvalCtx {
    action2_eval_ctx_for_station_tile_impl(
        map,
        stations,
        coord,
        owner_colour,
        climate,
        type_tables,
        grf_version,
        None,
    )
}

/// Pools de mundo necesarios para que las variables de carga de una estación
/// consulten el catchment vivo en vez del predicado persistido del save.
#[derive(Debug, Clone, Copy)]
pub struct StationAction2WorldContext<'a> {
    pub industries: &'a [Industry],
    /// Catálogo activo para que vars `60`–`69` puedan resolver cargos custom
    /// mediante la etiqueta de la CTT del GRF.
    pub cargo_spec_catalog: &'a [CargoSpecDef],
}

/// Variante con pools de mundo para los call sites reales de render y
/// animación. Las APIs anteriores siguen usando el fallback legacy.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_station_tile_with_world(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    world: StationAction2WorldContext<'_>,
) -> Action2EvalCtx {
    action2_eval_ctx_for_station_tile_impl(
        map,
        stations,
        coord,
        owner_colour,
        climate,
        type_tables,
        grf_version,
        Some(world),
    )
}

/// Variante del contexto de estación que además materializa las consultas a
/// teselas vecinas declaradas por el runtime Action2 del spec actual.
///
/// El catálogo sólo se necesita para traducir `0x68`/`0x6A`/`0x6B` a la
/// identidad `(GRFID, local_id)` de la spec vecina. Las APIs históricas que no
/// tienen catálogo siguen usando [`action2_eval_ctx_for_station_tile_with_grf`]
/// y conservan el fallback sin vecindad.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_station_tile_with_catalog(
    map: &Map,
    stations: &[Station],
    station_catalog: &[StationSpecDef],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
) -> Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_station_tile_with_grf(
        map,
        stations,
        coord,
        owner_colour,
        climate,
        type_tables,
        grf_version,
    );
    populate_station_neighbour_vars(
        &mut ctx,
        map,
        stations,
        station_catalog,
        coord,
        climate,
        grf_version,
    );
    ctx
}

/// Variante catalogue-aware con cobertura viva de cargos/industrias.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_station_tile_with_catalog_and_world(
    map: &Map,
    stations: &[Station],
    station_catalog: &[StationSpecDef],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    world: StationAction2WorldContext<'_>,
) -> Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_station_tile_with_world(
        map,
        stations,
        coord,
        owner_colour,
        climate,
        type_tables,
        grf_version,
        world,
    );
    populate_station_neighbour_vars(
        &mut ctx,
        map,
        stations,
        station_catalog,
        coord,
        climate,
        grf_version,
    );
    ctx
}

struct StationNeighbourScope<'a> {
    map: &'a Map,
    stations: &'a [Station],
    station_catalog: &'a [StationSpecDef],
    source: &'a Station,
    current_spec: &'a StationSpecDef,
    coord: TileCoord,
    climate: Climate,
    grf_version: u8,
}

impl StationNeighbourScope<'_> {
    fn populate(&self, ctx: &mut Action2EvalCtx) {
        for (variable, parameter) in requested_station_neighbour_vars(self.current_spec) {
            let nearby = nearby_station_tile(self.map, self.coord, parameter);
            let value = match variable {
                0x66 => {
                    nearby_station_animation_frame(self.map, self.stations, self.source, nearby)
                }
                0x67 => nearby_station_land_info(
                    self.map,
                    self.coord,
                    nearby,
                    self.climate,
                    self.grf_version,
                ),
                0x68 => nearby_station_info(
                    self.map,
                    self.stations,
                    self.station_catalog,
                    self.source,
                    self.current_spec,
                    self.coord,
                    nearby,
                ),
                0x6A => nearby_station_grfid(self.map, self.stations, self.station_catalog, nearby),
                0x6B => nearby_station_local_id(
                    self.map,
                    self.stations,
                    self.station_catalog,
                    self.current_spec,
                    nearby,
                ),
                _ => continue,
            };
            ctx.parameterized_vars.insert((variable, parameter), value);
        }
    }
}

fn populate_station_neighbour_vars(
    ctx: &mut Action2EvalCtx,
    map: &Map,
    stations: &[Station],
    station_catalog: &[StationSpecDef],
    coord: TileCoord,
    climate: Climate,
    grf_version: u8,
) {
    let Some(source) = station_at_tile(map, stations, coord) else {
        return;
    };
    let Some(current_spec) = station_spec_def(station_catalog, source.station_spec) else {
        return;
    };
    if !current_spec.from_newgrf || current_spec.newgrf_runtime.is_none() {
        return;
    }
    StationNeighbourScope {
        map,
        stations,
        station_catalog,
        source,
        current_spec,
        coord,
        climate,
        grf_version,
    }
    .populate(ctx);
}

fn requested_station_neighbour_vars(spec: &StationSpecDef) -> BTreeSet<(u8, u8)> {
    let mut requested = BTreeSet::new();
    let Some(runtime) = spec.newgrf_runtime.as_ref() else {
        return requested;
    };
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if matches!(term.variable, 0x66 | 0x67 | 0x68 | 0x6A | 0x6B)
                && let Some(parameter) = term.param
            {
                requested.insert((term.variable, parameter));
            }
        }
    }
    requested
}

fn signed_station_nibble(value: u8) -> i32 {
    let value = i32::from(value & 0x0F);
    if value >= 8 { value - 16 } else { value }
}

/// `GetNearbyTile` para `StationScopeResolver`: offsets firmados en nibbles y
/// wrap toroidal, igual que el resolver vial.
fn nearby_station_tile(map: &Map, base: TileCoord, parameter: u8) -> TileCoord {
    let (width, height) = map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return base;
    };
    if width == 0 || height == 0 {
        return base;
    }
    let dx = signed_station_nibble(parameter);
    let dy = signed_station_nibble(parameter >> 4);
    TileCoord::new(
        base.x.saturating_add(dx).rem_euclid(width),
        base.y.saturating_add(dy).rem_euclid(height),
    )
}

fn is_rail_station_tile(map: &Map, stations: &[Station], coord: TileCoord) -> bool {
    map.get(coord).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && matches!(
                station_type_from_m6(tile.m6),
                0 | STATION_TYPE_RAIL_WAYPOINT
            )
            && station_at_tile(map, stations, coord).is_some_and(|station| {
                matches!(
                    station.stop_kind,
                    crate::station::StopKind::RailStation | crate::station::StopKind::RailWaypoint
                )
            })
    })
}

fn nearby_station_animation_frame(
    map: &Map,
    stations: &[Station],
    source: &Station,
    nearby: TileCoord,
) -> u32 {
    if !is_rail_station_tile(map, stations, nearby) {
        return u32::MAX;
    }
    let Some(candidate) = station_at_tile(map, stations, nearby) else {
        return u32::MAX;
    };
    if candidate.pos != source.pos {
        return u32::MAX;
    }
    map.get(nearby).map_or(u32::MAX, |tile| u32::from(tile.m7))
}

fn nearby_station_land_info(
    map: &Map,
    source: TileCoord,
    nearby: TileCoord,
    climate: Climate,
    grf_version: u8,
) -> u32 {
    let Some(tile) = map.get(nearby) else {
        return u32::MAX;
    };
    let (tileh, raw_z) = tile_slope_and_z(map, nearby).unwrap_or((0, 0));
    let z = if grf_version >= 8 {
        raw_z
    } else {
        raw_z.saturating_mul(u8::try_from(TILE_PIXEL_HEIGHT).unwrap_or(8))
    };
    let axis_y = map
        .get(source)
        .is_some_and(|source_tile| source_tile.m5 & 1 != 0);
    let slope_swapped = axis_y && ((tileh & 1 != 0) != (tileh & 4 != 0));
    let slope = if slope_swapped { tileh ^ 5 } else { tileh };
    let tile_type = u32::from(tile_kind_as_ottd(tile.kind));
    let terrain = terrain_type_for_tile(map, nearby, climate, Some(tile));
    tile_type << 24 | u32::from(z) << 16 | (terrain << 2) << 8 | u32::from(slope)
}

fn nearby_station_info(
    map: &Map,
    stations: &[Station],
    station_catalog: &[StationSpecDef],
    source: &Station,
    current_spec: &StationSpecDef,
    source_coord: TileCoord,
    nearby: TileCoord,
) -> u32 {
    if !is_rail_station_tile(map, stations, nearby) {
        return u32::MAX;
    }
    let Some(candidate) = station_at_tile(map, stations, nearby) else {
        return u32::MAX;
    };
    let Some(tile) = map.get(nearby) else {
        return u32::MAX;
    };
    let source_axis = map
        .get(source_coord)
        .map_or(0, |source_tile| source_tile.m5 & 1);
    let nearby_axis = tile.m5 & 1;
    let mut result = u32::from((tile.m5 >> 1) & 0x03) << 12;
    if source_axis != nearby_axis {
        result |= 1 << 11;
    }
    if candidate.pos == source.pos {
        result |= 1 << 10;
    }
    if let Some(spec) = station_spec_def(station_catalog, candidate.station_spec)
        && spec.from_newgrf
    {
        result |= 1
            << if spec.newgrf_grfid == current_spec.newgrf_grfid {
                8
            } else {
                9
            };
        result |= u32::from(spec.newgrf_local_id);
    }
    result
}

fn nearby_station_grfid(
    map: &Map,
    stations: &[Station],
    station_catalog: &[StationSpecDef],
    nearby: TileCoord,
) -> u32 {
    if !is_rail_station_tile(map, stations, nearby) {
        return u32::MAX;
    }
    station_at_tile(map, stations, nearby)
        .and_then(|station| station_spec_def(station_catalog, station.station_spec))
        .filter(|spec| spec.from_newgrf)
        .map_or(0, |spec| spec.newgrf_grfid)
}

fn nearby_station_local_id(
    map: &Map,
    stations: &[Station],
    station_catalog: &[StationSpecDef],
    current_spec: &StationSpecDef,
    nearby: TileCoord,
) -> u32 {
    if !is_rail_station_tile(map, stations, nearby) {
        return u32::MAX;
    }
    station_at_tile(map, stations, nearby)
        .and_then(|station| station_spec_def(station_catalog, station.station_spec))
        .filter(|spec| spec.from_newgrf)
        .filter(|spec| spec.newgrf_grfid == current_spec.newgrf_grfid)
        .map_or(0xFFFE, |spec| u32::from(spec.newgrf_local_id))
}

/// Completa el contexto mínimo de una estación cuando el caller no tiene una
/// tesela disponible (por ejemplo una API legacy que sólo recibe `Station`).
///
/// `OpenTTD` usa los mismos valores centinela para las variables de andén,
/// vía y posición cuando el resolver no tiene `tile`. Mantenerlos explícitos
/// es importante: un Action2 que consulta una de estas variables debe obtener
/// el sentinel nativo, no caer silenciosamente al valor cero de un mapa vacío.
pub(crate) fn populate_station_scope_fallback_vars(ctx: &mut Action2EvalCtx, station: &Station) {
    // `0x2110000`: platforms/tracks/position with no station tile.
    const NO_TILE_PLATFORM_INFO: u32 = 0x0211_0000;
    for variable in [0x40, 0x41, 0x46, 0x47, 0x49] {
        ctx.vars.insert(variable, NO_TILE_PLATFORM_INFO);
    }
    // Terrain/rail type and PBS status have the same no-tile defaults as the
    // purchase/slope resolver.  Rail continuation and tile frame are not
    // available without a coordinate, so expose OpenTTD's unavailable value
    // for the former and a stable zero for the latter.
    ctx.vars.insert(0x42, 0);
    ctx.vars.insert(0x43, u32::from(station.owner.0));
    ctx.vars.insert(0x44, 2);
    ctx.vars.insert(0x45, u32::MAX);
    ctx.vars
        .insert(0x4A, u32::from(station.road_stop_animation_frame));
    populate_station_general_vars(ctx, station);
    populate_station_legacy_cargo_vars(ctx, station);
    ctx.vars.insert(
        0x5F,
        u32::from(station.newgrf_random_bits) << 8
            | u32::from(station.newgrf_waiting_random_triggers),
    );
}

/// Variables generales que pertenecen a la estación lógica y no a una
/// tesela concreta. Las cadenas y fechas se conservan en `Station` para que
/// los contextos legacy y map-aware compartan exactamente la misma respuesta.
fn populate_station_general_vars(ctx: &mut Action2EvalCtx, station: &Station) {
    let mut acceptance_mask = 0u32;
    for cargo in ALL_CARGO_TYPES {
        if station.accepts_cargo(cargo) {
            acceptance_mask |= 1_u32 << cargo.cargo_id();
        }
    }
    ctx.vars.insert(0x48, acceptance_mask);
    ctx.vars.insert(0x82, 50);
    // `Station::had_vehicle_of_type` is a persistent history, not the last
    // vehicle that loaded cargo. Waypoints expose their dedicated bit even
    // when no load/unload path exists.
    ctx.vars.insert(0x8A, station.had_vehicle_of_type_value());
    ctx.vars.insert(0x84, station.newgrf_string_id_value());
    ctx.vars.insert(0xFA, station.newgrf_build_date_value());
    ctx.vars.insert(0x86, 0);
    let airport_type = station.airport_ttd_type.map_or_else(
        || u32::from(station.airport_spec.as_ttd_airport_type()),
        u32::from,
    );
    ctx.vars.insert(0xF1, airport_type);
    let airport_blocks = station.airport_blocks;
    ctx.vars.insert(
        0xF6,
        u32::try_from(airport_blocks & u64::from(u32::MAX)).unwrap_or(u32::MAX),
    );
    ctx.vars.insert(
        0xF7,
        u32::try_from((airport_blocks >> 8) & u64::from(u8::MAX)).unwrap_or(0),
    );
    ctx.vars.insert(
        0xF2,
        station.road_stop_status_value(crate::station::StopKind::TruckStop),
    );
    ctx.vars.insert(
        0xF3,
        station.road_stop_status_value(crate::station::StopKind::BusStop),
    );
    ctx.vars.insert(0xF0, station.stop_kind.facilities_mask());
}

#[allow(clippy::too_many_arguments)]
fn action2_eval_ctx_for_station_tile_impl(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    world: Option<StationAction2WorldContext<'_>>,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let Some(st) = station_at_tile(map, stations, coord) else {
        return ctx;
    };
    populate_station_general_vars(&mut ctx, st);
    let tile = map.get(coord);
    let m5 = tile.map_or(0, |t| t.m5);
    let m6 = tile.map_or(0, |t| t.m6);
    let (tileh, z) = tile_slope_and_z(map, coord).unwrap_or((0, 0));

    let random = u32::from(st.newgrf_random_bits);
    ctx.random_bits = random;
    ctx.vars.insert(
        0x5F,
        random << 8 | u32::from(st.newgrf_waiting_random_triggers),
    );

    let nn_player = u32::from(st.owner.0);
    let c = u32::from(owner_colour & 0x0F);
    let var43 = nn_player | ((c | (c << 4)) << 24);
    ctx.vars.insert(0x43, var43);

    // `StationScopeResolver::GetVariable(0x44)`: rail station/waypoint PBS
    // status (`HasStationReservation`), con los valores de compra `2` y de
    // tesela `4`/`7` que usa OpenTTD. El bit vive en m6 junto al tipo de parada.
    let station_type = station_type_from_m6(m6);
    let var44 = if matches!(station_type, 0 | STATION_TYPE_RAIL_WAYPOINT) {
        if m6 & STATION_TILE_RESERVATION != 0 {
            7
        } else {
            4
        }
    } else {
        2
    };
    ctx.vars.insert(0x44, var44);
    if matches!(station_type, 0 | STATION_TYPE_RAIL_WAYPOINT) {
        ctx.vars
            .insert(0x45, rail_continuation_info(map, coord, m5));
    }

    // Var 10: info adicional (m5 + tileh) para selección de sprites.
    ctx.vars
        .insert(0x10, u32::from(m5) | (u32::from(tileh) << 8));
    // `StationResolverObject::GetVariable(0x4A)`: frame persistido en MAP7.
    // También alimenta la selección Action2 del renderer después de CB140–142.
    ctx.vars.insert(0x4A, tile.map_or(0, |t| u32::from(t.m7)));

    ctx.vars
        .insert(0x40, platform_info_for_tile(map, stations, coord, m5));
    if matches!(station_type, 0 | STATION_TYPE_RAIL_WAYPOINT) {
        ctx.vars.insert(
            0x46,
            platform_info_for_tile_variant(map, stations, coord, m5, true, false),
        );
        // `Station` conserva un único StationSpecId para toda la huella; por
        // eso el filtro de tipo de 0x47 es hoy idéntico al de 0x46. La
        // diferencia reaparecerá cuando el importador preserve specs por tile.
        ctx.vars.insert(
            0x47,
            platform_info_for_tile_variant(map, stations, coord, m5, true, false),
        );
        ctx.vars.insert(
            0x49,
            platform_info_for_tile_variant(map, stations, coord, m5, false, true),
        );
    }

    let terrain = terrain_type_for_tile(map, coord, climate, tile);
    let rail_tt = tile.map_or(0xFF_u32, |t| {
        if t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0 {
            u32::from(reverse_rail_type(type_tables, rail_type_from_tile(t)))
        } else {
            0xFF
        }
    });
    ctx.vars.insert(0x42, terrain | (rail_tt << 8));

    // Var 67 param 0: land info de la tesela actual (sin offsets).
    let tile_type_byte = tile.map_or(0u32, |t| u32::from(tile_kind_as_ottd(t.kind)));
    let land = tile_type_byte << 24 | u32::from(z) << 16 | (terrain << 2) << 8 | u32::from(tileh);
    ctx.vars.insert(0x67, land);

    let coverage =
        world.map(|world| crate::station::station_coverage_for(map, world.industries, st));
    let cargo_catalog = world.map_or(&[][..], |world| world.cargo_spec_catalog);
    if cargo_catalog.is_empty() {
        populate_station_cargo_vars(&mut ctx, st, type_tables, grf_version, climate, coverage);
    } else {
        populate_station_cargo_vars_with_catalog(
            &mut ctx,
            st,
            type_tables,
            grf_version,
            climate,
            coverage,
            cargo_catalog,
        );
    }
    populate_station_legacy_cargo_vars(&mut ctx, st);

    ctx
}

/// Materializa las variables de carga deprecated `0x8C..0xEC` de
/// `Station::GetNewGRFVariable`. A diferencia de `0x60..0x69`, estas variables
/// codifican directamente las doce ranuras nativas y no pasan por la CTT del
/// GRF. Mantenerlas en `vars` permite que las APIs legacy y map-aware
/// compartan el mismo valor sin inventar IDs para cargos custom.
fn populate_station_legacy_cargo_vars(ctx: &mut Action2EvalCtx, station: &Station) {
    for cargo_index in 0..crate::cargo::NUM_ORIGINAL_CARGO {
        let Ok(cargo_id) = u8::try_from(cargo_index) else {
            continue;
        };
        let Some(cargo) = CargoType::from_cargo_id(cargo_id) else {
            continue;
        };
        let entry = station.goods.get(cargo);
        let total = station.cargo_stock.get(cargo);
        let accepted = cargo_is_accepted(station, cargo, None);
        let packed_total = total.min(4095);
        let first_station = station_first_cargo_station_id(station, cargo);
        let periods_in_transit = station
            .cargo_packets
            .packets()
            .filter(|packet| packet.cargo == cargo)
            .map(|packet| u32::from(packet.periods_in_transit))
            .max()
            .unwrap_or(0);
        let base = 0x8C_u8.saturating_add(cargo_id.saturating_mul(8));
        ctx.vars.insert(base, total);
        ctx.vars.insert(
            base.saturating_add(1),
            (packed_total & 0x0F) | if accepted { 1 << 7 } else { 0 },
        );
        ctx.vars.insert(
            base.saturating_add(2),
            u32::from(station.time_since_pickup.get(cargo)),
        );
        ctx.vars
            .insert(base.saturating_add(3), u32::from(entry.rating));
        ctx.vars.insert(base.saturating_add(4), first_station);
        ctx.vars.insert(base.saturating_add(5), periods_in_transit);
        ctx.vars
            .insert(base.saturating_add(6), u32::from(entry.last_speed));
        ctx.vars
            .insert(base.saturating_add(7), u32::from(entry.last_age));
    }
}

/// Devuelve el primer `StationID` sólo cuando el modelo puede demostrar que el
/// packet nació en esta estación. Los packets conservan coordenadas y no un
/// pool index, por lo que una ruta cuyo origen no coincide usa el sentinel
/// `StationID::Invalid()` nativo en vez de adivinar una identidad.
fn station_first_cargo_station_id(station: &Station, cargo: CargoType) -> u32 {
    let Some(station_id) = station.ottd_station_id else {
        return u32::MAX;
    };
    let has_first_station = station
        .cargo_packets
        .packets()
        .any(|packet| packet.cargo == cargo && packet.first_station == Some(station.pos));
    if has_first_station {
        station_id
    } else {
        u32::MAX
    }
}

/// Materializa las variables de carga parametrizadas que puede consultar el
/// Action2 de una estación. Los ids locales se generan con la misma CTT y
/// fallback de versión que `param2` de CB140; los slots desconocidos (cargos
/// definidos por un GRF y ausentes del modelo) quedan deliberadamente sin
/// valor en vez de reutilizar otro cargo.
#[allow(clippy::large_types_passed_by_value)]
pub(crate) fn populate_station_cargo_vars(
    ctx: &mut Action2EvalCtx,
    station: &Station,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    climate: Climate,
    coverage: Option<StationCoverage>,
) {
    populate_station_cargo_vars_with_catalog(
        ctx,
        station,
        type_tables,
        grf_version,
        climate,
        coverage,
        &[],
    );
}

/// Variante que incluye los cargos definidos por `NewGRF` en el catálogo de
/// la partida. Las variables parametrizadas siguen usando sólo los ids que
/// tienen una ida y vuelta válida en la CTT, igual que el camino vanilla.
#[allow(clippy::large_types_passed_by_value)]
pub(crate) fn populate_station_cargo_vars_with_catalog(
    ctx: &mut Action2EvalCtx,
    station: &Station,
    type_tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    climate: Climate,
    coverage: Option<StationCoverage>,
    cargo_catalog: &[CargoSpecDef],
) {
    let mut cargos = Vec::with_capacity(ALL_CARGO_TYPES.len() + cargo_catalog.len());
    cargos.extend(ALL_CARGO_TYPES);
    for cargo in cargo_catalog.iter().filter_map(CargoSpecDef::cargo_type) {
        if !cargos.contains(&cargo) {
            cargos.push(cargo);
        }
    }
    for cargo in cargos {
        let local_id =
            local_cargo_id_with_catalog(type_tables, grf_version, cargo, climate, cargo_catalog);
        if local_id == 0xFF
            || cargo_from_local_id_with_catalog(
                type_tables,
                grf_version,
                local_id,
                climate,
                cargo_catalog,
            ) != Some(cargo)
        {
            continue;
        }
        for variable in [0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x69] {
            ctx.parameterized_vars.insert(
                (variable, local_id),
                station_cargo_var(station, cargo, variable, coverage),
            );
        }
    }
}

#[allow(clippy::large_types_passed_by_value)]
fn station_cargo_var(
    station: &Station,
    cargo: CargoType,
    variable: u8,
    coverage: Option<StationCoverage>,
) -> u32 {
    let entry = station.goods.get(cargo);
    match variable {
        // `GoodsEntry::TotalCount`, capped to the 12-bit Action2 contract.
        0x60 => station.cargo_stock.get(cargo).min(4095),
        0x61 => u32::from(station.time_since_pickup.get(cargo)),
        0x62 => {
            if entry.has_rating {
                u32::from(entry.rating)
            } else {
                u32::MAX
            }
        }
        // The packet queue retains the same maximum transit-period statistic
        // used by the cargo rating path; legacy stock-only saves naturally
        // return zero until their packets are hydrated.
        0x63 => station
            .cargo_packets
            .packets()
            .filter(|packet| packet.cargo == cargo)
            .map(|packet| u32::from(packet.periods_in_transit))
            .max()
            .unwrap_or(0),
        0x64 => {
            if entry.has_vehicle_ever_tried_loading() {
                u32::from(entry.last_speed) | (u32::from(entry.last_age) << 8)
            } else {
                0xFF00
            }
        }
        // GoodsEntry::Acceptance is driven by the catchment amount in
        // OpenTTD. When the caller has the live map/industry pools, use that
        // amount; legacy contexts retain the persisted type-only predicate.
        0x65 => u32::from(cargo_is_accepted(station, cargo, coverage)) << 3,
        0x69 => u32::from(entry.convert_state()),
        _ => 0,
    }
}

#[allow(clippy::large_types_passed_by_value)]
fn cargo_is_accepted(
    station: &Station,
    cargo: CargoType,
    coverage: Option<StationCoverage>,
) -> bool {
    let Some(coverage) = coverage else {
        return station.accepts_cargo(cargo);
    };
    if !station.accepts_cargo(cargo) {
        return false;
    }
    if coverage.exact_cargo_acceptance {
        return coverage.accepted_cargo.get(cargo)
            >= crate::house_spec::STATION_ACCEPTANCE_THRESHOLD;
    }
    let amount = match cargo {
        CargoType::Passengers => coverage.accepts_passengers,
        CargoType::Mail => coverage.accepts_mail,
        CargoType::Water => coverage.accepts_water,
        _ => coverage.accepts_goods,
    };
    amount >= crate::house_spec::STATION_ACCEPTANCE_THRESHOLD
}

fn tile_kind_as_ottd(kind: TileKind) -> u8 {
    match kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
        TileKind::House => 3,
        TileKind::Forest => 4, // MP_TREES
        TileKind::Station | TileKind::Airport => 5,
        TileKind::Water | TileKind::ShipDepot => 6,
        TileKind::Void => 7,
        TileKind::Industry => 8,
        // MP_CLEAR y desconocidos
        TileKind::Grass | TileKind::CoalField | TileKind::Unknown(_) => 0,
    }
}

fn terrain_type_for_tile(
    map: &Map,
    coord: TileCoord,
    climate: Climate,
    tile: Option<crate::map::Tile>,
) -> u32 {
    if climate.uses_snow_ground() {
        return 4;
    }
    if climate.uses_desert_patches() {
        // Aproximación: bit MAP7 nieve/desierto en road; en clear m5 desert.
        if let Some(t) = tile {
            if t.kind == TileKind::Road && (t.m7 & 0x20) != 0 {
                return 1;
            }
            if t.kind == TileKind::Grass {
                let ground = t.m5 & 0x07;
                if ground == crate::world_gen::CLEAR_GROUND_DESERT {
                    return 1;
                }
            }
        }
        // Vecino clear desert (estación suele tapar el clear).
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(n).is_some_and(|t| {
                t.kind == TileKind::Grass && (t.m5 & 0x07) == crate::world_gen::CLEAR_GROUND_DESERT
            }) {
                return 1;
            }
        }
    }
    0
}

fn is_rail_platform_tile(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .is_some_and(|t| t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0)
}

fn same_station(a: &Station, b: &Station) -> bool {
    a.pos == b.pos && a.stop_kind == b.stop_kind
}

fn find_rail_station_end(
    map: &Map,
    stations: &[Station],
    start: TileCoord,
    dx: i32,
    dy: i32,
    check_axis: bool,
) -> TileCoord {
    let Some(st) = station_at_tile(map, stations, start) else {
        return start;
    };
    let axis_y = map.get(start).is_some_and(|tile| tile.m5 & 1 != 0);
    let mut tile = start;
    loop {
        let next = TileCoord::new(tile.x + dx, tile.y + dy);
        if !is_rail_platform_tile(map, next) {
            break;
        }
        let Some(other) = station_at_tile(map, stations, next) else {
            break;
        };
        if !same_station(st, other) {
            break;
        }
        if check_axis && map.get(next).is_some_and(|candidate| candidate.m5 & 1 != 0) != axis_y {
            break;
        }
        tile = next;
    }
    tile
}

/// Replica `GetRailContinuationInfo` de `newgrf_station.cpp`.
///
/// Los ocho vecinos se mantienen en el orden de las tablas de `OpenTTD`. El
/// byte alto marca que el vecino tiene alguna vía; el byte bajo marca además
/// que esa vía alcanza la salida diagonal correspondiente de la plataforma.
fn rail_continuation_info(map: &Map, coord: TileCoord, m5: u8) -> u32 {
    // `TileOffsByDir` + `DiagdirReachesTracks` de OpenTTD, separados por eje.
    const X_NEIGHBOURS: [(i32, i32, u8); 8] = [
        (1, 0, 2),
        (-1, 0, 0),
        (0, 1, 1),
        (0, -1, 3),
        (1, 1, 2),
        (-1, 1, 0),
        (1, -1, 2),
        (-1, -1, 0),
    ];
    const Y_NEIGHBOURS: [(i32, i32, u8); 8] = [
        (0, 1, 1),
        (0, -1, 3),
        (1, 0, 2),
        (-1, 0, 0),
        (1, 1, 1),
        (1, -1, 3),
        (-1, 1, 1),
        (-1, -1, 3),
    ];
    let neighbours = if m5 & 1 != 0 {
        &Y_NEIGHBOURS
    } else {
        &X_NEIGHBOURS
    };

    let mut result = 0u32;
    for (index, &(dx, dy, exit)) in neighbours.iter().enumerate() {
        let neighbour = TileCoord::new(coord.x + dx, coord.y + dy);
        let tracks = rail_traversal_bits(map, neighbour);
        if tracks == 0 {
            continue;
        }
        result |= 1 << (index + 8);
        if tracks & rail_bits_touching_side(exit) != 0 {
            result |= 1 << index;
        }
    }
    result
}

fn pack_platform_info(gfx: u8, platforms: i32, length: i32, platform: i32, position: i32) -> u32 {
    let mut retval = 0u32;
    let len = length.max(1);
    let plats = platforms.max(1);
    let p = position.clamp(0, 15).cast_unsigned();
    let plat = platform.clamp(0, 15).cast_unsigned();
    retval |= p; // P
    retval |= (len - position - 1).clamp(0, 15).cast_unsigned() << 4; // p
    retval |= plat << 8; // C
    retval |= (plats - platform - 1).clamp(0, 15).cast_unsigned() << 12; // c
    retval |= len.min(15).cast_unsigned() << 16; // L
    retval |= plats.min(15).cast_unsigned() << 20; // N
    retval |= u32::from(gfx) << 24; // T
    retval
}

fn pack_platform_info_centered(
    gfx: u8,
    platforms: i32,
    length: i32,
    platform: i32,
    position: i32,
) -> u32 {
    let x = (platform - platforms / 2).clamp(-8, 7).cast_unsigned() & 0x0F;
    let y = (position - length / 2).clamp(-8, 7).cast_unsigned() & 0x0F;
    let mut retval = y | (x << 4);
    retval |= length.min(15).cast_unsigned() << 16;
    retval |= platforms.min(15).cast_unsigned() << 20;
    retval |= u32::from(gfx) << 24;
    retval
}

fn platform_info_for_tile(map: &Map, stations: &[Station], coord: TileCoord, m5: u8) -> u32 {
    platform_info_for_tile_variant(map, stations, coord, m5, false, false)
}

fn platform_info_for_tile_variant(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    m5: u8,
    centered: bool,
    check_axis: bool,
) -> u32 {
    if !is_rail_platform_tile(map, coord) {
        // Waypoints / no-rail: layout 1×1.
        return if centered {
            pack_platform_info_centered(m5 & 0x3F, 1, 1, 0, 0)
        } else {
            pack_platform_info(m5 & 0x3F, 1, 1, 0, 0)
        };
    }
    let end = |dx: i32, dy: i32| find_rail_station_end(map, stations, coord, dx, dy, check_axis);
    let sx = end(-1, 0).x;
    let sy = end(0, -1).y;
    let ex = end(1, 0).x + 1;
    let ey = end(0, 1).y + 1;

    let mut tx = coord.x - sx;
    let mut ty = coord.y - sy;
    let mut width = ex - sx;
    let mut height = ey - sy;

    let axis_y = m5 & 1 != 0;
    // Axis X: longitud en X, andenes en Y → swap como OpenTTD.
    if !axis_y {
        std::mem::swap(&mut width, &mut height);
        std::mem::swap(&mut tx, &mut ty);
    }

    if centered {
        pack_platform_info_centered(m5 & 0x3F, width, height, tx, ty)
    } else {
        pack_platform_info(m5 & 0x3F, width, height, tx, ty)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::airport_class::AirportSpecId;
    use crate::cargo_packet::CargoPacket;
    use crate::company::CompanyId;
    use crate::map::{Map, Tile, TileKind};
    use crate::newgrf_callback::action2_eval_ctx_from_station;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
    };
    use crate::station::{Station, StopKind};
    use crate::vehicle::VehicleKind;

    fn rail_station_tile(m5: u8) -> Tile {
        Tile {
            height: 0,
            kind: TileKind::Station,
            mapt: 0,
            m5,
            m1: 0,
            m6: 0, // rail
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    fn station_neighbour_runtime(
        vars: &[(u8, u8)],
    ) -> Box<crate::newgrf_sprites::TrainSpriteGraphics> {
        let mut runtime = crate::newgrf_sprites::TrainSpriteGraphics::default();
        runtime.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        for (index, &(variable, parameter)) in vars.iter().enumerate() {
            runtime.action2_var.insert(
                u8::try_from(2 + index).unwrap_or(u8::MAX),
                Action2VarEntry {
                    first: Action2VarTerm {
                        variable,
                        param: Some(parameter),
                        adjust: Action2VarAdjust {
                            and_mask: u32::MAX,
                            ..Action2VarAdjust::default()
                        },
                    },
                    ops: Vec::new(),
                    ranges: Vec::new(),
                    default: 0,
                },
            );
        }
        Box::new(runtime)
    }

    fn station_spec(
        id: u16,
        grfid: u32,
        local_id: u8,
        runtime: Box<crate::newgrf_sprites::TrainSpriteGraphics>,
    ) -> StationSpecDef {
        StationSpecDef {
            id: crate::station_class::StationSpecId::from_u16(id),
            class: crate::station_class::StationClassId::from_u16(1),
            label: format!("Station {id}"),
            short_label: format!("S{id}"),
            disallowed_platforms: 0,
            disallowed_lengths: 0,
            callback_mask: 0,
            flags: 0,
            animation_status: 0,
            animation_frames: 0,
            animation_speed: 0,
            animation_triggers: 0,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: local_id,
            newgrf_runtime: Some(runtime),
            newgrf_grfid: grfid,
            newgrf_grf_version: 8,
            newgrf_type_tables: None,
            custom_layouts: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn station_dynamic_vars_share_action2_eval_ctx_for_228() {
        // #228: vars dinámicas de estación alimentan el mismo `Action2EvalCtx`
        // que el resolver variational/callback (no un camino paralelo).
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        let mut tile = rail_station_tile(2);
        tile.m7 = 9;
        map.set_tile(c, tile).unwrap();
        let mut st = Station::new_with_kind(c, StopKind::RailStation);
        st.newgrf_random_bits = 0x42;
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], c, 1, Climate::Temperate, None);
        assert!(ctx.vars.contains_key(&0x40));
        assert!(ctx.vars.contains_key(&0x42));
        assert!(ctx.vars.contains_key(&0x43));
        assert!(ctx.vars.contains_key(&0x4A));
        assert!(ctx.vars.contains_key(&0x5F));
        assert_eq!(ctx.vars.get(&0x48).map(|value| value & 0b111), Some(0b010));
        assert_eq!(ctx.vars.get(&0x82), Some(&50));
        assert_eq!(ctx.vars.get(&0x86), Some(&0));
        assert_eq!(ctx.vars.get(&0xF0), Some(&1));
        assert!(ctx.vars.contains_key(&0x10));
        assert!(ctx.vars.contains_key(&0x67));
        assert_eq!(ctx.random_bits, 0x42);
        assert_eq!(ctx.vars.get(&0x4A), Some(&9));
    }

    #[test]
    fn station_ctx_resolves_neighbour_vars_with_wrap_and_grf_identity() {
        let mut map = Map::new_flat(8, 8, 0);
        let source = TileCoord::new(1, 1);
        let same = TileCoord::new(2, 1);
        let other = TileCoord::new(3, 1);
        map.set_tile(source, rail_station_tile(0)).unwrap();
        let mut same_tile = rail_station_tile(2);
        same_tile.m7 = 7;
        map.set_tile(same, same_tile).unwrap();
        let mut other_tile = rail_station_tile(1);
        other_tile.m7 = 9;
        map.set_tile(other, other_tile).unwrap();

        let mut current = Station::new_with_kind(source, StopKind::RailStation);
        current.station_spec = crate::station_class::StationSpecId::from_u16(1);
        let mut other_station = Station::new_with_kind(other, StopKind::RailStation);
        other_station.station_spec = crate::station_class::StationSpecId::from_u16(2);
        let stations = vec![current, other_station];
        let runtime = station_neighbour_runtime(&[
            (0x66, 0x01),
            (0x67, 0x01),
            (0x68, 0x01),
            (0x6A, 0x01),
            (0x6B, 0x01),
            (0x66, 0x02),
            (0x68, 0x02),
            (0x6A, 0x02),
            (0x6B, 0x02),
            (0x66, 0xFF),
            (0x68, 0xFF),
        ]);
        let catalog = vec![
            station_spec(1, 0x1111_0001, 4, runtime),
            station_spec(2, 0x2222_0002, 7, station_neighbour_runtime(&[])),
        ];
        let ctx = action2_eval_ctx_for_station_tile_with_catalog(
            &map,
            &stations,
            &catalog,
            source,
            0,
            Climate::Temperate,
            None,
            8,
        );

        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0x01)), Some(&7));
        assert!(ctx.parameterized_vars.contains_key(&(0x67, 0x01)));
        assert_eq!(ctx.parameterized_vars.get(&(0x68, 0x01)), Some(&0x1504));
        assert_eq!(
            ctx.parameterized_vars.get(&(0x6A, 0x01)),
            Some(&0x1111_0001)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x6B, 0x01)), Some(&4));

        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0x02)), Some(&u32::MAX));
        assert_eq!(ctx.parameterized_vars.get(&(0x68, 0x02)), Some(&0x0A07));
        assert_eq!(
            ctx.parameterized_vars.get(&(0x6A, 0x02)),
            Some(&0x2222_0002)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x6B, 0x02)), Some(&0xFFFE));
        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0xFF)), Some(&u32::MAX));
        assert_eq!(ctx.parameterized_vars.get(&(0x68, 0xFF)), Some(&u32::MAX));
    }

    #[test]
    fn station_ctx_var40_single_tile() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        map.set_tile(c, rail_station_tile(0)).unwrap();
        let mut st = Station::new_with_kind(c, StopKind::RailStation);
        st.newgrf_random_bits = 0xAB;
        st.owner = CompanyId(2);
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], c, 4, Climate::Temperate, None);
        assert_eq!(ctx.random_bits, 0xAB);
        assert_eq!(ctx.vars.get(&0x5F), Some(&(0xAB << 8)));
        let v40 = *ctx.vars.get(&0x40).unwrap();
        assert_eq!(v40 & 0x0F, 0, "P=0");
        assert_eq!((v40 >> 16) & 0x0F, 1, "L=1");
        assert_eq!((v40 >> 20) & 0x0F, 1, "N=1");
        let v43 = *ctx.vars.get(&0x43).unwrap();
        assert_eq!(v43 & 0xFF, 2);
        assert_eq!((v43 >> 24) & 0xFF, 0x44);
        assert_eq!(ctx.vars.get(&0x42), Some(&0)); // grass + rail 0
        assert_eq!(ctx.vars.get(&0x4A), Some(&0)); // frame MAP7
    }

    #[test]
    fn station_general_vars_encode_acceptance_and_facilities() {
        for (kind, facilities, low_mask) in [
            (StopKind::BusStop, 1_u32 << 2, 0b101_u32),
            (StopKind::TruckStop, 1_u32 << 1, 0b010_u32),
            (StopKind::Dock, 1_u32 << 4, 0b111_u32),
            (StopKind::Airport, 1_u32 << 3, 0b101_u32),
            (StopKind::RailWaypoint, (1_u32 << 0) | (1_u32 << 7), 0),
            (
                StopKind::RoadWaypoint,
                (1_u32 << 1) | (1_u32 << 2) | (1_u32 << 7),
                0,
            ),
        ] {
            let station = Station::new_with_kind(TileCoord::new(1, 1), kind);
            let ctx = action2_eval_ctx_from_station(&station);
            assert_eq!(ctx.vars.get(&0xF0), Some(&facilities));
            assert_eq!(
                ctx.vars.get(&0x48).map(|value| value & 0b111),
                Some(low_mask)
            );
        }
    }

    #[test]
    fn station_road_stop_status_vars_follow_native_stop_kind() {
        let mut truck = Station::new_with_kind(TileCoord::new(1, 1), StopKind::TruckStop);
        truck.road_stop_status = 0xC1;
        let truck_ctx = action2_eval_ctx_from_station(&truck);
        assert_eq!(truck_ctx.vars.get(&0xF2), Some(&0xC1));
        assert_eq!(truck_ctx.vars.get(&0xF3), Some(&0));

        let mut bus = Station::new_with_kind(TileCoord::new(2, 2), StopKind::BusStop);
        bus.road_stop_status = 0x42;
        let bus_ctx = action2_eval_ctx_from_station(&bus);
        assert_eq!(bus_ctx.vars.get(&0xF2), Some(&0));
        assert_eq!(bus_ctx.vars.get(&0xF3), Some(&0x42));

        let rail = Station::new_with_kind(TileCoord::new(3, 3), StopKind::RailStation);
        let rail_ctx = action2_eval_ctx_from_station(&rail);
        assert_eq!(rail_ctx.vars.get(&0xF2), Some(&0));
        assert_eq!(rail_ctx.vars.get(&0xF3), Some(&0));
    }

    #[test]
    fn station_var_8a_tracks_vehicle_history_and_json_roundtrip() {
        let mut station = Station::new_with_kind(TileCoord::new(1, 1), StopKind::RailStation);
        assert_eq!(
            action2_eval_ctx_from_station(&station).vars.get(&0x8A),
            Some(&0)
        );

        for kind in [
            VehicleKind::Train,
            VehicleKind::Bus,
            VehicleKind::Truck,
            VehicleKind::Aircraft,
            VehicleKind::Ship,
        ] {
            station.mark_vehicle_of_type(kind);
        }
        assert_eq!(station.had_vehicle_of_type_value(), 0x3E);
        let ctx = action2_eval_ctx_from_station(&station);
        assert_eq!(ctx.vars.get(&0x8A), Some(&0x3E));

        let encoded = serde_json::to_string(&station).expect("station JSON");
        let decoded: Station = serde_json::from_str(&encoded).expect("station JSON roundtrip");
        assert_eq!(decoded.had_vehicle_of_type, 0x3E);

        let waypoint = Station::new_with_kind(TileCoord::new(2, 2), StopKind::RailWaypoint);
        assert_eq!(
            action2_eval_ctx_from_station(&waypoint).vars.get(&0x8A),
            Some(&0x40)
        );
    }

    #[test]
    fn station_string_id_and_build_date_vars_follow_native_values() {
        let mut station = Station::new_with_kind(TileCoord::new(1, 1), StopKind::RailStation);
        station.newgrf_string_id = crate::station::STATION_STRING_ID_FALLBACK;
        station.build_date = crate::station::STATION_BUILD_DATE_DEFAULT + 321;

        let ctx = action2_eval_ctx_from_station(&station);
        assert_eq!(ctx.vars.get(&0x84), Some(&0x6027));
        assert_eq!(ctx.vars.get(&0xFA), Some(&321));

        let encoded = serde_json::to_string(&station).expect("station JSON");
        let decoded: Station = serde_json::from_str(&encoded).expect("station JSON roundtrip");
        assert_eq!(
            decoded.newgrf_string_id,
            crate::station::STATION_STRING_ID_FALLBACK
        );
        assert_eq!(decoded.newgrf_build_date_value(), 321);
    }

    #[test]
    fn station_general_vars_encode_airport_type_and_blocks() {
        let mut station = Station::new_with_kind(TileCoord::new(1, 1), StopKind::Airport);
        station.airport_spec = AirportSpecId::International;
        station.airport_blocks = 0x0001_2345;
        let legacy = action2_eval_ctx_from_station(&station);
        assert_eq!(legacy.vars.get(&0xF1), Some(&1));
        assert_eq!(legacy.vars.get(&0xF6), Some(&0x0001_2345));
        assert_eq!(legacy.vars.get(&0xF7), Some(&0x23));

        station.airport_newgrf_spec_id = Some(42);
        station.airport_ttd_type = Some(3);
        let mut map = Map::new_flat(4, 4, 0);
        map.set_tile(
            station.pos,
            Tile {
                height: 0,
                kind: TileKind::Station,
                mapt: 0,
                m5: 0,
                m1: 0,
                m6: 0,
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            },
        )
        .unwrap();
        let map_aware = action2_eval_ctx_for_station_tile(
            &map,
            std::slice::from_ref(&station),
            station.pos,
            0,
            Climate::Temperate,
            None,
        );
        assert_eq!(map_aware.vars.get(&0xF1), Some(&3));
        assert_eq!(map_aware.vars.get(&0xF6), Some(&0x0001_2345));
        assert_eq!(map_aware.vars.get(&0xF7), Some(&0x23));
    }

    #[test]
    fn station_ctx_var40_platform_length() {
        let mut map = Map::new_flat(10, 10, 0);
        // Eje X (m5 par): 3 teselas en X.
        for x in 2..5 {
            map.set_tile(TileCoord::new(x, 4), rail_station_tile(0))
                .unwrap();
        }
        let st = Station::new_with_kind(TileCoord::new(2, 4), StopKind::RailStation);
        let mid = TileCoord::new(3, 4);
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], mid, 0, Climate::Temperate, None);
        let v40 = *ctx.vars.get(&0x40).unwrap();
        assert_eq!(v40 & 0x0F, 1, "P=1 (medio)");
        assert_eq!((v40 >> 4) & 0x0F, 1, "p=1");
        assert_eq!((v40 >> 16) & 0x0F, 3, "L=3");
        assert_eq!((v40 >> 20) & 0x0F, 1, "N=1");
        let v46 = *ctx.vars.get(&0x46).unwrap();
        assert_eq!(
            v46 & 0xFF,
            0,
            "posición centrada en plataforma de longitud impar"
        );
        assert_eq!((v46 >> 16) & 0x0F, 3, "L centrada=3");
        assert_eq!((v46 >> 20) & 0x0F, 1, "N centrada=1");
        assert_eq!(
            ctx.vars.get(&0x47),
            Some(&v46),
            "spec homogéneo en la huella"
        );
        assert_eq!(
            ctx.vars.get(&0x49),
            Some(&v40),
            "var 49 conserva el eje homogéneo"
        );
    }

    #[test]
    fn station_ctx_snow_terrain() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        map.set_tile(c, rail_station_tile(0)).unwrap();
        let st = Station::new_with_kind(c, StopKind::RailStation);
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], c, 0, Climate::SubArctic, None);
        assert_eq!(ctx.vars.get(&0x42).map(|v| v & 0xFF), Some(4));
    }

    #[test]
    fn station_ctx_var44_reports_pbs_reservation_status() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        let mut tile = rail_station_tile(0);
        tile.m6 |= STATION_TILE_RESERVATION;
        map.set_tile(c, tile).unwrap();
        let st = Station::new_with_kind(c, StopKind::RailStation);
        let ctx = action2_eval_ctx_for_station_tile(
            &map,
            std::slice::from_ref(&st),
            c,
            0,
            Climate::Temperate,
            None,
        );
        assert_eq!(ctx.vars.get(&0x44), Some(&7));

        let mut free_tile = map.get(c).unwrap();
        free_tile.m6 &= !STATION_TILE_RESERVATION;
        map.set_tile(c, free_tile).unwrap();
        let ctx = action2_eval_ctx_for_station_tile(
            &map,
            std::slice::from_ref(&st),
            c,
            0,
            Climate::Temperate,
            None,
        );
        assert_eq!(ctx.vars.get(&0x44), Some(&4));

        let mut road_stop = rail_station_tile(0);
        road_stop.m6 = 3 << 3;
        map.set_tile(c, road_stop).unwrap();
        let ctx = action2_eval_ctx_for_station_tile(
            &map,
            &[Station::new_with_kind(c, StopKind::BusStop)],
            c,
            0,
            Climate::Temperate,
            None,
        );
        assert_eq!(ctx.vars.get(&0x44), Some(&2));
    }

    #[test]
    fn station_ctx_var45_reports_rail_continuation_bits() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        map.set_tile(c, rail_station_tile(0)).unwrap();

        let mut west = rail_station_tile(0);
        west.kind = TileKind::Rail;
        west.m5 = 0x01;
        map.set_tile(TileCoord::new(2, 3), west).unwrap();
        let east = west;
        map.set_tile(TileCoord::new(4, 3), east).unwrap();

        let station = Station::new_with_kind(c, StopKind::RailStation);
        let ctx =
            action2_eval_ctx_for_station_tile(&map, &[station], c, 0, Climate::Temperate, None);
        let continuation = *ctx.vars.get(&0x45).expect("var 45");
        assert_eq!(continuation & 0x03, 0x03, "ambos vecinos conectan");
        assert_eq!((continuation >> 8) & 0x03, 0x03, "ambos vecinos tienen vía");
        assert_eq!(continuation & !0x303, 0, "sin vecinos diagonales");
    }

    #[test]
    fn station_ctx_var42_uses_rail_translation() {
        use crate::newgrf_type_tables::GrfTypeTranslationTables;
        use crate::rail_type::{RailType, set_rail_type_on_tile};
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        let mut tile = rail_station_tile(0);
        tile = set_rail_type_on_tile(tile, RailType::Electric);
        map.set_tile(c, tile).unwrap();
        let st = Station::new_with_kind(c, StopKind::RailStation);
        let tables = GrfTypeTranslationTables {
            rail: vec![*b"MONO", *b"ELRL", *b"RAIL"],
            ..Default::default()
        };
        let ctx =
            action2_eval_ctx_for_station_tile(&map, &[st], c, 0, Climate::Temperate, Some(&tables));
        let v42 = *ctx.vars.get(&0x42).unwrap();
        assert_eq!((v42 >> 8) & 0xFF, 1); // ELRL at index 1
    }

    #[test]
    fn station_ctx_exposes_parameterized_cargo_scope() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        map.set_tile(c, rail_station_tile(0)).unwrap();
        let mut st = Station::new_with_kind(c, StopKind::RailStation);
        let mut packet = CargoPacket::new(CargoType::Coal, 23, c);
        packet.periods_in_transit = 6;
        st.push_waiting_packets([packet]);
        st.time_since_pickup.coal = 9;
        let entry = st.goods.get_mut(CargoType::Coal);
        entry.has_rating = true;
        entry.rating = 123;
        entry.last_speed = 77;
        entry.last_age = 4;
        entry.newgrf_state = 0b1101;

        let ctx = action2_eval_ctx_for_station_tile_with_grf(
            &map,
            &[st],
            c,
            0,
            Climate::Temperate,
            None,
            8,
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x60, 1)), Some(&23));
        assert_eq!(ctx.parameterized_vars.get(&(0x61, 1)), Some(&9));
        assert_eq!(ctx.parameterized_vars.get(&(0x62, 1)), Some(&123));
        assert_eq!(ctx.parameterized_vars.get(&(0x63, 1)), Some(&6));
        assert_eq!(ctx.parameterized_vars.get(&(0x64, 1)), Some(&1_101));
        assert_eq!(ctx.parameterized_vars.get(&(0x65, 1)), Some(&8));
        assert_eq!(ctx.parameterized_vars.get(&(0x69, 1)), Some(&13));
    }

    #[test]
    fn station_deprecated_cargo_vars_match_native_layout() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        map.set_tile(coord, rail_station_tile(0)).unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::RailStation);
        station.ottd_station_id = Some(17);
        station.cargo_stock.add(CargoType::Coal, 0x1234);
        station.time_since_pickup.coal = 9;
        let entry = station.goods.get_mut(CargoType::Coal);
        entry.rating = 123;
        entry.last_speed = 77;
        entry.last_age = 4;
        let mut packet = CargoPacket::new(CargoType::Coal, 23, coord).with_first_station(coord);
        packet.periods_in_transit = 6;
        station.push_waiting_packets([packet]);

        let legacy = action2_eval_ctx_from_station(&station);
        let map_aware = action2_eval_ctx_for_station_tile_with_grf(
            &map,
            std::slice::from_ref(&station),
            coord,
            0,
            Climate::Temperate,
            None,
            8,
        );
        for ctx in [&legacy, &map_aware] {
            assert_eq!(ctx.vars.get(&0x94), Some(&4683));
            assert_eq!(ctx.vars.get(&0x95), Some(&0x8F));
            assert_eq!(ctx.vars.get(&0x96), Some(&9));
            assert_eq!(ctx.vars.get(&0x97), Some(&123));
            assert_eq!(ctx.vars.get(&0x98), Some(&17));
            assert_eq!(ctx.vars.get(&0x99), Some(&6));
            assert_eq!(ctx.vars.get(&0x9A), Some(&77));
            assert_eq!(ctx.vars.get(&0x9B), Some(&4));
        }
        assert_eq!(legacy.vars.get(&0x8C), Some(&0));
        assert_eq!(legacy.vars.get(&0x8F), Some(&175));
        assert_eq!(legacy.vars.get(&0x90), Some(&u32::MAX));
    }

    #[test]
    fn station_world_scope_exposes_custom_cargo_through_ctt() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        map.set_tile(coord, rail_station_tile(0)).unwrap();
        let custom = CargoType::Custom(0);
        let mut station = Station::new_with_kind(coord, StopKind::RailStation);
        station.cargo_stock.add(custom, 17);
        station.push_waiting_packets([CargoPacket::new(custom, 5, coord)]);
        let tables = GrfTypeTranslationTables {
            cargo: vec![*b"TOFU"],
            ..GrfTypeTranslationTables::default()
        };
        let cargo_catalog = vec![CargoSpecDef {
            id: crate::cargo::CUSTOM_CARGO_OFFSET,
            label: "TOFU".into(),
            from_newgrf: true,
            ..CargoSpecDef::default()
        }];
        let ctx = action2_eval_ctx_for_station_tile_with_world(
            &map,
            std::slice::from_ref(&station),
            coord,
            0,
            Climate::Temperate,
            Some(&tables),
            8,
            StationAction2WorldContext {
                industries: &[],
                cargo_spec_catalog: &cargo_catalog,
            },
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x60, 0)), Some(&22));
        assert_eq!(ctx.parameterized_vars.get(&(0x63, 0)), Some(&0));
    }

    #[test]
    fn station_world_scope_uses_live_catchment_for_acceptance() {
        let mut map = Map::new_flat(5, 5, 0);
        let coord = TileCoord::new(2, 2);
        map.set_tile(coord, rail_station_tile(0)).unwrap();
        let station = Station::new_with_kind(coord, StopKind::BusStop);

        let legacy = action2_eval_ctx_for_station_tile_with_grf(
            &map,
            std::slice::from_ref(&station),
            coord,
            0,
            Climate::Temperate,
            None,
            8,
        );
        assert_eq!(legacy.parameterized_vars.get(&(0x65, 0)), Some(&8));

        let world_without_house = action2_eval_ctx_for_station_tile_with_world(
            &map,
            std::slice::from_ref(&station),
            coord,
            0,
            Climate::Temperate,
            None,
            8,
            StationAction2WorldContext {
                industries: &[],
                cargo_spec_catalog: &[],
            },
        );
        assert_eq!(
            world_without_house.parameterized_vars.get(&(0x65, 0)),
            Some(&0)
        );

        let mut house = map.get(TileCoord::new(3, 2)).unwrap();
        house.kind = TileKind::House;
        map.set_tile(TileCoord::new(3, 2), house).unwrap();
        let world_with_house = action2_eval_ctx_for_station_tile_with_world(
            &map,
            std::slice::from_ref(&station),
            coord,
            0,
            Climate::Temperate,
            None,
            8,
            StationAction2WorldContext {
                industries: &[],
                cargo_spec_catalog: &[],
            },
        );
        assert_eq!(
            world_with_house.parameterized_vars.get(&(0x65, 0)),
            Some(&8)
        );
    }
}
