//! Construcción acuática: depósito, muelle, canal, boya, acueducto y esclusa.

use crate::bridge_spec::{
    BridgeType, axis_line, bridge_build_cost_in, set_bridge_middle_mapt, set_bridge_type_m6,
};
use crate::economy::{ship_depot_build_cost, ship_depot_clear_cost, station_build_cost};
use crate::map::{
    Map, Tile, TileCoord, TileKind, WaterClass, has_tile_water_ground, inclined_slope_direction,
    is_tunnel_entrance_slope, make_water_tile, set_water_class_m1, tile_slope_and_z,
    water_class_from_m1,
};
use crate::{GameState, Station, StopKind};

use super::super::CommandError;
use super::shared::check_in_bounds;
use super::station::apply_station_m6;

/// Offset de la boca del depósito según `dir` (0=NE..3=NW, misma convención road/rail).
#[must_use]
const fn ship_depot_dir_offset(dir: u8) -> (i32, i32) {
    match dir & 0x03 {
        0 => (-1, 0),
        1 => (0, 1),
        2 => (1, 0),
        _ => (0, -1),
    }
}

#[must_use]
pub(in crate::command) fn ship_depot_exit_for_dir(
    map: &Map,
    depot_pos: TileCoord,
    dir: u8,
) -> Option<TileCoord> {
    let (dx, dy) = ship_depot_dir_offset(dir);
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some(c)
}

#[must_use]
fn ship_depot_other_tile_for_dir(depot_pos: TileCoord, dir: u8) -> TileCoord {
    let (dx, dy) = ship_depot_dir_offset(dir);
    TileCoord::new(depot_pos.x - dx, depot_pos.y - dy)
}

#[must_use]
fn ship_depot_entrance_faces_water(map: &Map, c: TileCoord, dir: u8) -> bool {
    ship_depot_exit_for_dir(map, c, dir)
        .is_some_and(|exit| map.get_kind(exit) == Some(TileKind::Water))
}

/// Codifica la dirección local de la boca como `part/eje` nativos de `m5`.
///
/// `OpenTTD` obtiene la dirección con `XYNSToDiagDir(axis, part)`, por lo que
/// los cuatro valores bajos válidos no coinciden con `dir` en orden numérico:
/// `0 -> 0`, `1 -> 3`, `2 -> 1` y `3 -> 2`.
#[must_use]
const fn ship_depot_m5_for_dir(dir: u8) -> u8 {
    const PART_AXIS_BY_DIR: [u8; 4] = [0, 3, 1, 2];
    0x30 | PART_AXIS_BY_DIR[dir as usize & 0x03]
}

fn check_ship_depot_water_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get(c) {
        Some(tile) if tile.kind == TileKind::Water && has_tile_water_ground(tile) => Ok(()),
        Some(tile) if tile.kind == TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => Err(CommandError::CannotPlaceStationOnOccupiedTile),
    }
}

pub(crate) fn check_ship_depot_placement(
    map: &Map,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    check_ship_depot_water_tile(map, c)?;
    check_ship_depot_water_tile(map, ship_depot_other_tile_for_dir(c, dir))?;
    if ship_depot_entrance_faces_water(map, c, dir) {
        Ok(())
    } else {
        Err(CommandError::StationNotAdjacentToTransport)
    }
}

/// Materializa una de las dos partes que `MakeShipDepot` escribe en `MP_WATER`.
///
/// El `DepotID` del pool común se escribe en los dos bytes de `MAP2`; el resto
/// de bytes se normaliza igual que el motor nativo para que save/reload no
/// conserve payload de la tesela anterior.
#[must_use]
fn make_ship_depot_tile(original: Tile, owner: u8, dir: u8, depot_id: u16) -> Tile {
    let mut tile = original;
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60 | (original.mapt & 0x0F);
    tile.m1 = set_water_class_m1(owner & 0x1F, water_class_from_m1(original.m1));
    let [m2, m2_hi] = depot_id.to_le_bytes();
    tile.m2 = m2;
    tile.m2_hi = m2_hi;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m5 = ship_depot_m5_for_dir(dir);
    tile.m6 &= 0x03;
    tile.m7 = 0;
    tile.m8 = 0;
    tile
}

pub(in crate::command) fn place_ship_depot_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    check_ship_depot_placement(&state.map, c, dir)?;
    let depot_id =
        crate::depot::next_free_depot_id(&state.map).ok_or(CommandError::DepotPoolFull)?;
    let other = ship_depot_other_tile_for_dir(c, dir);
    let original = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let other_original = state.map.get(other).ok_or(CommandError::OutOfBounds)?;
    let owner = state.active_company.0;
    let depot_tile = make_ship_depot_tile(original, owner, dir, depot_id);
    let other_tile = make_ship_depot_tile(other_original, owner, dir.wrapping_add(2), depot_id);
    // `MakeShipDepot` conserva la zona climática de cada parte, cambia el tipo
    // alto a MP_WATER y guarda WaterTileType::Depot (`0x30`) más part/eje en m5.
    // El segundo tile es la parte opuesta de la misma huella 2x1/1x2.
    state
        .map
        .set_tile(c, depot_tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_tile(other, other_tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= ship_depot_build_cost(&state.global_economy);
    Ok(())
}

/// Retira el depósito naval completo aunque el cursor apunte a cualquiera de
/// sus dos secciones, como `RemoveShipDepot` en `water_cmd.cpp`.
pub(in crate::command) fn clear_ship_depot(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_clear_ship_depot(state, c)?;
    let other =
        crate::depot::ship_depot_other_tile(&state.map, c).ok_or(CommandError::InvalidDepotTile)?;
    let tiles = [c, other];
    let water_classes = tiles.map(|tile| {
        state
            .map
            .get(tile)
            .map_or(WaterClass::Sea, |raw| water_class_from_m1(raw.m1))
    });
    for (tile, water_class) in tiles.into_iter().zip(water_classes) {
        make_water_tile(&mut state.map, tile, water_class)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= ship_depot_clear_cost(&state.global_economy);
    Ok(())
}

/// Validación de solo lectura para `RemoveShipDepot`.
///
/// El cursor puede estar en cualquiera de las dos secciones, pero la
/// propiedad y la ocupación se comprueban sobre la huella completa antes de
/// cambiar el mapa. El preview reutiliza esta función para no anunciar una
/// demolición que `clear_ship_depot` rechazaría.
pub(in crate::command) fn check_clear_ship_depot(
    state: &GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    let other =
        crate::depot::ship_depot_other_tile(&state.map, c).ok_or(CommandError::InvalidDepotTile)?;
    for tile in [c, other] {
        if !state.cheats.magic_bulldozer_active() {
            crate::command::require_tile_owned_by_active(state, tile)?;
        }
        if state.vehicles.iter().any(|vehicle| vehicle.pos == tile) {
            return Err(CommandError::VehicleInTheWay);
        }
    }
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
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    state.stations.push(st);
    state.economy.money -= station_build_cost(&state.global_economy);
    Ok(())
}

/// Canal: convierte hierba/bosque en agua navegable plana (`WaterClass::Canal`).
pub(crate) fn check_place_canal(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if tileh != 0 {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
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
    if state.map.get(c).is_some_and(crate::map::is_canal_tile) {
        return Ok(());
    }
    make_water_tile(&mut state.map, c, WaterClass::Canal).map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= station_build_cost(&state.global_economy) / 2;
    Ok(())
}

/// Río: plano o pendiente inclinada (no diagonal); `WaterClass::River`.
/// En `OpenTTD` solo editor; aquí es herramienta de pintura (sandbox / escenario).
pub(crate) fn check_place_river(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if tileh != 0 && !is_tunnel_entrance_slope(tileh) {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Water => Ok(()),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => Err(CommandError::CannotPlaceStationOnOccupiedTile),
    }
}

pub(in crate::command) fn place_river(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_river(&state.map, c)?;
    if state.map.get(c).is_some_and(crate::map::is_river_tile) {
        return Ok(());
    }
    make_water_tile(&mut state.map, c, WaterClass::River).map_err(|_| CommandError::OutOfBounds)?;
    crate::world_gen::clear_desert_zone_around_river(&mut state.map, c)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= station_build_cost(&state.global_economy) / 4;
    Ok(())
}

/// Boya: agua plana (no esclusa) sin estación previa.
pub(crate) fn check_place_buoy(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Water {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    // No sobre esclusa (subtype Lock = 2).
    if (tile.m5 >> 4) & 0x0F == 2 {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    Ok(())
}

pub(in crate::command) fn place_buoy(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_buoy(&state.map, &state.stations, c)?;
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    tile.kind = TileKind::Station;
    tile.mapt = 0x50;
    tile.m5 = 0;
    tile.m6 = apply_station_m6(tile.m6, StopKind::Buoy);
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    let mut st = Station::new_with_kind(c, StopKind::Buoy);
    st.owner = state.active_company;
    st.build_date = crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
    state.stations.push(st);
    state.economy.money -= station_build_cost(&state.global_economy) / 2;
    Ok(())
}

/// Dirección diagonal desde `from` hacia `to` (eje ortogonal).
fn aqueduct_toward_dir(from: TileCoord, to: TileCoord) -> Option<u8> {
    match (to.x - from.x, to.y - from.y) {
        (dx, 0) if dx > 0 => Some(2), // SW
        (dx, 0) if dx < 0 => Some(0), // NE
        (0, dy) if dy > 0 => Some(1), // SE
        (0, dy) if dy < 0 => Some(3), // NW
        _ => None,
    }
}

fn aqueduct_endpoint_slope_ok(map: &Map, endpoint: TileCoord, other: TileCoord) -> bool {
    let Some((tileh, _)) = tile_slope_and_z(map, endpoint) else {
        return false;
    };
    let Some(slope_dir) = inclined_slope_direction(tileh) else {
        return false;
    };
    aqueduct_toward_dir(endpoint, other) == Some(slope_dir)
}

/// Acueducto: vano ≥3, mismas alturas, rampas en pendiente enfrentadas.
pub(crate) fn check_place_aqueduct(
    map: &Map,
    a: TileCoord,
    b: TileCoord,
) -> Result<(), CommandError> {
    let line = axis_line(a, b);
    if line.len() < 3 {
        return Err(CommandError::InvalidBridgeSpan);
    }
    let (Some((_, za)), Some((_, zb))) = (tile_slope_and_z(map, a), tile_slope_and_z(map, b))
    else {
        return Err(CommandError::OutOfBounds);
    };
    if za != zb {
        return Err(CommandError::InvalidBridgeSpan);
    }
    if !aqueduct_endpoint_slope_ok(map, a, b) || !aqueduct_endpoint_slope_ok(map, b, a) {
        return Err(CommandError::InvalidBridgeSpan);
    }
    for (i, c) in line.iter().enumerate() {
        check_in_bounds(map, *c)?;
        let kind = map.get_kind(*c).unwrap_or(TileKind::Grass);
        let is_endpoint = i == 0 || i + 1 == line.len();
        if is_endpoint {
            match kind {
                TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Water => {}
                TileKind::Void => return Err(CommandError::CannotPlaceStationOnVoid),
                _ => return Err(CommandError::CannotPlaceStationOnOccupiedTile),
            }
        } else {
            match kind {
                TileKind::Grass
                | TileKind::Forest
                | TileKind::CoalField
                | TileKind::Water
                | TileKind::House => {}
                TileKind::Void => return Err(CommandError::CannotPlaceStationOnVoid),
                _ => return Err(CommandError::CannotPlaceStationOnOccupiedTile),
            }
        }
    }
    Ok(())
}

/// Rampa de acueducto: bit 7 + `TRANSPORT_WATER` (2) + dirección diagonal.
fn aqueduct_ramp_m5(dir: u8) -> u8 {
    const TRANSPORT_WATER: u8 = 2;
    0x80 | (TRANSPORT_WATER << 2) | (dir & 0x03)
}

pub(in crate::command) fn place_aqueduct(
    state: &mut GameState,
    a: TileCoord,
    b: TileCoord,
) -> Result<(), CommandError> {
    check_place_aqueduct(&state.map, a, b)?;
    let line = axis_line(a, b);
    let bridge_axis_y = (b.x - a.x).abs() < (b.y - a.y).abs();
    let bridge_type = BridgeType::Wooden;
    let cost = bridge_build_cost_in(&state.bridge_spec_catalog, bridge_type, a, b);
    for (i, c) in line.iter().enumerate() {
        let mut tile = state.map.get(*c).ok_or(CommandError::OutOfBounds)?;
        let is_endpoint = i == 0 || i + 1 == line.len();
        // Agua navegable; las alturas de esquina (pendiente) se conservan en el mapa.
        tile.kind = TileKind::Water;
        if is_endpoint {
            let other = if i == 0 { b } else { a };
            let dir = aqueduct_toward_dir(*c, other).unwrap_or(0);
            // `MP_TUNNELBRIDGE` para que el render detecte rampa (`ramp_tile`).
            tile.mapt = 0x90;
            tile.m5 = aqueduct_ramp_m5(dir);
            tile.m6 = set_bridge_type_m6(tile.m6, bridge_type);
        } else {
            tile.mapt = set_bridge_middle_mapt(0x60, bridge_axis_y);
            tile.m5 = 0;
            tile.m6 = set_bridge_type_m6(tile.m6, bridge_type);
        }
        state
            .map
            .set_tile(*c, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= cost;
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
    state.economy.money -= station_build_cost(&state.global_economy);
    Ok(())
}

/// Re-export para preview: docks usan check propio, no `check_station_placement`.
#[allow(dead_code)]
pub(crate) fn check_place_dock_or_station(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
    _dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    if stop_kind == StopKind::Dock {
        check_dock_placement(map, stations, c)
    } else if stop_kind == StopKind::Buoy {
        check_place_buoy(map, stations, c)
    } else {
        Err(CommandError::CannotPlaceStationOnOccupiedTile)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::world_gen::{CLEAR_GROUND_DESERT, clear_ground_m5};

    #[test]
    fn ship_depot_direction_uses_native_part_and_axis_encoding() {
        assert_eq!(ship_depot_m5_for_dir(0), 0x30);
        assert_eq!(ship_depot_m5_for_dir(1), 0x33);
        assert_eq!(ship_depot_m5_for_dir(2), 0x31);
        assert_eq!(ship_depot_m5_for_dir(3), 0x32);
        assert_eq!(
            ship_depot_m5_for_dir(7),
            0x32,
            "dir se normaliza a cuatro valores"
        );
    }

    #[test]
    fn place_river_clears_desert_zone_without_mutating_raw_object_payload() {
        let mut state = GameState::new(7, 7);
        let center = TileCoord::new(3, 3);
        let object_pos = TileCoord::new(2, 2);
        let mut object = state.map.get(object_pos).unwrap();
        object.mapt = 0xA1;
        object.m1 = 0x70;
        object.m2 = 9;
        object.m3 = 63;
        object.m5 = clear_ground_m5(CLEAR_GROUND_DESERT, 2);
        state.map.set_tile(object_pos, object).unwrap();

        place_river(&mut state, center).unwrap();

        let mut expected = object;
        expected.mapt = 0xA0;
        assert_eq!(state.map.get(object_pos), Some(expected));
    }
}
