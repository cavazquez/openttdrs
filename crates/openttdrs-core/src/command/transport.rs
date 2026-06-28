use crate::map::{
    Map, TileCoord, TileKind, complement_slope, inclined_slope_direction,
    rail_trackbits_valid_on_slope, resolve_tunnel_end, tile_slope_and_z, tunnel_entrance_m5,
    tunnel_path_tiles, tunnel_preview_path,
};
use crate::rail_signals::{
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SIGNAL_BUILD_COST,
    rail_tile_is_signals,
};
use crate::station::is_rail_waypoint_tile;

/// Bit 4 de `m5` en puentes: eje Y (si no, eje X).
pub const BRIDGE_AXIS_Y_M5: u8 = 0x10;
use crate::pathfinder::{
    diag_dir_offset, station_entrance_faces_rail, station_entrance_faces_road,
    station_site_tile_allows_build, station_site_tile_needs_clear,
};
use crate::{
    CLEAR_TILE_COST, DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST, ROAD_BUILD_COST,
    STATION_BUILD_COST, Station, StopKind, WAYPOINT_BUILD_COST,
};

use super::terraform::{apply_autoslope_if_needed, check_autoslope_flat};
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

#[inline]
fn existing_rail_trackbits(map: &Map, c: TileCoord) -> u8 {
    map.get(c)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F)
}

/// Valida `TrackBits` tras autoslope opcional (T3).
pub(crate) fn check_rail_trackbits_with_autoslope(
    map: &Map,
    c: TileCoord,
    final_bits: u8,
    tick: u64,
) -> Result<(), CommandError> {
    if check_rail_trackbits_on_tile(map, c, final_bits).is_ok() {
        return Ok(());
    }
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if tileh == 0 {
        return Err(CommandError::InvalidRailOnSlope);
    }
    check_autoslope_flat(map, c, tick)?;
    if !rail_trackbits_valid_on_slope(0, final_bits) {
        return Err(CommandError::InvalidRailOnSlope);
    }
    Ok(())
}

/// Valida `TrackBits` finales tras colocar vía (`CheckRailSlope` / `GetRailFoundation`).
pub(crate) fn check_rail_trackbits_on_tile(
    map: &Map,
    c: TileCoord,
    final_bits: u8,
) -> Result<(), CommandError> {
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if !rail_trackbits_valid_on_slope(tileh, final_bits) {
        return Err(CommandError::InvalidRailOnSlope);
    }
    Ok(())
}

/// `TrackBits` resultantes al combinar vía existente con piezas nuevas.
#[must_use]
pub(crate) fn merged_rail_trackbits_on_tile(map: &Map, c: TileCoord, add_bits: u8) -> u8 {
    merge_rail_trackbits(existing_rail_trackbits(map, c), add_bits)
}

/// `TileType::MP_RAILWAY` en el nibble alto de `mapt`.
const MP_RAILWAY_MAPT: u8 = 0x10;
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

/// ¿El vecino del lado `side` (`DiagDir`) conecta vía hacia `c`?
/// Un depósito solo conecta si su boca mira hacia `c`.
fn rail_neighbor_connects(map: &Map, c: TileCoord, side: u8) -> bool {
    let (dx, dy) = crate::pathfinder::diag_dir_offset(side);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    let Some(t) = map.get(n) else {
        return false;
    };
    if t.kind == TileKind::RailDepot {
        let (mx, my) = crate::pathfinder::diag_dir_offset(t.m5 & 0x03);
        return TileCoord::new(n.x + mx, n.y + my) == c;
    }
    connects_rail_network(t.kind)
}

/// `TrackBits` inferidos por vecinos: una pieza (recta o curva) por cada par
/// de lados con vía vecina, como el autorraíl de `OpenTTD`. También la usa el
/// cliente para previsualizar la pieza que colocaría `PlaceRail`.
#[must_use]
pub fn rail_trackbits_from_neighbors(map: &Map, c: TileCoord) -> u8 {
    let mut sides = [false; 4];
    for d in 0..4u8 {
        sides[usize::from(d)] = rail_neighbor_connects(map, c, d);
    }
    match sides.iter().filter(|s| **s).count() {
        // Sin vecinos: eje Y, como el comportamiento histórico del autorraíl.
        0 => RAIL_TB_Y,
        // Un solo vecino: recta a lo largo de ese eje (NE/SW → X, SE/NW → Y).
        1 => {
            if sides[0] || sides[2] {
                RAIL_TB_X
            } else {
                RAIL_TB_Y
            }
        }
        // Cuatro lados: cruce de dos rectas (X|Y) como cuando en `OpenTTD` una
        // línea atraviesa otra; la conexión sigue por la diagonal, sin curvas.
        4 => RAIL_TB_CROSS,
        _ => {
            let mut bits = 0;
            for a in 0..4u8 {
                for b in (a + 1)..4 {
                    if sides[a as usize] && sides[b as usize] {
                        bits |= crate::pathfinder::rail_bit_for_sides(a, b);
                    }
                }
            }
            bits
        }
    }
}

pub(crate) fn normalize_synthetic_rail_crossings(map: &mut Map) {
    const RAIL_TB_ALL: u8 = 0x3F;
    let (mw, mh) = map.dimensions();
    for y in 0..mh.cast_signed() {
        for x in 0..mw.cast_signed() {
            let c = TileCoord::new(x, y);
            let Some(mut t) = map.get(c) else {
                continue;
            };
            let bits5 = t.m5 & 0x3F;
            if t.kind != TileKind::Rail || (bits5 != RAIL_TB_CROSS && bits5 != RAIL_TB_ALL) {
                continue;
            }
            let bits = rail_trackbits_from_neighbors(map, c);
            if bits != bits5 {
                t.m5 = (t.m5 & 0xC0) | bits;
                let _ = map.set_tile(c, t);
            }
        }
    }
}

/// Rellena `TrackBits` vacíos (`m5 & 0x3F == 0`) inferidos de vecinos de vía.
/// Saves antiguos y depósitos mal tipados pueden dejar piezas sin bits explícitos.
pub(crate) fn normalize_rail_trackbits_from_neighbors(map: &mut Map) {
    let (mw, mh) = map.dimensions();
    for y in 0..mh.cast_signed() {
        for x in 0..mw.cast_signed() {
            let c = TileCoord::new(x, y);
            let Some(mut t) = map.get(c) else {
                continue;
            };
            if t.kind != TileKind::Rail || t.m5 & 0x3F != 0 {
                continue;
            }
            let bits = rail_trackbits_from_neighbors(map, c);
            if bits == 0 {
                continue;
            }
            t.m5 = (t.m5 & 0xC0) | bits;
            let _ = map.set_tile(c, t);
        }
    }
}

fn is_rail_path_endpoint(map: &Map, c: TileCoord) -> bool {
    let Some(t) = map.get(c) else {
        return false;
    };
    match t.kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => true,
        TileKind::Station => {
            let st = crate::station::station_type_from_m6(t.m6);
            st == 0 || st == crate::station::STATION_TYPE_RAIL_WAYPOINT
        }
        _ => false,
    }
}

fn is_rail_gap_fill_kind(kind: TileKind) -> bool {
    matches!(kind, TileKind::Grass | TileKind::Forest)
}

fn write_rail_gap_tile(map: &mut Map, c: TileCoord, axis_x: bool) {
    let Some(mut t) = map.get(c) else {
        return;
    };
    t.kind = TileKind::Rail;
    t.mapt = MP_RAILWAY_MAPT;
    let bits = if axis_x { RAIL_TB_X } else { RAIL_TB_Y };
    t.m5 = (t.m5 & 0xC0) | bits;
    let _ = map.set_tile(c, t);
}

/// Teselas `Clear`/`Forest` entre dos extremos de red ferroviaria en la misma fila
/// o columna se convierten en vía recta. Saves reales (p. ej. `stationlist-test.sav`)
/// suelen dejar huecos así entre depósito y línea.
pub(crate) fn bridge_collinear_rail_gaps(map: &mut Map) {
    let (mw, mh) = map.dimensions();
    for y in 0..mh.cast_signed() {
        bridge_line(map, true, y, mw.cast_signed());
    }
    for x in 0..mw.cast_signed() {
        bridge_line(map, false, x, mh.cast_signed());
    }
}

fn bridge_line(map: &mut Map, horizontal: bool, fixed: i32, len: i32) {
    let coord = |i: i32| {
        if horizontal {
            TileCoord::new(i, fixed)
        } else {
            TileCoord::new(fixed, i)
        }
    };
    let mut i = 0;
    while i < len {
        while i < len && !is_rail_path_endpoint(map, coord(i)) {
            i += 1;
        }
        if i >= len {
            break;
        }
        let start = i;
        i += 1;
        let gap_start = i;
        while i < len && map.get_kind(coord(i)).is_some_and(is_rail_gap_fill_kind) {
            i += 1;
        }
        if i < len && is_rail_path_endpoint(map, coord(i)) && gap_start < i {
            for g in gap_start..i {
                write_rail_gap_tile(map, coord(g), horizontal);
            }
        }
        if i == start {
            i += 1;
        }
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
        (2 << 6) | dir,
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
        0x10,
        (2 << 6) | dir,
        DEPOT_BUILD_COST,
    )?;
    // La tesela de salida gana las piezas de empalme (curvas/recta) que
    // conectan la boca del depósito con la vía existente, como en OpenTTD.
    if let Some((exit, _)) = rail_depot_exit_for_dir(&state.map, c, dir)
        && state.map.get_kind(exit) == Some(TileKind::Rail)
    {
        let junction_bits = rail_trackbits_from_neighbors(&state.map, exit);
        let _ = place_rail_bits(state, exit, junction_bits);
    }
    Ok(())
}

/// Tesela de salida del depósito de vía y trackbit del eje correspondiente
/// (X para NE/SW, Y para SE/NW).
pub(super) fn rail_depot_exit_for_dir(
    map: &Map,
    depot_pos: TileCoord,
    dir: u8,
) -> Option<(TileCoord, u8)> {
    let ((dx, dy), track_bit) = match dir & 0x03 {
        0 => ((-1_i32, 0_i32), RAIL_TB_X),
        1 => ((0_i32, 1_i32), RAIL_TB_Y),
        2 => ((1_i32, 0_i32), RAIL_TB_X),
        _ => ((0_i32, -1_i32), RAIL_TB_Y),
    };
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some((c, track_bit))
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

/// Bit alto en el parámetro `bits` de [`Command::PlaceRoadBits`]: fuerza el eje del
/// arrastre (0x0A / 0x05) aunque la tesela aún no tenga vecinos de carretera.
pub const ROAD_PLACE_FORCE_AXIS: u8 = 0x10;

pub(super) fn place_road_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_road_bits(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let force_axis = bits & ROAD_PLACE_FORCE_AXIS != 0;
    let requested = bits & 0x0F;
    let existing = state.map.get(c).map_or(0, |t| {
        if t.kind == TileKind::Road {
            t.m5 & 0x0F
        } else {
            0
        }
    });
    let road_bits = merge_road_bits_with_neighbors(&state.map, c, requested, existing, force_axis);
    write_normal_road_tile(state, c, road_bits)?;
    propagate_road_bits_to_neighbors(state, c, road_bits)?;
    state.economy.money -= ROAD_BUILD_COST;
    Ok(())
}

/// Tras un arrastre de carretera, re-alinea todas las teselas colocadas y sus vecinos.
pub fn finalize_road_drag_line(
    state: &mut GameState,
    tiles: &[TileCoord],
    axis_bits: u8,
) -> Result<(), CommandError> {
    let axis = axis_bits & 0x0F;
    for &c in tiles {
        if state.map.get_kind(c) != Some(TileKind::Road) {
            continue;
        }
        let existing = state.map.get(c).map_or(0, |t| t.m5 & 0x0F);
        let merged = merge_road_bits_with_neighbors(&state.map, c, axis, existing, true);
        write_normal_road_tile(state, c, merged)?;
    }
    for &c in tiles {
        if state.map.get_kind(c) != Some(TileKind::Road) {
            continue;
        }
        let bits = state.map.get(c).map_or(0, |t| t.m5 & 0x0F);
        propagate_road_bits_to_neighbors(state, c, bits)?;
    }
    Ok(())
}

/// Eje para herramientas bloqueadas (`RoadX` / `RoadY`): respeta la tool salvo rama
/// perpendicular al arrastrar **desde** una tesela de carretera recta.
#[must_use]
pub fn road_locked_tool_axis(map: &Map, start: TileCoord, end: TileCoord, tool_axis: u8) -> u8 {
    if let Some(tile_axis) = road_axis_from_start_tile(map, start) {
        let drag_horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
        if tile_axis == 0x0A && !drag_horizontal {
            return 0x05;
        }
        if tile_axis == 0x05 && drag_horizontal {
            return 0x0A;
        }
    }
    tool_axis
}

/// Eje inferido solo para la herramienta «Cruce de carretera» (genérica).
#[must_use]
pub fn infer_road_drag_axis(map: &Map, start: TileCoord, end: TileCoord, tool_axis: u8) -> u8 {
    if let Some(axis) = road_axis_from_start_tile(map, start) {
        let drag_horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
        if axis == 0x0A && !drag_horizontal {
            return 0x05;
        }
        if axis == 0x05 && drag_horizontal {
            return 0x0A;
        }
        return axis;
    }
    if let Some(axis) = road_axis_from_cardinal_neighbors(map, start) {
        return axis;
    }
    if let Some(axis) = road_axis_from_colinear_neighbor(map, start) {
        return axis;
    }
    if start == end {
        return tool_axis;
    }
    if (end.x - start.x).abs() >= (end.y - start.y).abs() {
        0x0A
    } else {
        0x05
    }
}

/// Línea recta de teselas para arrastrar carretera; devuelve teselas y eje efectivo.
#[must_use]
pub fn road_drag_line_tiles(
    map: &Map,
    from: (i32, i32),
    to: (i32, i32),
    tool_axis: u8,
) -> (Vec<(i32, i32)>, u8) {
    let start = TileCoord::new(from.0, from.1);
    let end = TileCoord::new(to.0, to.1);
    let axis = infer_road_drag_axis(map, start, end, tool_axis);
    let use_x = axis == 0x0A;
    let mut out = Vec::new();
    if use_x {
        let step = if to.0 >= from.0 { 1 } else { -1 };
        let mut x = from.0;
        loop {
            out.push((x, from.1));
            if x == to.0 {
                break;
            }
            x += step;
        }
    } else {
        let step = if to.1 >= from.1 { 1 } else { -1 };
        let mut y = from.1;
        loop {
            out.push((from.0, y));
            if y == to.1 {
                break;
            }
            y += step;
        }
    }
    (out, axis)
}

fn road_axis_from_start_tile(map: &Map, c: TileCoord) -> Option<u8> {
    let t = map.get(c)?;
    if t.kind != TileKind::Road {
        return None;
    }
    let b = t.m5 & 0x0F;
    if b & 0x0A != 0 && b & 0x05 == 0 {
        Some(0x0A)
    } else if b & 0x05 != 0 && b & 0x0A == 0 {
        Some(0x05)
    } else {
        None
    }
}

fn road_axis_from_cardinal_neighbors(map: &Map, c: TileCoord) -> Option<u8> {
    let has_w = map.get_kind(TileCoord::new(c.x - 1, c.y)) == Some(TileKind::Road);
    let has_e = map.get_kind(TileCoord::new(c.x + 1, c.y)) == Some(TileKind::Road);
    let has_n = map.get_kind(TileCoord::new(c.x, c.y - 1)) == Some(TileKind::Road);
    let has_s = map.get_kind(TileCoord::new(c.x, c.y + 1)) == Some(TileKind::Road);
    match (has_w || has_e, has_n || has_s) {
        (true, false) => Some(0x0A),
        (false, true) => Some(0x05),
        _ => None,
    }
}

/// Road bits resultantes al colocar (preview / HUD); no muta el mapa.
#[must_use]
pub fn preview_road_bits_at(map: &Map, c: TileCoord, requested: u8, force_axis: bool) -> u8 {
    let existing = map.get(c).map_or(0, |t| {
        if t.kind == TileKind::Road {
            t.m5 & 0x0F
        } else {
            0
        }
    });
    merge_road_bits_with_neighbors(map, c, requested & 0x0F, existing, force_axis)
}

/// Carretera en la misma fila/columna (±1 tesela) a distancia ≤3.
fn road_axis_from_colinear_neighbor(map: &Map, c: TileCoord) -> Option<u8> {
    let mut axis_h = false;
    let mut axis_v = false;
    for dx in -3..=3_i32 {
        for dy in -3..=3_i32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if map.get_kind(n) != Some(TileKind::Road) {
                continue;
            }
            if (n.y - c.y).abs() <= 1 && dx != 0 {
                axis_h = true;
            }
            if (n.x - c.x).abs() <= 1 && dy != 0 {
                axis_v = true;
            }
        }
    }
    match (axis_h, axis_v) {
        (true, false) => Some(0x0A),
        (false, true) => Some(0x05),
        (true, true) => {
            // Preferir continuar la línea más cercana en el eje dominante del arrastre.
            let mut nearest_h = i32::MAX;
            let mut nearest_v = i32::MAX;
            for dx in -3..=3_i32 {
                for dy in -3..=3_i32 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let n = TileCoord::new(c.x + dx, c.y + dy);
                    if map.get_kind(n) != Some(TileKind::Road) {
                        continue;
                    }
                    if (n.y - c.y).abs() <= 1 && dx != 0 {
                        nearest_h = nearest_h.min(dx.abs());
                    }
                    if (n.x - c.x).abs() <= 1 && dy != 0 {
                        nearest_v = nearest_v.min(dy.abs());
                    }
                }
            }
            if nearest_h <= nearest_v {
                Some(0x0A)
            } else {
                Some(0x05)
            }
        }
        _ => None,
    }
}
/// Road bits para la herramienta «Carretera» genérica (clic suelto): continúa el eje
/// del vecino cardinal o usa recta X en terreno vacío (evita cruce 0x0F aislado).
#[must_use]
pub fn road_bits_for_autoroute(map: &Map, c: TileCoord) -> u8 {
    let has_w = map.get_kind(TileCoord::new(c.x - 1, c.y)) == Some(TileKind::Road);
    let has_e = map.get_kind(TileCoord::new(c.x + 1, c.y)) == Some(TileKind::Road);
    let has_n = map.get_kind(TileCoord::new(c.x, c.y - 1)) == Some(TileKind::Road);
    let has_s = map.get_kind(TileCoord::new(c.x, c.y + 1)) == Some(TileKind::Road);
    match (has_w || has_e, has_n || has_s) {
        (true, true) => 0x0F,
        (false, true) => 0x05,
        (_, false) => 0x0A,
    }
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

/// Bits en `c` que apuntan a cada vecino con `TileKind::Road`.
fn road_bits_from_road_neighbors(map: &Map, c: TileCoord) -> u8 {
    let mut bits = 0u8;
    if map.get_kind(TileCoord::new(c.x - 1, c.y)) == Some(TileKind::Road) {
        bits |= 8;
    }
    if map.get_kind(TileCoord::new(c.x, c.y - 1)) == Some(TileKind::Road) {
        bits |= 1;
    }
    if map.get_kind(TileCoord::new(c.x + 1, c.y)) == Some(TileKind::Road) {
        bits |= 2;
    }
    if map.get_kind(TileCoord::new(c.x, c.y + 1)) == Some(TileKind::Road) {
        bits |= 4;
    }
    bits
}

/// Alinea el eje con vecinos colineales (evita recta horizontal + tool Y → tesela transversal).
fn merge_road_bits_with_neighbors(
    map: &Map,
    c: TileCoord,
    requested: u8,
    existing: u8,
    force_axis: bool,
) -> u8 {
    let has_w = map.get_kind(TileCoord::new(c.x - 1, c.y)) == Some(TileKind::Road);
    let has_e = map.get_kind(TileCoord::new(c.x + 1, c.y)) == Some(TileKind::Road);
    let has_n = map.get_kind(TileCoord::new(c.x, c.y - 1)) == Some(TileKind::Road);
    let has_s = map.get_kind(TileCoord::new(c.x, c.y + 1)) == Some(TileKind::Road);
    let connect = road_bits_from_road_neighbors(map, c);
    let axis_h = has_w || has_e;
    let axis_v = has_n || has_s;
    let straight = requested == 0x0A || requested == 0x05;

    let bits = if axis_h && axis_v {
        existing | requested | connect | 0x0A | 0x05
    } else if force_axis && straight {
        // Arrastre en línea: no girar 90° por un vecino cardinal suelto.
        connect | requested
    } else if axis_h && !axis_v {
        if existing & 0x05 == 0x05 && existing & 0x0A == 0 {
            connect | 0x05
        } else {
            connect | 0x0A
        }
    } else if axis_v && !axis_h {
        if existing & 0x0A == 0x0A && existing & 0x05 == 0 {
            connect | 0x0A
        } else {
            connect | 0x05
        }
    } else {
        existing | requested | connect
    };
    bits.max(1)
}

/// Bit en la tesela vecina que cierra el enlace con `bit` en `c` (tabla `DiagDirToRoadBits`).
const ROAD_LINK_TO_NEIGHBOR: [(u8, i32, i32, u8); 4] = [
    (8, -1, 0, 2), // NE → oeste recibe SW
    (1, 0, -1, 4), // NW → norte recibe SE
    (2, 1, 0, 8),  // SW → este recibe NE
    (4, 0, 1, 1),  // SE → sur recibe NW
];

/// Añade en vecinos con carretera el bit que apunta de vuelta a `c`.
fn propagate_road_bits_to_neighbors(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    for &(bit, dx, dy, reciproc) in &ROAD_LINK_TO_NEIGHBOR {
        if bits & bit == 0 {
            continue;
        }
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if state.map.get_kind(n) != Some(TileKind::Road) {
            continue;
        }
        let existing = state.map.get(n).map_or(0, |t| t.m5 & 0x0F);
        let merged = merge_road_bits_with_neighbors(&state.map, n, reciproc, existing, false);
        if merged != existing {
            write_normal_road_tile(state, n, merged)?;
        }
    }
    Ok(())
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
        StopKind::RailWaypoint => 7,
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

/// Huella `(ancho, alto)` en teselas de una estación de tren: con `axis_y` los
/// andenes corren a lo largo de Y (longitud en Y, andenes en X) y al revés con eje X.
#[must_use]
pub const fn rail_station_footprint(axis_y: bool, platforms: u8, length: u8) -> (i32, i32) {
    let p = platforms as i32;
    let l = length as i32;
    if axis_y { (p, l) } else { (l, p) }
}

/// Layout estándar de `OpenTTD` (`GetStationLayout`): un valor por tesela en orden
/// andén-mayor (`[andén][posición]`), con 0 plano, 2 edificio, 4/6 techos.
fn rail_station_layout(platforms: usize, length: usize) -> Vec<u8> {
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

pub(super) fn check_rail_station_area(
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

/// Estación de tren multi-tesela (`CmdBuildRailStation`): escribe todos los andenes
/// con el layout estándar y ancla la `Station` en el centro de la huella para que
/// la cobertura (radio 4) alcance los extremos de andenes de hasta 7 teselas.
pub(super) fn place_rail_station_area(
    state: &mut GameState,
    origin: TileCoord,
    axis_y: bool,
    platforms: u8,
    length: u8,
) -> Result<(), CommandError> {
    let platforms = platforms.clamp(1, 7);
    let length = length.clamp(1, 7);
    let (w, h) = rail_station_footprint(axis_y, platforms, length);
    check_rail_station_area(state, origin, w, h)?;

    let layout = rail_station_layout(usize::from(platforms), usize::from(length));
    for n in 0..platforms {
        for l in 0..length {
            let c = if axis_y {
                TileCoord::new(origin.x + i32::from(n), origin.y + i32::from(l))
            } else {
                TileCoord::new(origin.x + i32::from(l), origin.y + i32::from(n))
            };
            let idx = usize::from(n) * usize::from(length) + usize::from(l);
            let gfx = layout[idx] + u8::from(axis_y);
            if station_site_tile_needs_clear(state.map.get_kind(c).unwrap_or(TileKind::Grass)) {
                clear_station_site_tile(state, c)?;
            }
            let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            tile.kind = TileKind::Station;
            tile.mapt = 0x50;
            tile.m5 = gfx;
            tile.m6 = apply_station_m6(tile.m6, StopKind::RailStation);
            state
                .map
                .set_tile(c, tile)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.economy.money -= STATION_BUILD_COST;
        }
    }

    let anchor = TileCoord::new(origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
    state
        .stations
        .push(Station::new_with_kind(anchor, StopKind::RailStation));
    Ok(())
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

fn merge_rail_trackbits(existing: u8, add: u8) -> u8 {
    let merged = (existing | (add & 0x3F)) & 0x3F;
    if merged == 0 { add & 0x3F } else { merged }
}

pub(super) fn place_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = merged_rail_trackbits_on_tile(&state.map, c, bits);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

pub(super) fn set_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = (bits & 0x3F).max(RAIL_TB_X);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

pub(super) fn place_rail(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = rail_trackbits_from_neighbors(&state.map, c);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

/// Eje de waypoint: `false` = vía X, `true` = vía Y (`GetAxisForNewRailWaypoint`).
fn rail_waypoint_axis_from_trackbits(tb: u8) -> Option<bool> {
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

pub(super) fn place_rail_waypoint(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    check_place_rail_waypoint(&state.map, c, &state.stations)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let axis_y = rail_waypoint_axis_from_trackbits(tile.m5).unwrap_or(false);
    let mut out = tile;
    out.kind = TileKind::Station;
    out.mapt = 0x50;
    out.m5 = u8::from(axis_y);
    out.m6 = apply_station_m6(out.m6, StopKind::RailWaypoint);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .stations
        .push(Station::new_with_kind(c, StopKind::RailWaypoint));
    state.economy.money -= WAYPOINT_BUILD_COST;
    Ok(())
}

pub(crate) fn check_remove_rail(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Rail {
        return Err(CommandError::NoRailToRemove);
    }
    let subtype = (tile.m5 >> 6) & 0x3;
    if subtype != RAIL_TILE_NORMAL && subtype != RAIL_TILE_SIGNALS {
        return Err(CommandError::NoRailToRemove);
    }
    Ok(())
}

pub(crate) fn check_place_rail_signal_oriented(
    map: &Map,
    c: TileCoord,
    orientation: u8,
) -> Result<(), CommandError> {
    let tb = map
        .get(c)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F);
    let face = crate::rail_signals::signal_facing_for_orientation(tb, orientation);
    check_place_rail_signal(map, c, face)
}

pub(crate) fn check_place_rail_signal(
    map: &Map,
    c: TileCoord,
    face: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Rail {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let tb = tile.m5 & 0x3F;
    let Some(placement) = crate::rail_signals::signal_placement_for_facing(tb, face) else {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    };
    if rail_tile_is_signals(tile.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
        if present & (1 << placement.sig_bit) != 0 {
            return Err(CommandError::SignalAlreadyPresent);
        }
    }
    Ok(())
}

fn clear_rail_tile_to_grass(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    Ok(())
}

pub(super) fn remove_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_remove_rail(&state.map, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let existing = tile.m5 & 0x3F;
    let remove = bits & 0x3F;
    let new_tb = if remove == 0x3F {
        0
    } else {
        existing & !remove
    };
    if new_tb == 0 {
        clear_rail_tile_to_grass(state, c)?;
    } else {
        let subtype = (tile.m5 >> 6) & 0x3;
        let mut out = tile;
        out.m5 = new_tb | ((subtype & 0x3) << 6);
        if rail_tile_is_signals(out.m5) {
            let present = crate::rail_signals::rail_signal_present_mask(out.m3);
            let kept = present & trackbits_to_signal_present(new_tb);
            out.m3 = (out.m3 & 0x0F) | (kept << 4);
            out.m3hi = (out.m3hi & 0x0F) | (kept << 4);
        }
        state
            .map
            .set_tile(c, out)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    refresh_rail_neighbors(state, c)?;
    state.economy.money += RAIL_REMOVE_REFUND;
    Ok(())
}

/// Máscara aproximada de señales que siguen siendo válidas tras quitar `TrackBits`.
fn trackbits_to_signal_present(tb: u8) -> u8 {
    if tb == RAIL_TB_X || tb == RAIL_TB_Y {
        0b1100
    } else {
        0x0F
    }
}

pub(super) fn remove_rail(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    remove_rail_bits(state, c, 0x3F)
}

pub(super) fn place_rail_signal(
    state: &mut GameState,
    c: TileCoord,
    orientation: u8,
) -> Result<(), CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let tb = tile.m5 & 0x3F;
    let face = crate::rail_signals::signal_facing_for_orientation(tb, orientation);
    check_place_rail_signal(&state.map, c, face)?;
    let placement = crate::rail_signals::signal_placement_for_facing(tb, face)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let mut out = tile;
    if rail_tile_is_signals(out.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(out.m3);
        let merged = present | (1 << placement.sig_bit);
        out.m3 = (out.m3 & 0x0F) | (merged << 4);
        out.m3hi = (out.m3hi & 0x0F) | (merged << 4);
    } else {
        out.m5 = tb | (RAIL_TILE_SIGNALS << 6);
        out.m2 = placement.m2;
        out.m3 = placement.m3;
        out.m3hi = placement.m3hi;
    }
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= SIGNAL_BUILD_COST;
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
