//! Contexto Action2 para teselas de estación (vars de runtime).

use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::industry::Industry;
use crate::map::{
    Map, TileCoord, TileKind, rail_bits_touching_side, rail_traversal_bits, tile_slope_and_z,
};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{
    GrfTypeTranslationTables, cargo_from_local_id, local_cargo_id, reverse_rail_type,
};
use crate::rail_type::rail_type_from_tile;
use crate::station::{
    STATION_TILE_RESERVATION, STATION_TYPE_RAIL_WAYPOINT, Station, StationCoverage,
    station_at_tile, station_type_from_m6,
};
use crate::world_gen::Climate;

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
    let tile = map.get(coord);
    let m5 = tile.map_or(0, |t| t.m5);
    let m6 = tile.map_or(0, |t| t.m6);
    let (tileh, z) = tile_slope_and_z(map, coord).unwrap_or((0, 0));

    let random = u32::from(st.newgrf_random_bits);
    ctx.random_bits = random;
    ctx.vars.insert(0x5F, random << 8);

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
    populate_station_cargo_vars(&mut ctx, st, type_tables, grf_version, climate, coverage);

    ctx
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
    for cargo in ALL_CARGO_TYPES {
        let local_id = local_cargo_id(type_tables, grf_version, cargo, climate);
        if local_id == 0xFF
            || cargo_from_local_id(type_tables, grf_version, local_id, climate) != Some(cargo)
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
    use crate::cargo_packet::CargoPacket;
    use crate::company::CompanyId;
    use crate::map::{Map, Tile, TileKind};
    use crate::station::{Station, StopKind};

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
        assert!(ctx.vars.contains_key(&0x10));
        assert!(ctx.vars.contains_key(&0x67));
        assert_eq!(ctx.random_bits, 0x42);
        assert_eq!(ctx.vars.get(&0x4A), Some(&9));
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
            StationAction2WorldContext { industries: &[] },
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
            StationAction2WorldContext { industries: &[] },
        );
        assert_eq!(
            world_with_house.parameterized_vars.get(&(0x65, 0)),
            Some(&8)
        );
    }
}
