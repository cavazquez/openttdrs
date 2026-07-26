use std::collections::{HashSet, VecDeque};

use crate::vehicle::VehicleKind;
use crate::{GameState, TileCoord, pathfinder, vehicle_ai};

pub(super) fn drain_signal_globset_now(state: &mut GameState) {
    let wormholes = state.jgr_tunnel_wormholes();
    let wh = if wormholes.is_empty() {
        None
    } else {
        Some(&wormholes)
    };
    crate::rail_signals::drain_signal_globset_indexed_with_wormholes(
        &mut state.map,
        &state.vehicles,
        &mut state.runtime.signal_tile_dirty,
        &mut state.runtime.signal_globset,
        &mut state.runtime.signal_spatial_index,
        wh,
    );
}

/// Encola y, si `_globset` llega a 64 entradas, drena de inmediato (`SIG_GLOB_UPDATE`).
pub(super) fn enqueue_signal_glob_flush(state: &mut GameState, tile: TileCoord) {
    crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, tile);
    if crate::rail_signals::signal_globset_needs_flush(&state.runtime.signal_globset) {
        drain_signal_globset_now(state);
    }
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

    // Sincronizar primero todos los destinos permite adjudicar los andenes por
    // cercanía real. El orden del `Vec<Vehicle>` no debe darle prioridad a un
    // tren lejano sobre otro que ya está entrando en la estación.
    for vehicle in &mut state.vehicles {
        vehicle.sync_order_destination(&state.map);
    }
    let mut claimed_platform_tiles = HashSet::new();
    let mut station_route_resolved = vec![false; state.vehicles.len()];
    let mut train_priority: Vec<usize> = state
        .vehicles
        .iter()
        .enumerate()
        .filter(|(_, vehicle)| {
            vehicle.kind == VehicleKind::Train
                && vehicle.is_consist_head()
                && matches!(
                    vehicle.current_order_ref(),
                    Some(crate::vehicle::VehicleOrder::Station { .. })
                )
        })
        .map(|(index, _)| index)
        .collect();
    train_priority.sort_by_key(|&index| {
        let vehicle = &state.vehicles[index];
        (
            vehicle.pos.x.abs_diff(vehicle.dest.x) + vehicle.pos.y.abs_diff(vehicle.dest.y),
            vehicle.id,
        )
    });
    for index in train_priority {
        station_route_resolved[index] =
            route_train_to_available_platform(state, index, wh, &mut claimed_platform_tiles);
    }

    for i in 0..state.vehicles.len() {
        if state.vehicles[i].orders.is_empty() {
            state.vehicles[i].no_network_route_to_order = false;
            continue;
        }
        if station_route_resolved[i] {
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
        if let Some(path) = path {
            state.vehicles[i].path = path.into_iter().collect();
            state.vehicles[i].no_network_route_to_order = false;
        } else {
            // Estación multi-andén: el andén alineado puede no tener ruta
            // (PBS one-way / vía muerta); probar el resto de plataformas.
            let mut routed = false;
            if state.vehicles[i].kind == VehicleKind::Train
                && let Some(crate::vehicle::VehicleOrder::Station { station, .. }) =
                    state.vehicles[i].current_order_ref().copied()
            {
                let candidates =
                    crate::station::rail_station_stop_candidates(&state.map, station, from);
                for alt in candidates {
                    if alt == to {
                        continue;
                    }
                    let alt_path = pathfinder::find_rail_path_for_engine(
                        &state.map,
                        from,
                        alt,
                        wh,
                        state.vehicles[i].engine_id,
                    );
                    if let Some(path) = alt_path {
                        state.vehicles[i].dest = alt;
                        state.vehicles[i].path = path.into_iter().collect();
                        state.vehicles[i].no_network_route_to_order = false;
                        routed = true;
                        break;
                    }
                }
            }
            if !routed {
                state.vehicles[i].no_network_route_to_order = has_orders;
            }
        }
    }
}

/// Asigna un andén libre de forma independiente. Devuelve `true` cuando la
/// orden de estación quedó resuelta (con ruta o esperando un andén), para que
/// el fallback genérico no vuelva a elegir la primera plataforma ocupada.
fn route_train_to_available_platform(
    state: &mut GameState,
    vehicle_idx: usize,
    wormholes: Option<&pathfinder::TunnelWormholes>,
    claimed_platform_tiles: &mut HashSet<TileCoord>,
) -> bool {
    let vehicle = &state.vehicles[vehicle_idx];
    let Some(crate::vehicle::VehicleOrder::Station {
        station,
        stop_location,
        ..
    }) = vehicle.current_order_ref().copied()
    else {
        return false;
    };
    let from = vehicle.pos;
    let candidates = crate::station::rail_station_stop_candidates_osl(
        &state.map,
        station,
        from,
        stop_location,
        vehicle.cached_total_length,
    );
    if candidates.is_empty() {
        state.vehicles[vehicle_idx].no_network_route_to_order = true;
        state.vehicles[vehicle_idx].path.clear();
        return true;
    }

    let no_prior_reservations = HashSet::new();
    let mut occupied_fallback: Option<(TileCoord, Vec<TileCoord>, Vec<TileCoord>)> = None;
    for candidate in candidates {
        let platform =
            crate::station::rail_station_platform_track_tiles(&state.map, station, candidate);
        if platform.is_empty() {
            continue;
        }
        let claimed = platform
            .iter()
            .any(|tile| claimed_platform_tiles.contains(tile));
        let occupied = crate::rail_pbs::platform_track_reserved_or_occupied(
            &state.map,
            &state.vehicles,
            vehicle.id,
            station,
            candidate,
            &no_prior_reservations,
        );
        let path = if from == candidate {
            Some(Vec::new())
        } else {
            pathfinder::find_rail_path_for_engine(
                &state.map,
                from,
                candidate,
                wormholes,
                vehicle.engine_id,
            )
        };
        let Some(path) = path else {
            continue;
        };
        // Un andén adjudicado a otro tren en este mismo tick sigue siendo un
        // destino válido de espera. Quitarle toda ruta al tercer tren (dos
        // andenes ya adjudicados) puede inmovilizarlo en la estación opuesta y
        // provocar un deadlock; las señales/PBS ya forman la cola antes del
        // acceso sin permitir que dos consists ocupen la plataforma.
        if claimed || occupied {
            occupied_fallback.get_or_insert((candidate, path, platform));
            continue;
        }
        claimed_platform_tiles.extend(platform);
        let train = &mut state.vehicles[vehicle_idx];
        train.dest = candidate;
        train.path = VecDeque::from(path);
        train.no_network_route_to_order = false;
        return true;
    }

    // Si ambos andenes están ocupados, avanzar hacia uno sin reservarlo. Las
    // señales lo detendrán antes del acceso si todavía no se liberó; impedir la
    // salida bloquearía dos estaciones llenas para siempre.
    if let Some((candidate, path, _platform)) = occupied_fallback {
        // No marcar como nueva adjudicación un andén que ya está ocupado.
        // En particular, su tren actual debe poder conservar la ruta hasta el
        // punto de parada y luego salir; adjudicárselo a un tren en espera lo
        // dejaba sin path y congelaba toda la cola.
        let train = &mut state.vehicles[vehicle_idx];
        train.dest = candidate;
        train.path = VecDeque::from(path);
        train.no_network_route_to_order = false;
        return true;
    }
    state.vehicles[vehicle_idx].no_network_route_to_order = true;
    state.vehicles[vehicle_idx].path.clear();
    true
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
