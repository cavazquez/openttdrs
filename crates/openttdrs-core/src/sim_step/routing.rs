use crate::vehicle::VehicleKind;
use crate::{GameState, TileCoord, pathfinder, vehicle_ai};

pub(super) fn drain_signal_globset_now(state: &mut GameState) {
    let wormholes = state.jgr_tunnel_wormholes();
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    crate::rail_signals::drain_signal_globset_with_wormholes(
        &mut state.map,
        &state.vehicles,
        &mut state.runtime.signal_tile_dirty,
        &mut state.runtime.signal_globset,
        wh,
    );
}

pub(super) fn recompute_vehicle_paths(state: &mut GameState) {
    state.runtime.path_cache.begin_tick(state.tick.get());
    let wormholes =
        pathfinder::TunnelWormholes::from_jgr_records(&state.map, &state.jgr_tunnels_from_footer);
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    for i in 0..state.vehicles.len() {
        state.vehicles[i].sync_order_destination(&state.map);
        if state.vehicles[i].orders.is_empty() {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
        if !state.vehicles[i].path.is_empty() {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
        if state.vehicles[i].pos == state.vehicles[i].dest {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
        let from = state.vehicles[i].pos;
        let to = state.vehicles[i].dest;
        let has_orders = !state.vehicles[i].orders.is_empty();
        let net = pathfinder::path_network_for_vehicle(state.vehicles[i].kind);
        let path = if net == pathfinder::PathNetwork::Rail {
            pathfinder::find_rail_path_for_engine(
                &state.map,
                from,
                to,
                wh,
                state.vehicles[i].engine_id,
            )
        } else {
            pathfinder::find_path_cached(
                &state.map,
                &mut state.runtime.path_cache,
                from,
                to,
                net,
                wh,
            )
        };
        match path {
            Some(path) => {
                state.vehicles[i].path = path.into_iter().collect();
                state.vehicles[i].no_network_route_to_order = false;
            }
            None => {
                state.vehicles[i].no_network_route_to_order = has_orders;
            }
        }
    }
}

/// Extiende el camino de vehículos sin órdenes (paridad `OpenTTD`: trenes/barcos
/// siguen la red; carretera elige ramas al azar; aviones van al hangar).
pub(super) fn extend_orderless_vehicle_paths(state: &mut GameState) {
    let wormholes =
        pathfinder::TunnelWormholes::from_jgr_records(&state.map, &state.jgr_tunnels_from_footer);
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    for i in 0..state.vehicles.len() {
        if !state.vehicles[i].running || !state.vehicles[i].orders.is_empty() {
            continue;
        }
        if !state.vehicles[i].path.is_empty() {
            continue;
        }
        let pos = state.vehicles[i].pos;
        let prev = if state.vehicles[i].origin == pos {
            None
        } else {
            Some(state.vehicles[i].origin)
        };
        let preferred = dir_from_vehicle(&state.vehicles[i], prev);
        let id = state.vehicles[i].id;
        let tick = state.tick;

        match state.vehicles[i].kind {
            VehicleKind::Train => {
                if let Some(next) =
                    vehicle_ai::orderless_rail_next(&state.map, pos, prev, preferred, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Ship => {
                if let Some(next) =
                    vehicle_ai::orderless_water_next(&state.map, pos, prev, preferred, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Bus | VehicleKind::Truck => {
                if let Some(next) = vehicle_ai::orderless_road_next(&state.map, pos, prev, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Tram => {
                if let Some(next) = vehicle_ai::orderless_tram_next(&state.map, pos, prev, id, tick)
                {
                    state.vehicles[i].path.push_back(next);
                }
            }
            VehicleKind::Aircraft => {
                if pos != state.vehicles[i].dest {
                    continue;
                }
                let Some(hangar) = vehicle_ai::orderless_aircraft_hangar(&state.map, pos) else {
                    continue;
                };
                if hangar == pos {
                    continue;
                }
                state.vehicles[i].dest = hangar;
                if let Some(path) = pathfinder::find_path_cached(
                    &state.map,
                    &mut state.runtime.path_cache,
                    pos,
                    hangar,
                    pathfinder::PathNetwork::Air,
                    wh,
                ) {
                    state.vehicles[i].path = path.into_iter().collect();
                }
            }
        }
    }
}

pub(super) fn assign_orderless_wander_destinations(state: &mut GameState) {
    // Compat: camiones sin red de carretera siguen usando Manhattan hacia `dest`.
    for i in 0..state.vehicles.len() {
        if !matches!(
            state.vehicles[i].kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
        ) {
            continue;
        }
        if state.vehicles[i].running
            && state.vehicles[i].orders.is_empty()
            && state.vehicles[i].path.is_empty()
            && state.vehicles[i].pos == state.vehicles[i].dest
        {
            let prev = if state.vehicles[i].origin == state.vehicles[i].pos {
                None
            } else {
                Some(state.vehicles[i].origin)
            };
            let has_next = match state.vehicles[i].kind {
                VehicleKind::Tram => vehicle_ai::orderless_tram_next(
                    &state.map,
                    state.vehicles[i].pos,
                    prev,
                    state.vehicles[i].id,
                    state.tick,
                )
                .is_some(),
                _ => vehicle_ai::orderless_road_next(
                    &state.map,
                    state.vehicles[i].pos,
                    prev,
                    state.vehicles[i].id,
                    state.tick,
                )
                .is_some(),
            };
            if !has_next
                && let Some(dest) = vehicle_ai::orderless_wander_destination(
                    &state.map,
                    state.vehicles[i].id,
                    state.vehicles[i].pos,
                    state.vehicles[i].origin,
                    state.tick,
                )
            {
                state.vehicles[i].dest = dest;
            }
        }
    }
}

pub(super) fn dir_from_vehicle(vehicle: &crate::Vehicle, prev: Option<TileCoord>) -> u8 {
    if let Some(previous) = prev
        && let Some(dir) = crate::rail_signals::dir_from_to(previous, vehicle.pos)
    {
        return dir;
    }
    vehicle_ai::vehicle_direction_to_diag(vehicle.direction)
}
