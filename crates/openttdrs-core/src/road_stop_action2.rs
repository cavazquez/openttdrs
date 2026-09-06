//! Contexto Action2 para sprites runtime de `RoadStop`.
//!
//! El resolver conserva los valores propios de una parada vial que ya están
//! representados por el modelo: vista, tipo, terreno, road/tram, frame,
//! random y triggers pendientes. Para el renderer que conoce el catálogo,
//! también materializa las consultas parametrizadas a teselas vecinas
//! (`66`, `67`, `68`, `6A`, `6B`) de `RoadStopScopeResolver`.

use std::collections::BTreeSet;

use crate::cargo_spec::CargoSpecDef;
use crate::company::{Company, CompanyId, newgrf_company_info};
use crate::house_spec::{distance_square, get_town_radius_group};
use crate::industry::Industry;
use crate::map::{
    Map, TILE_PIXEL_HEIGHT, Tile, TileCoord, TileKind, tile_slope_and_z, water_class,
};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{GrfTypeTranslationTables, reverse_road_type};
use crate::road_stop_spec::{RoadStopSpecDef, road_stop_spec_def};
use crate::road_type::{
    RoadTypeDef, road_type_from_tile, tram_road_type_from_tile, vanilla_road_type_catalog,
};
use crate::station::{Station, StopKind, station_at_tile};
use crate::station_action2::populate_station_cargo_vars_with_catalog;
use crate::town::{HouseZone, Town};
use crate::world_gen::Climate;

/// Construye el contexto Action2 de una tesela `RoadStop` para render runtime.
///
/// Corresponde al subconjunto local de `RoadStopScopeResolver`: `40` (vista),
/// `41` (tipo), `42` (terreno/pendiente), `43`/`44` (road/tram), `49`
/// (frame), `50` (instancia in-world) y `5F` (random + triggers). La
/// traducción de road/tram usa los tipos vanilla disponibles en core; si el
/// save contiene un tipo externo que no está en ese catálogo queda en
/// `0xFF`, de forma observable y sin inventar una identidad local.
///
/// Esta variante conserva la API previa para callers que sólo tienen las
/// tablas de tipo. Para resolver los offsets parametrizados de Action2 usar
/// [`action2_eval_ctx_for_road_stop_tile_with_catalog`], que es la ruta del
/// renderer.
#[must_use]
pub fn action2_eval_ctx_for_road_stop_tile(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    view: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
) -> Action2EvalCtx {
    action2_eval_ctx_for_road_stop_tile_impl(
        map,
        stations,
        RoadStopAction2Resolution {
            road_stop_catalog: &[],
            road_type_catalog: &[],
            current_spec: None,
            type_tables,
            world: None,
        },
        coord,
        view,
        climate,
    )
}

/// Construye el contexto Action2 completo de una tesela `RoadStop`.
///
/// Además de las variables locales, inspecciona los Action2 del spec activo
/// para llenar sólo los pares `(variable, parámetro)` de vecindad que el GRF
/// realmente consulta. Así `68[01]` y `68[0F]` pueden coexistir en la misma
/// evaluación sin convertir todos los 256 offsets posibles en trabajo por
/// frame.
#[must_use]
pub fn action2_eval_ctx_for_road_stop_tile_with_catalog(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    coord: TileCoord,
    view: u8,
    climate: Climate,
) -> Action2EvalCtx {
    let current_spec = station_at_tile(map, stations, coord)
        .and_then(|station| station.road_stop_spec_at(coord))
        .and_then(|id| road_stop_spec_def(road_stop_catalog, id));
    let type_tables = current_spec.and_then(|spec| spec.newgrf_type_tables.as_ref());
    action2_eval_ctx_for_road_stop_tile_impl(
        map,
        stations,
        RoadStopAction2Resolution {
            road_stop_catalog,
            road_type_catalog: &[],
            current_spec,
            type_tables,
            world: None,
        },
        coord,
        view,
        climate,
    )
}

/// Variante explícita para callers que no disponen de los pools del mundo pero
/// sí del catálogo `RoadTypes`/`TramTypes` activo. Evita que una API de
/// integración externa traduzca siempre contra los dos tipos vanilla.
#[must_use]
pub fn action2_eval_ctx_for_road_stop_tile_with_catalog_and_road_types(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    road_type_catalog: &[RoadTypeDef],
    coord: TileCoord,
    view: u8,
    climate: Climate,
) -> Action2EvalCtx {
    let current_spec = station_at_tile(map, stations, coord)
        .and_then(|station| station.road_stop_spec_at(coord))
        .and_then(|id| road_stop_spec_def(road_stop_catalog, id));
    let type_tables = current_spec.and_then(|spec| spec.newgrf_type_tables.as_ref());
    action2_eval_ctx_for_road_stop_tile_impl(
        map,
        stations,
        RoadStopAction2Resolution {
            road_stop_catalog,
            road_type_catalog,
            current_spec,
            type_tables,
            world: None,
        },
        coord,
        view,
        climate,
    )
}

/// Variante del contexto completo que aporta los pools de pueblos y compañías
/// del `GameState`, necesarios para `RoadStopScopeResolver` vars `45`–`47`.
/// Las APIs históricas siguen devolviendo los valores seguros de compra cuando
/// esos pools no están disponibles.
#[must_use]
pub fn action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    world: RoadStopWorldContext<'_>,
    coord: TileCoord,
    view: u8,
    climate: Climate,
) -> Action2EvalCtx {
    let current_spec = station_at_tile(map, stations, coord)
        .and_then(|station| station.road_stop_spec_at(coord))
        .and_then(|id| road_stop_spec_def(road_stop_catalog, id));
    let type_tables = current_spec.and_then(|spec| spec.newgrf_type_tables.as_ref());
    action2_eval_ctx_for_road_stop_tile_impl(
        map,
        stations,
        RoadStopAction2Resolution {
            road_stop_catalog,
            road_type_catalog: &[],
            current_spec,
            type_tables,
            world: Some(world),
        },
        coord,
        view,
        climate,
    )
}

/// Pools del mundo que alimentan las variables urbanas y de compañía de una
/// parada vial.
#[derive(Clone, Copy)]
pub struct RoadStopWorldContext<'a> {
    pub towns: &'a [Town],
    pub companies: &'a [Company],
    pub industries: &'a [Industry],
    pub road_type_catalog: &'a [RoadTypeDef],
    /// Catálogo activo de cargos para invertir labels CTT en vars `60`–`69`.
    pub cargo_spec_catalog: &'a [CargoSpecDef],
}

/// Datos opcionales que diferencian el contexto local legacy del renderer
/// capaz de resolver `RoadStop` contra su catálogo activo.
#[derive(Clone, Copy)]
struct RoadStopAction2Resolution<'a> {
    road_stop_catalog: &'a [RoadStopSpecDef],
    road_type_catalog: &'a [RoadTypeDef],
    current_spec: Option<&'a RoadStopSpecDef>,
    type_tables: Option<&'a GrfTypeTranslationTables>,
    world: Option<RoadStopWorldContext<'a>>,
}

/// Materializa `RoadStopScope` `0x7A(parameter)` usando la Badge Translation
/// Table del GRF activo. `OpenTTD` conserva `UINT_MAX` para un índice local que
/// no existe en la tabla; mantener esa posición evita desplazar parámetros
/// posteriores y permite que Action2 distinga “desconocido” de “no asociado”.
pub(crate) fn populate_road_stop_badge_vars_for_spec(
    ctx: &mut Action2EvalCtx,
    spec: &RoadStopSpecDef,
) {
    let mut requested = BTreeSet::new();
    if let Some(runtime) = spec.newgrf_runtime.as_ref() {
        for entry in runtime.action2_var.values() {
            for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
                if term.variable == 0x7A
                    && let Some(parameter) = term.param
                {
                    requested.insert(parameter);
                }
            }
        }
    }
    requested.extend(
        spec.newgrf_badge_translation
            .iter()
            .enumerate()
            .filter_map(|(index, _)| u8::try_from(index).ok()),
    );
    for parameter in requested {
        let badge_id = spec
            .newgrf_badge_translation
            .get(usize::from(parameter))
            .copied()
            .unwrap_or(u16::MAX);
        let value = if badge_id == u16::MAX {
            u32::MAX
        } else {
            u32::from(spec.associated_badges.contains(&badge_id))
        };
        ctx.parameterized_vars.insert((0x7A, parameter), value);
    }
}

#[allow(clippy::too_many_lines)]
fn action2_eval_ctx_for_road_stop_tile_impl(
    map: &Map,
    stations: &[Station],
    resolution: RoadStopAction2Resolution<'_>,
    coord: TileCoord,
    view: u8,
    climate: Climate,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let Some(station) = station_at_tile(map, stations, coord) else {
        return ctx;
    };
    let tile = map.get(coord);
    let (tileh, _) = tile_slope_and_z(map, coord).unwrap_or((0, 0));
    let random = u32::from(station.newgrf_random_bits)
        | (u32::from(station.road_stop_random_bits_at(coord)) << 16);

    ctx.random_bits = random;
    ctx.persistent_registers
        .clone_from(&station.newgrf_persistent_regs);
    ctx.vars.insert(
        0x5F,
        random.wrapping_shl(8) | u32::from(station.newgrf_waiting_random_triggers),
    );
    ctx.vars.insert(0x40, u32::from(view));
    ctx.vars.insert(
        0x41,
        match station.stop_kind {
            StopKind::BusStop => 0,
            StopKind::TruckStop => 1,
            _ => 2,
        },
    );
    ctx.vars.insert(
        0x42,
        terrain_type_for_road_stop_tile(map, coord, climate, tile) | (u32::from(tileh) << 8),
    );

    let vanilla_types = vanilla_road_type_catalog();
    let road_type_catalog = if resolution.road_type_catalog.is_empty() {
        resolution.world.map_or(vanilla_types.as_slice(), |world| {
            if world.road_type_catalog.is_empty() {
                vanilla_types.as_slice()
            } else {
                world.road_type_catalog
            }
        })
    } else {
        resolution.road_type_catalog
    };
    let road_type = tile.map_or(u32::MAX, |tile| {
        u32::from(reverse_road_type(
            resolution.type_tables,
            road_type_catalog,
            road_type_from_tile(&tile),
        ))
    });
    let tram_type = tile.map_or(u32::MAX, |tile| {
        tram_road_type_from_tile(&tile).map_or(u32::MAX, |tram| {
            u32::from(reverse_road_type(
                resolution.type_tables,
                road_type_catalog,
                tram,
            ))
        })
    });
    ctx.vars.insert(0x43, road_type);
    ctx.vars.insert(0x44, tram_type);
    let (town_zone_distance, town_distance_square) = road_stop_town_vars(
        resolution.world.map(|world| world.towns),
        station.town_id,
        coord,
    );
    ctx.vars.insert(0x45, town_zone_distance);
    ctx.vars.insert(0x46, town_distance_square);
    ctx.vars.insert(
        0x47,
        road_stop_company_info(station.owner, resolution.world.map(|world| world.companies)),
    );
    ctx.vars
        .insert(0x49, u32::from(station.road_stop_animation_frame_at(coord)));
    ctx.vars.insert(0xF0, station.stop_kind.facilities_mask());
    // `RoadStopScopeResolver::GetVariable(0xFA)` exposes the BaseStation
    // build date on the same relative WORD scale used by other station types.
    ctx.vars.insert(0xFA, station.newgrf_build_date_value());
    // Bit 4 de var 50 sólo se usa cuando no existe tesela (picker/callback de
    // disponibilidad); esta ruta siempre resuelve una instancia en el mapa.
    ctx.vars.insert(0x50, 0);
    if let Some(spec) = resolution.current_spec {
        populate_road_stop_parent_scope(
            &mut ctx,
            resolution.world,
            station.town_id,
            coord,
            spec.grfid,
        );
        populate_road_stop_badge_vars_for_spec(&mut ctx, spec);
        let cargo_catalog = resolution
            .world
            .map_or(&[][..], |world| world.cargo_spec_catalog);
        populate_station_cargo_vars_with_catalog(
            &mut ctx,
            station,
            resolution.type_tables,
            spec.newgrf_grf_version,
            climate,
            resolution
                .world
                .map(|world| crate::station::station_coverage_for(map, world.industries, station)),
            cargo_catalog,
        );
        RoadStopNeighbourScope {
            map,
            stations,
            road_stop_catalog: resolution.road_stop_catalog,
            station,
            current_spec: spec,
            coord,
            climate,
        }
        .populate(&mut ctx);
    }
    ctx
}

/// Populates the `TownScopeResolver` parent used by a map-aware road stop.
///
/// `OpenTTD` stores the parent association on the native road-stop object.
/// Imported stations preserve that ID; missing/invalid IDs use the nearest
/// town (with ID as a stable tie-breaker) as the explicit fallback. Callers
/// without a world context intentionally retain an empty parent scope.
fn populate_road_stop_parent_scope(
    ctx: &mut Action2EvalCtx,
    world: Option<RoadStopWorldContext<'_>>,
    station_town_id: Option<u32>,
    coord: TileCoord,
    grfid: u32,
) {
    let Some(world) = world else {
        return;
    };
    if let Some(town) = road_stop_town(world.towns, station_town_id, coord) {
        town.copy_newgrf_parent_scope(grfid, ctx);
    }
}

fn road_stop_town_vars(
    towns: Option<&[Town]>,
    station_town_id: Option<u32>,
    coord: TileCoord,
) -> (u32, u32) {
    let Some(towns) = towns else {
        return (u32::from(HouseZone::TownEdge as u8) << 16, 0);
    };
    let Some(town) = road_stop_town(towns, station_town_id, coord) else {
        return (u32::from(HouseZone::TownEdge as u8) << 16, 0);
    };
    let manhattan = crate::economy::manhattan_distance(coord, town.pos);
    let zone = u32::from(get_town_radius_group(town, coord) as u8) << 16;
    (
        zone | manhattan.min(u32::from(u16::MAX)),
        distance_square(coord, town.pos),
    )
}

fn road_stop_town(towns: &[Town], station_town_id: Option<u32>, coord: TileCoord) -> Option<&Town> {
    station_town_id
        .and_then(|town_id| towns.iter().find(|town| town.id == town_id))
        .or_else(|| {
            towns
                .iter()
                .min_by_key(|town| (crate::economy::manhattan_distance(coord, town.pos), town.id))
        })
}

fn road_stop_company_info(owner: CompanyId, companies: Option<&[Company]>) -> u32 {
    newgrf_company_info(owner, companies, 0)
}

/// Datos invariantes de una evaluación Action2 de `RoadStop` con vecindad.
///
/// Agruparlos impide que las consultas `66`–`6B` mezclen mapa, catálogo o
/// spec actual cuando el renderer materializa varios offsets en una pasada.
struct RoadStopNeighbourScope<'a> {
    map: &'a Map,
    stations: &'a [Station],
    road_stop_catalog: &'a [RoadStopSpecDef],
    station: &'a Station,
    current_spec: &'a RoadStopSpecDef,
    coord: TileCoord,
    climate: Climate,
}

impl RoadStopNeighbourScope<'_> {
    fn populate(&self, ctx: &mut Action2EvalCtx) {
        for (variable, parameter) in requested_nearby_road_stop_vars(self.current_spec) {
            let nearby = nearby_tile(self.map, self.coord, parameter);
            let value = match variable {
                0x66 => {
                    nearby_road_stop_animation_frame(self.map, self.stations, self.station, nearby)
                }
                0x67 => nearby_land_info(
                    self.map,
                    self.stations,
                    nearby,
                    self.climate,
                    self.current_spec,
                ),
                0x68 => nearby_road_stop_info(
                    self.map,
                    self.stations,
                    self.road_stop_catalog,
                    self.station,
                    self.current_spec,
                    self.coord,
                    nearby,
                ),
                0x6A => {
                    nearby_road_stop_grfid(self.map, self.stations, self.road_stop_catalog, nearby)
                }
                0x6B => nearby_road_stop_local_id(
                    self.map,
                    self.stations,
                    self.road_stop_catalog,
                    self.current_spec,
                    nearby,
                ),
                _ => continue,
            };
            ctx.parameterized_vars.insert((variable, parameter), value);
        }
    }
}

fn requested_nearby_road_stop_vars(spec: &RoadStopSpecDef) -> BTreeSet<(u8, u8)> {
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

fn signed_nibble(value: u8) -> i32 {
    let value = i32::from(value & 0x0F);
    if value >= 8 { value - 16 } else { value }
}

/// `GetNearbyTile` para el scope vial: offsets de nibbles firmados y wrap de mapa.
fn nearby_tile(map: &Map, base: TileCoord, parameter: u8) -> TileCoord {
    let (width, height) = map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return base;
    };
    if width == 0 || height == 0 {
        return base;
    }
    let dx = signed_nibble(parameter);
    let dy = signed_nibble(parameter >> 4);
    TileCoord::new(
        base.x.saturating_add(dx).rem_euclid(width),
        base.y.saturating_add(dy).rem_euclid(height),
    )
}

fn is_road_stop_kind(kind: StopKind) -> bool {
    matches!(
        kind,
        StopKind::BusStop | StopKind::TruckStop | StopKind::RoadWaypoint
    )
}

fn road_stop_station_at<'a>(
    map: &Map,
    stations: &'a [Station],
    coord: TileCoord,
) -> Option<&'a Station> {
    (map.get_kind(coord) == Some(TileKind::Station))
        .then(|| station_at_tile(map, stations, coord))
        .flatten()
        .filter(|station| is_road_stop_kind(station.stop_kind))
}

fn same_station(left: &Station, right: &Station) -> bool {
    left.pos == right.pos
}

fn nearby_road_stop_animation_frame(
    map: &Map,
    stations: &[Station],
    source: &Station,
    nearby: TileCoord,
) -> u32 {
    road_stop_station_at(map, stations, nearby).map_or(u32::MAX, |candidate| {
        if same_station(source, candidate) {
            u32::from(source.road_stop_animation_frame_at(nearby))
        } else {
            u32::MAX
        }
    })
}

fn nearby_land_info(
    map: &Map,
    stations: &[Station],
    nearby: TileCoord,
    climate: Climate,
    current_spec: &RoadStopSpecDef,
) -> u32 {
    let Some(tile) = map.get(nearby) else {
        return 0;
    };
    let (tileh, raw_z) = tile_slope_and_z(map, nearby).unwrap_or((0, 0));
    let z = if current_spec.newgrf_grf_version >= 8 {
        raw_z
    } else {
        raw_z.saturating_mul(u8::try_from(TILE_PIXEL_HEIGHT).unwrap_or(8))
    };
    let water_bits =
        water_class(tile).map_or(0, |class| u32::from((class.as_u8() + 1) & 0x03) << 5);
    let terrain = terrain_type_for_road_stop_tile(map, nearby, climate, Some(tile));
    let tile_type = u32::from(tile_kind_as_ottd(map, stations, nearby, tile));
    let terrain_bits = water_bits | (terrain << 2) | (u32::from(tile.kind == TileKind::Water) << 1);
    tile_type << 24 | u32::from(z) << 16 | terrain_bits << 8 | u32::from(tileh)
}

fn tile_kind_as_ottd(map: &Map, stations: &[Station], coord: TileCoord, tile: Tile) -> u8 {
    if tile.kind == TileKind::Station
        && road_stop_station_at(map, stations, coord)
            .is_some_and(|station| station.stop_kind == StopKind::RoadWaypoint)
    {
        return 2;
    }
    match tile.kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
        TileKind::House => 3,
        TileKind::Forest => 4,
        TileKind::Station | TileKind::Airport => 5,
        TileKind::Water | TileKind::ShipDepot => 6,
        TileKind::Void => 7,
        TileKind::Industry => 8,
        TileKind::Grass | TileKind::CoalField | TileKind::Unknown(_) => 0,
    }
}

fn custom_road_stop_spec_at<'a>(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &'a [RoadStopSpecDef],
    coord: TileCoord,
) -> Option<&'a RoadStopSpecDef> {
    let station = road_stop_station_at(map, stations, coord)?;
    let id = station.road_stop_spec_at(coord)?;
    road_stop_spec_def(road_stop_catalog, id).filter(|spec| spec.from_newgrf)
}

fn nearby_road_stop_info(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    source: &Station,
    current_spec: &RoadStopSpecDef,
    source_coord: TileCoord,
    nearby: TileCoord,
) -> u32 {
    let Some(candidate) = road_stop_station_at(map, stations, nearby) else {
        return u32::MAX;
    };
    let Some(nearby_tile) = map.get(nearby) else {
        return u32::MAX;
    };
    let source_gfx = map.get(source_coord).map_or(0, |tile| tile.m5);
    let mut result = u32::from(nearby_tile.m5) << 12;
    if source_gfx != nearby_tile.m5 {
        result |= 1 << 11;
    }
    if same_station(source, candidate) {
        result |= 1 << 10;
    }
    result |= match candidate.stop_kind {
        StopKind::TruckStop => 1 << 16,
        StopKind::RoadWaypoint => 2 << 16,
        _ => 0,
    };
    if candidate.stop_kind == source.stop_kind {
        result |= 1 << 20;
    }
    if let Some(spec) = custom_road_stop_spec_at(map, stations, road_stop_catalog, nearby) {
        result |= 1
            << if spec.grfid == current_spec.grfid {
                8
            } else {
                9
            };
        result |= u32::from(spec.newgrf_local_id);
    }
    result
}

fn nearby_road_stop_grfid(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    nearby: TileCoord,
) -> u32 {
    if road_stop_station_at(map, stations, nearby).is_none() {
        return u32::MAX;
    }
    custom_road_stop_spec_at(map, stations, road_stop_catalog, nearby).map_or(0, |spec| spec.grfid)
}

fn nearby_road_stop_local_id(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    current_spec: &RoadStopSpecDef,
    nearby: TileCoord,
) -> u32 {
    if road_stop_station_at(map, stations, nearby).is_none() {
        return u32::MAX;
    }
    custom_road_stop_spec_at(map, stations, road_stop_catalog, nearby)
        .filter(|spec| spec.grfid == current_spec.grfid)
        .map_or(0xFFFE, |spec| u32::from(spec.newgrf_local_id))
}

fn terrain_type_for_road_stop_tile(
    map: &Map,
    coord: TileCoord,
    climate: Climate,
    tile: Option<Tile>,
) -> u32 {
    if climate.uses_snow_ground() {
        return 4;
    }
    if climate.uses_desert_patches() {
        if tile.is_some_and(|tile| (tile.m7 & 0x20) != 0) {
            return 1;
        }
        // Una parada tapa el clear original; conservar el chequeo inmediato
        // de StationScope para que desierto tropical siga visible a Action2.
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let nearby = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(nearby).is_some_and(|tile| {
                tile.kind == crate::map::TileKind::Grass
                    && (tile.m5 & 0x07) == crate::world_gen::CLEAR_GROUND_DESERT
            }) {
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::StationRandomTrigger;
    use crate::company::{Company, CompanyId};
    use crate::game_state::CompanyEconomy;
    use crate::map::{Tile, TileKind};
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteGraphics,
    };
    use crate::road_stop_spec::{ROADSTOP_DRAW_MODE_DEFAULT, ROADSTOP_TYPE_ALL};
    use crate::town::Town;

    fn road_stop_tile(m5: u8, m6: u8) -> Tile {
        Tile {
            height: 2,
            kind: TileKind::Station,
            mapt: 0x50,
            m5,
            m1: 0,
            m6,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    fn road_stop_spec(
        id: u16,
        grfid: u32,
        local_id: u8,
        runtime: Option<TrainSpriteGraphics>,
    ) -> RoadStopSpecDef {
        RoadStopSpecDef {
            id,
            class: 0,
            label: format!("Road stop {id}"),
            short_label: format!("RS{id}"),
            stop_type: ROADSTOP_TYPE_ALL,
            from_newgrf: true,
            grfid,
            newgrf_local_id: local_id,
            newgrf_grf_version: 8,
            draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            build_cost_multiplier: 16,
            clear_cost_multiplier: 16,
            bridgeable_info: [crate::road_stop_spec::RoadStopBridgeableInfo::default();
                crate::road_stop_spec::ROADSTOP_LAYOUT_COUNT],
            callback_mask: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: runtime.map(Box::new),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
        }
    }

    fn runtime_referencing_nearby_road_stop_vars() -> TrainSpriteGraphics {
        let requested = [
            (0x66, 0x00),
            (0x66, 0x01),
            (0x66, 0x02),
            (0x66, 0x03),
            (0x67, 0x01),
            (0x68, 0x00),
            (0x68, 0x01),
            (0x68, 0x02),
            (0x68, 0x03),
            (0x6A, 0x00),
            (0x6A, 0x01),
            (0x6A, 0x02),
            (0x6A, 0x03),
            (0x6B, 0x00),
            (0x6B, 0x01),
            (0x6B, 0x02),
            (0x6B, 0x03),
        ];
        let mut gfx = TrainSpriteGraphics::default();
        for (index, (variable, parameter)) in requested.into_iter().enumerate() {
            gfx.action2_var.insert(
                u8::try_from(index).unwrap(),
                Action2VarEntry {
                    first: Action2VarTerm {
                        variable,
                        param: Some(parameter),
                        adjust: Action2VarAdjust::default(),
                    },
                    ops: Vec::new(),
                    ranges: Vec::new(),
                    default: 0,
                },
            );
        }
        gfx
    }

    #[test]
    fn road_stop_ctx_exposes_runtime_random_view_type_and_frame() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 2);
        map.set_tile(
            coord,
            Tile {
                height: 0,
                kind: TileKind::Station,
                mapt: 0,
                m5: 4,
                m1: 0,
                m6: 1,
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            },
        )
        .unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.newgrf_random_bits = 0xA55A;
        station.road_stop_newgrf_random_bits = 0x3C;
        station.newgrf_waiting_random_triggers = StationRandomTrigger::VehicleLoads.mask();
        station.road_stop_animation_frame = 7;
        station.build_date = crate::station::STATION_BUILD_DATE_DEFAULT + 123;
        {
            let state = station.ensure_road_stop_tile_state(coord);
            state.random_bits = 0x3C;
            state.animation_frame = 7;
        }
        station.sync_legacy_road_stop_anchor();
        station.newgrf_persistent_regs.insert(4, 99);

        let ctx = action2_eval_ctx_for_road_stop_tile(
            &map,
            &[station],
            coord,
            4,
            Climate::Temperate,
            None,
        );
        assert_eq!(ctx.random_bits, 0x003C_A55A);
        assert_eq!(
            ctx.vars.get(&0x5F),
            Some(&(0x003C_A55A_u32 << 8 | u32::from(StationRandomTrigger::VehicleLoads.mask())))
        );
        assert_eq!(ctx.vars.get(&0x40), Some(&4));
        assert_eq!(ctx.vars.get(&0x41), Some(&0));
        assert_eq!(ctx.vars.get(&0x43), Some(&0));
        assert_eq!(ctx.vars.get(&0x44), Some(&u32::MAX));
        assert_eq!(ctx.vars.get(&0x45), Some(&0));
        assert_eq!(ctx.vars.get(&0x46), Some(&0));
        assert_eq!(ctx.vars.get(&0x47), Some(&0));
        assert_eq!(ctx.vars.get(&0x49), Some(&7));
        assert_eq!(ctx.vars.get(&0xF0), Some(&(1_u32 << 2)));
        assert_eq!(ctx.vars.get(&0xFA), Some(&123));
        assert_eq!(ctx.persistent_registers.get(&4), Some(&99));
    }

    #[test]
    fn road_stop_scope_exposes_badge_presence_and_unknown_sentinels() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 2);
        map.set_tile(coord, road_stop_tile(4, 3 << 3)).unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        station.sync_legacy_road_stop_anchor();

        let mut spec = road_stop_spec(7, 0x4242_0001, 0, None);
        spec.associated_badges = vec![17];
        spec.newgrf_badge_translation = vec![17, u16::MAX];
        let catalog = vec![spec];
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog(
            &map,
            std::slice::from_ref(&station),
            &catalog,
            coord,
            0,
            Climate::Temperate,
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x7A, 0)), Some(&1));
        assert_eq!(ctx.parameterized_vars.get(&(0x7A, 1)), Some(&u32::MAX));
    }

    #[test]
    fn road_stop_world_scope_exposes_custom_cargo_through_ctt() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        map.set_tile(coord, road_stop_tile(4, 3 << 3)).unwrap();
        let custom = crate::CargoType::Custom(0);
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        station.cargo_stock.add(custom, 19);
        let spec = {
            let mut spec = road_stop_spec(7, 1, 0, None);
            spec.newgrf_type_tables = Some(crate::GrfTypeTranslationTables {
                cargo: vec![*b"TOFU"],
                ..crate::GrfTypeTranslationTables::default()
            });
            spec
        };
        let catalog = vec![spec];
        let cargo_catalog = vec![CargoSpecDef {
            id: crate::cargo::CUSTOM_CARGO_OFFSET,
            label: "TOFU".into(),
            from_newgrf: true,
            ..CargoSpecDef::default()
        }];
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &map,
            std::slice::from_ref(&station),
            &catalog,
            RoadStopWorldContext {
                towns: &[],
                companies: &[],
                industries: &[],
                road_type_catalog: &[],
                cargo_spec_catalog: &cargo_catalog,
            },
            coord,
            0,
            Climate::Temperate,
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x60, 0)), Some(&19));
    }

    #[test]
    fn road_stop_ctx_exposes_town_and_company_scopes_with_world() {
        let mut map = Map::new_flat(8, 8, 0);
        let coord = TileCoord::new(1, 2);
        map.set_tile(coord, road_stop_tile(4, 3 << 3)).unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.owner = CompanyId(1);

        let mut town = Town {
            id: 7,
            pos: TileCoord::new(0, 0),
            squared_town_zone_radius: [100, 100, 0, 0, 0],
            ..Town::default()
        };
        town.fund_buildings_months = 0;
        let mut rival = Company::rival_transcargo(CompanyEconomy::default(), 3);
        rival.liveries[0].colour1 = 2;
        rival.liveries[0].colour2 = 9;
        let companies = vec![rival];
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &map,
            &[station],
            &[],
            RoadStopWorldContext {
                towns: &[town],
                companies: &companies,
                industries: &[],
                road_type_catalog: &[],
                cargo_spec_catalog: &[],
            },
            coord,
            0,
            Climate::Temperate,
        );

        assert_eq!(ctx.vars.get(&0x45), Some(&(1 << 16 | 3)));
        assert_eq!(ctx.vars.get(&0x46), Some(&5));
        assert_eq!(ctx.vars.get(&0x47), Some(&0x9201_0001));
    }

    #[test]
    fn road_stop_world_scope_exposes_parent_town_and_psa_by_grfid() {
        let mut map = Map::new_flat(8, 8, 0);
        let coord = TileCoord::new(1, 2);
        map.set_tile(coord, road_stop_tile(4, 3 << 3)).unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);

        let grfid = 0x4242_0001;
        let mut town = Town {
            id: 7,
            pos: TileCoord::new(0, 0),
            population: 65_535,
            squared_town_zone_radius: [100, 100, 0, 0, 0],
            larger_town: true,
            ..Town::default()
        };
        town.newgrf_persistent_regs
            .insert(grfid, std::collections::HashMap::from([(4, 0xAABB_CCDD)]));
        let catalog = vec![road_stop_spec(7, grfid, 0, None)];
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &map,
            std::slice::from_ref(&station),
            &catalog,
            RoadStopWorldContext {
                towns: std::slice::from_ref(&town),
                companies: &[],
                industries: &[],
                road_type_catalog: &[],
                cargo_spec_catalog: &[],
            },
            coord,
            0,
            Climate::Temperate,
        );

        assert_eq!(ctx.parent_vars.get(&0x40), Some(&1));
        assert_eq!(ctx.parent_vars.get(&0x41), Some(&7));
        assert_eq!(ctx.parent_vars.get(&0x82), Some(&65_535));
        assert_eq!(ctx.parent_persistent_registers.get(&4), Some(&0xAABB_CCDD));
    }

    #[test]
    fn road_stop_scope_prefers_native_town_for_vars_and_parent() {
        let mut map = Map::new_flat(10, 10, 0);
        let coord = TileCoord::new(1, 2);
        map.set_tile(coord, road_stop_tile(4, 3 << 3)).unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        station.town_id = Some(9);

        let grfid = 0x5151_0001;
        let mut nearest = Town {
            id: 7,
            pos: TileCoord::new(1, 3),
            population: 100,
            ..Town::default()
        };
        nearest
            .newgrf_persistent_regs
            .insert(grfid, std::collections::HashMap::from([(4, 0xAAAA)]));
        let mut native = Town {
            id: 9,
            pos: TileCoord::new(8, 8),
            population: 900,
            ..Town::default()
        };
        native
            .newgrf_persistent_regs
            .insert(grfid, std::collections::HashMap::from([(4, 0xBBBB)]));
        let towns = vec![nearest, native];
        let catalog = vec![road_stop_spec(7, grfid, 0, None)];
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &map,
            std::slice::from_ref(&station),
            &catalog,
            RoadStopWorldContext {
                towns: &towns,
                companies: &[],
                industries: &[],
                road_type_catalog: &[],
                cargo_spec_catalog: &[],
            },
            coord,
            0,
            Climate::Temperate,
        );

        assert_eq!(ctx.vars.get(&0x46), Some(&85));
        assert_eq!(ctx.parent_vars.get(&0x41), Some(&9));
        assert_eq!(ctx.parent_vars.get(&0x82), Some(&900));
        assert_eq!(ctx.parent_persistent_registers.get(&4), Some(&0xBBBB));
    }

    #[test]
    fn road_stop_cargo_acceptance_uses_live_catchment() {
        let coord = TileCoord::new(2, 2);
        let catalog = vec![road_stop_spec(7, 1, 0, None)];
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        let towns = Vec::new();
        let companies = Vec::new();
        let industries = Vec::new();

        let mut empty_map = Map::new_flat(8, 8, 0);
        empty_map
            .set_tile(coord, road_stop_tile(4, 3 << 3))
            .expect("road stop tile");
        let empty_ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &empty_map,
            std::slice::from_ref(&station),
            &catalog,
            RoadStopWorldContext {
                towns: &towns,
                companies: &companies,
                industries: &industries,
                road_type_catalog: &[],
                cargo_spec_catalog: &[],
            },
            coord,
            0,
            Climate::Temperate,
        );
        assert_eq!(empty_ctx.parameterized_vars.get(&(0x65, 0)), Some(&0));

        let mut accepted_map = empty_map.clone();
        accepted_map
            .set_completed_house(TileCoord::new(2, 3), 0, 0)
            .expect("house in catchment");
        let accepted_ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &accepted_map,
            std::slice::from_ref(&station),
            &catalog,
            RoadStopWorldContext {
                towns: &towns,
                companies: &companies,
                industries: &industries,
                road_type_catalog: &[],
                cargo_spec_catalog: &[],
            },
            coord,
            0,
            Climate::Temperate,
        );
        assert_eq!(accepted_ctx.parameterized_vars.get(&(0x65, 0)), Some(&8));
    }

    #[test]
    fn road_stop_world_scope_translates_external_road_type() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let mut tile = road_stop_tile(4, 3 << 3);
        tile.m3hi = 2;
        map.set_tile(coord, tile).expect("road stop tile");
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        let mut spec = road_stop_spec(7, 1, 0, None);
        spec.newgrf_type_tables = Some(crate::GrfTypeTranslationTables {
            road: vec![*b"RNEW"],
            ..crate::GrfTypeTranslationTables::default()
        });
        let catalog = vec![spec];
        let mut road_types = crate::road_type::vanilla_road_type_catalog();
        let mut external = road_types[0].clone();
        external.id = crate::RoadType::from_u8(2);
        external.label = "RNEW".into();
        external.short_label = "RNEW".into();
        external.from_newgrf = true;
        road_types.push(external);
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
            &map,
            std::slice::from_ref(&station),
            &catalog,
            RoadStopWorldContext {
                towns: &[],
                companies: &[],
                industries: &[],
                road_type_catalog: &road_types,
                cargo_spec_catalog: &[],
            },
            coord,
            0,
            Climate::Temperate,
        );
        assert_eq!(ctx.vars.get(&0x43), Some(&0));
    }

    #[test]
    fn road_stop_ctx_resolves_requested_nearby_scope_values() {
        let mut map = Map::new_flat(4, 4, 2);
        let source = TileCoord::new(1, 1);
        let same_station_tile = TileCoord::new(2, 1);
        let other_station_tile = TileCoord::new(3, 1);
        map.set_tile(source, road_stop_tile(4, 3 << 3)).unwrap();
        map.set_tile(same_station_tile, road_stop_tile(5, 3 << 3))
            .unwrap();
        map.set_tile(other_station_tile, road_stop_tile(6, 2 << 3))
            .unwrap();

        let mut current_station = Station::new_with_kind(source, StopKind::BusStop);
        current_station.joined_tiles.push(same_station_tile);
        {
            let state = current_station.ensure_road_stop_tile_state(source);
            state.spec = Some(1);
            state.animation_frame = 4;
        }
        {
            let state = current_station.ensure_road_stop_tile_state(same_station_tile);
            state.spec = Some(1);
            state.animation_frame = 9;
        }
        current_station.sync_legacy_road_stop_anchor();

        let mut neighbor_station = Station::new_with_kind(other_station_tile, StopKind::TruckStop);
        {
            let state = neighbor_station.ensure_road_stop_tile_state(other_station_tile);
            state.spec = Some(2);
            state.animation_frame = 12;
        }
        neighbor_station.sync_legacy_road_stop_anchor();

        let catalog = vec![
            road_stop_spec(
                1,
                0x1111_1111,
                0x11,
                Some(runtime_referencing_nearby_road_stop_vars()),
            ),
            road_stop_spec(2, 0x2222_2222, 0x22, None),
        ];
        let stations = vec![current_station, neighbor_station];
        let ctx = action2_eval_ctx_for_road_stop_tile_with_catalog(
            &map,
            &stations,
            &catalog,
            source,
            4,
            Climate::Temperate,
        );

        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0x00)), Some(&4));
        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0x01)), Some(&9));
        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0x02)), Some(&u32::MAX));
        assert_eq!(ctx.parameterized_vars.get(&(0x66, 0x03)), Some(&u32::MAX));
        // `05 02 20 00`: Station, z=2 (GRFv8), water-class sea+1, flat.
        assert_eq!(
            ctx.parameterized_vars.get(&(0x67, 0x01)),
            Some(&0x0502_2000)
        );
        // gfx=5, orientación distinta, misma estación y spec del mismo GRF.
        assert_eq!(
            ctx.parameterized_vars.get(&(0x68, 0x01)),
            Some(&0x0010_5D11)
        );
        assert_eq!(
            ctx.parameterized_vars.get(&(0x68, 0x00)),
            Some(&0x0010_4511)
        );
        // gfx=6, orientación distinta, truck, custom de otro GRF, local=0x22.
        assert_eq!(
            ctx.parameterized_vars.get(&(0x68, 0x02)),
            Some(&0x0001_6A22)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x68, 0x03)), Some(&u32::MAX));
        assert_eq!(
            ctx.parameterized_vars.get(&(0x6A, 0x00)),
            Some(&0x1111_1111)
        );
        assert_eq!(
            ctx.parameterized_vars.get(&(0x6A, 0x01)),
            Some(&0x1111_1111)
        );
        assert_eq!(
            ctx.parameterized_vars.get(&(0x6A, 0x02)),
            Some(&0x2222_2222)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x6A, 0x03)), Some(&u32::MAX));
        assert_eq!(ctx.parameterized_vars.get(&(0x6B, 0x00)), Some(&0x11));
        assert_eq!(ctx.parameterized_vars.get(&(0x6B, 0x01)), Some(&0x11));
        assert_eq!(ctx.parameterized_vars.get(&(0x6B, 0x02)), Some(&0xFFFE));
        assert_eq!(ctx.parameterized_vars.get(&(0x6B, 0x03)), Some(&u32::MAX));
    }

    #[test]
    fn nearby_tile_uses_signed_offsets_and_map_wrap() {
        let map = Map::new_flat(4, 4, 0);
        assert_eq!(
            nearby_tile(&map, TileCoord::new(0, 0), 0x0F),
            TileCoord::new(3, 0)
        );
        assert_eq!(
            nearby_tile(&map, TileCoord::new(0, 0), 0xF0),
            TileCoord::new(0, 3)
        );
    }
}
