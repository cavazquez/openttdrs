use crate::GameState;
use crate::depot::nearest_depot_tile_indexed;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, farthest_reachable_tile};
use crate::vehicle::{MAX_VEHICLE_NAME_CHARS, Vehicle, VehicleKind, VehicleOrder};

use super::error::OrderMoveDirection;
use super::transport::road_depot_exit_for_dir;
use super::{
    CommandError, in_bounds, require_tile_owned_by_active, require_vehicle_owned_by_active,
};

pub(super) fn set_vehicle_order_list(
    state: &mut GameState,
    id: u32,
    orders: Vec<VehicleOrder>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle_kind = state.vehicles[vehicle_idx].kind;
    let aircraft_engine_id = state.vehicles[vehicle_idx].engine_id;
    for order in &orders {
        in_bounds(&state.map, order.destination())?;
        match order {
            VehicleOrder::Station { station, .. } => {
                let Some(st) = state.stations.iter().find(|s| s.pos == *station) else {
                    return Err(CommandError::StationNotFound);
                };
                if !st.can_service_vehicle(vehicle_kind) || st.is_waypoint() {
                    return Err(CommandError::IncompatibleStopForVehicle);
                }
                // `CanVehicleUseStation`: avión/hélico vs flags FTA del aeropuerto.
                if vehicle_kind == VehicleKind::Aircraft
                    && let Some(engine_id) = aircraft_engine_id
                {
                    let is_heli =
                        crate::engine::engine_in_catalog(&state.engine_catalog, engine_id)
                            .map_or_else(
                                || crate::engine::aircraft_is_helicopter(engine_id),
                                crate::engine::aircraft_is_helicopter_def,
                            );
                    if !crate::airport_class::airport_allows_aircraft(st.airport_spec, is_heli) {
                        return Err(CommandError::IncompatibleStopForVehicle);
                    }
                }
            }
            VehicleOrder::Waypoint { waypoint, .. } => {
                let Some(st) = state.stations.iter().find(|s| s.pos == *waypoint) else {
                    return Err(CommandError::StationNotFound);
                };
                if !st.is_waypoint() || !st.can_service_vehicle(vehicle_kind) {
                    return Err(CommandError::IncompatibleStopForVehicle);
                }
            }
            VehicleOrder::Depot { depot, .. } => {
                in_bounds(&state.map, *depot)?;
                let kind = state.map.get_kind(*depot);
                let ok = match vehicle_kind {
                    VehicleKind::Train => kind == Some(TileKind::RailDepot),
                    VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
                        kind == Some(TileKind::RoadDepot)
                    }
                    VehicleKind::Ship => kind == Some(TileKind::ShipDepot),
                    VehicleKind::Aircraft => kind == Some(TileKind::Airport),
                };
                if !ok {
                    return Err(CommandError::InvalidDepotTile);
                }
            }
            VehicleOrder::Tile(_) | VehicleOrder::Conditional { .. } => {}
        }
    }
    let vehicle = &mut state.vehicles[vehicle_idx];
    vehicle.set_vehicle_orders(orders);
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn set_vehicle_orders(
    state: &mut GameState,
    id: u32,
    orders: Vec<TileCoord>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, id)?;
    for order in &orders {
        in_bounds(&state.map, *order)?;
    }
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.set_orders(orders);
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn set_vehicle_station_orders(
    state: &mut GameState,
    id: u32,
    stations: Vec<TileCoord>,
) -> Result<(), CommandError> {
    set_vehicle_order_list(
        state,
        id,
        stations.into_iter().map(VehicleOrder::station).collect(),
    )
}

/// Comando viejo: compra el motor por defecto del tipo (solo depósito de carretera).
pub(super) fn build_road_vehicle_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
    kind: VehicleKind,
) -> Result<(), CommandError> {
    if matches!(
        kind,
        VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft
    ) {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    build_vehicle_at_depot(state, depot_pos, crate::engine::default_engine_id(kind))
}

/// Compra el modelo `engine_id` en un depósito compatible, validando fondos.
#[allow(clippy::too_many_lines)]
pub(super) fn build_vehicle_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
    engine_id: u16,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let Some(tile) = state.map.get(depot_pos) else {
        return Err(CommandError::OutOfBounds);
    };
    // Validar ownership del depósito antes de comprar.
    require_tile_owned_by_active(state, depot_pos)?;
    let Some(engine) = crate::engine::engine_in_catalog(&state.engine_catalog, engine_id)
        .cloned()
        .or_else(|| crate::engine::engine_by_id(engine_id).cloned())
    else {
        return Err(CommandError::EngineNotFound);
    };
    let depot_ok = match engine.kind {
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            tile.kind == TileKind::RoadDepot
        }
        VehicleKind::Train => tile.kind == TileKind::RailDepot,
        VehicleKind::Ship => tile.kind == TileKind::ShipDepot,
        VehicleKind::Aircraft => {
            tile.kind == TileKind::Airport
                && crate::airport::airport_tile_is_hangar(&state.map, depot_pos)
        }
    };
    if !depot_ok {
        return Err(CommandError::InvalidDepotTile);
    }
    if engine.kind == VehicleKind::Aircraft {
        check_aircraft_heli_depot_compat(state, depot_pos, engine_id)?;
    }
    // Compatibilidad motor↔vía adyacente (eléctrico / mono / maglev).
    if engine.kind == VehicleKind::Train {
        let required = crate::rail_type::required_rail_type_for_engine(engine_id);
        if required != crate::rail_type::RailType::Rail {
            let neighbor_ok = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let n = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
                state.map.get(n).is_some_and(|t| {
                    t.kind == TileKind::Rail && crate::rail_type::rail_type_from_tile(t) == required
                })
            });
            if !neighbor_ok {
                return Err(match required {
                    crate::rail_type::RailType::Electric => {
                        CommandError::EngineRequiresElectricRail
                    }
                    crate::rail_type::RailType::Monorail => CommandError::EngineRequiresMonorail,
                    crate::rail_type::RailType::Maglev => CommandError::EngineRequiresMaglev,
                    crate::rail_type::RailType::Rail => CommandError::EngineRequiresElectricRail,
                });
            }
        }
    }
    // `Engine::GetCost` evalúa CB36 antes de crear la unidad. Usar una unidad
    // efímera reproduce el scope de compra y permite que el callback cambie
    // el factor sin mutar todavía la flota real.
    let mut cost_probe = Vehicle::new(0, engine.kind, depot_pos, depot_pos);
    cost_probe.engine_id = Some(engine.id);
    cost_probe.cargo_type = engine.cargo;
    let purchase_cost =
        crate::economy::vehicle_purchase_cost_with_callbacks(&engine, &mut cost_probe);
    if state.economy.money < purchase_cost {
        return Err(CommandError::InsufficientFunds);
    }
    let next_id = state
        .vehicles
        .iter()
        .map(|v| v.id)
        .max()
        .map_or(1, |v| v.saturating_add(1));
    let mut vehicle = Vehicle::new(next_id, engine.kind, depot_pos, depot_pos);
    vehicle.running = false;
    vehicle.engine_id = Some(engine.id);
    if vehicle.cargo_type.is_none() {
        vehicle.cargo_type = engine.cargo;
    }
    vehicle.unit_length = crate::newgrf_callback::vehicle_unit_length(&engine, &mut vehicle);
    crate::vehicle::init_vehicle_reliability_from_engine(&mut vehicle, &engine);
    let property_capacity = (engine.capacity > 0 || engine.cargo.is_some())
        .then(|| {
            crate::newgrf_callback::resolve_vehicle_capacity_property_callback(
                &engine,
                &mut vehicle,
            )
        })
        .flatten();
    let raw_capacity = property_capacity.or((engine.capacity > 0).then_some(engine.capacity));
    if let Some(raw_capacity) = raw_capacity {
        vehicle.capacity = crate::cargo_spec::apply_cargo_capacity_multiplier(
            raw_capacity,
            &state.cargo_spec_catalog,
            engine.cargo.unwrap_or(crate::cargo::CargoType::Passengers),
        );
    } else if property_capacity.is_none()
        && engine.kind == VehicleKind::Train
        && engine.is_train_engine()
    {
        // Loco sola: capacidad placeholder hasta enganchar vagones.
        vehicle.capacity = crate::vehicle::VEHICLE_CAPACITY;
    }
    if engine.is_wagon() {
        vehicle.cargo_type = engine.cargo;
        let raw_capacity = property_capacity.unwrap_or(engine.capacity);
        vehicle.capacity = crate::cargo_spec::apply_cargo_capacity_multiplier(
            raw_capacity,
            &state.cargo_spec_catalog,
            engine.cargo.unwrap_or(crate::cargo::CargoType::Goods),
        );
    }
    vehicle.build_tick = state.tick.get();
    vehicle.owner = state.active_company;
    if matches!(
        engine.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        vehicle.road_depot_phase = crate::vehicle::RoadDepotPhase::InDepot;
    }
    if engine.kind == VehicleKind::Train
        && let Some(mouth) = crate::depot::rail_depot_mouth_dir(&state.map, depot_pos)
    {
        vehicle.direction = crate::train_movement::train_depot_facing(mouth);
        vehicle.progress = 0;
        vehicle.depot_leave_cleared = false;
    }
    maybe_init_country_airport_fta(state, depot_pos, &mut vehicle);
    state.vehicles.push(vehicle);
    if engine.kind == VehicleKind::Train && engine.is_dual_headed() {
        spawn_dual_headed_rear(state, next_id, depot_pos, &engine);
    }
    if matches!(
        engine.kind,
        VehicleKind::Train | VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        // `AddArticulatedParts` runs immediately after allocating the front
        // vehicle in OpenTTD.  Keep the callback on the real front vehicle so
        // persistent Action2 registers are written back before the consist
        // cache is rebuilt.
        if !engine.is_dual_headed() {
            spawn_newgrf_articulated_parts(state, next_id, &engine);
        }
        if engine.kind == VehicleKind::Train {
            crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
            &mut state.vehicles,
            next_id,
            Some(&state.map),
            &state.engine_catalog,
            &state.cargo_spec_catalog,
            state.freight_trains,
        );
        }
    }
    state.economy.money -= purchase_cost;
    Ok(())
}

fn check_aircraft_heli_depot_compat(
    state: &GameState,
    depot_pos: TileCoord,
    engine_id: u16,
) -> Result<(), CommandError> {
    let heli_engine = crate::engine::engine_in_catalog(&state.engine_catalog, engine_id)
        .map_or_else(
            || crate::engine::aircraft_is_helicopter(engine_id),
            crate::engine::aircraft_is_helicopter_def,
        );
    // `CanVehicleUseStation` / `CmdBuildAircraft`: flags FTA del aeropuerto.
    let spec = state
        .stations
        .iter()
        .find(|s| s.covers_tile(depot_pos))
        .map(|s| s.airport_spec)
        .or_else(|| {
            crate::airport::airport_tile_is_heliport(&state.map, depot_pos)
                .then_some(crate::airport_class::AirportSpecId::Heliport)
        });
    let Some(spec) = spec else {
        return Ok(());
    };
    if !crate::airport_class::airport_allows_aircraft(spec, heli_engine) {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    Ok(())
}

fn maybe_init_country_airport_fta(state: &GameState, depot_pos: TileCoord, vehicle: &mut Vehicle) {
    if vehicle.kind != VehicleKind::Aircraft {
        return;
    }
    let Some(st) = state
        .stations
        .iter()
        .find(|s| crate::airport_fta::station_uses_airport_fta(s) && s.covers_tile(depot_pos))
    else {
        return;
    };
    if let Some(profile) = crate::airport_fta::fta_profile_for_spec(st.airport_spec) {
        crate::airport_fta::init_airport_fta_on_purchase(vehicle, profile.kind);
    }
}

/// Cabina trasera multihead (`AddRearEngineToMultiheadedTrain`).
fn spawn_dual_headed_rear(
    state: &mut GameState,
    front_id: u32,
    depot_pos: TileCoord,
    engine: &crate::engine::EngineDef,
) {
    let rear_id = front_id.saturating_add(1);
    let mut rear = Vehicle::new(rear_id, engine.kind, depot_pos, depot_pos);
    rear.running = false;
    rear.engine_id = Some(engine.id);
    if rear.cargo_type.is_none() {
        rear.cargo_type = engine.cargo;
    }
    rear.unit_length = crate::newgrf_callback::vehicle_unit_length(engine, &mut rear);
    let callback_capacity =
        crate::newgrf_callback::resolve_vehicle_capacity_property_callback(engine, &mut rear);
    let raw_capacity = callback_capacity.or((engine.capacity > 0).then_some(engine.capacity));
    let rear_capacity = raw_capacity.map_or(0, |raw| {
        crate::cargo_spec::apply_cargo_capacity_multiplier(
            raw,
            &state.cargo_spec_catalog,
            rear.cargo_type
                .or(engine.cargo)
                .unwrap_or(crate::CargoType::Passengers),
        )
    });
    rear.capacity = rear_capacity;
    rear.cargo_type = engine.cargo;
    rear.build_tick = state.tick.get();
    rear.owner = state.active_company;
    rear.direction = state
        .vehicles
        .iter()
        .find(|v| v.id == front_id)
        .map_or(crate::DIR_NE, |v| v.direction);
    rear.prev_unit = Some(front_id);
    rear.other_multiheaded_part = Some(front_id);
    rear.depot_leave_cleared = false;
    if let Some(front) = state.vehicles.iter_mut().find(|v| v.id == front_id) {
        front.next_unit = Some(rear_id);
        front.other_multiheaded_part = Some(rear_id);
        if engine.capacity > 0 || rear_capacity > 0 {
            front.capacity = rear_capacity;
            front.cargo_type = engine.cargo;
        }
    }
    state.vehicles.push(rear);
}

/// Materializa las piezas devueltas por `CBID_VEHICLE_ARTIC_ENGINE` al comprar
/// una locomotora `NewGRF`.
///
/// La cadena se resuelve con el motor frontal y el catálogo activo, igual que
/// `AddArticulatedParts` upstream. La orientación espejo se persiste por
/// unidad como `CUSTOM_VEHICLE_SPRITENUM_REVERSED` para que el renderer consulte
/// el grupo de sprites en la dirección invertida.
#[allow(clippy::too_many_lines)]
pub(crate) fn spawn_newgrf_articulated_parts(
    state: &mut GameState,
    front_id: u32,
    front_engine: &crate::engine::EngineDef,
) {
    const MAX_ARTICULATED_PARTS: u8 = 100;

    if !matches!(
        front_engine.kind,
        VehicleKind::Train | VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) || front_engine.newgrf_grfid == 0
        || front_engine.vehicle_callback_mask & (1 << 4) == 0
        || front_engine.newgrf_runtime.is_none()
    {
        return;
    }
    let grf_version = state
        .newgrf_stack
        .iter()
        .find(|entry| entry.grfid == front_engine.newgrf_grfid)
        .map_or(0, |entry| entry.grf_version);
    let Some(mut previous_id) = state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == front_id)
        .map(|vehicle| vehicle.id)
    else {
        return;
    };
    // Durante un reemplazo puede haber vagones comprados detrás de la cabeza;
    // las piezas nuevas se insertan delante de ellos, no los desconectan.
    let original_next = state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == front_id)
        .and_then(|vehicle| vehicle.next_unit);

    for index in 1..MAX_ARTICULATED_PARTS {
        let callback_part = {
            let Some(front) = state.vehicles.iter_mut().find(|v| v.id == front_id) else {
                break;
            };
            crate::newgrf_callback::resolve_vehicle_articulated_part_callback(
                front_engine,
                front,
                index,
                grf_version,
            )
        };
        let Some(crate::newgrf_callback::VehicleArticulatedPart { local_id, mirrored }) =
            callback_part
        else {
            break;
        };
        // El valor puede ser un WORD (GRF v8+); el catálogo conserva la misma
        // anchura para que no se confunda con el sentinel `0xFF`.
        let Some(part_engine) = state
            .engine_catalog
            .iter()
            .find(|candidate| {
                candidate.kind == front_engine.kind
                    && candidate.newgrf_grfid == front_engine.newgrf_grfid
                    && candidate.newgrf_local_id == local_id
            })
            .cloned()
        else {
            // A missing local engine means the GRF reported an invalid chain;
            // OpenTTD aborts materialisation at this point as well.
            break;
        };
        let Some(front_snapshot) = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == front_id)
            .map(|front| {
                (
                    front.pos,
                    front.origin,
                    front.dest,
                    front.owner,
                    front.direction,
                    front.build_tick,
                    front.cargo_type,
                )
            })
        else {
            break;
        };
        let part_id = next_vehicle_id(state);
        let (pos, origin, dest, owner, direction, build_tick, front_cargo_type) = front_snapshot;
        let mut part = Vehicle::new(part_id, front_engine.kind, pos, dest);
        part.running = false;
        part.engine_id = Some(part_engine.id);
        if part.cargo_type.is_none() {
            part.cargo_type = part_engine.cargo.or(front_cargo_type);
        }
        part.origin = origin;
        part.direction = direction;
        part.build_tick = build_tick;
        part.owner = owner;
        part.depot_leave_cleared = false;
        part.newgrf_articulated = true;
        part.newgrf_mirrored = mirrored;
        part.prev_unit = Some(previous_id);
        part.unit_length = crate::newgrf_callback::vehicle_unit_length(&part_engine, &mut part);
        crate::vehicle::init_vehicle_reliability_from_engine(&mut part, &part_engine);
        if matches!(
            front_engine.kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
        ) {
            part.road_depot_phase = crate::vehicle::RoadDepotPhase::InDepot;
            part.road_state = crate::road_movement::RVSB_IN_DEPOT;
        }
        let property_capacity = (part_engine.capacity > 0 || part_engine.cargo.is_some())
            .then(|| {
                crate::newgrf_callback::resolve_vehicle_capacity_property_callback(
                    &part_engine,
                    &mut part,
                )
            })
            .flatten();
        let raw_capacity = property_capacity.unwrap_or(part_engine.capacity);
        if raw_capacity > 0 {
            part.capacity = crate::cargo_spec::apply_cargo_capacity_multiplier(
                raw_capacity,
                &state.cargo_spec_catalog,
                part_engine
                    .cargo
                    .unwrap_or(crate::cargo::CargoType::Passengers),
            );
            part.cargo_type = part_engine.cargo;
        } else {
            part.capacity = 0;
            part.cargo_type = front_cargo_type;
        }
        if let Some(previous) = state
            .vehicles
            .iter_mut()
            .find(|vehicle| vehicle.id == previous_id)
        {
            previous.next_unit = Some(part_id);
        } else {
            break;
        }
        state.vehicles.push(part);
        previous_id = part_id;
    }
    if previous_id != front_id {
        if let Some(last) = state
            .vehicles
            .iter_mut()
            .find(|vehicle| vehicle.id == previous_id)
        {
            last.next_unit = original_next;
        }
        if let Some(next_id) = original_next
            && let Some(next) = state
                .vehicles
                .iter_mut()
                .find(|vehicle| vehicle.id == next_id)
        {
            next.prev_unit = Some(previous_id);
        }
    }
}

/// Obtiene un id libre para una unidad materializada, sin asumir que los ids
/// existentes sean contiguos (los SAV pueden contener huecos).
fn next_vehicle_id(state: &GameState) -> u32 {
    state
        .vehicles
        .iter()
        .map(|vehicle| vehicle.id)
        .max()
        .map_or(1, |id| id.saturating_add(1))
}

pub(super) fn attach_wagon_to_consist(
    state: &mut GameState,
    head_id: u32,
    wagon_id: u32,
) -> Result<(), CommandError> {
    attach_wagon_to_consist_ex(state, head_id, wagon_id, false)
}

fn attach_wagon_to_consist_ex(
    state: &mut GameState,
    head_id: u32,
    wagon_id: u32,
    keep_chain: bool,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, head_id)?;
    require_vehicle_owned_by_active(state, wagon_id)?;
    let head = state
        .vehicles
        .iter()
        .find(|v| v.id == head_id)
        .ok_or(CommandError::VehicleNotFound)?;
    let wagon = state
        .vehicles
        .iter()
        .find(|v| v.id == wagon_id)
        .ok_or(CommandError::VehicleNotFound)?;
    if head.kind != VehicleKind::Train || wagon.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if !head.is_consist_head() {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    let in_depot = |pos| matches!(state.map.get_kind(pos), Some(TileKind::RailDepot));
    if !in_depot(head.pos) || !in_depot(wagon.pos) || head.pos != wagon.pos {
        return Err(CommandError::VehicleNotInDepot);
    }
    let wagon_eng = wagon
        .engine_id
        .and_then(|id| {
            crate::engine::engine_in_catalog(&state.engine_catalog, id)
                .or_else(|| crate::engine::engine_by_id(id))
        })
        .ok_or(CommandError::EngineNotFound)?;
    if !wagon_eng.is_wagon() && wagon.prev_unit.is_some() {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    let attach = if keep_chain {
        crate::train_consist::attach_wagon_chain
    } else {
        crate::train_consist::attach_wagon
    };
    attach(&mut state.vehicles, head_id, wagon_id)
        .map_err(|()| CommandError::VehicleKindNotAllowed)?;
    crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
        &mut state.vehicles,
        head_id,
        Some(&state.map),
        &state.engine_catalog,
        &state.cargo_spec_catalog,
        state.freight_trains,
    );
    Ok(())
}

pub(super) fn detach_consist_unit(state: &mut GameState, unit_id: u32) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, unit_id)?;
    let unit = state
        .vehicles
        .iter()
        .find(|v| v.id == unit_id)
        .ok_or(CommandError::VehicleNotFound)?;
    if unit.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if !matches!(state.map.get_kind(unit.pos), Some(TileKind::RailDepot)) {
        return Err(CommandError::VehicleNotInDepot);
    }
    let old_head = crate::train_consist::consist_head_id(&state.vehicles, unit_id);
    crate::train_consist::detach_unit(&mut state.vehicles, unit_id)
        .map_err(|()| CommandError::VehicleNotFound)?;
    let mut heads = vec![unit_id];
    if let Some(old_head) = old_head {
        heads.push(old_head);
    }
    for head_id in heads {
        if state
            .vehicles
            .iter()
            .any(|vehicle| vehicle.id == head_id && vehicle.is_consist_head())
        {
            crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
                &mut state.vehicles,
                head_id,
                Some(&state.map),
                &state.engine_catalog,
                &state.cargo_spec_catalog,
                state.freight_trains,
            );
        }
    }
    Ok(())
}

pub(super) fn move_rail_vehicle(
    state: &mut GameState,
    head_id: u32,
    unit_id: u32,
    after_id: Option<u32>,
    move_chain: bool,
) -> Result<(), CommandError> {
    // Validar como detach; luego cortar con o sin cola.
    require_vehicle_owned_by_active(state, unit_id)?;
    let unit = state
        .vehicles
        .iter()
        .find(|v| v.id == unit_id)
        .ok_or(CommandError::VehicleNotFound)?;
    if unit.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if !matches!(state.map.get_kind(unit.pos), Some(TileKind::RailDepot)) {
        return Err(CommandError::VehicleNotInDepot);
    }
    let old_head = crate::train_consist::consist_head_id(&state.vehicles, unit_id);
    if move_chain {
        crate::train_consist::detach_unit_keep_tail(&mut state.vehicles, unit_id)
            .map_err(|()| CommandError::VehicleNotFound)?;
    } else {
        crate::train_consist::detach_unit(&mut state.vehicles, unit_id)
            .map_err(|()| CommandError::VehicleNotFound)?;
    }
    let attach_head = after_id
        .and_then(|aid| crate::train_consist::consist_head_id(&state.vehicles, aid))
        .unwrap_or(head_id);
    // Enganchar al final del consist (MVP: no inserta en medio).
    attach_wagon_to_consist_ex(state, attach_head, unit_id, move_chain)?;
    if let Some(old_head) = old_head
        && old_head != attach_head
        && state
            .vehicles
            .iter()
            .any(|vehicle| vehicle.id == old_head && vehicle.is_consist_head())
    {
        crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
            &mut state.vehicles,
            old_head,
            Some(&state.map),
            &state.engine_catalog,
            &state.cargo_spec_catalog,
            state.freight_trains,
        );
    }
    Ok(())
}

pub(super) fn sell_vehicle(state: &mut GameState, vehicle_id: u32) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let owner = vehicle.owner;
    let in_depot = matches!(
        state.map.get_kind(vehicle.pos),
        Some(TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    );
    if !in_depot {
        return Err(CommandError::VehicleNotInDepot);
    }
    let chain = crate::train_consist::sell_chain_ids(&state.vehicles, vehicle_id);
    if chain.is_empty() {
        return Err(CommandError::VehicleNotFound);
    }
    // Si vendemos un vagón del medio, desenganchar primero.
    if chain.len() == 1
        && state
            .vehicles
            .iter()
            .find(|v| v.id == vehicle_id)
            .is_some_and(|v| v.prev_unit.is_some() || v.next_unit.is_some())
    {
        let _ = crate::train_consist::detach_unit(&mut state.vehicles, vehicle_id);
    }
    let mut refund_total = 0_i64;
    for id in &chain {
        if let Some(v) = state.vehicles.iter().find(|x| x.id == *id) {
            refund_total +=
                crate::economy::vehicle_sell_refund_with_catalog(v, &state.engine_catalog);
        }
    }
    state.vehicles.retain(|v| !chain.contains(&v.id));
    // Recalcular cabezas restantes tocadas.
    let heads: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        .map(|v| v.id)
        .collect();
    for hid in heads {
        crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
            &mut state.vehicles,
            hid,
            Some(&state.map),
            &state.engine_catalog,
            &state.cargo_spec_catalog,
            state.freight_trains,
        );
    }
    state.credit_company(owner, refund_total);
    Ok(())
}

pub(super) fn toggle_vehicle_running(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let kind = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .map(|v| v.kind);
    let road_dest = match kind {
        Some(VehicleKind::Bus | VehicleKind::Truck) => state
            .vehicles
            .iter()
            .find(|v| v.id == vehicle_id)
            .and_then(|v| road_depot_exit_tile(state, v.pos))
            .and_then(|exit| {
                farthest_reachable_tile(&state.map, exit, PathNetwork::Road).or(Some(exit))
            }),
        Some(VehicleKind::Tram) => {
            if let Some(v) = state.vehicles.iter().find(|v| v.id == vehicle_id)
                && let Some(exit) = road_depot_exit_tile(state, v.pos)
            {
                ensure_tram_bits_for_depot_exit(state, v.pos, exit);
                farthest_reachable_tile(&state.map, exit, PathNetwork::Tram)
            } else {
                None
            }
        }
        _ => None,
    };
    let depot_mouth = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .filter(|v| v.kind == VehicleKind::Train)
        .and_then(|v| crate::depot::rail_depot_mouth_dir(&state.map, v.pos));
    let (was_running, vehicle_pos, vehicle_kind, now_running) = {
        let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
            return Err(CommandError::VehicleNotFound);
        };
        let was_running = vehicle.running;
        let vehicle_pos = vehicle.pos;
        let vehicle_kind = vehicle.kind;
        vehicle.running = !vehicle.running;
        let now_running = vehicle.running;
        if now_running && let Some(mouth) = depot_mouth {
            vehicle.direction = crate::train_movement::train_depot_facing(mouth);
            vehicle.progress = 0;
            vehicle.depart_turn = 0;
            vehicle.wait_counter = 0;
            vehicle.depot_leave_cleared = false;
            vehicle.pbs_stuck = false;
        }
        if now_running
            && vehicle.pos == vehicle.dest
            && let Some(dest) = road_dest
        {
            vehicle.dest = dest;
            vehicle.path.clear();
        }
        if now_running
            && matches!(
                vehicle.kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            )
            && state.map.get_kind(vehicle.pos) == Some(crate::TileKind::RoadDepot)
            && matches!(
                vehicle.road_depot_phase,
                crate::vehicle::RoadDepotPhase::None
            )
        {
            vehicle.road_depot_phase = crate::vehicle::RoadDepotPhase::InDepot;
            vehicle.progress = 0;
        }
        (was_running, vehicle_pos, vehicle_kind, now_running)
    };
    if now_running && !was_running && !state.news_first_vehicle_running_sent {
        state.news_first_vehicle_running_sent = true;
        crate::news::push_first_vehicle_running_news(state, vehicle_id, vehicle_pos, vehicle_kind);
    }
    Ok(())
}

fn road_depot_exit_tile(state: &GameState, depot_pos: TileCoord) -> Option<TileCoord> {
    let tile = state.map.get(depot_pos)?;
    if tile.kind != TileKind::RoadDepot {
        return None;
    }
    if let Some((exit, _)) = road_depot_exit_for_dir(&state.map, depot_pos, tile.m5 & 0x03)
        && traversable_road_kind(state.map.get_kind(exit))
    {
        return Some(exit);
    }
    let (mw, mh) = state.map.dimensions();
    [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .map(|(dx, dy)| TileCoord::new(depot_pos.x + dx, depot_pos.y + dy))
        .find(|c| {
            c.x >= 0
                && c.y >= 0
                && c.x < mw.cast_signed()
                && c.y < mh.cast_signed()
                && traversable_road_kind(state.map.get_kind(*c))
        })
}

fn traversable_road_kind(kind: Option<TileKind>) -> bool {
    matches!(
        kind,
        Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadTunnel)
    )
}

/// Asegura overlay de tranvía en depósito y boca al salir (copia bits de carretera si falta).
fn ensure_tram_bits_for_depot_exit(state: &mut GameState, depot: TileCoord, exit: TileCoord) {
    use crate::road_type::{
        RoadType, set_tram_road_type_on_tile, set_tram_track_bits_on_tile, tram_track_bits,
    };
    let Some(exit_tile) = state.map.get(exit) else {
        return;
    };
    if tram_track_bits(&exit_tile) == 0 {
        let road_bits = exit_tile.m5 & 0x0F;
        let bits = if road_bits == 0 { 0x0F } else { road_bits };
        let mut t = exit_tile;
        t = set_tram_track_bits_on_tile(t, bits);
        t = set_tram_road_type_on_tile(t, Some(RoadType::Tram));
        let _ = state.map.set_tile(exit, t);
    }
    let Some(depot_tile) = state.map.get(depot) else {
        return;
    };
    if tram_track_bits(&depot_tile) == 0 {
        let road_bits = depot_tile.m5 & 0x0F;
        let bits = if road_bits == 0 { 0x0F } else { road_bits };
        let mut t = depot_tile;
        t = set_tram_track_bits_on_tile(t, bits);
        t = set_tram_road_type_on_tile(t, Some(RoadType::Tram));
        let _ = state.map.set_tile(depot, t);
    }
}

pub(super) fn clone_vehicle_orders(
    state: &mut GameState,
    from_vehicle_id: u32,
    to_vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, from_vehicle_id)?;
    require_vehicle_owned_by_active(state, to_vehicle_id)?;
    let Some(src_idx) = state.vehicles.iter().position(|v| v.id == from_vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let Some(dst_idx) = state.vehicles.iter().position(|v| v.id == to_vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let src_orders = state.vehicles[src_idx].orders.clone();
    state.vehicles[dst_idx].set_vehicle_orders(src_orders);
    Ok(())
}

/// Compra una copia del vehículo origen (motor + órdenes) en el mismo depósito.
pub(super) fn clone_vehicle_at_depot(
    state: &mut GameState,
    source_vehicle_id: u32,
    depot_pos: TileCoord,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, source_vehicle_id)?;
    let (engine_id, orders) = {
        let Some(source) = state.vehicles.iter().find(|v| v.id == source_vehicle_id) else {
            return Err(CommandError::VehicleNotFound);
        };
        if source.pos != depot_pos {
            return Err(CommandError::VehicleNotInDepot);
        }
        (
            source
                .engine_id
                .unwrap_or_else(|| crate::engine::default_engine_id(source.kind)),
            source.orders.clone(),
        )
    };
    build_vehicle_at_depot(state, depot_pos, engine_id)?;
    let Some(new_vehicle) = state.vehicles.last_mut() else {
        return Err(CommandError::VehicleNotFound);
    };
    new_vehicle.set_vehicle_orders(orders);
    Ok(())
}

/// Vende todos los vehículos en la tesela de depósito.
pub(super) fn sell_all_vehicles_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
) -> Result<(), CommandError> {
    let owner = state.active_company;
    let ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.pos == depot_pos && v.owner == owner)
        .map(|v| v.id)
        .collect();
    for id in ids {
        sell_vehicle(state, id)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(super) fn refit_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
    cargo: crate::cargo::CargoType,
    unit_ids: &[u32],
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let targets: Vec<u32> = if unit_ids.is_empty() {
        vec![vehicle_id]
    } else {
        unit_ids.to_vec()
    };
    for &tid in &targets {
        require_vehicle_owned_by_active(state, tid)?;
        if tid != vehicle_id && !crate::same_consist(&state.vehicles, vehicle_id, tid) {
            return Err(CommandError::RefitNotAllowed);
        }
    }
    // `CMD_REFIT_VEHICLE` cobra por unidad antes de mutar el consist. El
    // callback `0x15E` se evalúa con el cargo/subtipo anterior; una consulta
    // fallida conserva el factor Action0 y los motores vanilla siguen siendo
    // gratuitos (`refit_cost = 0`).
    let mut refit_cost = 0_i64;
    for &tid in &targets {
        let Some(vehicle) = state.vehicles.iter().find(|v| v.id == tid) else {
            return Err(CommandError::VehicleNotFound);
        };
        if vehicle.cargo > 0 {
            return Err(CommandError::RefitNotAllowed);
        }
        if !crate::refit::vehicle_in_depot(&state.map, vehicle.pos) {
            return Err(CommandError::RefitNotAllowed);
        }
        let allowed = vehicle
            .engine_id
            .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
            .map_or_else(
                || {
                    crate::refit::refittable_cargo_types_with_catalog_and_climate(
                        vehicle,
                        &state.engine_catalog,
                        &state.cargo_spec_catalog,
                        state.climate,
                    )
                },
                |engine| {
                    crate::refit::refittable_cargo_types_for_engine_with_catalog_and_climate(
                        engine,
                        &state.cargo_spec_catalog,
                        state.climate,
                    )
                },
            );
        if !allowed.contains(&cargo) {
            return Err(CommandError::RefitNotAllowed);
        }
    }

    for &tid in &targets {
        let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == tid) else {
            return Err(CommandError::VehicleNotFound);
        };
        let engine = state.vehicles[vehicle_idx]
            .engine_id
            .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
            .cloned()
            .or_else(|| {
                state.vehicles[vehicle_idx]
                    .engine_id
                    .and_then(crate::engine::engine_by_id)
                    .cloned()
            });
        if let Some(engine) = engine {
            let subtype = state.vehicles[vehicle_idx].cargo_subtype;
            let (cost, _auto_refit_allowed) = crate::economy::vehicle_refit_cost_with_callbacks(
                &state.global_economy,
                &engine,
                &mut state.vehicles[vehicle_idx],
                cargo,
                subtype,
                state.climate,
                &state.cargo_spec_catalog,
            );
            refit_cost = refit_cost.saturating_add(cost);
        }
    }
    if refit_cost > state.economy.money {
        return Err(CommandError::InsufficientFunds);
    }

    for &tid in &targets {
        let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == tid) else {
            return Err(CommandError::VehicleNotFound);
        };
        let engine = vehicle
            .engine_id
            .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
            .cloned();
        // CB36 se evalúa con el cargo objetivo, igual que DetermineCapacity.
        vehicle.cargo_type = Some(cargo);
        let callback_capacity = engine.as_ref().and_then(|engine| {
            crate::newgrf_callback::resolve_vehicle_refit_capacity_callback(engine, vehicle, cargo)
        });
        let property_capacity = engine.as_ref().and_then(|engine| {
            crate::newgrf_callback::resolve_vehicle_capacity_property_callback(engine, vehicle)
                .map(|capacity| {
                    crate::cargo_spec::apply_cargo_capacity_multiplier(
                        capacity,
                        &state.cargo_spec_catalog,
                        cargo,
                    )
                })
                .or_else(|| {
                    (engine.capacity > 0).then(|| {
                        crate::cargo_spec::apply_cargo_capacity_multiplier(
                            engine.capacity,
                            &state.cargo_spec_catalog,
                            cargo,
                        )
                    })
                })
        });
        if let Some(capacity) = callback_capacity.or(property_capacity) {
            vehicle.capacity = capacity;
        }
        if let Some(engine) = engine.as_ref() {
            vehicle.unit_length = crate::newgrf_callback::vehicle_unit_length(engine, vehicle);
        }
    }
    state.economy.money -= refit_cost;
    Ok(())
}

pub(super) fn cycle_vehicle_order_depot_refit(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.runtime.fleet_index.slot(vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if index >= state.vehicles[vehicle_idx].orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let engine_id = state.vehicles[vehicle_idx].engine_id;
    let options = engine_id
        .and_then(|id| crate::engine::engine_in_catalog(&state.engine_catalog, id))
        .map_or_else(
            || {
                crate::refit::refittable_cargo_types_with_catalog_and_climate(
                    &state.vehicles[vehicle_idx],
                    &state.engine_catalog,
                    &state.cargo_spec_catalog,
                    state.climate,
                )
            },
            |engine| {
                crate::refit::refittable_cargo_types_for_engine_with_catalog_and_climate(
                    engine,
                    &state.cargo_spec_catalog,
                    state.climate,
                )
            },
        );
    let Some(updated) = state.vehicles[vehicle_idx].orders[index].with_cycled_depot_refit(&options)
    else {
        return Err(CommandError::OrderIndexOutOfRange);
    };
    state.vehicles[vehicle_idx].orders[index] = updated;
    Ok(())
}

pub(super) fn remove_vehicle_order_at(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders.remove(index);
    if vehicle.orders.is_empty() {
        vehicle.current_order = 0;
        vehicle.dest = vehicle.pos;
        vehicle.path.clear();
        vehicle.no_network_route_to_order = false;
        return Ok(());
    }
    if vehicle.current_order > index {
        vehicle.current_order -= 1;
    } else if vehicle.current_order == index {
        vehicle.current_order = vehicle.current_order.min(vehicle.orders.len() - 1);
    }
    vehicle.path.clear();
    vehicle.depart_turn = 0;
    vehicle.no_network_route_to_order = false;
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn skip_vehicle_order(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if vehicle.orders.is_empty() {
        return Ok(());
    }
    vehicle.path.clear();
    vehicle.depart_turn = 0;
    vehicle.no_network_route_to_order = false;
    vehicle.progress = 0;
    vehicle.current_order = (vehicle.current_order + 1) % vehicle.orders.len();
    vehicle.origin = vehicle.pos;
    vehicle.sync_order_destination(&state.map);
    Ok(())
}

pub(super) fn toggle_vehicle_order_full_load(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(
        state,
        vehicle_id,
        index,
        VehicleOrder::with_toggled_full_load,
    )
}

pub(super) fn toggle_vehicle_order_no_unload(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(
        state,
        vehicle_id,
        index,
        VehicleOrder::with_toggled_no_unload,
    )
}

fn toggle_vehicle_order_flag(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    toggle: impl FnOnce(VehicleOrder) -> Option<VehicleOrder>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let Some(updated) = toggle(vehicle.orders[index]) else {
        return Err(CommandError::OrderFlagNotApplicable);
    };
    vehicle.orders[index] = updated;
    if index == vehicle.current_order {
        vehicle.sync_order_destination(&state.map);
    }
    Ok(())
}

pub(super) fn append_goto_nearest_depot(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.runtime.fleet_index.slot(vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let (kind, pos) = {
        let v = &state.vehicles[vehicle_idx];
        (v.kind, v.pos)
    };
    let Some(depot) = nearest_depot_tile_indexed(
        &state.map,
        pos,
        kind,
        &mut state.runtime.depot_spatial_index,
    ) else {
        return Err(CommandError::DepotNotFound);
    };
    in_bounds(&state.map, depot)?;
    let vehicle = &mut state.vehicles[vehicle_idx];
    vehicle.append_order(VehicleOrder::depot(depot), &state.map);
    Ok(())
}

pub(super) fn rename_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
    name: Option<String>,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let normalized = name.and_then(|n| {
        let trimmed = n.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    if normalized
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_VEHICLE_NAME_CHARS)
    {
        return Err(CommandError::VehicleNameTooLong);
    }
    vehicle.name = normalized;
    Ok(())
}

pub(super) fn set_depot_vehicles_running(
    state: &mut GameState,
    depot_pos: TileCoord,
    running: bool,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let kind = state.map.get_kind(depot_pos);
    if !matches!(
        kind,
        Some(TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    ) {
        return Err(CommandError::InvalidDepotTile);
    }
    let owner = state.active_company;
    let ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.pos == depot_pos && v.owner == owner)
        .map(|v| v.id)
        .collect();
    for id in ids {
        if running && super::vehicle_fleet::can_start_vehicle_from_depot(state, id).is_err() {
            continue;
        }
        let Some(idx) = state.vehicles.iter().position(|v| v.id == id) else {
            continue;
        };
        let vehicle_pos = state.vehicles[idx].pos;
        let was_at_dest = state.vehicles[idx].pos == state.vehicles[idx].dest;
        let depot_mouth = if running && state.vehicles[idx].kind == VehicleKind::Train {
            crate::depot::rail_depot_mouth_dir(&state.map, vehicle_pos)
        } else {
            None
        };
        if state.vehicles[idx].running != running {
            if running && let Some(mouth) = depot_mouth {
                state.vehicles[idx].direction = crate::train_movement::train_depot_facing(mouth);
                state.vehicles[idx].progress = 0;
                state.vehicles[idx].depart_turn = 0;
                state.vehicles[idx].wait_counter = 0;
                state.vehicles[idx].depot_leave_cleared = false;
                state.vehicles[idx].pbs_stuck = false;
            }
            state.vehicles[idx].running = running;
            if running
                && was_at_dest
                && let Some(dest) = match state.vehicles[idx].kind {
                    VehicleKind::Tram => {
                        road_depot_exit_tile(state, vehicle_pos).and_then(|exit| {
                            ensure_tram_bits_for_depot_exit(state, vehicle_pos, exit);
                            farthest_reachable_tile(&state.map, exit, PathNetwork::Tram)
                        })
                    }
                    VehicleKind::Bus | VehicleKind::Truck => {
                        road_depot_exit_tile(state, vehicle_pos).and_then(|exit| {
                            farthest_reachable_tile(&state.map, exit, PathNetwork::Road)
                                .or(Some(exit))
                        })
                    }
                    _ => None,
                }
            {
                state.vehicles[idx].dest = dest;
                state.vehicles[idx].path.clear();
            }
        }
    }
    Ok(())
}

pub(super) fn move_vehicle_order(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    direction: OrderMoveDirection,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let other = match direction {
        OrderMoveDirection::Up if index > 0 => index - 1,
        OrderMoveDirection::Down if index + 1 < vehicle.orders.len() => index + 1,
        _ => return Err(CommandError::OrderIndexOutOfRange),
    };
    vehicle.orders.swap(index, other);
    if vehicle.current_order == index {
        vehicle.current_order = other;
    } else if vehicle.current_order == other {
        vehicle.current_order = index;
    }
    Ok(())
}

pub(super) fn toggle_vehicle_order_depot_stop(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(
        state,
        vehicle_id,
        index,
        VehicleOrder::with_toggled_depot_stop,
    )
}

pub(super) fn toggle_vehicle_order_depot_unbunch(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    // Solo una orden unbunch por lista (OpenTTD `STR_ERROR_UNBUNCHING_ONLY_ONE_ALLOWED`).
    let enabling = !vehicle.orders[index].depot_unbunch();
    if enabling
        && vehicle
            .orders
            .iter()
            .enumerate()
            .any(|(i, o)| i != index && o.depot_unbunch())
    {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let Some(updated) = vehicle.orders[index].with_toggled_depot_unbunch() else {
        return Err(CommandError::OrderIndexOutOfRange);
    };
    vehicle.orders[index] = updated;
    Ok(())
}

pub(super) fn set_vehicle_order_max_speed(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
    max_speed: u16,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    let order = vehicle.orders[index];
    if order.max_speed_limit() == 0
        && max_speed == 0
        && !matches!(
            order,
            VehicleOrder::Station { .. } | VehicleOrder::Waypoint { .. }
        )
    {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    if !matches!(
        order,
        VehicleOrder::Station { .. } | VehicleOrder::Waypoint { .. }
    ) {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders[index] = order.with_max_speed(max_speed);
    Ok(())
}

pub(super) fn turn_around_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if state.vehicles[idx].kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    if state.vehicles[idx].breakdown_ctr != 0 {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    // `CmdReverseTrainDirection`: velocidad a 0 + liberar reserva delante.
    state.vehicles[idx].cur_speed = 0;
    state.vehicles[idx].force_proceed = false;
    state.vehicles[idx].reverse_heading();
    state.vehicles[idx].path.clear();
    state.vehicles[idx].reserved_steps.clear();
    state.vehicles[idx].wait_counter = 0;
    state.vehicles[idx].pbs_stuck = false;
    state.vehicles[idx].no_network_route_to_order = false;
    state.vehicles[idx].sync_order_destination(&state.map);
    crate::rail_pbs::update_train_reservations(&state.map, &mut state.vehicles);
    if crate::rail_pbs::train_waiting_for_pbs_path(&state.map, &state.vehicles[idx]) {
        state.vehicles[idx].pbs_stuck = true;
        crate::news::push_vehicle_advice_news(
            state,
            vehicle_id,
            state.vehicles[idx].current_order,
            state.vehicles[idx].pos,
            crate::news::VehicleAdviceKind::PbsStuck,
        );
    }
    Ok(())
}

pub(super) fn force_vehicle_proceed(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if vehicle.kind != VehicleKind::Train {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    vehicle.force_proceed = true;
    Ok(())
}

pub(super) fn toggle_vehicle_timetable(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.timetable_active = !vehicle.timetable_active;
    if !vehicle.timetable_active {
        vehicle.timetable_wait_remaining = 0;
        vehicle.timetable_wait_kind = crate::vehicle::TimetableWaitKind::None;
    }
    Ok(())
}

pub(super) fn cycle_vehicle_order_wait(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    toggle_vehicle_order_flag(state, vehicle_id, index, VehicleOrder::with_cycled_wait)
}

pub(super) fn cycle_vehicle_order_travel(
    state: &mut GameState,
    vehicle_id: u32,
    index: usize,
) -> Result<(), CommandError> {
    require_vehicle_owned_by_active(state, vehicle_id)?;
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle = &mut state.vehicles[vehicle_idx];
    if index >= vehicle.orders.len() {
        return Err(CommandError::OrderIndexOutOfRange);
    }
    vehicle.orders[index] = vehicle.orders[index].with_cycled_travel();
    Ok(())
}

pub(super) fn set_autoreplace_rule(
    state: &mut GameState,
    from_engine_id: u16,
    to_engine_id: u16,
) -> Result<(), CommandError> {
    if from_engine_id == to_engine_id {
        return Err(CommandError::AutoreplaceNotAllowed);
    }
    let Some(from) = crate::engine::engine_by_id(from_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    let Some(to) = crate::engine::engine_by_id(to_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    if from.kind != to.kind {
        return Err(CommandError::AutoreplaceNotAllowed);
    }
    if let Some(rule) = state.autoreplace_rules.iter_mut().find(|r| {
        r.from_engine_id == from_engine_id
            && r.owner.unwrap_or(crate::CompanyId::PLAYER) == state.active_company
    }) {
        rule.to_engine_id = to_engine_id;
        rule.enabled = true;
    } else {
        state
            .autoreplace_rules
            .push(crate::autoreplace::AutoReplaceRule::new_for_company(
                from_engine_id,
                to_engine_id,
                state.active_company,
            ));
    }
    Ok(())
}

pub(super) fn clear_autoreplace_rule(
    state: &mut GameState,
    from_engine_id: u16,
) -> Result<(), CommandError> {
    let len_before = state.autoreplace_rules.len();
    state.autoreplace_rules.retain(|r| {
        r.from_engine_id != from_engine_id
            || r.owner.unwrap_or(crate::CompanyId::PLAYER) != state.active_company
    });
    if state.autoreplace_rules.len() == len_before {
        return Err(CommandError::AutoReplaceRuleNotFound);
    }
    Ok(())
}

pub(super) fn toggle_autoreplace_rule(
    state: &mut GameState,
    from_engine_id: u16,
) -> Result<(), CommandError> {
    let Some(rule) = state.autoreplace_rules.iter_mut().find(|r| {
        r.from_engine_id == from_engine_id
            && r.owner.unwrap_or(crate::CompanyId::PLAYER) == state.active_company
    }) else {
        return Err(CommandError::AutoReplaceRuleNotFound);
    };
    rule.enabled = !rule.enabled;
    Ok(())
}
