use crate::vehicle::{OrderUnloadType, VehicleKind};
use crate::{CargoType, GameState, TileCoord, TileKind, economy, station, town};

fn vehicle_load_unload_speed(state: &GameState, vehicle_idx: usize, cargo: CargoType) -> u32 {
    let configured = state
        .vehicles
        .get(vehicle_idx)
        .and_then(|vehicle| vehicle.engine_id)
        .and_then(|engine_id| crate::engine::engine_in_catalog(&state.engine_catalog, engine_id))
        .map_or(0, |engine| engine.load_amount);
    if configured == 0 {
        crate::cargo_packet::load_unload_speed(cargo)
    } else {
        u32::from(configured)
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn unload_vehicles(
    state: &mut GameState,
    _tick: u64,
    loaded_this_tick: &[bool],
    unloaded_this_tick: &mut [bool],
) {
    let mut link_graph_dirty = false;
    for (i, loaded_flag) in loaded_this_tick
        .iter()
        .enumerate()
        .take(state.vehicles.len())
    {
        if *loaded_flag {
            continue;
        }
        let vcargo = state.vehicles[i].cargo;
        if vcargo == 0 {
            continue;
        }
        state.vehicles[i].ensure_packets_from_legacy();
        let vpos = state.vehicles[i].pos;
        let vcargo_type = state.vehicles[i].cargo_type;
        let Some(station_idx) = station_index_at_vehicle(state, &state.vehicles[i]) else {
            continue;
        };
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
        if !st.can_service_vehicle(state.vehicles[i].kind) {
            continue;
        }
        // CargoDist / P2.19: `PrepareUnload` + `Stage` (TRANSFER/DELIVER/KEEP).
        let unload_type = state.vehicles[i]
            .orders
            .get(state.vehicles[i].current_order)
            .map_or(OrderUnloadType::UnloadIfPossible, |o| o.unload_type());
        let cargo_pct = if state.vehicles[i].capacity == 0 {
            0
        } else {
            u8::try_from(
                (u64::from(state.vehicles[i].cargo) * 100 / u64::from(state.vehicles[i].capacity))
                    .min(100),
            )
            .unwrap_or(100)
        };
        let next_stations = crate::VehicleOrder::get_next_stopping_station(
            &state.vehicles[i].orders,
            state.vehicles[i].cur_implicit_order_index,
            station_pos,
            Some(cargo_pct),
        );
        let accepted = state.stations[station_idx].accepts_cargo(cargo_type);
        let will_unload = crate::cargo_packet::prepare_unload(
            &mut state.vehicles[i].cargo_packets,
            accepted,
            station_pos,
            &next_stations,
            unload_type,
        );
        if !will_unload {
            continue;
        }

        let speed = vehicle_load_unload_speed(state, i, cargo_type);
        // Tras `Stage`, transfer/deliver están al frente de la lista.
        let unloadable = state.vehicles[i]
            .cargo_packets
            .staged_transfer
            .saturating_add(state.vehicles[i].cargo_packets.staged_deliver);
        let taken = state.vehicles[i]
            .cargo_packets
            .take_amount(speed.min(unloadable));
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
            link_graph_dirty = true;
        }
        let mut payment = 0_i64;
        let mut feeder_total = 0_i64;
        let mut feeder_income_by_owner: Vec<(crate::company::CompanyId, i64)> = Vec::new();
        let mut taken = taken;
        let mut transfer_mask = Vec::with_capacity(taken.len());
        let mut delivered_units = 0_u32;
        for packet in &mut taken {
            // P3.16: pago por tramos recorridos (`GetDistance`), no Manhattan origen→destino.
            let distance = packet.get_distance(station_pos);
            let pay_spec = crate::cargo_spec::payment_spec_for_cargo_climate(
                packet.cargo,
                &state.cargo_spec_catalog,
                state.climate,
            );
            let part = economy::transported_goods_income_with_spec(
                u32::from(packet.count),
                distance,
                packet.periods_in_transit,
                pay_spec,
                state.global_economy.inflation_payment,
            );
            let _ = crate::subsidy::try_award_subsidy(
                state,
                station_pos,
                packet.cargo,
                packet.source,
                vehicle_owner,
            );
            let part = part.saturating_mul(crate::subsidy::delivery_income_multiplier(
                state,
                station_pos,
                packet.cargo,
                packet.source,
                vehicle_owner,
            ));
            let action = crate::cargo_packet::choose_cargo_action(
                packet,
                station_pos,
                &next_stations,
                unload_type,
                accepted,
            );
            transfer_mask.push(action == crate::cargo_packet::CargoUnloadAction::Transfer);
            match action {
                crate::cargo_packet::CargoUnloadAction::Transfer => {
                    if !packet.feeder_paid
                        && let Some(first) = packet.first_station
                        && first != station_pos
                    {
                        let share = crate::company::feeder_share_of(part);
                        if share > 0 {
                            packet.feeder_share = packet.feeder_share.saturating_add(share);
                        }
                    }
                }
                crate::cargo_packet::CargoUnloadAction::Deliver => {
                    delivered_units = delivered_units.saturating_add(u32::from(packet.count));
                    let mut deliverer_part = part;
                    if !packet.feeder_paid
                        && packet.feeder_share > 0
                        && let Some(first) = packet.first_station
                        && first != station_pos
                        && let Some(feeder_st) = state.stations.iter().find(|s| s.pos == first)
                    {
                        let feeder_owner = feeder_st.owner;
                        let accumulated = packet.feeder_share;
                        state.credit_company(feeder_owner, accumulated);
                        feeder_total = feeder_total.saturating_add(accumulated);
                        if let Some((_, acc)) = feeder_income_by_owner
                            .iter_mut()
                            .find(|(id, _)| *id == feeder_owner)
                        {
                            *acc = acc.saturating_add(accumulated);
                        } else {
                            feeder_income_by_owner.push((feeder_owner, accumulated));
                        }
                        packet.feeder_paid = true;
                        deliverer_part = part.saturating_sub(accumulated);
                    }
                    payment = payment.saturating_add(deliverer_part);
                }
                crate::cargo_packet::CargoUnloadAction::Keep
                | crate::cargo_packet::CargoUnloadAction::Load => {}
            }
        }

        let town_cargo = cargo_type.is_town_cargo();
        if town_cargo && delivered_units > 0 {
            town::record_delivery_near_town(
                &mut state.towns,
                station_pos,
                cargo_type,
                delivered_units,
            );
        }
        let mut reinserted = Vec::new();
        for (mut p, was_transfer) in taken.into_iter().zip(transfer_mask) {
            // El modelo actual usa las estaciones como hub para todo freight;
            // una transferencia forzada de pax/mail también debe quedar allí.
            if !town_cargo || was_transfer {
                p.update_unloading_tile(station_pos);
                p.next_hop = None;
                reinserted.push(p);
            }
        }
        if !reinserted.is_empty() {
            state.stations[station_idx].push_waiting_packets(reinserted);
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
        // Freight baja en una estación como trasbordo: no es una entrega final
        // ni debe disparar una noticia. Transfer pax/mail tampoco entrega.
        let final_delivery = town_cargo && delivered_units > 0;
        if first_chunk && final_delivery {
            crate::news::push_cargo_delivery_news(
                state,
                unload_units,
                cargo_type,
                payment,
                station_pos,
                first_delivery,
            );
        }
        if first_chunk {
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

    // El pipeline Demand + MCF es global y costoso. Todas las descargas del tick
    // mutan primero el link graph; después publicamos un único snapshot coherente
    // para la fase de carga y reencaminamos paquetes una sola vez (#215).
    if link_graph_dirty {
        state.rebuild_station_flows();
    }
}

pub(super) fn load_vehicles(
    state: &mut GameState,
    loaded_this_tick: &mut [bool],
    unloaded_this_tick: &[bool],
) {
    // Si no existe ninguna fuente posible, ningún vehículo puede iniciar una
    // carga este tick. Evitar el barrido de toda la flota es especialmente
    // importante al importar un SAV que todavía no contiene `INDY`/stocks.
    // Un vehículo que ya estaba en carga debe seguir pasando por la lógica
    // normal para cerrar correctamente una orden `full_load` sin mercancía.
    let has_loadable_supply = has_loadable_supply(state);
    if !has_loadable_supply && !state.vehicles.iter().any(|vehicle| vehicle.cargo_loading) {
        return;
    }

    for (i, loaded_flag) in loaded_this_tick
        .iter_mut()
        .enumerate()
        .take(state.vehicles.len())
    {
        let allow_top_up = state.vehicles[i]
            .orders
            .get(state.vehicles[i].current_order)
            .is_some_and(|o| o.is_full_load_order());
        let no_load = state.vehicles[i]
            .orders
            .get(state.vehicles[i].current_order)
            .is_some_and(|o| o.no_load());
        if no_load {
            continue;
        }
        let loading = state.vehicles[i].cargo_loading;
        // Con carga a bordo: solo seguir cargando si full_load o carga gradual.
        if state.vehicles[i].cargo != 0 && !allow_top_up && !loading {
            continue;
        }
        if state.vehicles[i].cargo_unloading {
            // Descarga gradual: no cargar en el mismo tick.
            continue;
        }
        // Una locomotora definida por motor no transporta carga por sí sola:
        // necesita al menos un vagón en su consist. Los trenes genéricos sin
        // engine_id se preservan para escenarios y saves antiguos.
        let locomotive_without_wagon = state.vehicles[i].kind == VehicleKind::Train
            && state.vehicles[i].engine_id.is_some_and(|engine_id| {
                crate::engine::engine_by_id(engine_id)
                    .is_some_and(crate::engine::EngineDef::is_train_engine)
            })
            && {
                let vehicle_id = state.vehicles[i].id;
                state.runtime.fleet_index.slot(vehicle_id).map_or_else(
                    || crate::consist_unit_ids(&state.vehicles, vehicle_id).len(),
                    |_| state.runtime.fleet_index.consist(vehicle_id).len(),
                ) <= 1
            };
        if state.vehicles[i].capacity == 0 || locomotive_without_wagon {
            state.vehicles[i].cargo_loading = false;
            continue;
        }
        if (allow_top_up || loading) && state.vehicles[i].cargo >= state.vehicles[i].capacity {
            state.vehicles[i].cargo_loading = false;
            continue;
        }
        let vehicle_kind = state.vehicles[i].kind;
        let Some(station_idx) = station_index_for_industry_load(state, &state.vehicles[i]) else {
            // `station_index_for_industry_load` ya consulta la estación física
            // antes de contemplar el caso especial de una industria. Repetir
            // `station_index_at_vehicle` aquí hacía una segunda búsqueda para
            // cada vehículo que circula fuera de una estación.
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
        // Con `MoveGoodsToStation` la mina vuelca al andén: el camión en la tesela de la
        // industria carga de la estación que la cubre aunque no esté físicamente en ella.
        if physically_at || station_has_industry_waiting(state, station_idx, &state.vehicles[i]) {
            try_load_from_station_waiting_cargo(state, i, station_idx, loaded_flag);
        }
    }
}

/// Sólo una industria con stock o una estación con paquetes en espera puede
/// iniciar una carga. Las industrias que todavía no produjeron no obligan a
/// visitar toda la flota: el siguiente tick las verá cuando tengan stock.
fn has_loadable_supply(state: &GameState) -> bool {
    state.industries.iter().any(|industry| industry.stock > 0)
        || state
            .stations
            .iter()
            .any(|station| station.cargo_stock != crate::CargoStock::default())
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
                && station::industry_in_station_coverage(
                    ind,
                    station_pos,
                    station::station_catchment_radius(&state.stations[station_idx]),
                )
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
    let speed = vehicle_load_unload_speed(state, vehicle_idx, output);
    let load = state.industries[ind_idx].stock.min(room).min(speed);
    if load == 0 {
        return false;
    }

    let source = state.industries[ind_idx].pos;
    #[allow(clippy::cast_possible_truncation)]
    let count = load.min(u32::from(u16::MAX)) as u16;
    let mut packet = crate::cargo_packet::CargoPacket::new(output, count, source);
    packet.first_station = Some(station_pos);
    let order_hop = crate::VehicleOrder::get_next_stopping_station(
        &state.vehicles[vehicle_idx].orders,
        state.vehicles[vehicle_idx].cur_implicit_order_index,
        station_pos,
        None,
    )
    .into_iter()
    .next();
    packet.next_hop = crate::flow_stat::resolve_next_hop(
        state.cargo_dist.distribution,
        &state.runtime.station_flows,
        station_pos,
        output,
        station_pos,
        order_hop,
        &mut state.random,
    );
    packet.update_loading_tile(station_pos);
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
    let full_load_any = state.vehicles[vehicle_idx]
        .orders
        .get(state.vehicles[vehicle_idx].current_order)
        .is_some_and(|o| o.full_load_any());
    if full || full_load_any && industry_empty || industry_empty && !full_load {
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
            if cargo.is_freight()
                && !matches!(
                    cargo,
                    CargoType::Goods
                        | CargoType::Valuables
                        | CargoType::Steel
                        | CargoType::Paper
                        | CargoType::Food
                        | CargoType::Candy
                        | CargoType::Toys
                        | CargoType::FizzyDrinks
                )
                && !station::station_is_freight_pickup_stop(
                    &state.map,
                    &state.industries,
                    station_pos,
                    cargo,
                )
                && !state.vehicles[vehicle_idx].orders.is_empty()
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

    // Aunque no haya carga, el intento cuenta: desbloquea selectgoods / MoveGoodsToStation.
    let visit = state.vehicles[vehicle_idx].station_visit(state.tick.get());
    station::note_station_load_attempt(&mut state.stations[station_idx], cargo, visit);

    let available = stock.get(cargo);
    let company = state.vehicles[vehicle_idx].owner;
    let rating =
        station::station_rating_for_company_cargo(&state.stations[station_idx], company, cargo);
    let speed = vehicle_load_unload_speed(state, vehicle_idx, cargo);
    let mut load = station::load_amount_for_rating(available.min(room).min(speed), rating);
    if load == 0 && available > 0 && rating > 0 {
        load = 1.min(speed).min(room);
    }
    if load == 0 {
        return false;
    }

    let _ = state.stations[station_idx].cargo_packets.reserve(load);
    let mut taken = state.stations[station_idx].take_waiting_cargo(cargo, load);
    if taken.is_empty() {
        state.stations[station_idx]
            .cargo_packets
            .consume_reserved(load);
        return false;
    }
    // Feeder + next_hop: Manual = órdenes; Asymmetric/Symmetric = FlowStat.
    let order_hop = crate::VehicleOrder::get_next_stopping_station(
        &state.vehicles[vehicle_idx].orders,
        state.vehicles[vehicle_idx].cur_implicit_order_index,
        station_pos,
        None,
    )
    .into_iter()
    .next();
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
            &mut state.random,
        );
        packet.update_loading_tile(station_pos);
    }
    let loaded_units: u32 = taken.iter().map(|p| u32::from(p.count)).sum();
    // P2.20: consumir reserva de la cola indexada por hop.
    state.stations[station_idx]
        .cargo_packets
        .consume_reserved(loaded_units);
    let first_pickup = state.vehicles[vehicle_idx].cargo == 0;
    state.vehicles[vehicle_idx]
        .cargo_packets
        .append_packets(taken);
    state.vehicles[vehicle_idx].mark_cargo_loaded(station_pos);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    state.vehicles[vehicle_idx].last_pickup_station = Some(station_pos);
    state.vehicles[vehicle_idx].last_depart_tick = Some(state.tick.get());
    let visit = state.vehicles[vehicle_idx].station_visit(state.tick.get());
    station::on_station_cargo_pickup(&mut state.stations[station_idx], cargo, company, visit);
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
    let full_load_any = state.vehicles[vehicle_idx]
        .orders
        .get(state.vehicles[vehicle_idx].current_order)
        .is_some_and(|o| o.full_load_any());
    if full || full_load_any && station_empty || station_empty && !full_load {
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
    let unload_type = vehicle
        .orders
        .get(vehicle.current_order)
        .map_or(OrderUnloadType::UnloadIfPossible, |order| {
            order.unload_type()
        });
    if unload_type == OrderUnloadType::NoUnload {
        return false;
    }
    if matches!(
        unload_type,
        OrderUnloadType::Unload | OrderUnloadType::Transfer
    ) {
        return true;
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
    true
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
    if let Some(indexed) = state
        .runtime
        .terminal_spatial_index
        .at(vehicle.pos)
        .iter()
        .copied()
        .filter(|&idx| {
            state.stations.get(idx).is_some_and(|station| {
                station::vehicle_physically_at_station(&state.map, vehicle, station)
            })
        })
        .min_by_key(|&idx| {
            let station = &state.stations[idx];
            (station.pos.x - vehicle.pos.x).abs() + (station.pos.y - vehicle.pos.y).abs()
        })
    {
        return Some(indexed);
    }
    // En una partida importada todas las estaciones tienen el identificador
    // OpenTTD y el índice cubre sus teselas. Fuera de `MP_STATION`/aeropuerto
    // no hay nada que buscar. Los mapas nativos aún admiten paradas adyacentes
    // (y estaciones en road tiles), de modo que conservan el fallback.
    if stations_are_fully_map_indexed(state)
        && !matches!(
            state.map.get_kind(vehicle.pos),
            Some(TileKind::Station | TileKind::Airport)
        )
    {
        return None;
    }
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
    if stations_are_fully_map_indexed(state) && state.map.get_kind(vpos) != Some(TileKind::Industry)
    {
        return None;
    }
    state
        .stations
        .iter()
        .enumerate()
        .filter(|(_, station)| station.can_service_vehicle(vehicle.kind))
        .find(|(idx, station)| {
            state.industries.iter().any(|ind| {
                if ind.pos != vpos {
                    return false;
                }
                if !station::industry_in_station_coverage(
                    ind,
                    station.pos,
                    station::station_catchment_radius(station),
                ) {
                    return false;
                }
                // Stock aún en la industria, o ya repartido al andén de esta estación.
                ind.stock > 0 || state.stations[*idx].cargo_stock.get(ind.output_cargo()) > 0
            })
        })
        .map(|(idx, _)| idx)
}

fn stations_are_fully_map_indexed(state: &GameState) -> bool {
    !state.stations.is_empty()
        && state
            .stations
            .iter()
            .all(|station| station.ottd_station_id.is_some())
}

/// ¿La estación tiene en espera la carga que produce alguna industria de su cobertura?
fn station_has_industry_waiting(
    state: &GameState,
    station_idx: usize,
    vehicle: &crate::Vehicle,
) -> bool {
    let Some(station) = state.stations.get(station_idx) else {
        return false;
    };
    let radius = station::station_catchment_radius(station);
    let preferred = vehicle.cargo_type;
    state.industries.iter().any(|ind| {
        let output = ind.output_cargo();
        preferred.is_none_or(|c| c == output)
            && station.cargo_stock.get(output) > 0
            && station::industry_in_station_coverage(ind, station.pos, radius)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_supply_requires_stock_in_industry_or_station() {
        let mut state = GameState::new(4, 4);
        assert!(!has_loadable_supply(&state));

        let mut station = crate::Station::new(TileCoord::new(1, 1));
        station.cargo_stock.passengers = 1;
        state.stations.push(station);
        assert!(has_loadable_supply(&state));
    }

    #[test]
    fn newgrf_load_amount_overrides_cargo_fallback() {
        let mut state = GameState::new(4, 4);
        let vehicle = crate::Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        let engine_id = vehicle.engine_id;
        assert!(engine_id.is_some(), "Vehicle::new debe asignar engine_id");
        state.vehicles.push(vehicle);

        assert_eq!(
            vehicle_load_unload_speed(&state, 0, CargoType::Passengers),
            crate::cargo_packet::load_unload_speed(CargoType::Passengers)
        );
        let mut patched = false;
        if let Some(id) = engine_id {
            for engine in &mut state.engine_catalog {
                if engine.id == id {
                    engine.load_amount = 3;
                    patched = true;
                    break;
                }
            }
        }
        assert!(patched, "engine in catalog");
        assert_eq!(
            vehicle_load_unload_speed(&state, 0, CargoType::Passengers),
            3
        );
    }
}
