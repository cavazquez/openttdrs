//! Construcción acuática: depósito, muelle, canal, boya, acueducto y esclusa.

use crate::bridge_spec::{
    BridgeType, axis_line, bridge_above_axis_from_mapt, bridge_build_cost_in,
    bridge_height_over_tile, set_bridge_middle_mapt, set_bridge_type_m6,
};
use crate::economy::{
    airport_clear_cost, buoy_build_cost, buoy_clear_cost, canal_build_cost, canal_clear_cost,
    dock_build_cost, dock_clear_cost, fields_clear_cost, grass_clear_cost, lock_build_cost,
    lock_clear_cost, rail_clear_cost, rail_station_clear_cost, rail_waypoint_clear_cost,
    road_depot_clear_cost, road_stop_clear_cost_factored, rocks_clear_cost, rough_clear_cost,
    ship_depot_build_cost, ship_depot_clear_cost, signal_clear_cost, station_build_cost,
    train_depot_clear_cost, trees_clear_cost, water_clear_cost,
};
use crate::map::rail_bits::RAIL_TILE_NORMAL;
use crate::map::tree_tile_loop::{clear_density, clear_ground_type, tree_count};
use crate::map::{
    Map, Tile, TileCoord, TileKind, WaterClass, clear_neighbour_non_flooding_states,
    clear_tile_after_native_water_restore, has_tile_water_ground, inclined_slope_direction,
    is_map_object_tile, is_tunnel_entrance_slope, make_water_tile_with_random_bits,
    object_footprint_tiles, object_id_from_tile, object_origin_from_tile, object_type_dims_id,
    opposite_diag_dir, set_water_class_m1, tile_slope_and_z, water_class_after_native_clear,
    water_class_from_m1,
};
use crate::world_gen::{CLEAR_GROUND_FIELDS, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY};
use crate::{GameState, Station, StopKind};

use super::super::{CommandError, require_tile_owned_by_active};
use super::shared::{
    buoy_in_use_by_other_company, check_in_bounds, check_object_can_be_auto_cleared,
    check_object_can_be_cleared, clear_object_footprint_keep_water,
    clear_object_footprint_keep_water_without_charge, object_clear_money_delta, register_depot,
    road_stop_clear_cost_for_tile, unregister_depot,
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

fn object_clear_footprint(state: &GameState, c: TileCoord) -> Result<Vec<TileCoord>, CommandError> {
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
    object_clear_plan(state, tiles, true)
}

/// Planifica los objetos que `DoBuildLock` retira con `CMD_LANDSCAPE_CLEAR`.
///
/// A diferencia de depósitos, muelles y boyas, `DoBuildLock` no añade
/// `DoCommandFlag::Auto` al clear de sus tres partes. Por eso un objeto sin
/// `Autoremove` sigue siendo válido si la demolición manual lo permite; sólo
/// deben bloquearse los objetos `CannotRemove` o de otra compañía.
fn lock_clear_object_plan(
    state: &GameState,
    tiles: impl IntoIterator<Item = TileCoord>,
) -> Result<Vec<Vec<TileCoord>>, CommandError> {
    object_clear_plan(state, tiles, false)
}

fn object_clear_plan(
    state: &GameState,
    tiles: impl IntoIterator<Item = TileCoord>,
    automatic: bool,
) -> Result<Vec<Vec<TileCoord>>, CommandError> {
    let mut plan = Vec::new();
    for tile in tiles {
        let Some(raw) = state.map.get(tile) else {
            return Err(CommandError::OutOfBounds);
        };
        if !is_map_object_tile(raw.mapt) {
            continue;
        }
        if automatic {
            check_object_can_be_auto_cleared(state, tile)?;
        } else {
            check_object_can_be_cleared(state, tile)?;
        }
        let object_tiles = object_clear_footprint(state, tile)?;
        if !plan.contains(&object_tiles) {
            plan.push(object_tiles);
        }
    }
    Ok(plan)
}

/// Coste de una limpieza automática de objeto, convertido al signo de coste
/// de un comando compuesto (`CommandCost` positivo = dinero que se resta).
fn auto_clear_object_cost(state: &GameState, object_tiles: &[TileCoord]) -> i64 {
    let Some(&origin) = object_tiles.first() else {
        return crate::CLEAR_TILE_COST;
    };
    let Some(object_type) = state.map.object_type_at(origin) else {
        return crate::CLEAR_TILE_COST;
    };
    let tile_count = u32::try_from(object_tiles.len()).unwrap_or(u32::MAX);
    object_clear_money_delta(state, object_type, tile_count)
        .map_or(crate::CLEAR_TILE_COST, |delta| 0_i64.saturating_sub(delta))
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
    // `MakeWaterKeepingClass` empieza por `DoClearSquare`, que reactiva el
    // tile loop de agua de las ocho vecinas antes de materializar la nueva
    // superficie. Esto también aplica al agua que reaparece al quitar un
    // depósito, muelle, boya u objeto construido sobre agua.
    clear_neighbour_non_flooding_states(&mut state.map, c);
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
const WATER_TILE_TYPE_LOCK: u8 = 2;

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

/// Traduce `IsInsideMM(tile, 1, Map::Max() - 1)` cuando los bordes libres
/// están desactivados. El mapa local incluye el marco exterior en sus
/// dimensiones, igual que `Map::SizeX/Y()` en el motor nativo.
fn check_non_freeform_edge(
    map: &Map,
    c: TileCoord,
    freeform_edges: bool,
) -> Result<(), CommandError> {
    if freeform_edges {
        return Ok(());
    }
    let (width, height) = map.dimensions();
    let inside_x = u32::try_from(c.x).is_ok_and(|x| x >= 1 && x < width.saturating_sub(2));
    let inside_y = u32::try_from(c.y).is_ok_and(|y| y >= 1 && y < height.saturating_sub(2));
    if inside_x && inside_y {
        Ok(())
    } else {
        Err(CommandError::TooCloseToMapEdge)
    }
}

/// Validación de `ClearTile_Water` para agua plana, costa o esclusa.
pub(crate) fn check_clear_water(state: &GameState, c: TileCoord) -> Result<(), CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if tile.kind != TileKind::Water {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    match water_tile_type(tile) {
        WATER_TILE_TYPE_CLEAR => {
            check_non_freeform_edge(&state.map, c, state.construction.freeform_edges)?;
            check_clear_water_owner(state, tile)?;
        }
        WATER_TILE_TYPE_COAST => {}
        WATER_TILE_TYPE_LOCK => return check_clear_lock(state, c),
        // Unknown water structures must not be treated as plain water: doing
        // so would clear only one part of a multi-tile structure.
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
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if water_tile_type(tile) == WATER_TILE_TYPE_LOCK {
        return clear_lock(state, c);
    }
    check_clear_water(state, c)?;
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
    // `CmdBuildShipDepot` reserva la entrada del pool antes de invocar
    // `ClearTile(..., Auto)`. Mantener este orden evita que un mapa agotado
    // informe un bloqueo de la huella que el comando nativo todavía no llega
    // a inspeccionar.
    if crate::depot::next_free_depot_id(&state.map).is_none() {
        return Err(CommandError::DepotPoolFull);
    }
    // Al limpiar `WaterTileType::Clear`, `ClearTile_Water` conserva primero
    // el guard de bordes, luego la ocupación y finalmente la propiedad del
    // agua. Los canales de otra compañía no se pueden reemplazar con un
    // depósito; el mar y los ríos normalmente llevan OWNER_WATER.
    for tile in [origin, other] {
        let Some(raw) = state.map.get(tile) else {
            return Err(CommandError::OutOfBounds);
        };
        if raw.kind == TileKind::Water && water_tile_type(raw) == WATER_TILE_TYPE_CLEAR {
            check_non_freeform_edge(&state.map, tile, state.construction.freeform_edges)?;
            if state.vehicles.iter().any(|vehicle| vehicle.pos == tile) {
                return Err(CommandError::VehicleInTheWay);
            }
            check_clear_water_owner(state, raw)?;
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
fn make_ship_depot_tile(
    original: Tile,
    owner: u8,
    water_class: WaterClass,
    dir: u8,
    depot_id: u16,
) -> Tile {
    let mut tile = original;
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60 | (original.mapt & 0x0F);
    tile.m1 = set_water_class_m1(owner & 0x1F, water_class);
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
    // `CmdBuildShipDepot` captura `GetWaterClass` antes de ejecutar ambas
    // limpiezas. No se debe releer `m1` después: `MakeWaterKeepingClass`
    // puede convertir un mar elevado en canal o descartar una clase inclinada.
    let original_water_class = state
        .map
        .get(c)
        .map_or(WaterClass::Sea, |raw| water_class_from_m1(raw.m1));
    let other_water_class = state
        .map
        .get(other)
        .map_or(WaterClass::Sea, |raw| water_class_from_m1(raw.m1));
    let auto_clear_objects = auto_clear_object_plan(state, [c, other])?;
    for object_tiles in auto_clear_objects {
        clear_object_footprint_keep_water(state, object_tiles[0], &object_tiles)?;
    }
    // Para una superficie `MP_WATER` simple, `ClearTile_Water` ejecutado con
    // `Auto` llama a `DoClearSquare` antes de que el depósito la reemplace.
    // Las estructuras ya fueron rechazadas arriba y los objetos se encargan
    // de este mismo efecto dentro de `MakeWaterKeepingClass`.
    for tile in [c, other] {
        if state
            .map
            .get(tile)
            .is_some_and(|raw| raw.kind == TileKind::Water)
        {
            clear_neighbour_non_flooding_states(&mut state.map, tile);
        }
    }
    let original = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let other_original = state.map.get(other).ok_or(CommandError::OutOfBounds)?;
    let owner = state.active_company.0;
    let depot_tile = make_ship_depot_tile(original, owner, original_water_class, dir, depot_id);
    let other_tile = make_ship_depot_tile(
        other_original,
        owner,
        other_water_class,
        dir.wrapping_add(2),
        depot_id,
    );
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
    clear_ship_depot_impl(state, c, true)
}

/// Retira un depósito naval sin aplicar el coste cuando forma parte de un
/// comando compuesto, como `DoBuildLock`.
fn clear_ship_depot_impl(
    state: &mut GameState,
    c: TileCoord,
    charge_money: bool,
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
    if charge_money {
        state.economy.money -= ship_depot_clear_cost(&state.global_economy);
    }
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
    state.economy.money -= dock_build_cost(&state.global_economy);
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
    clear_dock_impl(state, c, true)
}

/// Variante de `RemoveDock` para comandos compuestos que liquidan el coste
/// total al final, como `DoBuildLock`.
fn clear_dock_impl(
    state: &mut GameState,
    c: TileCoord,
    charge_money: bool,
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
    if charge_money {
        state.economy.money -= dock_clear_cost(&state.global_economy);
    }
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
    state.economy.money -= buoy_build_cost(&state.global_economy);
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
            // `MakeAqueductBridgeRamp` usa el owner de la compañía en MAPO y
            // borra el estado de atraque. Los cinco bits bajos son el owner;
            // el resto de M1 conserva flags que no pertenecen a la propiedad.
            tile = crate::company::tile_with_owner(tile, state.active_company);
            tile.m1 &= !0x80;
            tile.m2 = 0;
            tile.m2_hi = 0;
            tile.m3 = 0;
            tile.m3hi = 0;
            tile.m5 = aqueduct_ramp_m5(dir);
            tile.m6 = set_bridge_type_m6(tile.m6, bridge_type);
            tile.m7 = 0;
            tile.m8 = 0;
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

/// Dirección diagonal que apunta desde el centro hacia el extremo alto.
///
/// `PlaceLock` conserva sólo el eje en su API local, así que la orientación
/// completa de `m5` se deriva de las alturas de los dos extremos, como la
/// dirección que `CmdBuildLock` obtiene de `GetInclinedSlopeDirection`.
#[must_use]
fn lock_direction_for_heights(axis_y: bool, first_height: u8, second_height: u8) -> u8 {
    let first_direction = if axis_y { 3 } else { 0 };
    let second_direction = if axis_y { 1 } else { 2 };
    if first_height < second_height {
        second_direction
    } else {
        first_direction
    }
}

/// Devuelve `[middle, lower, upper]` según la orientación nativa de la
/// esclusa. La dirección almacenada apunta siempre del centro al extremo alto.
#[must_use]
fn lock_tiles_from_middle(middle: TileCoord, direction: u8) -> [TileCoord; 3] {
    let (dx, dy) = crate::map::diag_dir_offset(direction);
    let lower = TileCoord::new(middle.x - dx, middle.y - dy);
    let upper = TileCoord::new(middle.x + dx, middle.y + dy);
    [middle, lower, upper]
}

/// Comprueba el despeje mínimo nativo para un puente sobre cada parte de la
/// esclusa (`GetLockPartMinimalBridgeHeight`).
fn check_lock_bridge_clearance(
    state: &GameState,
    tiles: [TileCoord; 3],
) -> Result<(), CommandError> {
    const MINIMAL_BRIDGE_HEIGHT: [u8; 3] = [2, 3, 2];
    for (tile, minimal_height) in tiles.into_iter().zip(MINIMAL_BRIDGE_HEIGHT) {
        let Some(bridge_height) = bridge_height_over_tile(&state.map, tile) else {
            continue;
        };
        let (slope, z) = tile_slope_and_z(&state.map, tile).ok_or(CommandError::OutOfBounds)?;
        let tile_max_z = z.saturating_add(if slope == 0 {
            0
        } else if slope & crate::SLOPE_STEEP != 0 {
            2
        } else {
            1
        });
        if bridge_height < tile_max_z.saturating_add(minimal_height) {
            return Err(CommandError::BridgeTooLowForLock);
        }
    }
    Ok(())
}

/// Resuelve el centro de una esclusa a partir de cualquier parte de `MP_WATER`.
/// También verifica que las tres partes pertenezcan a la misma estructura;
/// evita dejar una esclusa huérfana al limpiar un tile importado corrupto.
fn lock_geometry(state: &GameState, c: TileCoord) -> Result<([TileCoord; 3], u8), CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if tile.kind != TileKind::Water || water_tile_type(tile) != WATER_TILE_TYPE_LOCK {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let direction = tile.m5 & 0x03;
    let part = (tile.m5 >> 2) & 0x03;
    let (dx, dy) = crate::map::diag_dir_offset(direction);
    let middle = match part {
        0 => c,
        1 => TileCoord::new(c.x + dx, c.y + dy),
        2 => TileCoord::new(c.x - dx, c.y - dy),
        _ => return Err(CommandError::BuildingMustBeDemolished),
    };
    let tiles = lock_tiles_from_middle(middle, direction);
    for (coord, expected_part) in tiles.into_iter().zip([0_u8, 1, 2]) {
        let Some(raw) = state.map.get(coord) else {
            return Err(CommandError::OutOfBounds);
        };
        if raw.kind != TileKind::Water
            || water_tile_type(raw) != WATER_TILE_TYPE_LOCK
            || ((raw.m5 >> 2) & 0x03) != expected_part
            || (raw.m5 & 0x03) != direction
        {
            return Err(CommandError::BuildingMustBeDemolished);
        }
    }
    Ok((tiles, direction))
}

/// Comprueba la propiedad y ocupación de las tres partes de una esclusa.
fn check_clear_lock(state: &GameState, c: TileCoord) -> Result<(), CommandError> {
    let (tiles, _) = lock_geometry(state, c)?;
    let middle = state.map.get(tiles[0]).ok_or(CommandError::OutOfBounds)?;
    let owner = middle.m1 & 0x1F;
    let owner_none = crate::company::OWNER_NONE_M1 & 0x1F;
    if !state.cheats.magic_bulldozer_active()
        && owner != owner_none
        && owner != (state.active_company.0 & 0x1F)
    {
        return Err(CommandError::TileNotOwned);
    }
    if tiles
        .iter()
        .any(|tile| state.vehicles.iter().any(|vehicle| vehicle.pos == *tile))
    {
        return Err(CommandError::VehicleInTheWay);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct LockBuildTilePlan {
    /// Clase que `MakeLockTile` debe conservar en esta sección.
    water_class: WaterClass,
    /// `MakeLock` conserva el owner de un extremo que ya era agua; para tierra
    /// usa la compañía que construye la esclusa después de limpiarla.
    owner: u8,
    /// `DoBuildLock` llama a `CMD_LANDSCAPE_CLEAR` sobre el centro siempre y
    /// sobre un extremo sólo cuando no era `MP_WATER`.
    clear_on_build: bool,
    clear_cost: i64,
    add_canal_cost: bool,
}

fn road_type_cost_multiplier(state: &GameState, road_type: crate::road_type::RoadType) -> u16 {
    state
        .road_type_catalog
        .iter()
        .find(|definition| definition.id == road_type)
        .map_or(0, |definition| definition.cost_multiplier)
}

/// Prepara una carretera normal para `CMD_LANDSCAPE_CLEAR` sin `Auto`.
///
/// La rama manual de `ClearTile_Road` retira todas las piezas de carretera y
/// tranvía de una tesela normal. Cada pieza conserva el precio de su tipo; el
/// tranvía puede devolver dinero según su factor de construcción. Un cruce,
/// depósito, túnel o puente queda fuera de esta subetapa.
fn check_lock_road_tile(
    state: &GameState,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    let road_subtype = (tile.m5 >> 6) & 0x03;
    let road_bits = tile.m5 & 0x0F;
    let tram_bits = crate::road_type::tram_track_bits(&tile);
    if road_subtype != 0 || (road_bits == 0 && tram_bits == 0) {
        return Err(CommandError::MustRemoveRoadFirst);
    }

    // `CheckAllowRemoveRoad` permite carretera municipal y carretera neutral;
    // una red de otra compañía sigue bloqueando la limpieza manual compuesta.
    let owner = tile.m1 & 0x1F;
    let active = state.active_company.0 & 0x1F;
    if owner != active
        && owner != (crate::company::OWNER_TOWN_M1 & 0x1F)
        && owner != (crate::company::OWNER_NONE_M1 & 0x1F)
    {
        return Err(CommandError::TileNotOwned);
    }

    let road_cost = if road_bits == 0 {
        0
    } else {
        let road_type = crate::road_type::road_type_from_tile(&tile);
        crate::economy::road_clear_cost_factored(
            &state.global_economy,
            false,
            road_type_cost_multiplier(state, road_type),
        )
        .saturating_mul(i64::from(road_bits.count_ones()))
    };
    let tram_cost = if tram_bits == 0 {
        0
    } else {
        let tram_type = crate::road_type::tram_road_type_from_tile(&tile)
            .unwrap_or(crate::road_type::RoadType::TRAM);
        crate::economy::road_clear_cost_factored(
            &state.global_economy,
            true,
            road_type_cost_multiplier(state, tram_type),
        )
        .saturating_mul(i64::from(tram_bits.count_ones()))
    };

    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: road_cost.saturating_add(tram_cost),
        add_canal_cost: !is_middle,
    })
}

fn check_lock_road_crossing_owner(state: &GameState, owner: u8) -> Result<(), CommandError> {
    let owner = owner & 0x1F;
    let active = state.active_company.0 & 0x1F;
    if owner != active
        && owner != (crate::company::OWNER_TOWN_M1 & 0x1F)
        && owner != (crate::company::OWNER_NONE_M1 & 0x1F)
    {
        return Err(CommandError::TileNotOwned);
    }
    Ok(())
}

/// Prepara un cruce a nivel para `ClearTile_Road` sin `Auto`.
fn check_lock_road_crossing_tile(
    state: &GameState,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    let road_type = crate::road_type::road_type_from_tile(&tile);
    let road_present = road_type.as_u8() != 0x3F;
    let tram_type = crate::road_type::tram_road_type_from_tile(&tile);
    if !road_present && tram_type.is_none() {
        return Err(CommandError::MustRemoveRoadFirst);
    }
    if road_present {
        check_lock_road_crossing_owner(state, tile.m7)?;
    }
    if tram_type.is_some() {
        check_lock_road_crossing_owner(state, tile.m3 >> 4)?;
    }
    let road_cost = if road_present {
        crate::economy::road_clear_cost_factored(
            &state.global_economy,
            false,
            road_type_cost_multiplier(state, road_type),
        )
        .saturating_mul(2)
    } else {
        0
    };
    let tram_cost = tram_type.map_or(0, |tram_type| {
        crate::economy::road_clear_cost_factored(
            &state.global_economy,
            true,
            road_type_cost_multiplier(state, tram_type),
        )
        .saturating_mul(2)
    });
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: road_cost.saturating_add(tram_cost),
        add_canal_cost: !is_middle,
    })
}

/// Prepara el despeje manual de un depósito de carretera.
fn check_lock_road_depot_tile(
    state: &GameState,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    let owner = tile.m1 & 0x1F;
    if owner != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: road_depot_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

/// Prepara el despeje manual de un depósito ferroviario.
fn check_lock_rail_depot_tile(
    state: &GameState,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    let owner = tile.m1 & 0x1F;
    if owner != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: train_depot_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

/// Prepara la retirada manual de una parada de bus o camión.
///
/// `ClearTile_Station` delega en `RemoveRoadStop` cuando el clear no lleva
/// `Auto`. El coste vanilla usa la categoría de la parada; una parada
/// `NewGRF` reemplaza ese precio con su multiplicador de limpieza por tesela.
fn check_lock_road_stop_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    let stop_kind = crate::station::stop_kind_from_m6(tile.m6);
    if !matches!(stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let station = state
        .stations
        .iter()
        .find(|station| station.covers_tile(c) && station.stop_kind == stop_kind);
    let owner = station.map_or(tile.m1 & 0x1F, |station| station.owner.0 & 0x1F);
    if owner != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    let clear_cost = road_stop_clear_cost_for_tile(state, c)
        .unwrap_or_else(|| road_stop_clear_cost_factored(&state.global_economy, stop_kind, 16));
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost,
        add_canal_cost: !is_middle,
    })
}

/// Retira una tesela de parada vial de la entidad `Station` local.
///
/// `RemoveRoadStop` conserva la estación mientras queden otras paradas del
/// mismo tipo y mantiene la identidad de órdenes. El modelo local representa
/// esa lista como `pos + joined_tiles`, así que al demoler el ancla promueve
/// la primera tesela restante y redirige las órdenes que usaban la coordenada
/// retirada.
fn remove_lock_road_stop_state(state: &mut GameState, c: TileCoord) {
    state.newgrf_animated_station_tiles.remove(&c);
    let Some(station_index) = state.stations.iter().position(|station| {
        station.covers_tile(c)
            && matches!(station.stop_kind, StopKind::BusStop | StopKind::TruckStop)
    }) else {
        return;
    };
    let old_pos = state.stations[station_index].pos;
    let mut remaining_tiles = state.stations[station_index].joined_tiles.clone();
    remaining_tiles.retain(|tile| *tile != c);
    let new_pos = if old_pos == c {
        remaining_tiles.first().copied()
    } else {
        Some(old_pos)
    };
    let Some(new_pos) = new_pos else {
        state.stations.remove(station_index);
        return;
    };
    remaining_tiles.retain(|tile| *tile != new_pos);
    let had_tile_states = !state.stations[station_index]
        .road_stop_tile_states
        .is_empty();
    let station = &mut state.stations[station_index];
    station.pos = new_pos;
    station.joined_tiles = remaining_tiles;
    station.road_stop_tile_states.retain(|(tile, _)| *tile != c);
    if had_tile_states {
        station.normalize_road_stop_tile_states();
    } else {
        station.sync_legacy_road_stop_anchor();
    }
    for vehicle in &mut state.vehicles {
        for order in &mut vehicle.orders {
            super::station::rewrite_order_station(order, c, new_pos);
        }
    }
    for list in &mut state.shared_order_lists {
        for order in &mut list.orders {
            super::station::rewrite_order_station(order, c, new_pos);
        }
    }
    for subsidy in &mut state.subsidies {
        if subsidy.dest_station_pos == c {
            subsidy.dest_station_pos = new_pos;
        }
    }
}

/// Prepara la retirada manual de un waypoint vial.
///
/// `RemoveRoadWaypointStop` comparte la categoría de limpieza de camiones,
/// pero no es una parada de carga y por eso conserva una rama separada del
/// estado `Station` de bus/camión.
fn check_lock_road_waypoint_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    if crate::station::stop_kind_from_m6(tile.m6) != StopKind::RoadWaypoint {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let station = state
        .stations
        .iter()
        .find(|station| station.pos == c && station.stop_kind == StopKind::RoadWaypoint);
    let owner = station.map_or(tile.m1 & 0x1F, |station| station.owner.0 & 0x1F);
    if owner != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: rail_waypoint_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

fn remove_lock_road_waypoint_state(state: &mut GameState, c: TileCoord) {
    state.newgrf_animated_station_tiles.remove(&c);
    state
        .stations
        .retain(|station| !(station.pos == c && station.stop_kind == StopKind::RoadWaypoint));
}

/// Prepara la retirada manual de un waypoint ferroviario.
///
/// `ClearTile_Station` selecciona `RemoveRailWaypoint` para este tipo de
/// tesela. En el runtime actual el waypoint ocupa una sola tesela, por lo que
/// el coste y la limpieza de la entidad son ambos unitarios.
fn check_lock_rail_waypoint_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    if crate::station::stop_kind_from_m6(tile.m6) != StopKind::RailWaypoint {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let station = state
        .stations
        .iter()
        .find(|station| station.pos == c && station.stop_kind == StopKind::RailWaypoint);
    let owner = station.map_or(tile.m1 & 0x1F, |station| station.owner.0 & 0x1F);
    if owner != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: rail_waypoint_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

fn remove_lock_rail_waypoint_state(state: &mut GameState, c: TileCoord) {
    state.newgrf_animated_station_tiles.remove(&c);
    state
        .stations
        .retain(|station| !(station.pos == c && station.stop_kind == StopKind::RailWaypoint));
}

/// Devuelve el ancla y la huella rail que `RemoveRailStation` debe retirar.
///
/// La asignación usa la misma geometría que las reservas y las estaciones
/// unidas: una tesela puede pertenecer a una estación distinta aunque el
/// flood-fill físico de plataformas sea contiguo.
fn rail_station_clear_info(
    state: &GameState,
    c: TileCoord,
) -> Result<(TileCoord, Vec<TileCoord>), CommandError> {
    state
        .stations
        .iter()
        .filter(|station| station.stop_kind == StopKind::RailStation)
        .find_map(|station| {
            let owned_tiles =
                crate::station::rail_station_owned_tiles(&state.map, &state.stations, station);
            owned_tiles
                .contains(&c)
                .then_some((station.pos, owned_tiles))
        })
        .ok_or(CommandError::BuildingMustBeDemolished)
}

/// Prepara la retirada manual de una estación ferroviaria completa.
///
/// `RemoveRailStation` cobra `PR_CLEAR_STATION_RAIL` por cada tesela de la
/// huella, valida vehículos sobre toda el área y elimina una sola entidad
/// lógica. Las estaciones intermodales siguen esperando su propia reducción:
/// borrar aquí sólo la parte ferroviaria dejaría instalaciones incompatibles.
fn check_lock_rail_station_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    if crate::station::stop_kind_from_m6(tile.m6) != StopKind::RailStation {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let (anchor, owned_tiles) = rail_station_clear_info(state, c)?;
    let station = state
        .stations
        .iter()
        .find(|station| station.pos == anchor && station.stop_kind == StopKind::RailStation)
        .ok_or(CommandError::BuildingMustBeDemolished)?;
    if station.effective_facilities() != 0x01 {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    if owned_tiles
        .iter()
        .any(|&tile| state.vehicles.iter().any(|vehicle| vehicle.pos == tile))
    {
        return Err(CommandError::VehicleInTheWay);
    }
    if (station.owner.0 & 0x1F) != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    let footprint_count = i64::try_from(owned_tiles.len()).unwrap_or(i64::MAX);
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: rail_station_clear_cost(&state.global_economy).saturating_mul(footprint_count),
        add_canal_cost: !is_middle,
    })
}

fn remove_lock_rail_station_state(
    state: &mut GameState,
    anchor: TileCoord,
    owned_tiles: &[TileCoord],
) {
    for &tile in owned_tiles {
        state.newgrf_animated_station_tiles.remove(&tile);
    }
    state
        .stations
        .retain(|station| !(station.pos == anchor && station.stop_kind == StopKind::RailStation));
}

/// Ejecuta la parte de `RemoveRailStation` que ocurre antes de `MakeLock`.
fn clear_lock_rail_station(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    let (anchor, owned_tiles) = rail_station_clear_info(state, c)?;
    for &tile in &owned_tiles {
        clear_tile_after_native_water_restore(&mut state.map, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
        clear_neighbour_non_flooding_states(&mut state.map, tile);
        super::rail::refresh_rail_neighbors(state, tile)?;
        crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, tile);
    }
    remove_lock_rail_station_state(state, anchor, &owned_tiles);
    Ok(())
}

/// Prepara la retirada de un depósito naval durante `DoBuildLock`.
///
/// `ClearTile_Water` considera el depósito como una estructura, no como agua
/// plana: una llamada sin `Auto` entra en `RemoveShipDepot`, que restaura sus
/// dos partes y devuelve un único coste de demolición.
fn check_lock_ship_depot_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    check_clear_ship_depot(state, c)?;
    let original_water_class = if has_tile_water_ground(tile) {
        water_class_from_m1(tile.m1)
    } else {
        WaterClass::Canal
    };
    // El centro conserva la clase capturada antes de `RemoveShipDepot`.
    // Para un extremo, `MakeLock` consulta la clase de la superficie ya
    // restaurada; resolverla aquí mantiene preview y ejecución idénticos.
    let water_class = if is_middle {
        original_water_class
    } else {
        match water_class_after_native_clear(&state.map, c, original_water_class) {
            WaterClass::Invalid => WaterClass::Canal,
            water_class => water_class,
        }
    };
    let owner = if is_middle {
        state.active_company.0
    } else {
        match water_class {
            WaterClass::Sea | WaterClass::River => crate::company::OWNER_WATER_M1 & 0x1F,
            WaterClass::Canal => tile.m1 & 0x1F,
            WaterClass::Invalid => state.active_company.0,
        }
    };
    Ok(LockBuildTilePlan {
        water_class,
        owner,
        clear_on_build: true,
        clear_cost: ship_depot_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

/// Ejecuta `RemoveShipDepot` como parte de `DoBuildLock`, sin doble cobro.
fn clear_lock_ship_depot(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    clear_ship_depot_impl(state, c, false)
}

/// Devuelve el ancla y la huella que `RemoveAirport` debe retirar.
fn airport_clear_info(
    state: &GameState,
    c: TileCoord,
) -> Result<(TileCoord, Vec<TileCoord>), CommandError> {
    state
        .stations
        .iter()
        .filter(|station| station.has_airport_facility())
        .find_map(|station| {
            let tiles = if station.airport_tiles.is_empty() {
                vec![station.pos]
            } else {
                station.airport_tiles.clone()
            };
            (tiles.contains(&c)
                && tiles
                    .iter()
                    .all(|&tile| state.map.get_kind(tile) == Some(TileKind::Airport)))
            .then_some((station.pos, tiles))
        })
        .ok_or(CommandError::BuildingMustBeDemolished)
}

/// Comprueba la retirada de un aeropuerto puro antes de `MakeLock`.
fn check_lock_airport_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
) -> Result<LockBuildTilePlan, CommandError> {
    let (anchor, tiles) = airport_clear_info(state, c)?;
    let station = state
        .stations
        .iter()
        .find(|station| station.pos == anchor && station.has_airport_facility())
        .ok_or(CommandError::BuildingMustBeDemolished)?;
    if station.effective_facilities() & !0x08 != 0 {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    if (station.owner.0 & 0x1F) != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    let aircraft_in_way = state.vehicles.iter().any(|vehicle| {
        let on_airport = tiles.contains(&vehicle.pos)
            && (vehicle.kind != crate::vehicle::VehicleKind::Aircraft
                || vehicle.aircraft_phase != crate::vehicle::AircraftPhase::Flying);
        let in_fta = vehicle.kind == crate::vehicle::VehicleKind::Aircraft
            && vehicle.airport_fta_station == Some(anchor)
            && vehicle.aircraft_phase != crate::vehicle::AircraftPhase::Flying;
        on_airport || in_fta
    });
    if aircraft_in_way {
        return Err(CommandError::VehicleInTheWay);
    }
    let footprint_count = i64::try_from(tiles.len()).unwrap_or(i64::MAX);
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: airport_clear_cost(&state.global_economy).saturating_mul(footprint_count),
        add_canal_cost: !is_middle,
    })
}

/// Ruido que `RemoveAirport` debe descontar del pueblo más cercano.
fn airport_noise_for_station(state: &GameState, station: &Station) -> Option<(usize, u8)> {
    let (town_idx, distance) = crate::town::nearest_town_index(&state.towns, station.pos)?;
    let noise_level = station
        .airport_newgrf_spec_id
        .and_then(|id| {
            crate::airport_class::newgrf_airport_spec_def(&state.airport_spec_catalog, id)
        })
        .map(|def| def.noise_level)
        .or_else(|| {
            crate::airport_class::airport_spec_def(station.airport_spec).map(|def| def.noise_level)
        })
        .unwrap_or(0);
    Some((
        town_idx,
        crate::airport_class::airport_noise_for_distance(noise_level, distance, 8),
    ))
}

/// Ejecuta `RemoveAirport` sobre toda la huella antes de `MakeLock`.
fn clear_lock_airport(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    let (anchor, tiles) = airport_clear_info(state, c)?;
    let station = state
        .stations
        .iter()
        .find(|station| station.pos == anchor && station.has_airport_facility())
        .ok_or(CommandError::BuildingMustBeDemolished)?;
    if station.effective_facilities() & !0x08 != 0 {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let noise = airport_noise_for_station(state, station);
    for &tile in &tiles {
        clear_tile_after_native_water_restore(&mut state.map, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
        clear_neighbour_non_flooding_states(&mut state.map, tile);
        state.newgrf_animated_airport_tiles.remove(&tile);
    }
    if let Some((town_idx, amount)) = noise {
        state.towns[town_idx].noise_reached = state.towns[town_idx]
            .noise_reached
            .saturating_sub(u16::from(amount));
    }
    for vehicle in &mut state.vehicles {
        if vehicle.airport_fta_station == Some(anchor) {
            vehicle.airport_fta_station = None;
            vehicle.airport_blocks_held = 0;
            vehicle.airport_fta_active = false;
        }
    }
    state
        .stations
        .retain(|station| !(station.pos == anchor && station.has_airport_facility()));
    Ok(())
}

/// Prepara la retirada manual de una boya durante `DoBuildLock`.
///
/// `RemoveBuoy` conserva la clase del agua subyacente sólo para la parte
/// central; un extremo que deja de ser una boya se materializa como canal por
/// `MakeLock`. La comprobación de uso sigue la guardia nativa de waypoint.
fn check_lock_buoy_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    if crate::station::stop_kind_from_m6(tile.m6) != StopKind::Buoy {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    if buoy_in_use_by_other_company(state, c) {
        return Err(CommandError::BuoyInUse);
    }
    Ok(LockBuildTilePlan {
        water_class: if is_middle {
            water_class_from_m1(tile.m1)
        } else {
            WaterClass::Canal
        },
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: buoy_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

fn remove_lock_buoy_state(state: &mut GameState, c: TileCoord) {
    state.newgrf_animated_station_tiles.remove(&c);
    state
        .stations
        .retain(|station| !(station.pos == c && station.stop_kind == StopKind::Buoy));
}

/// Prepara la retirada manual de un muelle durante `DoBuildLock`.
///
/// `RemoveDock` limpia sus dos piezas aunque el comando se haya apuntado a la
/// sección acuática. El preflight exige una huella completa para no convertir
/// una estación importada incompleta en agua silenciosamente.
fn check_lock_dock_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    if crate::station::dock_footprint_for_tile(&state.map, c).is_none() {
        return Err(CommandError::MustDemolishDockFirst);
    }
    let _ = check_clear_dock(state, c)?;
    Ok(LockBuildTilePlan {
        water_class: if is_middle && has_tile_water_ground(tile) {
            water_class_from_m1(tile.m1)
        } else {
            WaterClass::Canal
        },
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: dock_clear_cost(&state.global_economy),
        add_canal_cost: !is_middle,
    })
}

/// Cuenta las llamadas a `CMD_REMOVE_SINGLE_SIGNAL` que hace
/// `ClearTile_Track` al retirar todos los carriles de una tesela.
fn rail_signal_clear_count(tile: Tile) -> i64 {
    if !crate::rail_signals::rail_tile_is_signals(tile.m5) {
        return 0;
    }
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    (0..6_u8)
        .filter_map(crate::rail_signals::SignalTrack::from_u8)
        .filter(|track| {
            tile.m5 & track.track_bit() != 0
                && present & crate::rail_signals::signal_on_track_mask(*track) != 0
        })
        .fold(0_i64, |count, _| count + 1)
}

/// Prepara una vía normal para `CMD_LANDSCAPE_CLEAR` sin `Auto`.
///
/// `DoBuildLock` retira cada pieza con `CMD_REMOVE_SINGLE_RAIL`, incluyendo
/// las señales presentes. Las teselas de depósito, túnel/puente y otros
/// subtipos siguen requiriendo demolición explícita.
fn check_lock_rail_tile(state: &GameState, tile: Tile) -> Result<LockBuildTilePlan, CommandError> {
    let owner = tile.m1 & 0x1F;
    if owner != (state.active_company.0 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    let subtype = (tile.m5 >> 6) & 0x03;
    if subtype != RAIL_TILE_NORMAL && subtype != crate::map::RAIL_TILE_SIGNALS {
        return Err(CommandError::BuildingMustBeDemolished);
    }
    let rail_type = crate::rail_type::rail_type_from_tile(tile);
    let track_count = i64::from((tile.m5 & 0x3F).count_ones());
    let rail_cost = rail_clear_cost(
        &state.global_economy,
        crate::rail_type::rail_build_cost_multiplier_for_type(
            rail_type,
            &state.runtime.rail_type_props,
        ),
    )
    .saturating_mul(track_count);
    let signal_cost =
        signal_clear_cost(&state.global_economy).saturating_mul(rail_signal_clear_count(tile));
    Ok(LockBuildTilePlan {
        water_class: WaterClass::Canal,
        owner: state.active_company.0,
        clear_on_build: true,
        clear_cost: rail_cost.saturating_add(signal_cost),
        add_canal_cost: false,
    })
}

/// Precio de `ClearTile_Clear` para una tesela de terreno.
fn clear_land_cost(state: &GameState, tile: Tile) -> i64 {
    let ground = clear_ground_type(tile.m5);
    let density = clear_density(tile.m5);
    let base = match ground {
        CLEAR_GROUND_GRASS => grass_clear_cost(&state.global_economy),
        CLEAR_GROUND_ROCKY => rocks_clear_cost(&state.global_economy),
        CLEAR_GROUND_FIELDS => fields_clear_cost(&state.global_economy),
        // `CLEAR_SNOW` y `CLEAR_DESERT` comparten `PR_CLEAR_ROUGH` en la
        // tabla nativa. Los valores desconocidos siguen el caso default de
        // `ClearTile_Clear` para no convertir una tesela importada en gratis.
        _ => rough_clear_cost(&state.global_economy),
    };
    if tile.m3 & 0x10 != 0 {
        let rough = rough_clear_cost(&state.global_economy);
        let grass = grass_clear_cost(&state.global_economy);
        let surcharge = if rough >= grass {
            rough - grass
        } else {
            grass - rough
        };
        base.saturating_add(surcharge)
    } else if ground != CLEAR_GROUND_GRASS || density != 0 {
        base
    } else {
        0
    }
}

/// Precio de `ClearTile_Trees`, incluido el multiplicador tropical nativo.
fn clear_tree_cost(state: &GameState, tile: Tile) -> i64 {
    let tree_multiplier = if (20..27).contains(&tile.m3) { 4 } else { 1 };
    trees_clear_cost(&state.global_economy)
        .saturating_mul(i64::from(tree_count(tile.m5)))
        .saturating_mul(tree_multiplier)
}

/// Prepara la parte de una esclusa ocupada por un objeto autoremovible.
fn check_lock_object_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    check_object_can_be_cleared(state, c)?;
    let original_water_class = water_class_from_m1(tile.m1);
    let has_water_ground = has_tile_water_ground(tile);
    let water_class = if is_middle {
        if has_water_ground {
            original_water_class
        } else {
            WaterClass::Canal
        }
    } else if has_water_ground {
        water_class_after_native_clear(&state.map, c, original_water_class)
    } else {
        WaterClass::Invalid
    };
    let water_class = if water_class == WaterClass::Invalid {
        WaterClass::Canal
    } else {
        water_class
    };
    let owner = if is_middle {
        state.active_company.0
    } else {
        match water_class {
            WaterClass::Sea | WaterClass::River => crate::company::OWNER_WATER_M1 & 0x1F,
            WaterClass::Canal => tile.m1 & 0x1F,
            WaterClass::Invalid => state.active_company.0,
        }
    };
    Ok(LockBuildTilePlan {
        water_class,
        owner,
        clear_on_build: false,
        clear_cost: 0,
        add_canal_cost: !is_middle,
    })
}

/// Error de `CMD_LANDSCAPE_CLEAR | Auto` para estructuras que no se pueden
/// sobreconstruir durante `DoBuildLock`.
///
/// Las carreteras y vías tienen validadores propios porque sus ramas manuales
/// calculan un coste por pieza, mientras `ClearTile_Track | Auto` siempre
/// rechaza la vía normal.
#[must_use]
fn lock_structure_clear_error(tile: Tile) -> Option<CommandError> {
    match tile.kind {
        TileKind::House | TileKind::Airport | TileKind::ShipDepot => {
            Some(CommandError::BuildingMustBeDemolished)
        }
        TileKind::Industry => Some(CommandError::IndustryInTheWay),
        TileKind::Station => match crate::station::stop_kind_from_m6(tile.m6) {
            StopKind::Dock => Some(CommandError::MustDemolishDockFirst),
            StopKind::Buoy => Some(CommandError::BuoyInTheWay),
            StopKind::OilRig => Some(CommandError::OilRigInTheWay),
            _ => Some(CommandError::BuildingMustBeDemolished),
        },
        TileKind::RoadBridge | TileKind::RailBridge => Some(CommandError::MustDemolishBridgeFirst),
        TileKind::RoadTunnel | TileKind::RailTunnel => Some(CommandError::MustDemolishTunnelFirst),
        TileKind::Grass
        | TileKind::Water
        | TileKind::Forest
        | TileKind::CoalField
        | TileKind::Road
        | TileKind::RoadDepot
        | TileKind::RailDepot
        | TileKind::Rail
        | TileKind::Void
        | TileKind::Unknown(_) => None,
    }
}

/// Prepara una tesela de agua para `CMD_LANDSCAPE_CLEAR` dentro de una
/// esclusa, distinguiendo agua plana de costa.
fn check_lock_water_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
    tile: Tile,
) -> Result<LockBuildTilePlan, CommandError> {
    let occupied_error = if is_middle {
        CommandError::CannotPlaceStationOnOccupiedTile
    } else {
        CommandError::BuildingMustBeDemolished
    };
    match water_tile_type(tile) {
        WATER_TILE_TYPE_CLEAR => {
            let water_class = water_class_from_m1(tile.m1);
            if is_middle {
                check_non_freeform_edge(&state.map, c, state.construction.freeform_edges)?;
                check_clear_water_owner(state, tile)?;
                let clear_cost = if water_class == WaterClass::Canal {
                    canal_clear_cost(&state.global_economy)
                } else {
                    water_clear_cost(&state.global_economy)
                };
                Ok(LockBuildTilePlan {
                    water_class,
                    owner: state.active_company.0,
                    clear_on_build: true,
                    clear_cost,
                    add_canal_cost: false,
                })
            } else {
                // Los extremos acuáticos no pasan por LandscapeClear en
                // `DoBuildLock`; por eso no se comprueba ownership ni se
                // cobra el despeje aquí.
                Ok(LockBuildTilePlan {
                    water_class,
                    owner: tile.m1 & 0x1F,
                    clear_on_build: false,
                    clear_cost: 0,
                    add_canal_cost: false,
                })
            }
        }
        WATER_TILE_TYPE_COAST => {
            let clear_cost = if is_middle {
                let (tileh, _) =
                    tile_slope_and_z(&state.map, c).ok_or(CommandError::OutOfBounds)?;
                if is_slope_with_one_corner_raised(tileh) {
                    water_clear_cost(&state.global_economy)
                } else {
                    rough_clear_cost(&state.global_economy)
                }
            } else {
                0
            };
            Ok(LockBuildTilePlan {
                // `HasTileWaterGround` es false para una costa, así que el
                // centro nativo cae a canal. Un extremo conserva su clase.
                water_class: if is_middle {
                    WaterClass::Canal
                } else {
                    water_class_from_m1(tile.m1)
                },
                owner: if is_middle {
                    state.active_company.0
                } else {
                    tile.m1 & 0x1F
                },
                clear_on_build: is_middle,
                clear_cost,
                add_canal_cost: false,
            })
        }
        _ => Err(occupied_error),
    }
}

/// Prepara la parte de una esclusa como lo haría `DoBuildLock` antes de
/// ejecutar las limpiezas. Esta fase no muta el mapa: preview y ejecución
/// comparten exactamente las mismas guardas y costes.
fn check_lock_build_tile(
    state: &GameState,
    c: TileCoord,
    is_middle: bool,
) -> Result<LockBuildTilePlan, CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let occupied_error = if is_middle {
        CommandError::CannotPlaceStationOnOccupiedTile
    } else {
        CommandError::BuildingMustBeDemolished
    };

    // `DoBuildLock` llama a `CMD_LANDSCAPE_CLEAR` sin `CommandFlag::Auto`.
    // La huella completa se planifica aparte para cobrarla una sola vez y para
    // no sobrescribirla durante el preflight.
    if is_map_object_tile(tile.mapt) {
        return check_lock_object_tile(state, c, is_middle, tile);
    }

    match tile.kind {
        TileKind::Water => check_lock_water_tile(state, c, is_middle, tile),
        TileKind::Grass => {
            if tile.ottd_type_nibble() != 0 {
                return Err(occupied_error);
            }
            Ok(LockBuildTilePlan {
                water_class: WaterClass::Canal,
                owner: state.active_company.0,
                clear_on_build: true,
                clear_cost: clear_land_cost(state, tile),
                add_canal_cost: !is_middle,
            })
        }
        TileKind::Forest => Ok(LockBuildTilePlan {
            water_class: WaterClass::Canal,
            owner: state.active_company.0,
            clear_on_build: true,
            clear_cost: clear_tree_cost(state, tile),
            add_canal_cost: !is_middle,
        }),
        // `CoalField` es la representación semántica histórica de un campo
        // `MP_CLEAR`; su limpieza usa la entrada `PR_CLEAR_FIELDS` nativa.
        TileKind::CoalField => Ok(LockBuildTilePlan {
            water_class: WaterClass::Canal,
            owner: state.active_company.0,
            clear_on_build: true,
            clear_cost: fields_clear_cost(&state.global_economy),
            add_canal_cost: !is_middle,
        }),
        TileKind::Station
            if matches!(
                crate::station::stop_kind_from_m6(tile.m6),
                StopKind::BusStop | StopKind::TruckStop
            ) =>
        {
            check_lock_road_stop_tile(state, c, is_middle, tile)
        }
        TileKind::Station
            if crate::station::stop_kind_from_m6(tile.m6) == StopKind::RoadWaypoint =>
        {
            check_lock_road_waypoint_tile(state, c, is_middle, tile)
        }
        TileKind::Station
            if crate::station::stop_kind_from_m6(tile.m6) == StopKind::RailWaypoint =>
        {
            check_lock_rail_waypoint_tile(state, c, is_middle, tile)
        }
        TileKind::Station
            if crate::station::stop_kind_from_m6(tile.m6) == StopKind::RailStation =>
        {
            check_lock_rail_station_tile(state, c, is_middle, tile)
        }
        TileKind::ShipDepot => check_lock_ship_depot_tile(state, c, is_middle, tile),
        TileKind::Airport => check_lock_airport_tile(state, c, is_middle),
        TileKind::Station if crate::station::stop_kind_from_m6(tile.m6) == StopKind::Dock => {
            check_lock_dock_tile(state, c, is_middle, tile)
        }
        TileKind::Station if crate::station::stop_kind_from_m6(tile.m6) == StopKind::Buoy => {
            check_lock_buoy_tile(state, c, is_middle, tile)
        }
        TileKind::Road if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) => {
            check_lock_road_crossing_tile(state, is_middle, tile)
        }
        TileKind::Road => check_lock_road_tile(state, is_middle, tile),
        TileKind::RoadDepot => check_lock_road_depot_tile(state, is_middle, tile),
        TileKind::RailDepot => check_lock_rail_depot_tile(state, is_middle, tile),
        TileKind::Rail => check_lock_rail_tile(state, tile),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => lock_structure_clear_error(tile).map_or_else(|| Err(occupied_error), Err),
    }
}

/// Planifica el mismo orden de limpiezas que `DoBuildLock`.
///
/// El centro siempre se limpia primero; los extremos se vuelven a evaluar
/// después porque `RemoveDock` puede haber convertido la otra pieza del
/// muelle en agua o en terreno. La copia evita mutar el estado durante el
/// preview y permite que la ejecución use exactamente los mismos planes.
fn plan_lock_tiles(
    state: &GameState,
    tiles: [TileCoord; 3],
) -> Result<([LockBuildTilePlan; 3], Vec<Vec<TileCoord>>), CommandError> {
    let auto_clear_objects = lock_clear_object_plan(state, tiles)?;
    let mut working = state.clone();
    let mut cleared_objects = vec![false; auto_clear_objects.len()];
    let mut plans = [None; 3];
    for (index, &coord) in tiles.iter().enumerate() {
        let plan = check_lock_build_tile(&working, coord, index == 0)?;
        plans[index] = Some(plan);
        clear_lock_build_tile(&mut working, coord, plan)?;
        if let Some((object_index, object_tiles)) =
            auto_clear_objects
                .iter()
                .enumerate()
                .find(|(object_index, object_tiles)| {
                    !cleared_objects[*object_index] && object_tiles.contains(&coord)
                })
        {
            clear_object_footprint_keep_water_without_charge(
                &mut working,
                object_tiles[0],
                object_tiles,
            )?;
            cleared_objects[object_index] = true;
        }
    }
    Ok((
        [
            plans[0].ok_or(CommandError::OutOfBounds)?,
            plans[1].ok_or(CommandError::OutOfBounds)?,
            plans[2].ok_or(CommandError::OutOfBounds)?,
        ],
        auto_clear_objects,
    ))
}

/// Ejecuta `DoClearSquare` para una parte de tierra o para el centro acuático.
fn clear_lock_build_tile(
    state: &mut GameState,
    c: TileCoord,
    plan: LockBuildTilePlan,
) -> Result<(), CommandError> {
    if !plan.clear_on_build {
        return Ok(());
    }
    let was_dock = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == StopKind::Dock
    });
    if was_dock {
        return clear_dock_impl(state, c, false);
    }
    let was_ship_depot = state.map.get_kind(c) == Some(TileKind::ShipDepot);
    if was_ship_depot {
        return clear_lock_ship_depot(state, c);
    }
    let was_airport = state.map.get_kind(c) == Some(TileKind::Airport);
    if was_airport {
        return clear_lock_airport(state, c);
    }
    let was_rail_station = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == StopKind::RailStation
    });
    if was_rail_station {
        return clear_lock_rail_station(state, c);
    }
    let was_rail = matches!(
        state.map.get_kind(c),
        Some(TileKind::Rail | TileKind::RailDepot)
    );
    let was_road_stop = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && matches!(
                crate::station::stop_kind_from_m6(tile.m6),
                StopKind::BusStop | StopKind::TruckStop
            )
    });
    let was_road_waypoint = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == StopKind::RoadWaypoint
    });
    let was_rail_waypoint = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == StopKind::RailWaypoint
    });
    let was_buoy = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == StopKind::Buoy
    });
    let depot_id = state
        .map
        .get(c)
        .filter(|tile| matches!(tile.kind, TileKind::RoadDepot | TileKind::RailDepot))
        .and_then(crate::depot::depot_id_from_tile);
    clear_tile_after_native_water_restore(&mut state.map, c)
        .map_err(|_| CommandError::OutOfBounds)?;
    clear_neighbour_non_flooding_states(&mut state.map, c);
    if was_road_stop {
        remove_lock_road_stop_state(state, c);
    }
    if was_road_waypoint {
        remove_lock_road_waypoint_state(state, c);
    }
    if was_rail_waypoint {
        remove_lock_rail_waypoint_state(state, c);
    }
    if was_buoy {
        remove_lock_buoy_state(state, c);
    }
    if let Some(depot_id) = depot_id {
        unregister_depot(state, depot_id);
    }
    if was_rail || was_rail_waypoint {
        super::rail::refresh_rail_neighbors(state, c)?;
        crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, c);
    }
    Ok(())
}

/// Escribe una parte de `MakeLockTile`, normalizando todos los campos raw que
/// `OpenTTD` reinicia al reemplazar la tesela.
#[must_use]
fn make_lock_tile(
    original: Tile,
    owner: u8,
    direction: u8,
    part: u8,
    water_class: WaterClass,
) -> Tile {
    let mut tile = original;
    tile.kind = TileKind::Water;
    tile.mapt = 0x60 | (original.mapt & 0x0F);
    tile.m1 = set_water_class_m1(owner & 0x1F, water_class);
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m5 = 0x20 | ((part & 0x03) << 2) | (direction & 0x03);
    tile.m6 &= 0x03;
    tile.m7 = 0;
    tile.m8 = 0;
    tile
}

/// Esclusa: agua + vecinos del eje con `|Δheight| == 1`.
pub(crate) fn check_place_lock(
    state: &GameState,
    c: TileCoord,
    axis_y: bool,
) -> Result<(), CommandError> {
    let map = &state.map;
    check_in_bounds(map, c)?;
    let (a, b) = lock_axis_neighbors(c, axis_y);
    check_in_bounds(map, a)?;
    check_in_bounds(map, b)?;
    if [c, a, b]
        .iter()
        .any(|tile| state.vehicles.iter().any(|vehicle| vehicle.pos == *tile))
    {
        return Err(CommandError::VehicleInTheWay);
    }
    let ha = map.get(a).map_or(0, |t| t.height);
    let hb = map.get(b).map_or(0, |t| t.height);
    if ha.abs_diff(hb) != 1 {
        return Err(CommandError::CannotPlaceStationOnOccupiedTile);
    }
    let direction = lock_direction_for_heights(axis_y, ha, hb);
    let [middle, lower, upper] = lock_tiles_from_middle(c, direction);
    let _ = plan_lock_tiles(state, [middle, lower, upper])?;
    check_non_freeform_edge(map, lower, state.construction.freeform_edges)?;
    check_lock_bridge_clearance(state, [middle, lower, upper])?;
    Ok(())
}

pub(in crate::command) fn place_lock(
    state: &mut GameState,
    c: TileCoord,
    axis_y: bool,
) -> Result<(), CommandError> {
    check_place_lock(state, c, axis_y)?;
    let (first, second) = lock_axis_neighbors(c, axis_y);
    let first_height = state
        .map
        .get(first)
        .ok_or(CommandError::OutOfBounds)?
        .height;
    let second_height = state
        .map
        .get(second)
        .ok_or(CommandError::OutOfBounds)?
        .height;
    let direction = lock_direction_for_heights(axis_y, first_height, second_height);
    let [middle, lower, upper] = lock_tiles_from_middle(c, direction);
    let (plans, auto_clear_objects) = plan_lock_tiles(state, [middle, lower, upper])?;
    let object_clear_cost = auto_clear_objects.iter().fold(0_i64, |cost, object_tiles| {
        cost.saturating_add(auto_clear_object_cost(state, object_tiles))
    });
    let construction_cost = plans.iter().fold(
        lock_build_cost(&state.global_economy).saturating_add(object_clear_cost),
        |cost, plan| {
            cost.saturating_add(plan.clear_cost)
                .saturating_add(if plan.add_canal_cost {
                    canal_build_cost(&state.global_economy)
                } else {
                    0
                })
        },
    );
    let mut cleared_objects = vec![false; auto_clear_objects.len()];
    for (coord, plan) in [(middle, plans[0]), (lower, plans[1]), (upper, plans[2])] {
        clear_lock_build_tile(state, coord, plan)?;
        if let Some((index, object_tiles)) = auto_clear_objects
            .iter()
            .enumerate()
            .find(|(index, object_tiles)| !cleared_objects[*index] && object_tiles.contains(&coord))
        {
            clear_object_footprint_keep_water_without_charge(state, object_tiles[0], object_tiles)?;
            cleared_objects[index] = true;
        }
    }
    let middle_original = state.map.get(middle).ok_or(CommandError::OutOfBounds)?;
    let lower_original = state.map.get(lower).ok_or(CommandError::OutOfBounds)?;
    let upper_original = state.map.get(upper).ok_or(CommandError::OutOfBounds)?;
    let middle_tile = make_lock_tile(
        middle_original,
        plans[0].owner,
        direction,
        0,
        plans[0].water_class,
    );
    let lower_tile = make_lock_tile(
        lower_original,
        plans[1].owner,
        direction,
        1,
        plans[1].water_class,
    );
    let upper_tile = make_lock_tile(
        upper_original,
        plans[2].owner,
        direction,
        2,
        plans[2].water_class,
    );
    for (coord, tile) in [(c, middle_tile), (lower, lower_tile), (upper, upper_tile)] {
        state
            .map
            .set_tile(coord, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= construction_cost;
    Ok(())
}

/// Retira una esclusa completa desde cualquiera de sus tres partes.
pub(in crate::command) fn clear_lock(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    let (tiles, _) = lock_geometry(state, c)?;
    check_clear_lock(state, c)?;
    let [middle, lower, upper] = tiles;
    let classes = tiles.map(|tile| {
        state
            .map
            .get(tile)
            .map_or(WaterClass::Sea, |raw| water_class_from_m1(raw.m1))
    });

    if classes[0] == WaterClass::River {
        let random_bits = u8::try_from(state.random.next() & 0xFF).unwrap_or(0);
        crate::map::make_water_tile_with_random_bits(
            &mut state.map,
            middle,
            WaterClass::River,
            random_bits,
        )
        .map_err(|_| CommandError::OutOfBounds)?;
    } else {
        clear_tile_after_native_water_restore(&mut state.map, middle)
            .map_err(|_| CommandError::OutOfBounds)?;
        clear_neighbour_non_flooding_states(&mut state.map, middle);
    }

    // `RemoveLock` restaura primero `tile + delta` (Upper) y luego
    // `tile - delta` (Lower); ese orden también define el consumo de RNG.
    make_water_tile_after_native_clear(state, upper, classes[2])?;
    make_water_tile_after_native_clear(state, lower, classes[1])?;
    for tile in [middle, lower, upper] {
        refresh_ship_docking_tiles_around(state, tile);
    }
    state.economy.money -= lock_clear_cost(&state.global_economy);
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
