//! Construcción acuática: depósito, muelle, canal, boya, acueducto y esclusa.

use crate::bridge_spec::{
    BridgeType, axis_line, bridge_above_axis_from_mapt, bridge_build_cost_in,
    set_bridge_middle_mapt, set_bridge_type_m6,
};
use crate::economy::{
    canal_clear_cost, rough_clear_cost, ship_depot_build_cost, ship_depot_clear_cost,
    station_build_cost, water_clear_cost,
};
use crate::map::{
    Map, Tile, TileCoord, TileKind, WaterClass, clear_neighbour_non_flooding_states,
    clear_tile_after_native_water_restore, has_tile_water_ground, inclined_slope_direction,
    is_map_object_tile, is_tunnel_entrance_slope, make_water_tile_with_random_bits,
    object_footprint_tiles, object_id_from_tile, object_origin_from_tile, object_type_dims_id,
    opposite_diag_dir, set_water_class_m1, tile_slope_and_z, water_class_after_native_clear,
    water_class_from_m1,
};
use crate::{GameState, Station, StopKind};

use super::super::{CommandError, require_tile_owned_by_active};
use super::shared::{
    check_in_bounds, check_object_can_be_auto_cleared, clear_object_footprint_keep_water,
    register_depot, unregister_depot,
};
use super::station::apply_station_m6;

#[must_use]
fn ship_depot_other_tile_for_dir(depot_pos: TileCoord, dir: u8) -> TileCoord {
    crate::depot::ship_depot_footprint(depot_pos, dir)[1]
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

fn auto_clear_object_footprint(
    state: &GameState,
    c: TileCoord,
) -> Result<Vec<TileCoord>, CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let object_id = object_id_from_tile(&tile).ok_or(CommandError::ObjectInTheWay)?;
    let (origin, width, height) = if let Some(object) = state
        .objects
        .iter()
        .find(|object| object.object_id == object_id)
    {
        let width = u8::try_from(object.width)
            .ok()
            .filter(|width| *width > 0)
            .ok_or(CommandError::ObjectInTheWay)?;
        let height = u8::try_from(object.height)
            .ok()
            .filter(|height| *height > 0)
            .ok_or(CommandError::ObjectInTheWay)?;
        (object.tile, width, height)
    } else {
        let object_type = state
            .map
            .object_type_at(c)
            .ok_or(CommandError::ObjectInTheWay)?;
        let origin = object_origin_from_tile(&tile, c).ok_or(CommandError::ObjectInTheWay)?;
        let (width, height) = object_type_dims_id(object_type, &state.object_spec_catalog);
        if width == 0 || height == 0 {
            return Err(CommandError::ObjectInTheWay);
        }
        (origin, width, height)
    };
    let object_tiles = object_footprint_tiles(origin, width, height);
    for &object_tile in &object_tiles {
        check_in_bounds(&state.map, object_tile)?;
        if !state
            .map
            .get(object_tile)
            .is_some_and(|raw| is_map_object_tile(raw.mapt))
        {
            return Err(CommandError::ObjectInTheWay);
        }
    }
    if object_tiles.contains(&c) {
        Ok(object_tiles)
    } else {
        Err(CommandError::ObjectInTheWay)
    }
}

/// Planifica los objetos que `CmdLandscapeClear(... | Auto)` eliminaría.
///
/// El motor marca una huella ya limpiada para que un objeto que ocupa las dos
/// partes del depósito sólo se destruya una vez. La lista deduplicada conserva
/// esa misma regla para que preview y ejecución sean atómicos.
fn auto_clear_object_plan(
    state: &GameState,
    tiles: impl IntoIterator<Item = TileCoord>,
) -> Result<Vec<Vec<TileCoord>>, CommandError> {
    let mut plan = Vec::new();
    for tile in tiles {
        let Some(raw) = state.map.get(tile) else {
            return Err(CommandError::OutOfBounds);
        };
        if !is_map_object_tile(raw.mapt) {
            continue;
        }
        check_object_can_be_auto_cleared(state, tile)?;
        let object_tiles = auto_clear_object_footprint(state, tile)?;
        if !plan.contains(&object_tiles) {
            plan.push(object_tiles);
        }
    }
    Ok(plan)
}

/// `CheckForDockingTile` de `OpenTTD`: una sección de agua puede ser alcanzada
/// por un barco si tiene al lado la parte acuática de un muelle, una industria
/// con estación neutral o una tesela de oil rig.
fn ship_depot_has_docking_neighbor(state: &GameState, c: TileCoord) -> bool {
    (0..4).any(|dir| {
        let (dx, dy) = crate::map::diag_dir_offset(dir);
        let neighbor = TileCoord::new(c.x + dx, c.y + dy);
        let Some(tile) = state.map.get(neighbor) else {
            return false;
        };
        match tile.kind {
            TileKind::Station => match crate::station::stop_kind_from_m6(tile.m6) {
                // `IsDockWaterPart` usa GFX_DOCK_BASE_WATER_PART = 4.
                StopKind::Dock => tile.m5 >= 4,
                StopKind::OilRig => true,
                _ => false,
            },
            TileKind::Industry => {
                let industry_id = crate::map::industry_instance_id(&tile);
                state.stations.iter().any(|station| {
                    station.stop_kind == StopKind::OilRig
                        && station.neutral_industry_id == Some(industry_id)
                })
            }
            _ => false,
        }
    })
}

/// Actualiza `SetDockingTile` sobre una sección de agua recién materializada.
fn refresh_ship_depot_docking_tile(state: &mut GameState, c: TileCoord) {
    let Some(mut tile) = state.map.get(c) else {
        return;
    };
    if !matches!(tile.kind, TileKind::Water | TileKind::ShipDepot) {
        return;
    }
    if ship_depot_has_docking_neighbor(state, c) {
        tile.m1 |= 0x80;
    } else {
        tile.m1 &= !0x80;
    }
    let _ = state.map.set_tile(c, tile);
}

/// Reevalúa las teselas de agua vecinas a una instalación acuática.
///
/// `UpdateStationDockingTiles` no sólo modifica la tesela recién escrita: un
/// muelle nuevo o demolido también cambia el bit `DockingTile` de los tiles de
/// agua que lo rodean. La misma rutina sirve para el ciclo de vida de docks y
/// depósitos, manteniendo el orden de actualización determinista.
fn refresh_ship_docking_tiles_around(state: &mut GameState, center: TileCoord) {
    for dir in 0..4 {
        let (dx, dy) = crate::map::diag_dir_offset(dir);
        refresh_ship_depot_docking_tile(state, TileCoord::new(center.x + dx, center.y + dy));
    }
}

/// Materializa agua restaurada con el consumo de `MakeWaterKeepingClass`.
///
/// `OpenTTD` sólo toma `Random()` para canal/río; el mar usa el byte `MAP4 = 0`.
pub(in crate::command::transport) fn make_water_tile_after_native_clear(
    state: &mut GameState,
    c: TileCoord,
    water_class: WaterClass,
) -> Result<(), CommandError> {
    let water_class = water_class_after_native_clear(&state.map, c, water_class);
    if water_class == WaterClass::Invalid {
        clear_tile_after_native_water_restore(&mut state.map, c)
            .map_err(|_| CommandError::OutOfBounds)?;
        return Ok(());
    }
    let random_bits = match water_class {
        WaterClass::Canal | WaterClass::River => {
            u8::try_from(state.random.next() & 0xFF).unwrap_or(0)
        }
        WaterClass::Sea | WaterClass::Invalid => 0,
    };
    make_water_tile_with_random_bits(&mut state.map, c, water_class, random_bits)
        .map_err(|_| CommandError::OutOfBounds)
}

fn check_ship_depot_water_tile(state: &GameState, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(&state.map, c)?;
    match state.map.get(c) {
        Some(tile) if has_tile_water_ground(tile) => {
            if matches!(tile.kind, TileKind::Water | TileKind::ShipDepot) {
                Ok(())
            } else if tile.kind == TileKind::Station {
                match crate::station::stop_kind_from_m6(tile.m6) {
                    StopKind::Dock => Err(CommandError::MustDemolishDockFirst),
                    StopKind::Buoy => Err(CommandError::BuoyInTheWay),
                    StopKind::OilRig => Err(CommandError::OilRigInTheWay),
                    _ => Err(CommandError::BuildingMustBeDemolished),
                }
            } else if tile.kind == TileKind::Industry {
                // `ClearTile_Industry(..., Auto)` siempre devuelve
                // `GENERIC_OBJECT_IN_THE_WAY`; una industria no se demuele
                // como parte de la construcción de otro objeto.
                Err(CommandError::IndustryInTheWay)
            } else if is_map_object_tile(tile.mapt) {
                check_object_can_be_auto_cleared(state, c)
            } else {
                Err(CommandError::CannotPlaceStationOnOccupiedTile)
            }
        }
        Some(tile) if tile.kind == TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => Err(CommandError::CannotPlaceStationOnOccupiedTile),
    }
}

const WATER_TILE_TYPE_CLEAR: u8 = 0;
const WATER_TILE_TYPE_COAST: u8 = 1;

#[must_use]
fn water_tile_type(tile: Tile) -> u8 {
    (tile.m5 >> 4) & 0x0F
}

#[must_use]
const fn is_slope_with_one_corner_raised(tileh: u8) -> bool {
    matches!(tileh, 1 | 2 | 4 | 8)
}

fn check_clear_water_owner(state: &GameState, tile: Tile) -> Result<(), CommandError> {
    let owner = tile.m1 & 0x1F;
    let owner_none = crate::company::OWNER_NONE_M1 & 0x1F;
    let owner_water = crate::company::OWNER_WATER_M1 & 0x1F;
    if owner != owner_none && owner != owner_water && owner != state.active_company.0 {
        Err(CommandError::TileNotOwned)
    } else {
        Ok(())
    }
}

/// Validación de `ClearTile_Water` para agua plana o costa.
pub(crate) fn check_clear_water(state: &GameState, c: TileCoord) -> Result<(), CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if tile.kind != TileKind::Water {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    match water_tile_type(tile) {
        WATER_TILE_TYPE_CLEAR => {
            check_clear_water_owner(state, tile)?;
        }
        WATER_TILE_TYPE_COAST => {}
        // Locks have a three-tile lifecycle and are handled by their own
        // remover; never let generic clear leave two lock pieces orphaned.
        _ => return Err(CommandError::BuildingMustBeDemolished),
    }
    if state.vehicles.iter().any(|vehicle| vehicle.pos == c) {
        return Err(CommandError::VehicleInTheWay);
    }
    Ok(())
}

/// Implementa la rama de agua clara/costa de `ClearTile_Water`.
pub(in crate::command) fn clear_water_tile(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_clear_water(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let cost = match water_tile_type(tile) {
        WATER_TILE_TYPE_CLEAR => {
            if water_class_from_m1(tile.m1) == WaterClass::Canal {
                canal_clear_cost(&state.global_economy)
            } else {
                water_clear_cost(&state.global_economy)
            }
        }
        WATER_TILE_TYPE_COAST => {
            let (tileh, _) = tile_slope_and_z(&state.map, c).ok_or(CommandError::OutOfBounds)?;
            if is_slope_with_one_corner_raised(tileh) {
                water_clear_cost(&state.global_economy)
            } else {
                rough_clear_cost(&state.global_economy)
            }
        }
        _ => return Err(CommandError::BuildingMustBeDemolished),
    };
    clear_tile_after_native_water_restore(&mut state.map, c)
        .map_err(|_| CommandError::OutOfBounds)?;
    clear_neighbour_non_flooding_states(&mut state.map, c);
    state.economy.money -= cost;
    Ok(())
}

pub(crate) fn check_ship_depot_placement(
    state: &GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    let [origin, other] = crate::ship_depot_footprint(c, dir);
    // `CmdBuildShipDepot` comprueba primero la clase de suelo de ambas
    // teselas. Mantener esa fase separada conserva el error nativo cuando la
    // segunda parte cae fuera del mapa o no es agua.
    for tile in [origin, other] {
        check_ship_depot_water_tile(state, tile)?;
    }
    // `IsBridgeAbove` mira los bits de MAPT aunque el suelo inferior sea agua.
    // Un puente sobre cualquiera de las dos partes bloquea la construcción.
    for tile in [origin, other] {
        if state
            .map
            .get(tile)
            .is_some_and(|raw| bridge_above_axis_from_mapt(raw.mapt).is_some())
        {
            return Err(CommandError::MustDemolishBridgeFirst);
        }
    }
    // `IsTileFlat` se evalúa sobre las dos teselas; esto también impide
    // construir sobre rápidos o una pendiente de terreno importada.
    for tile in [origin, other] {
        if tile_slope_and_z(&state.map, tile).is_none_or(|(tileh, _)| tileh != 0) {
            return Err(CommandError::SiteUnsuitable);
        }
    }
    // Locks y depósitos también son `MP_WATER` con una clase válida, pero
    // `ClearTile_Water` los rechaza cuando recibe `Auto`; nunca se deben
    // sobrescribir silenciosamente durante la construcción.
    for tile in [origin, other] {
        if state.map.get(tile).is_some_and(|raw| {
            matches!(raw.kind, TileKind::Water | TileKind::ShipDepot) && (raw.m5 >> 4) & 0x0F != 0
        }) {
            return Err(CommandError::BuildingMustBeDemolished);
        }
    }
    // `CmdBuildShipDepot` no consulta una tercera tesela delante de la boca:
    // su contrato sólo exige agua en las dos teselas que reemplaza. La
    // navegación podrá usar el mapa contiguo después de construir; imponer
    // aquí una entrada adicional rechaza depósitos válidos junto a tierra.
    let _ = auto_clear_object_plan(state, [origin, other])?;
    Ok(())
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
    check_ship_depot_placement(state, c, dir)?;
    let depot_id =
        crate::depot::next_free_depot_id(&state.map).ok_or(CommandError::DepotPoolFull)?;
    let other = ship_depot_other_tile_for_dir(c, dir);
    let auto_clear_objects = auto_clear_object_plan(state, [c, other])?;
    for object_tiles in auto_clear_objects {
        clear_object_footprint_keep_water(state, object_tiles[0], &object_tiles)?;
    }
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
    refresh_ship_depot_docking_tile(state, c);
    refresh_ship_depot_docking_tile(state, other);
    register_depot(state, depot_id, c);
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
    let depot_id = state
        .map
        .get(c)
        .and_then(crate::depot::depot_id_from_tile)
        .ok_or(CommandError::InvalidDepotTile)?;
    let tiles = [c, other];
    let water_classes = tiles.map(|tile| {
        state
            .map
            .get(tile)
            .map_or(WaterClass::Sea, |raw| water_class_from_m1(raw.m1))
    });
    for (tile, water_class) in tiles.into_iter().zip(water_classes) {
        make_water_tile_after_native_clear(state, tile, water_class)?;
    }
    refresh_ship_depot_docking_tile(state, c);
    refresh_ship_depot_docking_tile(state, other);
    unregister_depot(state, depot_id);
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

/// Muelle: pieza de tierra inclinada, pieza de agua contigua y un segundo
/// tile de agua plano para la boca.
///
/// `CmdBuildDock` recibe la tesela de tierra. La pieza acuática que queda
/// junto a ella se convierte en `MP_STATION` con `GFX_DOCK_BASE_WATER_PART`,
/// pero la siguiente tesela sigue siendo agua libre y debe existir para que un
/// barco pueda aproximarse.
pub(crate) fn check_dock_placement(
    map: &Map,
    stations: &[Station],
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let dir = dir & 0x03;
    let water = crate::station::dock_water_tile(c, dir);
    if [c, water].iter().any(|&tile| {
        map.get(tile)
            .is_some_and(|raw| matches!(raw.kind, TileKind::Station | TileKind::Airport))
    }) {
        return Err(CommandError::StationAlreadyExists);
    }
    let approach = crate::station::dock_water_tile(water, dir);
    if stations
        .iter()
        .any(|s| s.covers_tile(c) || s.covers_tile(water))
    {
        return Err(CommandError::StationAlreadyExists);
    }
    // This stage only has a lossless clear path for plain clear ground.  Do
    // not silently replace a forest until the command also models native
    // auto-clear costs and callbacks.
    if map.get_kind(c) != Some(TileKind::Grass) {
        return Err(CommandError::SiteUnsuitable);
    }
    let Some((land_tileh, _)) = tile_slope_and_z(map, c) else {
        return Err(CommandError::SiteUnsuitable);
    };
    let Some(slope_direction) = inclined_slope_direction(land_tileh) else {
        return Err(CommandError::SiteUnsuitable);
    };
    if opposite_diag_dir(slope_direction) != dir {
        return Err(CommandError::SiteUnsuitable);
    }
    let Some(water_tile) = map.get(water) else {
        return Err(CommandError::SiteUnsuitable);
    };
    let water_is_object = is_map_object_tile(water_tile.mapt);
    if !has_tile_water_ground(water_tile)
        || (!water_is_object && water_tile.kind != TileKind::Water)
        || tile_slope_and_z(map, water).is_none_or(|(tileh, _)| tileh != 0)
    {
        return Err(CommandError::SiteUnsuitable);
    }
    let Some(approach_tile) = map.get(approach) else {
        return Err(CommandError::SiteUnsuitable);
    };
    if approach_tile.kind != TileKind::Water
        || tile_slope_and_z(map, approach).is_none_or(|(tileh, _)| tileh != 0)
    {
        return Err(CommandError::SiteUnsuitable);
    }
    Ok(())
}

/// Validación completa de `CmdBuildDock` para preview y ejecución.
///
/// `CmdBuildDock` delega la limpieza de cada pieza a
/// `CmdLandscapeClear(... | Auto)`. La variante geométrica anterior no puede
/// consultar el catálogo de objetos ni sus propietarios, por eso esta entrada
/// agrega el mismo preflight de objetos autoremovibles sin mutar el estado.
pub(in crate::command) fn check_dock_placement_with_state(
    state: &GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_dock_placement(&state.map, &state.stations, c, dir)?;
    let dir = dir & 0x03;
    let water = crate::station::dock_water_tile(c, dir);
    auto_clear_object_plan(state, [c, water])?;
    Ok(())
}

/// Primer `StationID` libre para una estación nueva escrita en `MAP2`.
///
/// Las estaciones creadas por versiones antiguas del port pueden conservar
/// `m2 = 0` sin un `ottd_station_id`; ese valor se considera ocupado para no
/// asociar por accidente un muelle nuevo a una estación legacy.
fn next_station_id(state: &GameState) -> Option<u16> {
    let mut used = std::collections::BTreeSet::new();
    for station in &state.stations {
        if let Some(id) = station
            .ottd_station_id
            .and_then(|id| u16::try_from(id).ok())
        {
            used.insert(id);
        }
    }
    for tile in state.map.tiles() {
        if matches!(tile.kind, TileKind::Station | TileKind::Airport) {
            used.insert(u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8));
        }
    }
    (0..=u16::MAX).find(|id| !used.contains(id))
}

fn station_can_join_dock(state: &GameState, station: &Station) -> bool {
    station.owner == state.active_company
        && station.stop_kind != StopKind::OilRig
        && station.has_dock_facility()
        && !crate::station::dock_station_tiles(&state.map, station).is_empty()
}

/// Encuentra la única estación naval propia que `GetStationAround` podría
/// asociar al área del nuevo muelle.
fn find_adjacent_dock_station(
    state: &GameState,
    c: TileCoord,
    dir: u8,
) -> Result<Option<(usize, u16)>, CommandError> {
    let water = crate::station::dock_water_tile(c, dir & 0x03);
    let new_tiles = [c, water];
    let mut found = None;
    for (index, station) in state.stations.iter().enumerate() {
        if !station_can_join_dock(state, station) {
            continue;
        }
        let existing_tiles = crate::station::dock_station_tiles(&state.map, station);
        if !crate::station::station_tile_sets_adjacent(&new_tiles, &existing_tiles) {
            continue;
        }
        let station_id = super::station::dock_station_native_id(state, station)
            .ok_or(CommandError::CannotJoinStations)?;
        if found.is_some() {
            return Err(CommandError::CannotJoinStations);
        }
        found = Some((index, station_id));
    }
    Ok(found)
}

/// Resuelve un `station_to_join` nativo a la estación lógica local.
///
/// El ID se valida contra la estación y contra la huella física de muelle. No
/// basta con encontrar una estación con el mismo `ottd_station_id`: una
/// estación intermodal puede compartirlo, pero debe demostrar que conserva al
/// menos una pieza naval real antes de aceptar otra.
fn find_dock_station_by_native_id(
    state: &GameState,
    station_to_join: u16,
) -> Result<(usize, u16), CommandError> {
    let mut found = None;
    for (index, station) in state.stations.iter().enumerate() {
        if !station_can_join_dock(state, station)
            || super::station::dock_station_native_id(state, station) != Some(station_to_join)
        {
            continue;
        }
        if found.is_some() {
            return Err(CommandError::CannotJoinStations);
        }
        found = Some((index, station_to_join));
    }
    found.ok_or(CommandError::CannotJoinStations)
}

fn find_dock_joining_station(
    state: &GameState,
    c: TileCoord,
    dir: u8,
    station_to_join: Option<u16>,
) -> Result<Option<(usize, u16)>, CommandError> {
    let Some(station_to_join) = station_to_join else {
        return find_adjacent_dock_station(state, c, dir);
    };
    let (station_index, station_id) = find_dock_station_by_native_id(state, station_to_join)?;
    let water = crate::station::dock_water_tile(c, dir & 0x03);
    let existing_tiles =
        crate::station::dock_station_tiles(&state.map, &state.stations[station_index]);
    let adjacent = crate::station::station_tile_sets_adjacent(&[c, water], &existing_tiles);
    if !state.construction.distant_join_stations && !adjacent {
        return Err(CommandError::CannotJoinStations);
    }
    Ok(Some((station_index, station_id)))
}

pub(in crate::command) fn check_dock_placement_at_station_with_state(
    state: &GameState,
    c: TileCoord,
    dir: u8,
    station_to_join: u16,
) -> Result<(), CommandError> {
    check_dock_placement_with_state(state, c, dir)?;
    let _ = find_dock_joining_station(state, c, dir, Some(station_to_join))?;
    Ok(())
}

pub(in crate::command) fn place_dock_at_station(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    station_to_join: u16,
) -> Result<(), CommandError> {
    place_dock_with_station(state, c, dir, Some(station_to_join))
}

pub(in crate::command) fn place_dock(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    place_dock_with_station(state, c, dir, None)
}

fn place_dock_with_station(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    station_to_join: Option<u16>,
) -> Result<(), CommandError> {
    if let Some(station_id) = station_to_join {
        check_dock_placement_at_station_with_state(state, c, dir, station_id)?;
    } else {
        check_dock_placement_with_state(state, c, dir)?;
    }
    let dir = dir & 0x03;
    let water = crate::station::dock_water_tile(c, dir);
    let auto_clear_objects = auto_clear_object_plan(state, [c, water])?;
    for object_tiles in auto_clear_objects {
        clear_object_footprint_keep_water(state, object_tiles[0], &object_tiles)?;
    }
    let joining_station = find_dock_joining_station(state, c, dir, station_to_join)?;
    let station_id = joining_station.map_or_else(
        || next_station_id(state).ok_or(CommandError::StationPoolFull),
        |(_, id)| Ok(id),
    )?;
    let mut land_tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let mut water_tile = state.map.get(water).ok_or(CommandError::OutOfBounds)?;
    let water_class = water_class_from_m1(water_tile.m1);
    let station_owner = state.active_company.0 & 0x1F;

    let [station_id_low, station_id_high] = station_id.to_le_bytes();
    land_tile.kind = TileKind::Station;
    // `MakeStation` changes only MAPT's type nibble and clears the water
    // class/docking fields of the land part (`WaterClass::Invalid`).
    land_tile.mapt = 0x50 | (land_tile.mapt & 0x0F);
    land_tile.m1 = set_water_class_m1(station_owner, WaterClass::Invalid);
    land_tile.m2 = station_id_low;
    land_tile.m2_hi = station_id_high;
    land_tile.m3 = 0;
    land_tile.m3hi = 0;
    land_tile.m5 = dir;
    land_tile.m6 = apply_station_m6(land_tile.m6, StopKind::Dock);
    land_tile.m7 = 0;
    land_tile.m8 = 0;
    water_tile.kind = TileKind::Station;
    // The water part receives the original water class, but `MakeStation`
    // still clears its DockingTile bit before docking is recalculated.
    water_tile.mapt = 0x50 | (water_tile.mapt & 0x0F);
    water_tile.m1 = set_water_class_m1(station_owner, water_class);
    water_tile.m2 = station_id_low;
    water_tile.m2_hi = station_id_high;
    water_tile.m3 = 0;
    water_tile.m3hi = 0;
    water_tile.m5 = crate::station::DOCK_WATER_PART_GFX + (dir & 1);
    water_tile.m6 = apply_station_m6(water_tile.m6, StopKind::Dock);
    water_tile.m7 = 0;
    water_tile.m8 = 0;
    state
        .map
        .set_tile(c, land_tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_tile(water, water_tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    if let Some((station_index, _)) = joining_station {
        let station = &mut state.stations[station_index];
        station.ottd_station_id = Some(u32::from(station_id));
        for tile in [c, water] {
            if tile != station.pos && !station.joined_tiles.contains(&tile) {
                station.joined_tiles.push(tile);
            }
        }
    } else {
        let mut st = Station::new_with_kind(c, StopKind::Dock);
        st.owner = state.active_company;
        st.ottd_station_id = Some(u32::from(station_id));
        st.build_date =
            crate::station::STATION_BUILD_DATE_DEFAULT.saturating_add(state.calendar.date);
        st.joined_tiles.push(water);
        state.stations.push(st);
    }
    refresh_ship_docking_tiles_around(state, c);
    refresh_ship_docking_tiles_around(state, water);
    state.economy.money -= station_build_cost(&state.global_economy);
    Ok(())
}

/// Comprueba la demolición de cualquiera de las dos piezas de un muelle.
pub(in crate::command) fn check_clear_dock(
    state: &GameState,
    c: TileCoord,
) -> Result<[TileCoord; 2], CommandError> {
    let Some(tile) = state.map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Station
        || crate::station::stop_kind_from_m6(tile.m6) != StopKind::Dock
    {
        return Err(CommandError::StationNotFound);
    }
    let footprint = crate::station::dock_footprint_for_tile(&state.map, c).unwrap_or([c, c]);
    for tile in footprint {
        if !state.cheats.magic_bulldozer_active() {
            require_tile_owned_by_active(state, tile)?;
        }
        if state.vehicles.iter().any(|vehicle| vehicle.pos == tile) {
            return Err(CommandError::VehicleInTheWay);
        }
    }
    Ok(footprint)
}

/// Quita una huella naval de su estación lógica antes de modificar el mapa.
///
/// Una estación puede contener varios muelles. Si se demuele el muelle que
/// servía de ancla, se promueve otra pieza de tierra; de lo contrario sólo se
/// quitan las dos coordenadas demolidas. Las órdenes locales guardan
/// coordenadas, así que se redirigen a la nueva ancla como el `StationID`
/// estable del motor nativo.
fn remove_dock_footprint_from_station(state: &mut GameState, footprint: [TileCoord; 2]) {
    let Some(station_index) = state.stations.iter().position(|station| {
        station.stop_kind == StopKind::Dock
            && crate::station::dock_station_tiles(&state.map, station)
                .iter()
                .any(|tile| footprint.contains(tile))
    }) else {
        return;
    };
    let owned_tiles =
        crate::station::dock_station_tiles(&state.map, &state.stations[station_index]);
    let remaining_tiles: Vec<_> = owned_tiles
        .iter()
        .copied()
        .filter(|tile| !footprint.contains(tile))
        .collect();
    let old_pos = state.stations[station_index].pos;
    let new_pos = if footprint.contains(&old_pos) {
        remaining_tiles
            .iter()
            .copied()
            .find(|tile| crate::station::dock_land_tile(&state.map, *tile) == Some(*tile))
    } else {
        Some(old_pos)
    };

    let Some(new_pos) = new_pos else {
        state.stations.remove(station_index);
        return;
    };
    let station = &mut state.stations[station_index];
    station.pos = new_pos;
    station.joined_tiles = remaining_tiles
        .into_iter()
        .filter(|tile| *tile != new_pos)
        .collect();
    for vehicle in &mut state.vehicles {
        for order in &mut vehicle.orders {
            for &tile in &footprint {
                super::station::rewrite_order_station(order, tile, new_pos);
            }
        }
    }
    for list in &mut state.shared_order_lists {
        for order in &mut list.orders {
            for &tile in &footprint {
                super::station::rewrite_order_station(order, tile, new_pos);
            }
        }
    }
    for subsidy in &mut state.subsidies {
        if footprint.contains(&subsidy.dest_station_pos) {
            subsidy.dest_station_pos = new_pos;
        }
    }
}

/// Demuele un muelle completo, restaurando su agua y refrescando los marcadores
/// de amarre de los vecinos.
pub(in crate::command) fn clear_dock(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    let footprint = check_clear_dock(state, c)?;
    let [land, water] = footprint;
    let legacy_single_tile = land == water;
    remove_dock_footprint_from_station(state, footprint);
    if legacy_single_tile {
        let water_class = state
            .map
            .get(land)
            .map_or(WaterClass::Sea, |tile| water_class_from_m1(tile.m1));
        make_water_tile_after_native_clear(state, land, water_class)?;
    } else {
        let mut land_tile = state.map.get(land).ok_or(CommandError::OutOfBounds)?;
        land_tile.kind = TileKind::Grass;
        land_tile.mapt = 0;
        land_tile.m1 = 0;
        land_tile.m2 = 0;
        land_tile.m2_hi = 0;
        land_tile.m3 = 0;
        land_tile.m3hi = 0;
        land_tile.m5 = 0;
        land_tile.m6 = 0;
        land_tile.m7 = 0;
        land_tile.m8 = 0;
        state
            .map
            .set_tile(land, land_tile)
            .map_err(|_| CommandError::OutOfBounds)?;
        let water_class = state
            .map
            .get(water)
            .map_or(WaterClass::Sea, |tile| water_class_from_m1(tile.m1));
        make_water_tile_after_native_clear(state, water, water_class)?;
    }
    refresh_ship_docking_tiles_around(state, land);
    if !legacy_single_tile {
        refresh_ship_docking_tiles_around(state, water);
    }
    state.economy.money -= crate::CLEAR_TILE_COST;
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
    let random_bits = u8::try_from(state.random.next() & 0xFF).unwrap_or(0);
    make_water_tile_with_random_bits(&mut state.map, c, WaterClass::Canal, random_bits)
        .map_err(|_| CommandError::OutOfBounds)?;
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
    let random_bits = u8::try_from(state.random.next() & 0xFF).unwrap_or(0);
    make_water_tile_with_random_bits(&mut state.map, c, WaterClass::River, random_bits)
        .map_err(|_| CommandError::OutOfBounds)?;
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
    let station_id = next_station_id(state).ok_or(CommandError::StationPoolFull)?;
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let water_class = water_class_from_m1(tile.m1);
    let [station_id_low, station_id_high] = station_id.to_le_bytes();
    tile.kind = TileKind::Station;
    // `SetTileType(MP_STATION)` changes only the type nibble. This preserves
    // the bridge/tropical metadata in the low nibble of MAPT.
    tile.mapt = 0x50 | (tile.mapt & 0x0F);
    // `MakeBuoy` keeps the owner and water class of the underlying water but
    // always clears DockingTile. The low five bits are MAPO owner; bit 7 is a
    // separate docking flag.
    tile.m1 = set_water_class_m1(tile.m1 & !0x80, water_class);
    tile.m2 = station_id_low;
    tile.m2_hi = station_id_high;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m5 = 0;
    tile.m6 = apply_station_m6(tile.m6, StopKind::Buoy);
    tile.m7 = 0;
    tile.m8 = 0;
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    let mut st = Station::new_with_kind(c, StopKind::Buoy);
    // OpenTTD waypoints are neutral even though MakeBuoy stores the owner of
    // the underlying water in the tile so removal can restore that water.
    st.owner = crate::company::CompanyId::NONE;
    st.ottd_station_id = Some(u32::from(station_id));
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
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    if stop_kind == StopKind::Dock {
        check_dock_placement(map, stations, c, dir)
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

    #[test]
    fn place_canal_and_river_consume_random_for_native_map4() {
        let mut canal_state = GameState::new(7, 7);
        canal_state.random = crate::cargodist::parity::Randomizer {
            state: [0x1122_3344, 0x5566_7788],
        };
        let mut expected_canal_random = canal_state.random;
        let expected_canal_bits = expected_canal_random.next() as u8;

        place_canal(&mut canal_state, TileCoord::new(2, 2)).expect("place canal");

        assert_eq!(canal_state.random, expected_canal_random);
        assert_eq!(
            canal_state
                .map
                .get(TileCoord::new(2, 2))
                .expect("canal")
                .m3hi,
            expected_canal_bits
        );

        let mut river_state = GameState::new(7, 7);
        river_state.random = crate::cargodist::parity::Randomizer {
            state: [0x1122_3344, 0x5566_7788],
        };
        let mut expected_river_random = river_state.random;
        let expected_river_bits = expected_river_random.next() as u8;

        place_river(&mut river_state, TileCoord::new(2, 2)).expect("place river");

        assert_eq!(river_state.random, expected_river_random);
        assert_eq!(
            river_state
                .map
                .get(TileCoord::new(2, 2))
                .expect("river")
                .m3hi,
            expected_river_bits
        );
    }

    #[test]
    fn native_clear_promotes_elevated_sea_to_canal() {
        let mut state = GameState::new(8, 8);
        let water = TileCoord::new(3, 3);
        for corner in [
            water,
            TileCoord::new(water.x + 1, water.y),
            TileCoord::new(water.x, water.y + 1),
            TileCoord::new(water.x + 1, water.y + 1),
        ] {
            state.map.set_height(corner, 1).expect("elevated corner");
        }
        let mut tile = state.map.get(water).expect("water tile");
        tile.kind = TileKind::Station;
        tile.m1 = set_water_class_m1(tile.m1, WaterClass::Sea);
        state.map.set_tile(water, tile).expect("station water");
        state.random = crate::cargodist::parity::Randomizer {
            state: [0x1122_3344, 0x5566_7788],
        };
        let mut expected_random = state.random;
        let expected_bits = u8::try_from(expected_random.next() & 0xFF).unwrap_or(0);

        make_water_tile_after_native_clear(&mut state, water, WaterClass::Sea)
            .expect("restore elevated sea");

        let restored = state.map.get(water).expect("restored water");
        assert_eq!(restored.kind, TileKind::Water);
        assert_eq!(water_class_from_m1(restored.m1), WaterClass::Canal);
        assert_eq!(restored.m3hi, expected_bits);
        assert_eq!(state.random, expected_random);
    }

    #[test]
    fn native_clear_turns_sloped_canal_into_clear_land() {
        let mut state = GameState::new(8, 8);
        let water = TileCoord::new(3, 3);
        state.map.set_height(water, 1).expect("north corner");
        state
            .map
            .set_height(TileCoord::new(water.x + 1, water.y), 0)
            .expect("west corner");
        state
            .map
            .set_height(TileCoord::new(water.x, water.y + 1), 1)
            .expect("east corner");
        state
            .map
            .set_height(TileCoord::new(water.x + 1, water.y + 1), 0)
            .expect("south corner");
        assert_eq!(tile_slope_and_z(&state.map, water), Some((12, 0)));

        let mut tile = state.map.get(water).expect("water tile");
        tile.kind = TileKind::Station;
        tile.mapt = 0x0B;
        tile.m1 = set_water_class_m1(0x03, WaterClass::Canal);
        tile.m2 = 0xAA;
        tile.m2_hi = 0xBB;
        tile.m3 = 0xCC;
        tile.m3hi = 0xDD;
        tile.m5 = 0xEE;
        tile.m6 = 0xFF;
        tile.m7 = 0x12;
        tile.m8 = 0x3456;
        state.map.set_tile(water, tile).expect("sloped station");
        state.random = crate::cargodist::parity::Randomizer {
            state: [0x1122_3344, 0x5566_7788],
        };
        let expected_random = state.random;

        make_water_tile_after_native_clear(&mut state, water, WaterClass::Canal)
            .expect("clear sloped canal");

        let cleared = state.map.get(water).expect("clear tile");
        assert_eq!(cleared.kind, TileKind::Grass);
        assert_eq!(cleared.mapt, 0x0B);
        assert_eq!(cleared.m1, crate::company::OWNER_NONE_M1);
        assert_eq!(cleared.m2, 0);
        assert_eq!(cleared.m2_hi, 0);
        assert_eq!(cleared.m3, 0);
        assert_eq!(cleared.m3hi, 0);
        assert_eq!(cleared.m5, 0);
        assert_eq!(cleared.m6, 0);
        assert_eq!(cleared.m7, 0);
        assert_eq!(cleared.m8, 0);
        assert_eq!(state.random, expected_random);
    }
}
