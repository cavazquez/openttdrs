use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::{diag_dir_offset, station_site_tile_allows_build};
use crate::{DEPOT_BUILD_COST, GameState, ROAD_BUILD_COST};

use super::super::terraform::apply_autoslope_if_needed;
use super::super::{CommandError, require_tile_owned_by_active, tile_owner};

#[allow(unused_imports)]
use crate::command::transport::internal::{check_in_bounds, place_single_transport_tile};

pub(crate) fn check_place_road_bits(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => Ok(()),
    }
}

pub(in crate::command) fn place_road(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    place_road_bits(state, c, 0x05)
}

pub(in crate::command::transport) fn road_depot_entrance_faces_road(
    map: &Map,
    depot_pos: TileCoord,
    dir: u8,
) -> bool {
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

pub(in crate::command) fn place_road_depot_dir(
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

pub(in crate::command) fn road_depot_exit_for_dir(
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

pub const ROAD_PLACE_FORCE_AXIS: u8 = 0x10;

pub(in crate::command) fn place_road_bits(
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

pub(in crate::command::transport) fn road_axis_from_start_tile(
    map: &Map,
    c: TileCoord,
) -> Option<u8> {
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

pub(in crate::command::transport) fn road_axis_from_cardinal_neighbors(
    map: &Map,
    c: TileCoord,
) -> Option<u8> {
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

pub(in crate::command::transport) fn road_axis_from_colinear_neighbor(
    map: &Map,
    c: TileCoord,
) -> Option<u8> {
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

pub(in crate::command) fn set_road_bits(
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

pub(in crate::command) fn write_normal_road_tile(
    state: &mut GameState,
    c: TileCoord,
    road_bits: u8,
) -> Result<(), CommandError> {
    require_tile_owned_by_active(state, c)?;
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    // Conservar overlay de tranvía si ya era carretera (UI-6c).
    let preserve_tram = tile.kind == TileKind::Road;
    let tram_bits = if preserve_tram { tile.m3 & 0x0F } else { 0 };
    let tram_m8 = if preserve_tram { tile.m8 & 0x0FC0 } else { 0 };
    tile.kind = TileKind::Road;
    // MP_ROAD normal tile: low nibble stores road bits, high bits subtype=0.
    tile.mapt = 0x20;
    tile.m5 = road_bits & 0x0F;
    tile.m1 = state.active_company.0;
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = tram_bits;
    tile.m3hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = tram_m8;
    tile = crate::road_type::set_road_type_on_tile(tile, state.current_road_type);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)
}

/// Coloca / fusiona bits de tranvía en `m3` (misma máscara que road bits).
pub(in crate::command) fn place_tram_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_road_bits(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let force_axis = bits & ROAD_PLACE_FORCE_AXIS != 0;
    let requested = bits & 0x0F;
    let existing_road = state.map.get(c).map_or(0, |t| {
        if t.kind == TileKind::Road {
            t.m5 & 0x0F
        } else {
            0
        }
    });
    let existing_tram = state.map.get(c).map_or(0, |t| {
        if matches!(
            t.kind,
            TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel
        ) {
            t.m3 & 0x0F
        } else {
            0
        }
    });
    // Asegurar tesela Road (conserva road bits existentes; puede quedar en 0).
    // Puente/túnel: solo overlay m3, sin degradar a Road.
    match state.map.get_kind(c) {
        Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel) => {}
        _ => {
            write_normal_road_tile(state, c, existing_road)?;
        }
    }
    let tram_bits =
        merge_tram_bits_with_neighbors(&state.map, c, requested, existing_tram, force_axis);
    write_tram_geometry(state, c, tram_bits)?;
    propagate_tram_bits_to_neighbors(state, c, tram_bits)?;
    state.economy.money -= ROAD_BUILD_COST;
    Ok(())
}

/// Quita el overlay de tranvía (`m3`/`m8`) sin demoler la carretera.
pub(in crate::command) fn remove_tram_bits(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    use crate::road_type::{
        set_tram_road_type_on_tile, set_tram_track_bits_on_tile, tram_track_bits,
    };
    check_in_bounds(&state.map, c)?;
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !matches!(
        tile.kind,
        TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel
    ) {
        return Err(CommandError::NoTramToRemove);
    }
    if tram_track_bits(&tile) == 0 {
        return Ok(());
    }
    let mut out = set_tram_track_bits_on_tile(tile, 0);
    out = set_tram_road_type_on_tile(out, None);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= ROAD_BUILD_COST / 2;
    Ok(())
}

fn write_tram_geometry(
    state: &mut GameState,
    c: TileCoord,
    tram_bits: u8,
) -> Result<(), CommandError> {
    use crate::road_type::{set_tram_road_type_on_tile, set_tram_track_bits_on_tile};
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !matches!(
        tile.kind,
        TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel
    ) {
        tile.kind = TileKind::Road;
        tile.mapt = 0x20;
    }
    let bits = tram_bits & 0x0F;
    tile = set_tram_track_bits_on_tile(tile, bits);
    tile = set_tram_road_type_on_tile(
        tile,
        if bits == 0 {
            None
        } else {
            Some(state.current_tram_type)
        },
    );
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)
}

fn tram_bits_from_tram_neighbors(map: &Map, c: TileCoord) -> u8 {
    use crate::road_type::tram_track_bits;
    let mut bits = 0u8;
    let west = TileCoord::new(c.x - 1, c.y);
    let north = TileCoord::new(c.x, c.y - 1);
    let east = TileCoord::new(c.x + 1, c.y);
    let south = TileCoord::new(c.x, c.y + 1);
    if map.get_kind(west) == Some(TileKind::Road)
        && map.get(west).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 8;
    }
    if map.get_kind(north) == Some(TileKind::Road)
        && map.get(north).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 1;
    }
    if map.get_kind(east) == Some(TileKind::Road)
        && map.get(east).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 2;
    }
    if map.get_kind(south) == Some(TileKind::Road)
        && map.get(south).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 4;
    }
    bits
}

fn merge_tram_bits_with_neighbors(
    map: &Map,
    c: TileCoord,
    requested: u8,
    existing: u8,
    force_axis: bool,
) -> u8 {
    use crate::road_type::tram_track_bits;
    let has_w = map
        .get(TileCoord::new(c.x - 1, c.y))
        .is_some_and(|t| t.kind == TileKind::Road && tram_track_bits(&t) != 0);
    let has_e = map
        .get(TileCoord::new(c.x + 1, c.y))
        .is_some_and(|t| t.kind == TileKind::Road && tram_track_bits(&t) != 0);
    let has_n = map
        .get(TileCoord::new(c.x, c.y - 1))
        .is_some_and(|t| t.kind == TileKind::Road && tram_track_bits(&t) != 0);
    let has_s = map
        .get(TileCoord::new(c.x, c.y + 1))
        .is_some_and(|t| t.kind == TileKind::Road && tram_track_bits(&t) != 0);
    let connect = tram_bits_from_tram_neighbors(map, c);
    let axis_h = has_w || has_e;
    let axis_v = has_n || has_s;
    let straight = requested == 0x0A || requested == 0x05;

    let bits = if axis_h && axis_v {
        existing | requested | connect | 0x0A | 0x05
    } else if force_axis && straight {
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
    (bits & 0x0F).max(1)
}

fn propagate_tram_bits_to_neighbors(
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
        let existing = state.map.get(n).map_or(0, |t| t.m3 & 0x0F);
        let merged = merge_tram_bits_with_neighbors(&state.map, n, reciproc, existing, false);
        if merged != existing {
            write_tram_geometry(state, n, merged)?;
        }
    }
    Ok(())
}

pub(in crate::command::transport) fn road_bits_from_road_neighbors(map: &Map, c: TileCoord) -> u8 {
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

pub(in crate::command::transport) fn merge_road_bits_with_neighbors(
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

pub(in crate::command::transport) const ROAD_LINK_TO_NEIGHBOR: [(u8, i32, i32, u8); 4] = [
    (8, -1, 0, 2), // NE → oeste recibe SW
    (1, 0, -1, 4), // NW → norte recibe SE
    (2, 1, 0, 8),  // SW → este recibe NE
    (4, 0, 1, 1),  // SE → sur recibe NW
];

pub(in crate::command::transport) fn propagate_road_bits_to_neighbors(
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
        if tile_owner(state, n).is_some_and(|o| o != state.active_company) {
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

pub(in crate::command::transport) fn road_stop_m5(dir: u8) -> u8 {
    dir & 0x03
}

pub(in crate::command::transport) fn road_bits_toward_neighbor(dx: i32, dy: i32) -> u8 {
    match (dx, dy) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0x05,
    }
}

pub(in crate::command::transport) fn road_stop_stub_bits(dir: u8) -> u8 {
    let (dx, dy) = diag_dir_offset(dir);
    road_bits_toward_neighbor(dx, dy)
}

pub(in crate::command::transport) fn road_link_bits_toward_stop(dir: u8) -> u8 {
    let (dx, dy) = diag_dir_offset(dir);
    road_bits_toward_neighbor(-dx, -dy)
}

pub(in crate::command::transport) fn merge_road_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    if state.map.get_kind(c) != Some(TileKind::Road) {
        return Ok(());
    }
    let existing = state.map.get(c).map_or(0, |t| t.m5 & 0x0F);
    let merged = (existing | (bits & 0x0F)).max(0x01);
    write_normal_road_tile(state, c, merged)
}

pub(in crate::command::transport) fn connect_road_stop(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
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
