use crate::vehicle::VehicleKind;
use crate::{CargoType, GameState, STATION_COVERAGE_RADIUS, TileCoord, economy, station, town};

#[allow(clippy::too_many_lines)]
pub(super) fn unload_vehicles(
    state: &mut GameState,
    tick: u64,
    loaded_this_tick: &[bool],
    unloaded_this_tick: &mut [bool],
) {
    for (i, loaded_flag) in loaded_this_tick
        .iter()
        .enumerate()
        .take(state.vehicles.len())
    {
        if *loaded_flag {
            continue;
        }
        state.vehicles[i].ensure_packets_from_legacy();
        let vpos = state.vehicles[i].pos;
        let vcargo = state.vehicles[i].cargo;
        let vcargo_type = state.vehicles[i].cargo_type;
        let Some(station_idx) = station_index_at_vehicle(state, &state.vehicles[i]) else {
            continue;
        };
        if vcargo == 0 {
            continue;
        }
        if !vehicle_should_unload_at_station(&state.vehicles[i], state) {
            continue;
        }
        let cargo_type = vcargo_type.unwrap_or(CargoType::Goods);
        let Some(st) = state.stations.get(station_idx) else {
            continue;
        };
        if !station_matches_current_order(&state.vehicles[i], st.pos) {
            continue;
        }
        let station_pos = st.pos;
        if !st.accepts_cargo(cargo_type) || !st.can_service_vehicle(state.vehicles[i].kind) {
            continue;
        }
        // CargoDist: no bajar si `next_hop` apunta a otra estación.
        let reinsert = !cargo_type.is_town_cargo();
        if state.vehicles[i].cargo_packets.packets.iter().all(|p| {
            crate::cargo_packet::decide_cargo_unload_action(p, station_pos, reinsert)
                == crate::cargo_packet::CargoUnloadAction::Keep
        }) {
            continue;
        }

        let speed = crate::cargo_packet::load_unload_speed(cargo_type);
        let taken = state.vehicles[i].cargo_packets.take_amount(speed);
        if taken.is_empty() {
            continue;
        }
        let unload_units: u32 = taken.iter().map(|p| u32::from(p.count)).sum();
        let vehicle_owner = state.vehicles[i].owner;
        if let Some(from) = state.vehicles[i].last_pickup_station {
            let capacity = state.vehicles[i].capacity.max(unload_units);
            let travel_time = state.vehicles[i]
                .last_depart_tick
                .map(|depart| state.tick.get().saturating_sub(depart))
                .and_then(|t| u32::try_from(t).ok())
                .unwrap_or(0);
            state.link_graph.record_trip(
                from,
                station_pos,
                cargo_type,
                unload_units,
                capacity,
                travel_time,
            );
            state.rebuild_station_flows();
        }
        let mut payment = 0_i64;
        let mut feeder_total = 0_i64;
        let mut feeder_income_by_owner: Vec<(crate::company::CompanyId, i64)> = Vec::new();
        let mut taken = taken;
        for packet in &mut taken {
            let distance = economy::manhattan_distance(packet.source, station_pos);
            let mut part = economy::transported_goods_income(
                u32::from(packet.count),
                distance,
                packet.periods_in_transit,
                packet.cargo,
                tick,
            );
            let _ = crate::subsidy::try_award_subsidy(
                state,
                station_pos,
                packet.cargo,
                packet.source,
                vehicle_owner,
            );
            part = part.saturating_mul(crate::subsidy::delivery_income_multiplier(
                state,
                station_pos,
                packet.cargo,
                packet.source,
                vehicle_owner,
            ));
            // Feeder: 75 % al owner de first_station si es distinta del destino.
            if !packet.feeder_paid
                && let Some(first) = packet.first_station
                && first != station_pos
            {
                let share = crate::company::feeder_share_of(part);
                if share > 0
                    && let Some(feeder_st) = state.stations.iter().find(|s| s.pos == first)
                {
                    let feeder_owner = feeder_st.owner;
                    state.credit_company(feeder_owner, share);
                    feeder_total = feeder_total.saturating_add(share);
                    if let Some((_, acc)) = feeder_income_by_owner
                        .iter_mut()
                        .find(|(id, _)| *id == feeder_owner)
                    {
                        *acc = acc.saturating_add(share);
                    } else {
                        feeder_income_by_owner.push((feeder_owner, share));
                    }
                    part = part.saturating_sub(share);
                    packet.feeder_share = packet.feeder_share.saturating_add(share);
                    packet.feeder_paid = true;
                }
            }
            payment = payment.saturating_add(part);
        }

        let town_cargo = cargo_type.is_town_cargo();
        if town_cargo {
            town::record_delivery_near_town(
                &mut state.towns,
                station_pos,
                cargo_type,
                unload_units,
            );
        }
        if !town_cargo {
            // Trasbordo: limpiar next_hop; el siguiente vehículo lo vuelve a fijar.
            let mut taken = taken;
            for p in &mut taken {
                p.next_hop = None;
            }
            state.stations[station_idx].push_waiting_packets(taken);
        }
        state.stations[station_idx].income += payment.cast_unsigned();
        state.credit_company(vehicle_owner, payment);
        let shown = payment.saturating_add(feeder_total);
        state.stats.cargo_income_earned += shown.cast_unsigned();
        if let Some(c) = state.companies.get_mut(vehicle_owner.index()) {
            c.cargo_income_earned += payment.cast_unsigned();
        }
        let profit_vehicle_id = state.vehicles[i].id;
        let head_id =
            crate::consist_head_id(&state.vehicles, profit_vehicle_id).unwrap_or(profit_vehicle_id);
        if let Some(head) = state.vehicles.iter_mut().find(|v| v.id == head_id) {
            head.profit_this_year = head.profit_this_year.saturating_add(payment);
        }
        for (fo, share) in feeder_income_by_owner {
            if let Some(c) = state.companies.get_mut(fo.index()) {
                c.cargo_income_earned += share.cast_unsigned();
            }
        }
        state
            .runtime
            .pending_income_popups
            .push(crate::IncomePopup {
                amount: payment,
                at: vpos,
            });
        state
            .runtime
            .pending_sim_events
            .push(crate::sim_events::SimEvent::Income {
                amount: payment,
                at: vpos,
            });
        let first_chunk = !state.vehicles[i].cargo_unloading;
        let first_delivery = state.stats.cargo_deliveries == 0 && first_chunk;
        if first_chunk {
            crate::news::push_cargo_delivery_news(
                state,
                unload_units,
                cargo_type,
                payment,
                station_pos,
                first_delivery,
            );
            state.stats.cargo_deliveries += 1;
            if let Some(c) = state.companies.get_mut(vehicle_owner.index()) {
                c.cargo_deliveries += 1;
            }
        }
        state.stats.cargo_units_delivered += u64::from(unload_units);
        state.vehicles[i].sync_cargo_from_packets();
        unloaded_this_tick[i] = true;

        if state.vehicles[i].cargo == 0 {
            state.vehicles[i].cargo_unloading = false;
            state.vehicles[i].clear_cargo();
            state.vehicles[i].last_pickup_station = None;
            state.vehicles[i].last_depart_tick = None;
            // Avanzar orden al terminar descarga (road stop o plataforma rail).
            state.vehicles[i].advance_after_unloading();
            state.vehicles[i].sync_order_destination(&state.map);
        } else {
            state.vehicles[i].cargo_unloading = true;
        }
    }
}

pub(super) fn load_vehicles(
    state: &mut GameState,
    loaded_this_tick: &mut [bool],
    unloaded_this_tick: &[bool],
) {
    for (i, loaded_flag) in loaded_this_tick
        .iter_mut()
        .enumerate()
        .take(state.vehicles.len())
    {
        let allow_top_up = state.vehicles[i]
            .orders
            .get(state.vehicles[i].current_order)
            .is_some_and(|o| o.full_load());
        let loading = state.vehicles[i].cargo_loading;
        // Con carga a bordo: solo seguir cargando si full_load o carga gradual.
        if state.vehicles[i].cargo != 0 && !allow_top_up && !loading {
            continue;
        }
        if state.vehicles[i].cargo_unloading {
            // Descarga gradual: no cargar en el mismo tick.
            continue;
        }
        if (allow_top_up || loading) && state.vehicles[i].cargo >= state.vehicles[i].capacity {
            state.vehicles[i].cargo_loading = false;
            continue;
        }
        let vehicle_kind = state.vehicles[i].kind;
        let Some(station_idx) = station_index_for_industry_load(state, &state.vehicles[i]) else {
            if let Some(station_idx) = station_index_at_vehicle(state, &state.vehicles[i]) {
                try_load_at_station(state, i, station_idx, loaded_flag, unloaded_this_tick[i]);
            }
            continue;
        };
        let Some(station) = state.stations.get(station_idx) else {
            continue;
        };
        if !station.can_service_vehicle(vehicle_kind) {
            continue;
        }
        if !station_matches_current_order(&state.vehicles[i], station.pos) {
            continue;
        }
        let physically_at =
            station_index_at_vehicle(state, &state.vehicles[i]) == Some(station_idx);

        if try_load_from_industry(state, i, station_idx, loaded_flag) {
            continue;
        }
        if unloaded_this_tick[i] {
            continue;
        }
        if physically_at {
            try_load_from_station_waiting_cargo(state, i, station_idx, loaded_flag);
        }
    }
}

fn try_load_at_station(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
    unloaded_this_tick: bool,
) {
    let vehicle = &state.vehicles[vehicle_idx];
    let Some(station) = state.stations.get(station_idx) else {
        return;
    };
    if !station.can_service_vehicle(vehicle.kind) {
        return;
    }
    if !station_matches_current_order(vehicle, station.pos) {
        return;
    }
    if try_load_from_industry(state, vehicle_idx, station_idx, loaded_flag) {
        return;
    }
    if unloaded_this_tick {
        return;
    }
    try_load_from_station_waiting_cargo(state, vehicle_idx, station_idx, loaded_flag);
}

fn try_load_from_industry(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
) -> bool {
    state.vehicles[vehicle_idx].ensure_packets_from_legacy();
    let vcap = state.vehicles[vehicle_idx].capacity;
    let room = vcap.saturating_sub(state.vehicles[vehicle_idx].cargo);
    let vcargo_type = state.vehicles[vehicle_idx].cargo_type;
    let station_pos = state.stations[station_idx].pos;

    let Some(ind_idx) = state
        .industries
        .iter()
        .enumerate()
        .filter(|(_, ind)| {
            let output = ind.output_cargo();
            ind.stock > 0
                && vcargo_type.is_none_or(|c| c == output)
                && state.stations[station_idx].accepts_cargo(output)
                && station::industry_in_station_coverage(ind, station_pos, STATION_COVERAGE_RADIUS)
        })
        .min_by_key(|(_, ind)| {
            (ind.pos.x - station_pos.x).unsigned_abs() + (ind.pos.y - station_pos.y).unsigned_abs()
        })
        .map(|(i, _)| i)
    else {
        return false;
    };

    if room == 0 {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
        return true;
    }

    let output = state.industries[ind_idx].output_cargo();
    let speed = crate::cargo_packet::load_unload_speed(output);
    let load = state.industries[ind_idx].stock.min(room).min(speed);
    if load == 0 {
        return false;
    }

    let source = state.industries[ind_idx].pos;
    #[allow(clippy::cast_possible_truncation)]
    let count = load.min(u32::from(u16::MAX)) as u16;
    let mut packet = crate::cargo_packet::CargoPacket::new(output, count, source);
    packet.first_station = Some(station_pos);
    let order_hop = crate::VehicleOrder::next_station_hop(
        &state.vehicles[vehicle_idx].orders,
        state.vehicles[vehicle_idx].current_order,
        station_pos,
    );
    packet.next_hop = crate::flow_stat::resolve_next_hop(
        state.cargo_dist.distribution,
        &state.runtime.station_flows,
        station_pos,
        output,
        station_pos,
        order_hop,
        &mut state.cargo_rng,
    );
    let first_pickup = state.vehicles[vehicle_idx].cargo == 0;
    state.vehicles[vehicle_idx].cargo_packets.push(packet);
    state.vehicles[vehicle_idx].mark_cargo_loaded(source);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    state.vehicles[vehicle_idx].last_pickup_station = Some(station_pos);
    state.vehicles[vehicle_idx].last_depart_tick = Some(state.tick.get());
    state.industries[ind_idx].stock -= u32::from(count);
    state.industries[ind_idx].transported_total = state.industries[ind_idx]
        .transported_total
        .saturating_add(u64::from(count));
    *loaded_flag = true;
    if first_pickup {
        state.stats.cargo_pickups += 1;
    }
    state.stats.cargo_units_loaded += u64::from(count);

    let full = state.vehicles[vehicle_idx].cargo >= vcap;
    let industry_empty = state.industries[ind_idx].stock == 0;
    let full_load = state.vehicles[vehicle_idx]
        .orders
        .get(state.vehicles[vehicle_idx].current_order)
        .is_some_and(|o| o.full_load());
    if full || (!full_load && industry_empty) {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
    } else {
        state.vehicles[vehicle_idx].cargo_loading = true;
    }
    true
}

#[allow(clippy::too_many_lines)] // carga gradual + filtros freight / hub
fn try_load_from_station_waiting_cargo(
    state: &mut GameState,
    vehicle_idx: usize,
    station_idx: usize,
    loaded_flag: &mut bool,
) -> bool {
    state.vehicles[vehicle_idx].ensure_packets_from_legacy();
    state.stations[station_idx].ensure_packets_from_stock();
    let kind = state.vehicles[vehicle_idx].kind;
    let vcap = state.vehicles[vehicle_idx].capacity;
    let room = vcap.saturating_sub(state.vehicles[vehicle_idx].cargo);
    if room == 0 {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
        return true;
    }
    let preferred = state.vehicles[vehicle_idx].cargo_type;
    let stock = state.stations[station_idx].cargo_stock;

    let station_pos = state.stations[station_idx].pos;
    let cargo = match kind {
        VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => {
            preferred.unwrap_or(CargoType::Passengers)
        }
        VehicleKind::Truck | VehicleKind::Train => {
            let Some(cargo) = stock.pick_freight_to_load(preferred) else {
                if state.vehicles[vehicle_idx].cargo_loading {
                    state.vehicles[vehicle_idx].cargo_loading = false;
                    state.vehicles[vehicle_idx].advance_after_loading();
                }
                return false;
            };
            if matches!(
                cargo,
                CargoType::Coal
                    | CargoType::Wood
                    | CargoType::Oil
                    | CargoType::Grain
                    | CargoType::Livestock
                    | CargoType::IronOre
            ) && !station::station_is_freight_pickup_stop(
                &state.map,
                &state.industries,
                station_pos,
                cargo,
            ) && !state.vehicles[vehicle_idx].orders.is_empty()
            {
                return false;
            }
            cargo
        }
        VehicleKind::Ship => {
            if preferred.is_some_and(|c| !c.is_freight()) {
                preferred.unwrap_or(CargoType::Passengers)
            } else if let Some(cargo) = stock.pick_freight_to_load(preferred) {
                cargo
            } else if stock.get(CargoType::Passengers) > 0 {
                CargoType::Passengers
            } else {
                return false;
            }
        }
    };

    if !state.stations[station_idx].accepts_cargo(cargo) {
        return false;
    }

    let available = stock.get(cargo);
    let company = state.vehicles[vehicle_idx].owner;
    let rating =
        station::station_rating_for_company_cargo(&state.stations[station_idx], company, cargo);
    let speed = crate::cargo_packet::load_unload_speed(cargo);
    let mut load = station::load_amount_for_rating(available.min(room).min(speed), rating);
    if load == 0 && available > 0 && rating > 0 {
        load = 1.min(speed).min(room);
    }
    if load == 0 {
        return false;
    }

    let mut taken = state.stations[station_idx].take_waiting_cargo(cargo, load);
    if taken.is_empty() {
        return false;
    }
    // Feeder + next_hop: Manual = órdenes; Asymmetric/Symmetric = FlowStat.
    let order_hop = crate::VehicleOrder::next_station_hop(
        &state.vehicles[vehicle_idx].orders,
        state.vehicles[vehicle_idx].current_order,
        station_pos,
    );
    let distribution = state.cargo_dist.distribution;
    for packet in &mut taken {
        if packet.first_station.is_none() {
            packet.first_station = Some(station_pos);
        }
        let origin = packet.first_station.unwrap_or(station_pos);
        packet.next_hop = crate::flow_stat::resolve_next_hop(
            distribution,
            &state.runtime.station_flows,
            station_pos,
            packet.cargo,
            origin,
            order_hop,
            &mut state.cargo_rng,
        );
    }
    let loaded_units: u32 = taken.iter().map(|p| u32::from(p.count)).sum();
    let first_pickup = state.vehicles[vehicle_idx].cargo == 0;
    state.vehicles[vehicle_idx]
        .cargo_packets
        .append_packets(taken);
    state.vehicles[vehicle_idx].mark_cargo_loaded(station_pos);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    state.vehicles[vehicle_idx].last_pickup_station = Some(station_pos);
    state.vehicles[vehicle_idx].last_depart_tick = Some(state.tick.get());
    station::on_station_cargo_pickup(&mut state.stations[station_idx], cargo, company);
    *loaded_flag = true;
    if first_pickup {
        state.stats.cargo_pickups += 1;
    }
    state.stats.cargo_units_loaded += u64::from(loaded_units);

    let full = state.vehicles[vehicle_idx].cargo >= vcap;
    let station_empty = state.stations[station_idx].cargo_stock.get(cargo) == 0;
    let full_load = state.vehicles[vehicle_idx]
        .orders
        .get(state.vehicles[vehicle_idx].current_order)
        .is_some_and(|o| o.full_load());
    if full || (!full_load && station_empty) {
        state.vehicles[vehicle_idx].cargo_loading = false;
        state.vehicles[vehicle_idx].advance_after_loading();
    } else {
        state.vehicles[vehicle_idx].cargo_loading = true;
    }
    true
}

fn vehicle_should_unload_at_station(vehicle: &crate::Vehicle, state: &GameState) -> bool {
    if vehicle.cargo == 0 {
        return false;
    }
    // En parada física (bahía road o plataforma rail) o descarga gradual en curso.
    let Some(station_idx) = station_index_at_vehicle(state, vehicle) else {
        return false;
    };
    let at_stop = station::vehicle_at_road_stop(&state.map, vehicle)
        || vehicle.cargo_unloading
        || station::vehicle_physically_at_station(
            &state.map,
            vehicle,
            &state.stations[station_idx],
        );
    if !at_stop {
        return false;
    }
    let station_pos = state.stations[station_idx].pos;
    if let Some(cargo) = vehicle.cargo_type {
        // Pax/mail: nunca descargar donde se embarcó.
        // `cargo_source` tras sync apunta a la casa productora, no a la parada;
        // usar `last_pickup_station` (y first_station en decide_unload).
        if cargo.is_town_cargo()
            && (vehicle.last_pickup_station == Some(station_pos)
                || vehicle.cargo_source == Some(station_pos))
        {
            return false;
        }
        // Freight: no descargar en la parada de la orden actual si es de recogida
        // (mina en cobertura). No usar la estación física sola: una entrega
        // cercana a la mina también tendría cobertura.
        if let Some(crate::VehicleOrder::Station { station, .. }) =
            vehicle.orders.get(vehicle.current_order)
            && station::station_is_freight_pickup_stop(
                &state.map,
                &state.industries,
                *station,
                cargo,
            )
        {
            return false;
        }
        // Sin órdenes: no descargar en el origen del lote (carga en hub/industria).
        if vehicle.orders.is_empty() && vehicle.cargo_source == Some(station_pos) {
            return false;
        }
        // Hub de transferencia: acaba de cargar aquí y aún tiene otro destino.
        // (No aplica si `manhattan_to_dest == 0`: entrega en la misma estación
        // que cubría la industria de recogida.)
        if vehicle.orders.is_empty()
            && vehicle.last_pickup_station == Some(station_pos)
            && vehicle.manhattan_to_dest() > 0
        {
            return false;
        }
    }
    !vehicle
        .orders
        .get(vehicle.current_order)
        .is_some_and(|o| o.no_unload())
}

fn station_matches_current_order(vehicle: &crate::Vehicle, station_pos: TileCoord) -> bool {
    let Some(order) = vehicle.orders.get(vehicle.current_order) else {
        return true;
    };
    if matches!(
        order,
        crate::VehicleOrder::Tile(_)
            | crate::VehicleOrder::Waypoint { .. }
            | crate::VehicleOrder::Depot { .. }
    ) {
        return true;
    }
    matches!(
        order,
        crate::VehicleOrder::Station { station, .. } if *station == station_pos
    )
}

fn station_index_at_vehicle(state: &GameState, vehicle: &crate::Vehicle) -> Option<usize> {
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| station::vehicle_physically_at_station(&state.map, vehicle, station))
        .min_by_key(|(_, station)| {
            (station.pos.x - vehicle.pos.x).abs() + (station.pos.y - vehicle.pos.y).abs()
        })
        .map(|(idx, _)| idx)
}

/// Estación válida para cargar desde industria: en la parada o sobre la tesela de la industria.
fn station_index_for_industry_load(state: &GameState, vehicle: &crate::Vehicle) -> Option<usize> {
    if let Some(idx) = station_index_at_vehicle(state, vehicle) {
        return Some(idx);
    }
    let vpos = vehicle.pos;
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| station.can_service_vehicle(vehicle.kind))
        .find(|(_, station)| {
            state.industries.iter().any(|ind| {
                ind.pos == vpos
                    && ind.stock > 0
                    && station::industry_in_station_coverage(
                        ind,
                        station.pos,
                        STATION_COVERAGE_RADIUS,
                    )
            })
        })
        .map(|(idx, _)| idx)
}
