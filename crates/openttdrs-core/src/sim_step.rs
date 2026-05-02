use crate::{
    CARGO_DELIVERY_PAYMENT, CargoType, GameState, STATION_COVERAGE_RADIUS, TileCoord, pathfinder,
    station, vehicle_ai,
};

pub(crate) fn step(state: &mut GameState) {
    state.tick.advance();
    let t = state.tick.get();

    produce_industries(state, t);

    let mut loaded_this_tick = vec![false; state.vehicles.len()];
    load_vehicles(state, &mut loaded_this_tick);
    unload_vehicles(state, &loaded_this_tick);
    assign_orderless_wander_destinations(state);
    recompute_vehicle_paths(state);
    move_vehicles(state);
}

fn produce_industries(state: &mut GameState, tick: u64) {
    for industry in &mut state.industries {
        let before = industry.stock;
        industry.produce(tick);
        state.stats.industry_cargo_units_produced +=
            u64::from(industry.stock.saturating_sub(before));
    }
}

fn load_vehicles(state: &mut GameState, loaded_this_tick: &mut [bool]) {
    // Carga: el vehículo toma cargo compatible dentro de la cobertura de una estación cercana.
    for (i, loaded_flag) in loaded_this_tick
        .iter_mut()
        .enumerate()
        .take(state.vehicles.len())
    {
        let vpos = state.vehicles[i].pos;
        let vcap = state.vehicles[i].capacity;
        let vcargo_type = state.vehicles[i].cargo_type;
        let Some(station_idx) = station_index_covering_tile(state, vpos) else {
            continue;
        };
        let Some(station) = state.stations.get(station_idx) else {
            continue;
        };
        let station_pos = station.pos;
        if state.vehicles[i].cargo != 0 {
            continue;
        }
        if !station.can_service_vehicle(state.vehicles[i].kind) {
            continue;
        }
        if let Some(ind) = state.industries.iter_mut().find(|ind| {
            let output = ind.output_cargo();
            ind.stock > 0
                && vcargo_type.is_none_or(|c| c == output)
                && station.accepts_cargo(output)
                && station::industry_in_station_coverage(ind, station_pos, STATION_COVERAGE_RADIUS)
        }) {
            let load = ind.stock.min(vcap);
            state.vehicles[i].cargo_type = Some(ind.output_cargo());
            state.vehicles[i].cargo = load;
            ind.stock -= load;
            if load > 0 {
                *loaded_flag = true;
                state.stats.cargo_pickups += 1;
                state.stats.cargo_units_loaded += u64::from(load);
            }
        }
    }
}

fn unload_vehicles(state: &mut GameState, loaded_this_tick: &[bool]) {
    // Descarga: el vehículo entrega en la estación cuya cobertura pisa.
    for (i, loaded_flag) in loaded_this_tick
        .iter()
        .enumerate()
        .take(state.vehicles.len())
    {
        if *loaded_flag {
            continue;
        }
        let vpos = state.vehicles[i].pos;
        let vcargo = state.vehicles[i].cargo;
        let vcargo_type = state.vehicles[i].cargo_type;
        let Some(station_idx) = station_index_covering_tile(state, vpos) else {
            continue;
        };
        if vcargo == 0 {
            continue;
        }
        let cargo_type = vcargo_type.unwrap_or(CargoType::Goods);
        if let Some(st) = state.stations.get_mut(station_idx) {
            if !st.accepts_cargo(cargo_type) || !st.can_service_vehicle(state.vehicles[i].kind) {
                continue;
            }
            st.stock += vcargo;
            st.cargo_stock.add(cargo_type, vcargo);
            st.income += u64::from(vcargo);
            state.economy.money += i64::from(vcargo) * CARGO_DELIVERY_PAYMENT;
            state.stats.cargo_deliveries += 1;
            state.stats.cargo_units_delivered += u64::from(vcargo);
            state.vehicles[i].cargo = 0;
        }
    }
}

fn assign_orderless_wander_destinations(state: &mut GameState) {
    // Vehículos sin órdenes: pasean por la red en vez de rebotar entre origen/destino.
    for i in 0..state.vehicles.len() {
        if state.vehicles[i].running
            && state.vehicles[i].orders.is_empty()
            && state.vehicles[i].path.is_empty()
            && state.vehicles[i].pos == state.vehicles[i].dest
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

fn recompute_vehicle_paths(state: &mut GameState) {
    // Recomputa el path BFS para vehículos que lo necesiten (path vacío y no en destino).
    for i in 0..state.vehicles.len() {
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
        match pathfinder::find_path(&state.map, from, to, net) {
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

fn move_vehicles(state: &mut GameState) {
    for vehicle in &mut state.vehicles {
        vehicle.step();
    }
}

fn station_index_covering_tile(state: &GameState, tile: TileCoord) -> Option<usize> {
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| {
            station::station_covers_tile(station.pos, tile, STATION_COVERAGE_RADIUS)
        })
        .min_by_key(|(_, station)| (station.pos.x - tile.x).abs() + (station.pos.y - tile.y).abs())
        .map(|(idx, _)| idx)
}
