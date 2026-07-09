//! Construcción acuática: depósito, muelle, canal y esclusa.

use crate::map::{Map, TileCoord, TileKind};
use crate::{DEPOT_BUILD_COST, GameState, STATION_BUILD_COST, Station, StopKind};

use super::super::CommandError;
use super::shared::check_in_bounds;
use super::station::{apply_station_m6, check_station_placement};

/// Offset de la boca del depósito según `dir` (0=NE..3=NW, misma convención road/rail).
#[must_use]
pub(in crate::command) fn ship_depot_exit_for_dir(
    map: &Map,
    depot_pos: TileCoord,
    dir: u8,
) -> Option<TileCoord> {
    let (dx, dy) = match dir & 0x03 {
        0 => (-1_i32, 0_i32),
        1 => (0_i32, 1_i32),
        2 => (1_i32, 0_i32),
        _ => (0_i32, -1_i32),
    };
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some(c)
}

#[must_use]
fn ship_depot_entrance_faces_water(map: &Map, c: TileCoord, dir: u8) -> bool {
    ship_depot_exit_for_dir(map, c, dir)
        .is_some_and(|exit| map.get_kind(exit) == Some(TileKind::Water))
}

pub(crate) fn check_ship_depot_placement(
    map: &Map,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => {
            if ship_depot_entrance_faces_water(map, c, dir & 0x03) {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => Err(CommandError::CannotPlaceStationOnOccupiedTile),
    }
}

pub(in crate::command) fn place_ship_depot_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    check_ship_depot_placement(&state.map, c, dir)?;
    state
        .map
        .set_kind(c, TileKind::ShipDepot)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x60, (2 << 6) | dir)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= DEPOT_BUILD_COST;
    Ok(())
}

/// Muelle: agua plana con al menos un vecino de tierra (costa).
pub(crate) fn check_dock_placement(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    if map.get_kind(c) != Some(TileKind::Water) {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    let land_neighbor = [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .any(|(dx, dy)| {
            let n = TileCoord::new(c.x + dx, c.y + dy);
            map.get_kind(n).is_some_and(|k| {
                !matches!(
                    k,
                    TileKind::Water | TileKind::ShipDepot | TileKind::Void | TileKind::Station
                )
            })
        });
    if !land_neighbor {
        return Err(CommandError::StationNotAdjacentToTransport);
    }
    Ok(())
}

pub(in crate::command) fn place_dock(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_dock_placement(&state.map, &state.stations, c)?;
    let m5 = u8::from(dir & 1 != 0);
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.kind = TileKind::Station;
    tile.mapt = 0x50;
    tile.m5 = m5;
    tile.m6 = apply_station_m6(tile.m6, StopKind::Dock);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    let mut st = Station::new_with_kind(c, StopKind::Dock);
    st.owner = state.active_company;
    state.stations.push(st);
    state.economy.money -= STATION_BUILD_COST;
    Ok(())
}

/// Canal: convierte hierba/bosque en agua navegable (sin bajar altura).
pub(crate) fn check_place_canal(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Water => Ok(()),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => Err(CommandError::CannotPlaceStationOnOccupiedTile),
    }
}

pub(in crate::command) fn place_canal(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_canal(&state.map, c)?;
    if state.map.get_kind(c) == Some(TileKind::Water) {
        return Ok(());
    }
    state
        .map
        .set_kind(c, TileKind::Water)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x60, 0)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= STATION_BUILD_COST / 2;
    Ok(())
}

fn lock_axis_neighbors(c: TileCoord, axis_y: bool) -> (TileCoord, TileCoord) {
    if axis_y {
        (TileCoord::new(c.x, c.y - 1), TileCoord::new(c.x, c.y + 1))
    } else {
        (TileCoord::new(c.x - 1, c.y), TileCoord::new(c.x + 1, c.y))
    }
}

/// Esclusa: agua + vecinos del eje con `|Δheight| == 1`.
pub(crate) fn check_place_lock(map: &Map, c: TileCoord, axis_y: bool) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if map.get_kind(c) != Some(TileKind::Water) {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    let (a, b) = lock_axis_neighbors(c, axis_y);
    check_in_bounds(map, a)?;
    check_in_bounds(map, b)?;
    if !crate::ship_movement::is_water_network_tile_at(map, a)
        || !crate::ship_movement::is_water_network_tile_at(map, b)
    {
        return Err(CommandError::StationNotAdjacentToTransport);
    }
    let ha = map.get(a).map_or(0, |t| t.height);
    let hb = map.get(b).map_or(0, |t| t.height);
    if ha.abs_diff(hb) != 1 {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    Ok(())
}

pub(in crate::command) fn place_lock(
    state: &mut GameState,
    c: TileCoord,
    axis_y: bool,
) -> Result<(), CommandError> {
    check_place_lock(&state.map, c, axis_y)?;
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    // Water subtype Lock = 2 in bits 4–7; bit 0 of low nibble = axis.
    tile.m5 = (2 << 4) | u8::from(axis_y);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= STATION_BUILD_COST;
    Ok(())
}

/// Re-export para preview: docks usan check propio, no `check_station_placement`.
#[allow(dead_code)]
pub(crate) fn check_place_dock_or_station(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    if stop_kind == StopKind::Dock {
        check_dock_placement(map, stations, c)
    } else {
        check_station_placement(map, stations, c, dir, stop_kind)
    }
}
