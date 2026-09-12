use crate::company::OWNER_NONE_M1;
use crate::economy::{object_clear_cost_factored, road_stop_clear_cost_factored};
use crate::map::{
    Map, OBJECT_TYPE_COMPANY_HEADQUARTERS, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND,
    OBJECT_TYPE_STATUE_COMPANY, OBJECT_TYPE_TRANSMITTER, TileCoord, TileKind, WaterClass,
    has_tile_water_ground, is_map_object_tile, object_id_from_tile, object_type_from_tile,
    water_class_from_m1,
};
use crate::object_spec::{
    NEW_OBJECT_OFFSET, OBJECT_FLAG_AUTOREMOVE, OBJECT_FLAG_CANNOT_REMOVE, OBJECT_FLAG_CLEAR_INCOME,
    OWNED_LAND_COST_FACTOR,
};
use crate::vehicle::VehicleOrder;
use crate::{CLEAR_TILE_COST, GameState, StopKind};

use super::super::{CommandError, in_bounds, require_tile_owned_by_active, tile_owner};

#[allow(unused_imports)]
use crate::command::transport::internal::{
    RAIL_DIAG_MASK, RAIL_PARALLEL_MASK, RAIL_TB_HORZ, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y,
    check_rail_trackbits_on_tile, existing_rail_trackbits, offset_along_horz_rail,
    offset_along_vert_rail, write_normal_rail_tile,
};

pub(crate) fn check_in_bounds(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(map, c)
}

pub(in crate::command::transport) fn trackbits_to_signal_present(tb: u8) -> u8 {
    if tb == RAIL_TB_X || tb == RAIL_TB_Y {
        0b1100
    } else {
        0x0F
    }
}

pub(in crate::command::transport) fn propagate_rail_diag_to_neighbors(
    state: &mut GameState,
    c: TileCoord,
    add: u8,
) -> Result<(), CommandError> {
    let add = add & RAIL_DIAG_MASK;
    if add == 0 {
        return Ok(());
    }
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if state.map.get_kind(n) != Some(TileKind::Rail) {
            continue;
        }
        if tile_owner(state, n).is_some_and(|o| o != state.active_company) {
            continue;
        }
        let existing = existing_rail_trackbits(&state.map, n);
        let existing_diag = existing & RAIL_DIAG_MASK;
        if existing_diag == 0 || existing_diag == add {
            continue;
        }
        let merged = existing | add;
        if merged == existing {
            continue;
        }
        check_rail_trackbits_on_tile(&state.map, n, merged)?;
        write_normal_rail_tile(state, n, merged)?;
    }
    Ok(())
}

pub(in crate::command::transport) fn road_stop_clear_cost_for_tile(
    state: &GameState,
    c: TileCoord,
) -> Option<i64> {
    state
        .stations
        .iter()
        .find(|station| {
            station.covers_tile(c)
                && matches!(station.stop_kind, StopKind::BusStop | StopKind::TruckStop)
        })
        .and_then(|station| {
            station.road_stop_spec_at(c).and_then(|spec_id| {
                crate::road_stop_spec::road_stop_spec_def(&state.road_stop_spec_catalog, spec_id)
                    .map(|spec| {
                        road_stop_clear_cost_factored(
                            &state.global_economy,
                            station.stop_kind,
                            spec.clear_cost_multiplier,
                        )
                    })
            })
        })
}

/// Cambio de dinero al retirar un `MP_OBJECT` con la fórmula nativa.
///
/// Los objetos vanilla sin coste usan factor cero. Un objeto importado sin
/// spec conserva el fallback histórico del llamador, porque no es posible
/// recuperar su multiplicador Action0 de forma fiable.
pub(in crate::command::transport) fn object_clear_money_delta(
    state: &GameState,
    object_type: u16,
    tile_count: u32,
) -> Option<i64> {
    let (cost_factor, clear_income) = if object_type == u16::from(OBJECT_TYPE_OWNED_LAND) {
        (OWNED_LAND_COST_FACTOR, true)
    } else if object_type < NEW_OBJECT_OFFSET {
        (0, false)
    } else {
        let spec = crate::object_spec::object_spec_def(&state.object_spec_catalog, object_type)?;
        (
            spec.clear_cost_factor,
            spec.flags & OBJECT_FLAG_CLEAR_INCOME != 0,
        )
    };
    let cost = object_clear_cost_factored(&state.global_economy, cost_factor, tile_count);
    Some(if clear_income { cost } else { -cost })
}

pub(in crate::command::transport) fn junction_merge_for_neighbor(
    holder_tb: u8,
    neighbor_tb: u8,
    dx: i32,
    dy: i32,
) -> Option<u8> {
    if neighbor_tb & RAIL_PARALLEL_MASK == 0 || neighbor_tb.count_ones() != 1 {
        return None;
    }
    let neighbor_horz = neighbor_tb & RAIL_TB_HORZ != 0;
    let neighbor_vert = neighbor_tb & RAIL_TB_VERT != 0;

    let holder_horz = holder_tb & RAIL_TB_HORZ != 0 && holder_tb & RAIL_TB_VERT == 0;
    let holder_vert = holder_tb & RAIL_TB_VERT != 0 && holder_tb & RAIL_TB_HORZ == 0;

    if holder_horz && neighbor_vert && offset_along_vert_rail(dx, dy) {
        return Some(holder_tb | neighbor_tb);
    }
    if holder_vert && neighbor_horz && offset_along_horz_rail(dx, dy) {
        return Some(holder_tb | neighbor_tb);
    }
    None
}

pub(in crate::command::transport) fn refresh_track_junction_from_neighbor(
    state: &mut GameState,
    holder: TileCoord,
    neighbor: TileCoord,
    neighbor_tb: u8,
) -> Result<(), CommandError> {
    let holder_tb = existing_rail_trackbits(&state.map, holder);
    if holder_tb == 0 {
        return Ok(());
    }
    let dx = neighbor.x - holder.x;
    let dy = neighbor.y - holder.y;
    let Some(merged) = junction_merge_for_neighbor(holder_tb, neighbor_tb, dx, dy) else {
        return Ok(());
    };
    if merged == holder_tb {
        return Ok(());
    }
    check_rail_trackbits_on_tile(&state.map, holder, merged)?;
    write_normal_rail_tile(state, holder, merged)
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

pub(crate) fn check_clear_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if map.get_kind(c) == Some(TileKind::Void) {
        Err(CommandError::CannotPlaceRoadOnVoid)
    } else {
        Ok(())
    }
}

/// Replica el rechazo de `ClearTile_Object` para objetos que declaran
/// `CannotRemove`. El bulldozer mágico es la excepción nativa.
pub(in crate::command) fn check_object_can_be_cleared(
    state: &GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    if state.cheats.magic_bulldozer_active() {
        return Ok(());
    }
    let Some(tile) = state.map.get(c) else {
        return Ok(());
    };
    if !is_map_object_tile(tile.mapt) {
        return Ok(());
    }
    let owner = tile.m1 & 0x1F;
    if owner != OWNER_NONE_M1 && owner != state.active_company.0 {
        return Err(CommandError::TileNotOwned);
    }
    let Some(object_type) = state.map.object_type_at(c) else {
        return Ok(());
    };
    let vanilla_cannot_remove = matches!(
        object_type,
        ty if ty == u16::from(OBJECT_TYPE_TRANSMITTER)
            || ty == u16::from(OBJECT_TYPE_LIGHTHOUSE)
            || ty == u16::from(OBJECT_TYPE_STATUE_COMPANY)
            || ty == u16::from(OBJECT_TYPE_COMPANY_HEADQUARTERS)
    );
    let newgrf_cannot_remove =
        crate::object_spec::object_spec_def(&state.object_spec_catalog, object_type)
            .is_some_and(|spec| spec.flags & OBJECT_FLAG_CANNOT_REMOVE != 0);
    if vanilla_cannot_remove || newgrf_cannot_remove {
        Err(CommandError::ObjectCannotBeRemoved)
    } else {
        Ok(())
    }
}

/// Comprueba la limpieza implícita que hacen los comandos de construcción
/// marcados con `DoCommandFlag::Auto`. A diferencia de una demolición manual,
/// `OpenTTD` sólo deja pasar objetos con `Autoremove`.
pub(in crate::command) fn check_object_can_be_auto_cleared(
    state: &GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    let Some(tile) = state.map.get(c) else {
        return Ok(());
    };
    if !is_map_object_tile(tile.mapt) {
        return Ok(());
    }
    let Some(object_type) = state.map.object_type_at(c) else {
        // Sin el tipo no se puede demostrar que el objeto sea autoremovible;
        // conservarlo evita sobrescribir un `MP_OBJECT` importado.
        return Err(CommandError::ObjectInTheWay);
    };
    let autoremove = object_type == u16::from(OBJECT_TYPE_OWNED_LAND)
        || crate::object_spec::object_spec_def(&state.object_spec_catalog, object_type)
            .is_some_and(|spec| spec.flags & OBJECT_FLAG_AUTOREMOVE != 0);
    if !autoremove {
        return Err(CommandError::ObjectInTheWay);
    }
    let owner = tile.m1 & 0x1F;
    if owner != OWNER_NONE_M1 && owner != state.active_company.0 {
        return Err(CommandError::TileNotOwned);
    }
    Ok(())
}

fn order_targets_tile(order: VehicleOrder, target: TileCoord) -> bool {
    matches!(
        order,
        VehicleOrder::Station { .. } | VehicleOrder::Waypoint { .. }
    ) && order.destination() == target
}

/// Comprueba `HasStationInUse(..., include_company = false)` para una boya.
///
/// Las boyas son waypoints neutrales: el dueño de la orden no se obtiene de
/// la fila `Station`, sino del vehículo que la ejecuta. Las listas
/// compartidas se inspeccionan a través de los vehículos que las referencian,
/// que es la única fuente local de compañía para ese pool.
pub(in crate::command::transport) fn buoy_in_use_by_other_company(
    state: &GameState,
    c: TileCoord,
) -> bool {
    state.vehicles.iter().any(|vehicle| {
        vehicle.owner != state.active_company
            && (vehicle
                .orders
                .iter()
                .copied()
                .any(|order| order_targets_tile(order, c))
                || vehicle.shared_order_id.is_some_and(|shared_id| {
                    state
                        .shared_order_lists
                        .iter()
                        .find(|list| list.id == shared_id)
                        .is_some_and(|list| {
                            list.orders
                                .iter()
                                .copied()
                                .any(|order| order_targets_tile(order, c))
                        })
                }))
    })
}

fn depot_kind_at(state: &GameState, tile: TileCoord) -> Option<TileKind> {
    state.map.get_kind(tile).filter(|kind| {
        matches!(
            kind,
            TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot
        )
    })
}

/// Límite nativo de `MAX_LENGTH_DEPOT_NAME_CHARS`: incluye el terminador NUL,
/// por lo que el comando acepta como máximo 31 caracteres Unicode.
pub const MAX_DEPOT_NAME_CHARS: usize = 32;

/// Replica el contador `town_cn` de `MakeDefaultName(Depot)`.
///
/// El contador es independiente por pueblo y tipo de transporte; el ID del
/// pool no interviene en el nombre visible. El mapa ya contiene el depósito
/// nuevo al llamar a esta función, pero la fila aún no fue registrada.
fn next_depot_town_cn(
    state: &GameState,
    town_id: u32,
    depot_kind: Option<TileKind>,
    excluded_depot_id: Option<u16>,
) -> u16 {
    (0..=u16::MAX)
        .find(|candidate| {
            !state.depots.iter().any(|depot| {
                excluded_depot_id != Some(depot.depot_id)
                    && depot.town_id == Some(town_id)
                    && depot.town_cn == *candidate
                    && depot_kind_at(state, depot.tile) == depot_kind
            })
        })
        .unwrap_or(u16::MAX)
}

/// Registra la fila semántica que `new Depot(tile)` crea junto con `MAP2`.
///
/// `MakeDefaultName` asocia el depósito al pueblo más cercano y asigna el
/// primer ordinal libre para ese pueblo y tipo. El texto sigue vacío para que
/// el cliente pueda resolver la plantilla según el locale activo.
pub(in crate::command::transport) fn register_depot(
    state: &mut GameState,
    depot_id: u16,
    tile: TileCoord,
) {
    state.depots.retain(|depot| depot.depot_id != depot_id);
    let depot_kind = depot_kind_at(state, tile);
    let (town_id, town_cn) = crate::town::nearest_town_index(&state.towns, tile)
        .and_then(|(town_index, _)| state.towns.get(town_index))
        .map_or((None, 0), |town| {
            (
                Some(town.id),
                next_depot_town_cn(state, town.id, depot_kind, None),
            )
        });
    state.depots.push(crate::sav::SavDepot {
        depot_id,
        tile,
        town_id,
        town_cn,
        name: String::new(),
        build_date: crate::news::openttd_date_from_calendar_day_index(u64::from(
            state.calendar.date,
        )),
    });
}

/// Retira la fila `DEPT` después de que el mapa ya fue limpiado con éxito.
pub(in crate::command::transport) fn unregister_depot(state: &mut GameState, depot_id: u16) {
    state.depots.retain(|depot| depot.depot_id != depot_id);
}

/// Renombra un depósito usando el mismo contrato que `CmdRenameDepot`.
///
/// El comando recibe una posición porque es la forma en que el cliente
/// identifica la ventana. Para depósitos navales, ambas secciones resuelven
/// al mismo `DepotID` y se valida la propiedad sobre `Depot::xy` (la sección
/// norte persistida).
pub(in crate::command) fn rename_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
    name: Option<String>,
) -> Result<(), CommandError> {
    let tile = state
        .map
        .get(depot_pos)
        .ok_or(CommandError::DepotNotFound)?;
    let depot_id = crate::depot::depot_id_from_tile(tile).ok_or(CommandError::DepotNotFound)?;
    let canonical_tile = crate::depot::canonical_depot_command_tile(&state.map, depot_pos);

    // Saves JSON antiguos no tenían DEPT en el estado de juego. La creación
    // perezosa conserva la capacidad de renombrar esos depósitos y permite
    // que el siguiente guardado los migre al pool semántico.
    if !state.depots.iter().any(|depot| depot.depot_id == depot_id) {
        register_depot(state, depot_id, canonical_tile);
    }
    let depot_idx = state
        .depots
        .iter()
        .position(|depot| depot.depot_id == depot_id)
        .ok_or(CommandError::DepotNotFound)?;
    let actual_tile = state.depots[depot_idx].tile;
    require_tile_owned_by_active(state, actual_tile)?;

    let normalized = name.unwrap_or_default();
    if normalized.chars().count() >= MAX_DEPOT_NAME_CHARS {
        return Err(CommandError::DepotNameTooLong);
    }
    if !normalized.is_empty()
        && state
            .depots
            .iter()
            .any(|depot| !depot.name.is_empty() && depot.name == normalized)
    {
        return Err(CommandError::DepotNameTaken);
    }

    if normalized.is_empty() {
        state.depots[depot_idx].name.clear();
        let (town_id, town_cn) = crate::town::nearest_town_index(&state.towns, actual_tile)
            .and_then(|(town_index, _)| state.towns.get(town_index))
            .map_or((None, 0), |town| {
                (
                    Some(town.id),
                    next_depot_town_cn(
                        state,
                        town.id,
                        depot_kind_at(state, actual_tile),
                        Some(depot_id),
                    ),
                )
            });
        state.depots[depot_idx].town_id = town_id;
        state.depots[depot_idx].town_cn = town_cn;
    } else {
        state.depots[depot_idx].name = normalized;
    }
    Ok(())
}

pub(in crate::command) fn transport_tile_is_buildable(kind: TileKind) -> bool {
    !matches!(kind, TileKind::Water | TileKind::Void)
}

pub(in crate::command) fn build_error_for_kind(kind: TileKind) -> CommandError {
    match kind {
        TileKind::Water => CommandError::CannotPlaceRoadOnWater,
        TileKind::Void => CommandError::CannotPlaceRoadOnVoid,
        _ => CommandError::OutOfBounds,
    }
}

pub(in crate::command) fn place_single_transport_tile(
    state: &mut GameState,
    c: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost: i64,
) -> Result<(), CommandError> {
    place_single_transport_tile_with_optional_depot_id(
        state,
        c,
        kind_to_place,
        mapt,
        m5,
        cost,
        None,
    )
}

pub(in crate::command) fn place_single_transport_tile_with_depot_id(
    state: &mut GameState,
    c: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost: i64,
    depot_id: u16,
) -> Result<(), CommandError> {
    place_single_transport_tile_with_optional_depot_id(
        state,
        c,
        kind_to_place,
        mapt,
        m5,
        cost,
        Some(depot_id),
    )
}

fn place_single_transport_tile_with_optional_depot_id(
    state: &mut GameState,
    c: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost: i64,
    depot_id: Option<u16>,
) -> Result<(), CommandError> {
    check_single_transport_tile(&state.map, c)?;
    require_tile_owned_by_active(state, c)?;
    state
        .map
        .set_kind(c, kind_to_place)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, mapt, m5)
        .map_err(|_| CommandError::OutOfBounds)?;
    if let Some(depot_id) = depot_id {
        state
            .map
            .set_m2_u16(c, depot_id)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    let _ = state.map.set_m1(c, state.active_company.0);
    state.economy.money -= cost;
    Ok(())
}

pub(crate) fn axis_line(a: TileCoord, b: TileCoord) -> Vec<TileCoord> {
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

fn check_town_demolition_rating(
    state: &GameState,
    c: TileCoord,
    kind: TileKind,
) -> Result<(), CommandError> {
    if state.cheats.magic_bulldozer_active() {
        return Ok(());
    }
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !crate::company::CompanyId::is_town_owner_m1(tile.m1) {
        return Ok(());
    }
    let check_type = match kind {
        TileKind::RoadBridge
        | TileKind::RailBridge
        | TileKind::RoadTunnel
        | TileKind::RailTunnel => crate::town::TownRatingCheckType::TunnelBridgeRemove,
        TileKind::Road => crate::town::TownRatingCheckType::RoadRemove,
        _ => return Ok(()),
    };
    let Some((idx, dist)) = crate::town::nearest_town_index(&state.towns, c) else {
        return Ok(());
    };
    if dist > crate::town::TOWN_AUTHORITY_RADIUS {
        return Ok(());
    }
    if crate::town::check_town_rating(
        &state.towns[idx],
        state.active_company,
        check_type,
        state.town_council_tolerance,
    ) {
        Ok(())
    } else {
        Err(CommandError::AuthorityRatingTooLow)
    }
}

fn clear_object_footprint_impl(
    state: &mut GameState,
    c: TileCoord,
    object_tiles: &[TileCoord],
    keep_water: bool,
    charge_money: bool,
) -> Result<(), CommandError> {
    let object_id = state.map.get(c).and_then(|tile| object_id_from_tile(&tile));
    let object_clear_delta = state.map.object_type_at(c).and_then(|object_type| {
        object_clear_money_delta(
            state,
            object_type,
            u32::try_from(object_tiles.len()).unwrap_or(u32::MAX),
        )
    });
    let statue_owner = state
        .map
        .get(c)
        .filter(|tile| object_type_from_tile(tile) == Some(OBJECT_TYPE_STATUE_COMPANY))
        .map(|tile| crate::company::CompanyId(tile.m1));
    let preserved_water = if keep_water {
        object_tiles
            .iter()
            .map(|&tile| {
                state
                    .map
                    .get(tile)
                    .and_then(|raw| has_tile_water_ground(raw).then(|| water_class_from_m1(raw.m1)))
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    for tile in object_tiles {
        if !state.cheats.magic_bulldozer_active() {
            require_tile_owned_by_active(state, *tile)?;
        }
    }
    for (index, &tile) in object_tiles.iter().enumerate() {
        if let Some(water_class) = preserved_water.get(index).copied().flatten() {
            super::water::make_water_tile_after_native_clear(state, tile, water_class)?;
        } else {
            state
                .map
                .set_kind(tile, TileKind::Grass)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(tile, 0x00, 0x00)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        let _ = state.map.set_m2(tile, 0);
        crate::command::sign::remove_signs_at(state, tile);
    }
    // Un objeto importado moderno comparte ObjectID en todas sus teselas;
    // el layout local histórico usa m2 como offset, por lo que también
    // quitamos la instancia cuyo origen cae dentro del footprint.
    state.objects.retain(|object| {
        let same_id = object_id.is_some_and(|id| object.object_id == id);
        !(same_id || object_tiles.contains(&object.tile))
    });
    state.sav_objects_dirty = true;
    state.stations.retain(|s| !object_tiles.contains(&s.pos));
    // `Object` upstream conserva el pueblo de la estatua. El port no mantiene
    // ese pool, por lo que la estatua se vincula al pueblo más cercano.
    if let Some(owner) = statue_owner
        && let Some((town_idx, _)) = crate::town::nearest_town_index(&state.towns, c)
    {
        state.towns[town_idx].set_statue(owner, false);
    }
    if charge_money {
        if let Some(delta) = object_clear_delta {
            state.economy.money += delta;
        } else {
            state.economy.money -= CLEAR_TILE_COST;
        }
    }
    Ok(())
}

fn clear_object_footprint(
    state: &mut GameState,
    c: TileCoord,
    object_tiles: &[TileCoord],
) -> Result<(), CommandError> {
    clear_object_footprint_impl(state, c, object_tiles, false, true)
}

pub(in crate::command::transport) fn clear_object_footprint_keep_water(
    state: &mut GameState,
    c: TileCoord,
    object_tiles: &[TileCoord],
) -> Result<(), CommandError> {
    clear_object_footprint_impl(state, c, object_tiles, true, true)
}

/// Variante para comandos compuestos que liquidan el coste total una sola
/// vez después de limpiar todos sus blockers.
pub(in crate::command::transport) fn clear_object_footprint_keep_water_without_charge(
    state: &mut GameState,
    c: TileCoord,
    object_tiles: &[TileCoord],
) -> Result<(), CommandError> {
    clear_object_footprint_impl(state, c, object_tiles, true, false)
}

pub(in crate::command) fn clear_tile(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_clear_tile(&state.map, c)?;
    if state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == crate::StopKind::Dock
    }) {
        return super::water::clear_dock(state, c);
    }
    if state.map.get_kind(c) == Some(TileKind::ShipDepot) {
        return super::water::clear_ship_depot(state, c);
    }
    if state.map.get_kind(c) == Some(TileKind::Water) {
        return super::water::clear_water_tile(state, c);
    }
    let depot_id = state.map.get(c).and_then(crate::depot::depot_id_from_tile);
    if let Some(kind) = state.map.get_kind(c) {
        check_town_demolition_rating(state, c, kind)?;
    }
    let is_buoy = state.map.get(c).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && crate::station::stop_kind_from_m6(tile.m6) == crate::station::StopKind::Buoy
    });
    let is_neutral_buoy = is_buoy
        && state.stations.iter().any(|station| {
            station.pos == c
                && station.stop_kind == crate::station::StopKind::Buoy
                && station.owner == crate::company::CompanyId::NONE
        });
    // Native `CmdLandscapeClear` lets any company remove a buoy. Its tile
    // owner still belongs to the underlying water and is restored by
    // `RemoveBuoy`, so it must not be used as a station ownership gate here.
    if !state.cheats.magic_bulldozer_active() && !is_neutral_buoy {
        require_tile_owned_by_active(state, c)?;
    }
    if is_buoy && buoy_in_use_by_other_company(state, c) {
        return Err(CommandError::BuoyInUse);
    }
    check_object_can_be_cleared(state, c)?;
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
    if let Some(object_tiles) =
        crate::map::object_footprint_at(&state.map, c, &state.object_spec_catalog)
    {
        clear_object_footprint(state, c, &object_tiles)?;
        return Ok(());
    }

    // Una boya es una estación superpuesta sobre agua. Al retirarla, la
    // tesela subyacente debe volver a ser agua (con su clase original), no
    // hierba: de lo contrario se destruye un canal, mar o río navegable.
    if is_buoy {
        let water_class = state
            .map
            .get(c)
            .map_or(WaterClass::Sea, |tile| water_class_from_m1(tile.m1));
        super::water::make_water_tile_after_native_clear(state, c, water_class)?;
        let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
        tile.m6 = 0;
        state
            .map
            .set_tile(c, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
        state.stations.retain(|station| station.pos != c);
        state.newgrf_animated_station_tiles.remove(&c);
        state.economy.money -= crate::economy::buoy_clear_cost(&state.global_economy);
        return Ok(());
    }

    // A custom RoadStop supplies its own Action0 `0x15` clear multiplier. The
    // native command charges the category-specific clear price even though the
    // tile itself is reset to grass below.
    let road_stop_clear_cost = road_stop_clear_cost_for_tile(state, c);

    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    if let Some(depot_id) = depot_id {
        unregister_depot(state, depot_id);
    }
    state.stations.retain(|s| s.pos != c);
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    crate::command::sign::remove_signs_at(state, c);
    state.economy.money -= road_stop_clear_cost.unwrap_or(CLEAR_TILE_COST);
    Ok(())
}
