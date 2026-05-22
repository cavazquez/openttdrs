use crate::map::{
    Map, TileCoord, TileKind, complement_slope, inclined_slope_direction, resolve_tunnel_end,
    tile_slope_and_z, tunnel_entrance_m5, tunnel_path_tiles, tunnel_preview_path,
};

/// Bit 4 de `m5` en puentes: eje Y (si no, eje X).
pub const BRIDGE_AXIS_Y_M5: u8 = 0x10;
use crate::pathfinder::{
    diag_dir_offset, station_entrance_faces_rail, station_entrance_faces_road,
    station_site_tile_allows_build, station_site_tile_needs_clear,
};
use crate::{
    CLEAR_TILE_COST, DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST, ROAD_BUILD_COST,
    STATION_BUILD_COST, Station, StopKind,
};

use super::{CommandError, in_bounds};

pub(crate) fn check_in_bounds(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(map, c)
}

pub(crate) fn check_place_road_bits(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => Ok(()),
    }
}

pub(crate) fn check_place_rail(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        _ => Ok(()),
    }
}

/// `TileType::MP_RAILWAY` en el nibble alto de `mapt`.
const MP_RAILWAY_MAPT: u8 = 0x10;
/// `RailTileType::Normal` en bits 6–7 de `m5`.
const RAIL_TILE_NORMAL: u8 = 0;
/// `TrackBits` (`track_type.h`).
const RAIL_TB_X: u8 = 1;
const RAIL_TB_Y: u8 = 2;
const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;

#[inline]
fn connects_rail_network(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail
            | TileKind::Station
            | TileKind::RailDepot
            | TileKind::RailTunnel
            | TileKind::RailBridge
    )
}

/// `TrackBits` inferidos por vecinos (misma lógica que el render sintético del cliente).
fn rail_trackbits_from_neighbors(map: &Map, c: TileCoord) -> u8 {
    let rail_at = |x: i32, y: i32| -> bool {
        map.get_kind(TileCoord::new(x, y))
            .is_some_and(connects_rail_network)
    };
    let along_map_x = rail_at(c.x - 1, c.y) || rail_at(c.x + 1, c.y);
    let along_map_y = rail_at(c.x, c.y - 1) || rail_at(c.x, c.y + 1);
    match (along_map_x, along_map_y) {
        (true, false) => RAIL_TB_X,
        (true, true) => RAIL_TB_CROSS,
        (false, _) => RAIL_TB_Y,
    }
}

fn write_normal_rail_tile(
    state: &mut GameState,
    c: TileCoord,
    trackbits: u8,
) -> Result<(), CommandError> {
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.kind = TileKind::Rail;
    tile.mapt = MP_RAILWAY_MAPT;
    tile.m5 = (trackbits & 0x3F) | (RAIL_TILE_NORMAL << 6);
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

fn refresh_rail_trackbits(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    if !state.map.get_kind(c).is_some_and(|k| k == TileKind::Rail) {
        return Ok(());
    }
    let tb = rail_trackbits_from_neighbors(&state.map, c);
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.mapt = MP_RAILWAY_MAPT;
    tile.m5 = (tb & 0x3F) | (tile.m5 & 0xC0);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)
}

fn refresh_rail_neighbors(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        refresh_rail_trackbits(state, TileCoord::new(c.x + dx, c.y + dy))?;
    }
    Ok(())
}

pub(crate) fn check_single_transport_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let kind = map.get_kind(c).unwrap_or(TileKind::Grass);
    if transport_tile_is_buildable(kind) {
        Ok(())
    } else {
        Err(build_error_for_kind(kind))
    }
}

/// Validación de puente: orillas al mismo `GetTileZ` y tramo central sobre agua o terreno más bajo.
pub(crate) fn check_bridge(map: &Map, a: TileCoord, b: TileCoord) -> Result<(), CommandError> {
    let line = axis_line(a, b);
    if line.len() < 3 {
        return Err(CommandError::InvalidBridgeSpan);
    }
    let (Some(start_z), Some(end_z)) = (
        tile_slope_and_z(map, a).map(|(_, z)| z),
        tile_slope_and_z(map, b).map(|(_, z)| z),
    ) else {
        return Err(CommandError::OutOfBounds);
    };
    if start_z != end_z {
        return Err(CommandError::InvalidBridgeSpan);
    }
    let mut span_has_gap = false;
    for (i, c) in line.iter().enumerate() {
        check_in_bounds(map, *c)?;
        let kind = map.get_kind(*c).unwrap_or(TileKind::Grass);
        let is_endpoint = i == 0 || i + 1 == line.len();
        if is_endpoint {
            if !transport_tile_is_buildable(kind) {
                return Err(build_error_for_kind(kind));
            }
        } else if kind == TileKind::Water {
            span_has_gap = true;
        } else {
            if !transport_tile_is_buildable(kind) {
                return Err(build_error_for_kind(kind));
            }
            if tile_slope_and_z(map, *c).is_some_and(|(_, z)| z < start_z) {
                span_has_gap = true;
            }
        }
    }
    if span_has_gap {
        Ok(())
    } else {
        Err(CommandError::InvalidBridgeSpan)
    }
}

/// Túnel al estilo `OpenTTD`: pendiente inclinada en la entrada, recorrido diagonal hasta
/// el mismo `GetTileZ` y pendiente complementaria en la salida.
pub(crate) fn check_tunnel(map: &Map, start: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, start)?;
    let (start_tileh, _) =
        tile_slope_and_z(map, start).ok_or(CommandError::InvalidTunnelEndpoints)?;
    if inclined_slope_direction(start_tileh).is_none() {
        return Err(CommandError::InvalidTunnelEndpoints);
    }
    let Some(path) = tunnel_preview_path(map, start) else {
        return Err(CommandError::InvalidTunnelEndpoints);
    };
    if path.len() < 2 {
        return Err(CommandError::InvalidTunnelEndpoints);
    }
    for c in &path {
        check_in_bounds(map, *c)?;
        let kind = map.get_kind(*c).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(kind) {
            return Err(build_error_for_kind(kind));
        }
    }
    Ok(())
}

pub(crate) fn check_tunnel_or_bridge(
    map: &Map,
    a: TileCoord,
    b: TileCoord,
    is_tunnel: bool,
) -> Result<(), CommandError> {
    if is_tunnel {
        check_tunnel(map, a)
    } else {
        check_bridge(map, a, b)
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
                station_entrance_faces_road(map, c, dir)
            };
            if entrance_ok {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
    }
}

pub(crate) fn check_clear_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if map.get_kind(c) == Some(TileKind::Void) {
        Err(CommandError::CannotPlaceRoadOnVoid)
    } else {
        Ok(())
    }
}

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
    check_single_transport_tile(&state.map, c)?;
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

/// La boca del depósito de carretera debe dar a una tesela con red de carretera.
#[must_use]
fn road_depot_entrance_faces_road(map: &Map, depot_pos: TileCoord, dir: u8) -> bool {
    let Some((exit, _)) = road_depot_exit_for_dir(map, depot_pos, dir) else {
        return false;
    };
    map.get_kind(exit).is_some_and(|kind| {
        matches!(
            kind,
            TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
        )
    })
}

pub(crate) fn check_road_depot_placement(
    map: &Map,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        k if !station_site_tile_allows_build(k) => {
            Err(CommandError::CannotPlaceStationOnOccupiedTile)
        }
        _ => {
            if road_depot_entrance_faces_road(map, c, dir & 0x03) {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
    }
}

pub(crate) fn check_rail_depot_placement(
    map: &Map,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        k if !station_site_tile_allows_build(k) => {
            Err(CommandError::CannotPlaceStationOnOccupiedTile)
        }
        _ => {
            if station_entrance_faces_rail(map, c, dir & 0x03) {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
    }
}

pub(super) fn place_road_depot_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    check_road_depot_placement(&state.map, c, dir)?;
    place_single_transport_tile(
        state,
        c,
        TileKind::RoadDepot,
        0x20,
        0x20 | dir,
        DEPOT_BUILD_COST,
    )?;
    if let Some((exit, road_bits)) = road_depot_exit_for_dir(&state.map, c, dir)
        && state.map.get_kind(exit) == Some(TileKind::Road)
    {
        let _ = place_road_bits(state, exit, road_bits);
    }
    Ok(())
}

pub(super) fn place_rail_depot_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    check_rail_depot_placement(&state.map, c, dir)?;
    place_single_transport_tile(
        state,
        c,
        TileKind::RailDepot,
        0x20,
        0x20 | dir,
        DEPOT_BUILD_COST,
    )
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
    let is_tunnel = matches!(kind_to_place, TileKind::RoadTunnel | TileKind::RailTunnel);
    check_tunnel_or_bridge(&state.map, a, b, is_tunnel)?;
    let line = if is_tunnel {
        let end = resolve_tunnel_end(&state.map, a).ok_or(CommandError::InvalidTunnelEndpoints)?;
        let (start_tileh, _) =
            tile_slope_and_z(&state.map, a).ok_or(CommandError::InvalidTunnelEndpoints)?;
        let (end_tileh, _) =
            tile_slope_and_z(&state.map, end).ok_or(CommandError::InvalidTunnelEndpoints)?;
        if complement_slope(start_tileh) != end_tileh {
            return Err(CommandError::InvalidTunnelEndpoints);
        }
        tunnel_path_tiles(&state.map, a, end)
    } else {
        axis_line(a, b)
    };
    let is_rail = matches!(kind_to_place, TileKind::RailTunnel | TileKind::RailBridge);
    let bridge_axis_y = !is_tunnel && (b.x - a.x).abs() < (b.y - a.y).abs();
    let cost = cost_per_tile * i64::try_from(line.len()).unwrap_or(i64::MAX);
    for c in line {
        let m5_tile = if is_tunnel {
            tile_slope_and_z(&state.map, c)
                .and_then(|(h, _)| tunnel_entrance_m5(h, is_rail))
                .unwrap_or(0)
        } else if bridge_axis_y {
            m5 | BRIDGE_AXIS_Y_M5
        } else {
            m5
        };
        state
            .map
            .set_kind(c, kind_to_place)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(c, mapt, m5_tile)
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
    check_place_road_bits(&state.map, c)?;
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

pub(super) fn set_road_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_road_bits(&state.map, c)?;
    let road_bits = (bits & 0x0F).max(0x01);
    write_normal_road_tile(state, c, road_bits)?;
    state.economy.money -= ROAD_BUILD_COST;
    Ok(())
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

pub(super) fn place_station_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    place_stop_kind(state, c, dir, StopKind::TruckStop)
}

#[inline]
fn ottd_station_type_bits(stop_kind: StopKind) -> u8 {
    match stop_kind {
        StopKind::RailStation => 0,
        StopKind::TruckStop => 2,
        StopKind::BusStop => 3,
    }
}

#[inline]
fn apply_station_m6(m6: u8, stop_kind: StopKind) -> u8 {
    (m6 & !0x78) | (ottd_station_type_bits(stop_kind) << 3)
}

#[inline]
fn road_stop_m5(dir: u8) -> u8 {
    dir & 0x03
}

/// Road bit hacia un vecino ortogonal (`road_bits_for_render` del cliente).
fn road_bits_toward_neighbor(dx: i32, dy: i32) -> u8 {
    match (dx, dy) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0x05,
    }
}

/// Tramo de acceso en la tesela de parada (hacia la carretera en `dir`).
fn road_stop_stub_bits(dir: u8) -> u8 {
    let (dx, dy) = diag_dir_offset(dir);
    road_bits_toward_neighbor(dx, dy)
}

/// Bit en la carretera vecina que apunta de vuelta hacia la parada.
fn road_link_bits_toward_stop(dir: u8) -> u8 {
    let (dx, dy) = diag_dir_offset(dir);
    road_bits_toward_neighbor(-dx, -dy)
}

/// Fusiona road bits en una tesela de carretera existente (sin coste de construcción).
fn merge_road_bits(state: &mut GameState, c: TileCoord, bits: u8) -> Result<(), CommandError> {
    if state.map.get_kind(c) != Some(TileKind::Road) {
        return Ok(());
    }
    let existing = state.map.get(c).map_or(0, |t| t.m5 & 0x0F);
    let merged = (existing | (bits & 0x0F)).max(0x01);
    write_normal_road_tile(state, c, merged)
}

/// Une la parada con la carretera adyacente (`MakeRoadStop` + boca hacia la red).
fn connect_road_stop(state: &mut GameState, c: TileCoord, dir: u8) -> Result<(), CommandError> {
    let (dx, dy) = diag_dir_offset(dir);
    let road_pos = TileCoord::new(c.x + dx, c.y + dy);
    merge_road_bits(state, road_pos, road_link_bits_toward_stop(dir))?;
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.m3 = road_stop_stub_bits(dir);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)
}

/// `StationGfx` con edificio pequeño (`station_land.h` entradas 2/3).
fn rail_station_gfx_from_axis(axis_y: bool) -> u8 {
    if axis_y { 3 } else { 2 }
}

fn rail_axis_y_from_trackbits(tb: u8) -> bool {
    let tb = tb & 0x3F;
    if tb & RAIL_TB_X != 0 && tb & RAIL_TB_Y == 0 {
        return false;
    }
    if tb & RAIL_TB_Y != 0 && tb & RAIL_TB_X == 0 {
        return true;
    }
    true
}

/// `m5` gfx para estación de tren: eje alineado con la vía vecina (`GetRailStationAxis`).
fn rail_station_m5(map: &Map, c: TileCoord, dir: u8) -> u8 {
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

pub(super) fn place_rail_station(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_station_placement(&state.map, &state.stations, c, dir, StopKind::RailStation)?;
    station_placement_on_tile(state, c, dir, StopKind::RailStation)
}

/// Limpia bosque en la tesela de la parada (`LandscapeClear` en `station_cmd.cpp`).
fn clear_station_site_tile(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
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

fn station_placement_on_tile(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
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
    tile.m6 = apply_station_m6(tile.m6, stop_kind);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    if matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        connect_road_stop(state, c, dir)?;
    }
    state.stations.push(Station::new_with_kind(c, stop_kind));
    state.economy.money -= STATION_BUILD_COST;
    Ok(())
}

pub(super) fn place_stop_kind(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    check_station_placement(&state.map, &state.stations, c, dir, stop_kind)?;
    station_placement_on_tile(state, c, dir, stop_kind)
}

pub(super) fn place_rail(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    let tb = rail_trackbits_from_neighbors(&state.map, c);
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    refresh_rail_trackbits(state, c)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

pub(super) fn clear_tile(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    check_clear_tile(&state.map, c)?;
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
