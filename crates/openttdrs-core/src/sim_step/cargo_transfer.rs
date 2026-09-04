use crate::vehicle::{OrderUnloadType, VehicleKind, VehicleRandomTrigger};
use crate::{CargoType, GameState, TileCoord, TileKind, economy, station, town};

/// Convierte un pago de economía en el contador acumulativo de ingresos.
///
/// `Money` es firmado porque un callback `NewGRF` puede devolver un ajuste
/// negativo (por ejemplo, una penalización de entrega), mientras que los
/// contadores históricos de estaciones, compañías y estadísticas son `u64`.
/// Convertir directamente con `cast_unsigned` transforma un valor negativo
/// en un número enorme y puede desbordar el contador en el siguiente tick.
/// `OpenTTD` no registra una penalización como ingreso, por lo que la conversión
/// correcta para estos contadores es saturar los valores no positivos a cero.
#[inline]
fn positive_money(amount: i64) -> u64 {
    u64::try_from(amount).unwrap_or_default()
}

/// Obtiene (o crea) el pago de la cabeza de convoy que está atendiendo una
/// parada. `OpenTTD` mantiene un `CargoPayment` por cabeza mientras la orden de
/// carga/descarga está abierta; el id lógico se traduce al `REF_VEHICLE` nativo
/// sólo al guardar.
fn ensure_cargo_payment(state: &mut GameState, front_vehicle_id: u32) -> usize {
    if let Some(index) = state
        .cargo_payments
        .iter()
        .position(|payment| payment.front_vehicle_id == Some(front_vehicle_id))
    {
        return index;
    }
    let id = state
        .cargo_payments
        .iter()
        .map(|payment| payment.id)
        .max()
        .unwrap_or(0)
        .saturating_add(u32::from(!state.cargo_payments.is_empty()));
    state.cargo_payments.push(crate::CargoPaymentState {
        id,
        front_vehicle_ref: None,
        front_vehicle_id: Some(front_vehicle_id),
        route_profit: 0,
        visual_profit: 0,
        visual_transfer: 0,
    });
    state.cargo_payments.len() - 1
}

/// Descarta pagos creados por el runtime una vez que la cabeza y todas sus
/// unidades terminaron la descarga. Las entradas importadas desde `CAPY`
/// conservan su referencia nativa y se mantienen para round-trip.
fn purge_finished_runtime_payments(state: &mut GameState) {
    state.cargo_payments.retain(|payment| {
        let Some(front_id) = payment.front_vehicle_id else {
            return true;
        };
        if payment.front_vehicle_ref.is_some() {
            return true;
        }
        crate::consist_unit_ids(&state.vehicles, front_id)
            .into_iter()
            .filter_map(|id| state.vehicles.iter().find(|vehicle| vehicle.id == id))
            .any(|vehicle| vehicle.cargo > 0 || vehicle.cargo_unloading || vehicle.cargo_loading)
    });
}

/// Ejecuta un trigger CB140 con área `TA_WHOLE` después de que la economía
/// cambió de verdad la cola de carga de una estación.
fn trigger_station_cargo_animation(
    state: &mut GameState,
    station_pos: TileCoord,
    trigger: crate::StationAnimationTrigger,
    cargo: CargoType,
) {
    let dirty =
        crate::map::trigger_newgrf_station_animation_for_station_with_world_and_cargo_catalog(
            &mut state.map,
            state.tick.get(),
            &mut state.stations,
            &state.companies,
            &state.industries,
            &state.cargo_spec_catalog,
            state.climate,
            &state.station_spec_catalog,
            &mut state.newgrf_animated_station_tiles,
            station_pos,
            trigger,
            Some(cargo),
        );
    state.runtime.industry_tile_dirty.extend(dirty);
    let airport_trigger = match trigger {
        crate::StationAnimationTrigger::NewCargo => Some(crate::AirportAnimationTrigger::NewCargo),
        crate::StationAnimationTrigger::CargoTaken => {
            Some(crate::AirportAnimationTrigger::CargoTaken)
        }
        _ => None,
    };
    if let Some(airport_trigger) = airport_trigger {
        super::trigger_airport_animation_at(state, station_pos, airport_trigger, Some(cargo));
    }
    super::trigger_road_stop_animation_at(state, station_pos, trigger, Some(cargo));
}

/// Ejecuta CB140 tras una carga/descarga: plataforma en trenes y tesela exacta
/// en `RoadStops`.
fn trigger_station_vehicle_load_animation(
    state: &mut GameState,
    station_pos: TileCoord,
    vehicle_pos: TileCoord,
) {
    let dirty =
        crate::map::trigger_newgrf_station_animation_for_platform_with_world_and_cargo_catalog(
            &mut state.map,
            state.tick.get(),
            &mut state.stations,
            &state.companies,
            &state.industries,
            &state.cargo_spec_catalog,
            state.climate,
            &state.station_spec_catalog,
            &mut state.newgrf_animated_station_tiles,
            station_pos,
            vehicle_pos,
            crate::StationAnimationTrigger::VehicleLoads,
        );
    state.runtime.industry_tile_dirty.extend(dirty);
    super::trigger_road_stop_animation_at(
        state,
        vehicle_pos,
        crate::StationAnimationTrigger::VehicleLoads,
        None,
    );
}

/// Ejecuta la cadena de randomización de vehículo para un evento económico.
///
/// Los eventos de carga/descarga deben pasar por la cabeza del consist cuando
/// corresponde (`NewCargo`/`Empty`), pero la función común también acepta una
/// unidad concreta para que `NewCargo` pueda evaluar primero el vehículo que
/// tomó la carga. Mantener el puente aquí evita que cada camino de carga
/// replique la combinación de catálogo, semilla y tick.
fn trigger_vehicle_randomisation_event(
    state: &mut GameState,
    vehicle_id: u32,
    trigger: VehicleRandomTrigger,
) {
    let world_seed = state.world_seed;
    let tick = state.tick.get();
    let _ = crate::newgrf_callback::trigger_vehicle_randomisation_chain(
        &mut state.vehicles,
        vehicle_id,
        &state.engine_catalog,
        trigger,
        world_seed,
        tick,
    );
}

/// Dispara `Empty` exactamente al vaciarse por completo un consist.
///
/// El bucle de descarga visita las unidades de forma independiente; por eso
/// sólo se llama después de sincronizar una unidad que quedó vacía y se
/// comprueba el estado de toda la cadena. En el tick en que se vacía la última
/// unidad, ninguna iteración posterior puede volver a entrar aquí porque su
/// carga ya es cero.
fn trigger_vehicle_empty_if_consist_empty(state: &mut GameState, vehicle_idx: usize) {
    let Some(vehicle) = state.vehicles.get(vehicle_idx) else {
        return;
    };
    if vehicle.cargo != 0 {
        return;
    }
    let vehicle_id = vehicle.id;
    let Some(head_id) = crate::consist_head_id(&state.vehicles, vehicle_id) else {
        return;
    };
    let unit_ids = crate::consist_unit_ids(&state.vehicles, head_id);
    if unit_ids.is_empty()
        || !unit_ids.iter().all(|id| {
            state
                .vehicles
                .iter()
                .find(|candidate| candidate.id == *id)
                .is_some_and(|candidate| candidate.cargo == 0)
        })
    {
        return;
    }
    trigger_vehicle_randomisation_event(state, head_id, VehicleRandomTrigger::Empty);
}

fn vehicle_load_unload_speed(state: &mut GameState, vehicle_idx: usize, cargo: CargoType) -> u32 {
    let Some(vehicle) = state.vehicles.get_mut(vehicle_idx) else {
        return crate::cargo_packet::load_unload_speed(cargo);
    };
    let configured_engine = vehicle
        .engine_id
        .and_then(|engine_id| crate::engine::engine_in_catalog(&state.engine_catalog, engine_id))
        .cloned();
    let callback_amount = configured_engine.as_ref().and_then(|engine| {
        crate::newgrf_callback::resolve_vehicle_load_amount_callback(engine, vehicle)
    });
    let configured = callback_amount.or_else(|| configured_engine.map(|engine| engine.load_amount));
    configured
        .filter(|amount| *amount > 0)
        .map_or_else(|| crate::cargo_packet::load_unload_speed(cargo), u32::from)
}

/// Refresca las capacidades que pueden cambiar mediante CB36 antes de
/// `LoadUnloadStation`.
///
/// `OpenTTD` vuelve a consultar `GetCapacity` durante la fase de carga, no sólo
/// al comprar o refitar. Esto es observable para callbacks que dependen del
/// cargo actual, de registros persistentes o del estado del vehículo. Los
/// consist ferroviarios se recalculan de una vez (la cabeza conserva la suma y
/// cada vagón recibe su capacidad propia); las otras clases actualizan su
/// unidad directamente. Los callbacks de capacidad de los motores vanilla y
/// los `NewGRF` sin runtime siguen por el camino legacy; el peso de la carga
/// se refresca para todas las formaciones porque sí afecta a la física.
pub(super) fn refresh_runtime_vehicle_capacities(state: &mut GameState) {
    let train_heads: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
        .map(|vehicle| vehicle.id)
        .collect();

    for head_id in train_heads {
        crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
            &mut state.vehicles,
            head_id,
            Some(&state.map),
            &state.engine_catalog,
            &state.cargo_spec_catalog,
            state.freight_trains,
        );
    }

    for index in 0..state.vehicles.len() {
        if state.vehicles[index].kind == VehicleKind::Train {
            continue;
        }
        let Some(engine_id) = state.vehicles[index].engine_id else {
            continue;
        };
        let Some(engine) =
            crate::engine::engine_in_catalog(&state.engine_catalog, engine_id).cloned()
        else {
            continue;
        };
        if engine.newgrf_grfid == 0 || engine.newgrf_runtime.is_none() {
            continue;
        }
        let Some(raw_capacity) = crate::newgrf_callback::resolve_vehicle_capacity_property_callback(
            &engine,
            &mut state.vehicles[index],
        ) else {
            continue;
        };
        let cargo = state.vehicles[index].cargo_type.or(engine.cargo).unwrap_or(
            match state.vehicles[index].kind {
                VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => {
                    CargoType::Passengers
                }
                VehicleKind::Truck | VehicleKind::Ship => CargoType::Goods,
                VehicleKind::Train => unreachable!("trenes se actualizan por consist"),
            },
        );
        state.vehicles[index].capacity = crate::cargo_spec::apply_cargo_capacity_multiplier(
            raw_capacity,
            &state.cargo_spec_catalog,
            cargo,
        );
    }
}

/// Entrega carga a las industrias cubiertas por una estación.
///
/// Es el equivalente reducido de `DeliverGoodsToIndustry`: las industrias se
/// ordenan por la menor distancia `DistanceMax` de cualquiera de sus teselas,
/// se excluye la industria de origen y se recorren los slots hasta agotar la
/// cantidad o la capacidad `uint16` de `AcceptedCargo::waiting`. El callback
/// `CBID_INDUSTRY_REFUSE_CARGO` puede omitir una industria temporalmente; la
/// producción se difiere hasta terminar toda la fase de carga/descarga.
#[allow(clippy::too_many_lines)]
fn deliver_goods_to_industries(
    state: &mut GameState,
    station_idx: usize,
    cargo: CargoType,
    amount: u32,
    source: TileCoord,
    company: crate::company::CompanyId,
) -> (u32, Vec<usize>) {
    if amount == 0 {
        return (0, Vec::new());
    }
    let Some(station) = state.stations.get(station_idx) else {
        return (0, Vec::new());
    };
    let station_pos = station.pos;
    let station_owner = station.owner;
    let station_id = station.ottd_station_id;
    let neutral_industry_id = station.neutral_industry_id;
    let serve_neutral_industries = state.serve_neutral_industries;
    let station_town = town::nearest_town_index(&state.towns, station_pos)
        .filter(|(_, distance)| *distance <= town::TOWN_AUTHORITY_RADIUS)
        .and_then(|(index, _)| state.towns.get(index).map(|town| town.id));
    let cargo_source = cargo_source_for_monitor(state, cargo, source);
    let radius = station::station_catchment_radius(station);
    let mut candidates: Vec<(usize, u32, u16)> = state
        .industries
        .iter()
        .enumerate()
        .filter(|(_, industry)| {
            !industry.contains_tile(source)
                && industry.accepts_cargo(cargo)
                && industry
                    .exclusive_supplier
                    .is_none_or(|supplier| supplier == station_owner)
                && (serve_neutral_industries
                    || neutral_industry_id.is_none_or(|id| id == industry.instance_id))
                && (serve_neutral_industries
                    || industry
                        .neutral_station_id
                        .is_none_or(|id| station_id == Some(id)))
                && station::industry_in_station_coverage(industry, station_pos, radius)
        })
        .map(|(index, industry)| {
            let distance = industry
                .tiles
                .iter()
                .copied()
                .chain(std::iter::once(industry.pos))
                .map(|tile| {
                    let dx = (tile.x - station_pos.x).unsigned_abs();
                    let dy = (tile.y - station_pos.y).unsigned_abs();
                    dx.max(dy)
                })
                .min()
                .unwrap_or(0);
            (index, distance, industry.instance_id)
        })
        .collect();
    // `IndustryCompare` uses distance then the pool index as a tie breaker.
    // `instance_id` is the native pool index for SAV entities, while the
    // vector index remains a deterministic fallback for procedural games.
    candidates
        .sort_unstable_by_key(|&(index, distance, instance_id)| (distance, instance_id, index));

    let mut remaining = amount;
    let mut accepted_total = 0_u32;
    let mut destinations = Vec::new();
    let accepted_date = state
        .economy_timer
        .date
        .saturating_add(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR);
    for (industry_idx, _, _) in candidates {
        if remaining == 0 {
            break;
        }
        let newgrf_def = state.industries[industry_idx]
            .newgrf_type_id
            .and_then(|type_id| {
                state
                    .industry_spec_catalog
                    .iter()
                    .find(|def| def.id == type_id)
                    .cloned()
            });
        if newgrf_def.as_ref().and_then(|def| {
            crate::newgrf_callback::resolve_industry_refuse_cargo_callback_with_catalog(
                def,
                &mut state.industries[industry_idx],
                cargo,
                &state.cargo_spec_catalog,
            )
        }) == Some(true)
        {
            continue;
        }
        let waiting = state.industries[industry_idx].accepted_cargo_waiting(cargo);
        let room = u32::from(u16::MAX).saturating_sub(waiting);
        let accepted = remaining.min(room);
        if accepted == 0 {
            continue;
        }
        state.industries[industry_idx].record_accepted_cargo(cargo, accepted, accepted_date);
        state.industries[industry_idx].was_cargo_delivered = true;
        state.runtime.cargo_monitor.add_cargo_delivery(
            cargo,
            company,
            accepted,
            cargo_source,
            station_town,
            Some(state.industries[industry_idx].instance_id),
        );
        // Keep one destination per station unload even when several packets of
        // the same consist carry this cargo; OpenTTD uses a set for this list.
        if !destinations.contains(&industry_idx) {
            destinations.push(industry_idx);
        }
        remaining -= accepted;
        accepted_total = accepted_total.saturating_add(accepted);
    }

    (accepted_total, destinations)
}

/// Resuelve el origen de un packet al identificador que usa `CargoMonitor`.
///
/// Las cargas de industria conservan la tesela de producción en
/// `CargoPacket::source`. Pasajeros/correo nacen en casas; para saves con
/// `MAP2` se usa el `TownID` persistido y, como en el resto del modelo, se cae a
/// la ciudad más cercana cuando ese dato no está disponible.
fn cargo_source_for_monitor(
    state: &GameState,
    cargo: CargoType,
    source: TileCoord,
) -> crate::cargo_monitor::CargoSource {
    if let Some(industry) = state
        .industries
        .iter()
        .find(|industry| industry.contains_tile(source))
    {
        return crate::cargo_monitor::CargoSource::Industry(industry.instance_id);
    }
    if !cargo.is_town_cargo() {
        return crate::cargo_monitor::CargoSource::Unknown;
    }

    let persisted_town_id = state
        .map
        .get(source)
        .filter(|tile| tile.kind == TileKind::House)
        .map(|tile| u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8))
        .and_then(|town_id| state.towns.iter().find(|town| town.id == town_id))
        .map(|town| town.id);
    persisted_town_id
        .or_else(|| {
            town::nearest_town_index(&state.towns, source)
                .and_then(|(index, _)| state.towns.get(index).map(|town| town.id))
        })
        .map_or(crate::cargo_monitor::CargoSource::Unknown, |town_id| {
            crate::cargo_monitor::CargoSource::Town(town_id)
        })
}

/// Consume the deferred `DeliverGoodsToIndustry` set after vehicle loading.
///
/// This ordering mirrors `LoadUnloadStation`: production triggered by a
/// delivery cannot be loaded by another vehicle until the next station pass.
pub(super) fn trigger_pending_industry_deliveries(state: &mut GameState) {
    let pending = std::mem::take(&mut state.runtime.pending_industry_deliveries);
    if pending.is_empty() {
        return;
    }
    super::economy::trigger_delivered_industries(state, &pending);
}

#[allow(clippy::too_many_lines)]
pub(super) fn unload_vehicles(
    state: &mut GameState,
    _tick: u64,
    loaded_this_tick: &[bool],
    unloaded_this_tick: &mut [bool],
) {
    let mut link_graph_dirty = false;
    let mut delivered_industries = Vec::new();
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
        let accepted = crate::station::station_accepts_cargo_with_newgrf(
            &state.map,
            &mut state.industries,
            &state.towns,
            &state.industry_tile_spec_catalog,
            &state.industry_spec_catalog,
            state.climate,
            &state.stations[station_idx],
            cargo_type,
        );
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

        // `PrepareUnload` crea el pago antes de clasificar cada paquete. Usar
        // la cabeza del consist evita generar una entrada por vagón y permite
        // guardar un CAPY coherente incluso si la descarga es gradual.
        let payment_front_id =
            crate::train_consist::consist_head_id(&state.vehicles, state.vehicles[i].id)
                .unwrap_or(state.vehicles[i].id);
        let payment_index = ensure_cargo_payment(state, payment_front_id);

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
            let current_payment =
                economy::cargo_current_payment(pay_spec, state.global_economy.inflation_payment);
            let part =
                crate::cargo_spec::cargo_spec_for_type(&state.cargo_spec_catalog, packet.cargo)
                    .and_then(|def| {
                        crate::newgrf_callback::resolve_cargo_profit_callback(
                            def,
                            u32::from(packet.count),
                            distance,
                            packet.periods_in_transit,
                            current_payment,
                        )
                    })
                    .unwrap_or(part);
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
                            if let Some(cargo_payment) = state.cargo_payments.get_mut(payment_index)
                            {
                                // `PayTransfer` sólo actualiza la parte visual
                                // del pago; el crédito monetario se concreta
                                // cuando la entrega final liquida el feeder.
                                cargo_payment.visual_transfer =
                                    cargo_payment.visual_transfer.saturating_add(share);
                            }
                        }
                    }
                }
                crate::cargo_packet::CargoUnloadAction::Deliver => {
                    let (accepted_industry, destinations) = deliver_goods_to_industries(
                        state,
                        station_idx,
                        packet.cargo,
                        u32::from(packet.count),
                        packet.source,
                        vehicle_owner,
                    );
                    for industry_idx in destinations {
                        if !delivered_industries.contains(&industry_idx) {
                            delivered_industries.push(industry_idx);
                        }
                    }
                    // `DeliverGoodsToIndustry` ya registró cada porción
                    // aceptada por una industria. El remanente corresponde a
                    // la aceptación de la estación/pueblo y debe pasar por la
                    // segunda llamada nativa de `AddCargoDelivery`.
                    let accepted_industry = accepted_industry.min(u32::from(packet.count));
                    let station_town = town::nearest_town_index(&state.towns, station_pos)
                        .filter(|(_, distance)| *distance <= town::TOWN_AUTHORITY_RADIUS)
                        .and_then(|(index, _)| state.towns.get(index).map(|town| town.id));
                    let cargo_source = cargo_source_for_monitor(state, packet.cargo, packet.source);
                    state.runtime.cargo_monitor.add_cargo_delivery(
                        packet.cargo,
                        vehicle_owner,
                        u32::from(packet.count).saturating_sub(accepted_industry),
                        cargo_source,
                        station_town,
                        None,
                    );
                    delivered_units = delivered_units.saturating_add(u32::from(packet.count));
                    let gross_part = part;
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
                    if let Some(cargo_payment) = state.cargo_payments.get_mut(payment_index) {
                        cargo_payment.route_profit =
                            cargo_payment.route_profit.saturating_add(gross_part);
                        cargo_payment.visual_profit =
                            cargo_payment.visual_profit.saturating_add(deliverer_part);
                    }
                }
                crate::cargo_packet::CargoUnloadAction::Keep
                | crate::cargo_packet::CargoUnloadAction::Load => {}
            }
        }

        let town_cargo = cargo_type.is_town_cargo();
        if delivered_units > 0 {
            // `GoodsEntry::State` mirrors the upstream station flags used by
            // NewGRF var 69. A final delivery (including direct delivery to
            // an industry) marks cargo as ever/currently accepted; the
            // bigtick flag is cleared by the next acceptance interval.
            state.stations[station_idx]
                .goods
                .get_mut(cargo_type)
                .mark_final_delivery();
        }
        if town_cargo && delivered_units > 0 {
            town::record_delivery_near_town(
                &mut state.towns,
                station_pos,
                cargo_type,
                delivered_units,
            );
        }
        let mut reinserted = Vec::new();
        let mut reinserted_cargos = Vec::new();
        for (mut p, was_transfer) in taken.into_iter().zip(transfer_mask) {
            // El modelo actual usa las estaciones como hub para todo freight;
            // una transferencia forzada de pax/mail también debe quedar allí.
            if !town_cargo || was_transfer {
                p.update_unloading_tile(station_pos);
                p.next_hop = None;
                if !reinserted_cargos.contains(&p.cargo) {
                    reinserted_cargos.push(p.cargo);
                }
                reinserted.push(p);
            }
        }
        if !reinserted.is_empty() {
            state.stations[station_idx].push_waiting_packets(reinserted);
            for cargo in reinserted_cargos {
                trigger_station_cargo_animation(
                    state,
                    station_pos,
                    crate::StationAnimationTrigger::NewCargo,
                    cargo,
                );
            }
        }
        trigger_station_vehicle_load_animation(state, station_pos, vpos);
        state.stations[station_idx].income = state.stations[station_idx]
            .income
            .saturating_add(positive_money(payment));
        state.credit_company(vehicle_owner, payment);
        let shown = payment.saturating_add(feeder_total);
        state.stats.cargo_income_earned = state
            .stats
            .cargo_income_earned
            .saturating_add(positive_money(shown));
        if let Some(c) = state.companies.get_mut(vehicle_owner.index()) {
            c.cargo_income_earned = c
                .cargo_income_earned
                .saturating_add(positive_money(payment));
        }
        let profit_vehicle_id = state.vehicles[i].id;
        let head_id =
            crate::consist_head_id(&state.vehicles, profit_vehicle_id).unwrap_or(profit_vehicle_id);
        if let Some(head) = state.vehicles.iter_mut().find(|v| v.id == head_id) {
            head.profit_this_year = head.profit_this_year.saturating_add(payment);
        }
        for (fo, share) in feeder_income_by_owner {
            if let Some(c) = state.companies.get_mut(fo.index()) {
                c.cargo_income_earned = c.cargo_income_earned.saturating_add(positive_money(share));
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
        // OpenTTD calls `PlayVehicleSound(VSE_LOAD_UNLOAD)` when the route
        // profit is committed. `shown` includes feeder income, so transfers
        // also receive the callback even when the direct delivery amount is
        // zero.
        if shown != 0 {
            state
                .runtime
                .pending_sim_events
                .push(crate::sim_events::SimEvent::VehicleLoadUnload {
                    vehicle_id: state.vehicles[i].id,
                    at: vpos,
                    kind: state.vehicles[i].kind,
                });
        }
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
        if state.vehicles[i].cargo == 0 {
            trigger_vehicle_empty_if_consist_empty(state, i);
        }
    }

    for industry_idx in delivered_industries {
        if !state
            .runtime
            .pending_industry_deliveries
            .contains(&industry_idx)
        {
            state.runtime.pending_industry_deliveries.push(industry_idx);
        }
    }

    // El pipeline Demand + MCF es global y costoso. Todas las descargas del tick
    // mutan primero el link graph; después publicamos un único snapshot coherente
    // para la fase de carga y reencaminamos paquetes una sola vez (#215).
    if link_graph_dirty {
        state.rebuild_station_flows();
    }
    purge_finished_runtime_payments(state);
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
    state.industries.iter().any(|industry| {
        industry.stock > 0 || industry.newgrf_extra_produced_cargo != crate::CargoStock::default()
    }) || state
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
    if first_pickup {
        let vehicle_id = state.vehicles[vehicle_idx].id;
        trigger_vehicle_randomisation_event(state, vehicle_id, VehicleRandomTrigger::NewCargo);
    }
    state.vehicles[vehicle_idx].cargo_packets.push(packet);
    state.vehicles[vehicle_idx].mark_cargo_loaded(source);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    state.vehicles[vehicle_idx].last_pickup_station = Some(station_pos);
    state.vehicles[vehicle_idx].last_depart_tick = Some(state.tick.get());
    state.industries[ind_idx].stock -= u32::from(count);
    state.industries[ind_idx].record_produced_cargo(output, u32::from(count), u32::from(count));
    state.industries[ind_idx].transported_total = state.industries[ind_idx]
        .transported_total
        .saturating_add(u64::from(count));
    let vehicle_pos = state.vehicles[vehicle_idx].pos;
    trigger_station_vehicle_load_animation(state, station_pos, vehicle_pos);
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
    let visit = state.vehicles[vehicle_idx]
        .station_visit_with_callbacks_and_catalog(state.tick.get(), &state.engine_catalog);
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
    if first_pickup {
        let vehicle_id = state.vehicles[vehicle_idx].id;
        trigger_vehicle_randomisation_event(state, vehicle_id, VehicleRandomTrigger::NewCargo);
    }
    state.vehicles[vehicle_idx]
        .cargo_packets
        .append_packets(taken);
    state.vehicles[vehicle_idx].mark_cargo_loaded(station_pos);
    state.vehicles[vehicle_idx].sync_cargo_from_packets();
    state.vehicles[vehicle_idx].last_pickup_station = Some(station_pos);
    state.vehicles[vehicle_idx].last_depart_tick = Some(state.tick.get());
    let visit = state.vehicles[vehicle_idx]
        .station_visit_with_callbacks_and_catalog(state.tick.get(), &state.engine_catalog);
    station::on_station_cargo_pickup(&mut state.stations[station_idx], cargo, company, visit);
    if state.stations[station_idx].cargo_stock.get(cargo) == 0 {
        trigger_station_cargo_animation(
            state,
            station_pos,
            crate::StationAnimationTrigger::CargoTaken,
            cargo,
        );
    }
    let vehicle_pos = state.vehicles[vehicle_idx].pos;
    trigger_station_vehicle_load_animation(state, station_pos, vehicle_pos);
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
#[allow(clippy::unwrap_used)] // Fixtures de mapa acotado construidos en cada prueba.
mod tests {
    use super::*;

    fn cb36_literal_runtime(value: u16) -> crate::newgrf_sprites::TrainSpriteGraphics {
        use crate::newgrf_sprites::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
        };

        let mut gfx = crate::newgrf_sprites::TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: u32::from(value),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn cb140_trigger_byte_runtime() -> crate::newgrf_sprites::TrainSpriteGraphics {
        use crate::newgrf_sprites::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
        };

        let mut gfx = crate::newgrf_sprites::TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x18,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn state_with_newgrf_rail_station(trigger_mask: u16) -> (GameState, TileCoord) {
        let pos = TileCoord::new(1, 1);
        let mut state = GameState::new(4, 4);
        let mut tile = state.map.get(pos).unwrap();
        tile.kind = TileKind::Station;
        tile.mapt = 0x50;
        tile.m5 = 0;
        tile.m6 = 0;
        state.map.set_tile(pos, tile).unwrap();
        state.stations.push(crate::Station::new_with_kind(
            pos,
            crate::StopKind::RailStation,
        ));
        let def = &mut state.station_spec_catalog[0];
        def.from_newgrf = true;
        def.animation_triggers = trigger_mask;
        def.newgrf_runtime = Some(Box::new(cb140_trigger_byte_runtime()));
        (state, pos)
    }

    fn state_with_newgrf_road_stop(trigger_mask: u16) -> (GameState, TileCoord) {
        let pos = TileCoord::new(1, 1);
        let mut state = GameState::new(4, 4);
        let mut tile = state.map.get(pos).unwrap();
        tile.kind = TileKind::Station;
        tile.mapt = 0x50;
        tile.m5 = crate::RSV_DRIVE_THROUGH_X;
        tile.m6 = 2;
        state.map.set_tile(pos, tile).unwrap();
        let mut station = crate::Station::new_with_kind(pos, crate::StopKind::BusStop);
        station.road_stop_spec = Some(7);
        state.stations.push(station);
        state.road_stop_spec_catalog.push(crate::RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "RoadStop animado".into(),
            short_label: "RSAN".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 0x5253_414E,
            newgrf_local_id: 0,
            newgrf_grf_version: 0,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            callback_mask: 0,
            animation_status: 1,
            animation_frames: u8::MAX,
            animation_speed: 0,
            animation_triggers: trigger_mask,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(cb140_trigger_byte_runtime())),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        });
        (state, pos)
    }

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
    fn negative_cargo_payment_does_not_wrap_income_counters() {
        assert_eq!(positive_money(i64::MIN), 0);
        assert_eq!(positive_money(-1), 0);
        assert_eq!(positive_money(0), 0);
        assert_eq!(positive_money(42), 42);
        assert_eq!(positive_money(i64::MAX), i64::MAX as u64);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn direct_delivery_skips_source_and_defers_industry_production() {
        let mut state = GameState::new(16, 16);
        let station_pos = TileCoord::new(5, 5);
        state.stations.push(crate::Station::new_with_kind(
            station_pos,
            crate::station::StopKind::TruckStop,
        ));
        state.towns.push(crate::Town {
            id: 7,
            pos: station_pos,
            ..crate::Town::default()
        });
        let source_pos = TileCoord::new(5, 4);
        let destination_pos = TileCoord::new(5, 6);
        state.industries.push(
            crate::Industry::with_tiles_spec(
                source_pos,
                crate::IndustryKind::Factory,
                crate::IndustrySpec::Factory,
                vec![source_pos],
                0,
            )
            .with_instance_id(3),
        );
        state.industries.push(
            crate::Industry::with_tiles_spec(
                destination_pos,
                crate::IndustryKind::Factory,
                crate::IndustrySpec::Factory,
                vec![destination_pos],
                0,
            )
            .with_instance_id(4),
        );

        let source_monitor = crate::encode_cargo_industry_monitor(
            crate::company::CompanyId::PLAYER,
            CargoType::Livestock,
            3,
        );
        let destination_monitor = crate::encode_cargo_industry_monitor(
            crate::company::CompanyId::PLAYER,
            CargoType::Livestock,
            4,
        );
        let town_monitor = crate::encode_cargo_town_monitor(
            crate::company::CompanyId::PLAYER,
            CargoType::Livestock,
            7,
        );
        let _ = state
            .runtime
            .cargo_monitor
            .get_pickup_amount(source_monitor, true);
        let _ = state
            .runtime
            .cargo_monitor
            .get_delivery_amount(destination_monitor, true);
        let _ = state
            .runtime
            .cargo_monitor
            .get_delivery_amount(town_monitor, true);

        let (accepted, destinations) = deliver_goods_to_industries(
            &mut state,
            0,
            CargoType::Livestock,
            12,
            source_pos,
            crate::company::CompanyId::PLAYER,
        );
        assert_eq!(accepted, 12);
        assert_eq!(destinations, vec![1]);
        assert_eq!(
            state.industries[0].accepted_cargo_waiting(CargoType::Livestock),
            0
        );
        assert_eq!(
            state.industries[1].accepted_cargo_waiting(CargoType::Livestock),
            12
        );
        assert_eq!(
            state.industries[1].last_accepted_date(CargoType::Livestock),
            crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR
        );
        assert_eq!(
            state
                .runtime
                .cargo_monitor
                .get_pickup_amount(source_monitor, true),
            12
        );
        assert_eq!(
            state
                .runtime
                .cargo_monitor
                .get_delivery_amount(destination_monitor, true),
            12
        );
        assert_eq!(
            state
                .runtime
                .cargo_monitor
                .get_delivery_amount(town_monitor, true),
            12
        );
        assert!(state.industries[1].was_cargo_delivered);
        assert_eq!(state.industries[1].stock, 0);

        state.runtime.pending_industry_deliveries = destinations;
        trigger_pending_industry_deliveries(&mut state);
        assert_eq!(
            state.industries[1].accepted_cargo_waiting(CargoType::Livestock),
            0
        );
        assert_eq!(state.industries[1].stock, 12);
        assert_eq!(state.stats.industry_cargo_units_produced, 12);
    }

    #[test]
    fn exclusive_supplier_uses_station_owner_for_industry_delivery() {
        let mut state = GameState::new(16, 16);
        let station_pos = TileCoord::new(5, 5);
        state.stations.push(crate::Station::new_with_kind(
            station_pos,
            crate::station::StopKind::TruckStop,
        ));
        let source_pos = TileCoord::new(5, 4);
        let destination_pos = TileCoord::new(5, 6);
        state.industries.push(
            crate::Industry::with_tiles_spec(
                source_pos,
                crate::IndustryKind::Factory,
                crate::IndustrySpec::Factory,
                vec![source_pos],
                0,
            )
            .with_instance_id(3),
        );
        let mut destination = crate::Industry::with_tiles_spec(
            destination_pos,
            crate::IndustryKind::Factory,
            crate::IndustrySpec::Factory,
            vec![destination_pos],
            0,
        )
        .with_instance_id(4);
        destination.exclusive_supplier = Some(crate::company::CompanyId(1));
        state.industries.push(destination);

        let (accepted, destinations) = deliver_goods_to_industries(
            &mut state,
            0,
            CargoType::Livestock,
            12,
            source_pos,
            crate::company::CompanyId::PLAYER,
        );
        assert_eq!(accepted, 0);
        assert!(destinations.is_empty());
        assert_eq!(
            state.industries[1].accepted_cargo_waiting(CargoType::Livestock),
            0
        );

        state.stations[0].owner = crate::company::CompanyId(1);
        let (accepted, destinations) = deliver_goods_to_industries(
            &mut state,
            0,
            CargoType::Livestock,
            12,
            source_pos,
            crate::company::CompanyId(1),
        );
        assert_eq!(accepted, 12);
        assert_eq!(destinations, vec![1]);
        assert_eq!(
            state.industries[1].accepted_cargo_waiting(CargoType::Livestock),
            12
        );
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
            vehicle_load_unload_speed(&mut state, 0, CargoType::Passengers),
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
            vehicle_load_unload_speed(&mut state, 0, CargoType::Passengers),
            3
        );
    }

    #[test]
    fn capacity_callback_is_refreshed_during_cargo_phase() {
        let mut state = GameState::new(4, 4);
        let mut engine = crate::engine::engines_table()
            .iter()
            .find(|engine| engine.kind == VehicleKind::Bus)
            .cloned()
            .unwrap();
        engine.id = 65_101;
        engine.newgrf_grfid = 0x4341_5036;
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(cb36_literal_runtime(42)));
        state.engine_catalog.push(engine.clone());

        let mut bus = crate::Vehicle::new(
            12,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        bus.engine_id = Some(engine.id);
        state.vehicles.push(bus);
        state.runtime.fleet_index.rebuild(&state.vehicles);

        refresh_runtime_vehicle_capacities(&mut state);
        assert_eq!(state.vehicles[0].capacity, 42);

        state
            .engine_catalog
            .iter_mut()
            .find(|candidate| candidate.id == engine.id)
            .unwrap()
            .newgrf_runtime = Some(Box::new(cb36_literal_runtime(7)));
        refresh_runtime_vehicle_capacities(&mut state);
        assert_eq!(state.vehicles[0].capacity, 7);
    }

    #[test]
    fn runtime_cargo_payment_is_keyed_by_front_and_purged_after_consist_finishes() {
        let mut state = GameState::new(4, 4);
        state.vehicles.push(crate::Vehicle::new(
            41,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        ));

        let index = ensure_cargo_payment(&mut state, 41);
        state.cargo_payments[index].route_profit = 123;
        state.vehicles[0].cargo = 8;
        purge_finished_runtime_payments(&mut state);
        assert_eq!(state.cargo_payments.len(), 1);
        assert_eq!(state.cargo_payments[0].front_vehicle_id, Some(41));
        assert_eq!(state.cargo_payments[0].route_profit, 123);

        state.vehicles[0].cargo = 0;
        state.vehicles[0].cargo_loading = false;
        state.vehicles[0].cargo_unloading = false;
        purge_finished_runtime_payments(&mut state);
        assert!(state.cargo_payments.is_empty());
    }

    #[test]
    fn unloading_freight_into_station_triggers_new_cargo_cb140() {
        let (mut state, pos) =
            state_with_newgrf_rail_station(crate::STATION_ANIMATION_TRIGGER_NEW_CARGO);
        let source = TileCoord::new(0, 1);
        let mut train = crate::Vehicle::new(7, VehicleKind::Train, pos, pos);
        train
            .cargo_packets
            .push(crate::CargoPacket::new(CargoType::Coal, 1, source));
        train.sync_cargo_from_packets();
        train.last_pickup_station = Some(source);
        state.vehicles.push(train);

        let mut unloaded = vec![false];
        unload_vehicles(&mut state, 1, &[false], &mut unloaded);

        assert!(unloaded[0]);
        assert_eq!(state.stations[0].cargo_stock.get(CargoType::Coal), 1);
        assert_eq!(map_frame(&state, pos), 1, "NewCargo ordinal llega a CB140");
        assert!(state.newgrf_animated_station_tiles.contains(&pos));
        assert!(
            state
                .runtime
                .pending_sim_events
                .iter()
                .any(|event| matches!(
                    event,
                    crate::sim_events::SimEvent::VehicleLoadUnload {
                        vehicle_id: 7,
                        kind: VehicleKind::Train,
                        ..
                    }
                ))
        );
    }

    #[test]
    fn loading_last_waiting_cargo_triggers_cargo_taken_cb140() {
        let (mut state, pos) =
            state_with_newgrf_rail_station(crate::STATION_ANIMATION_TRIGGER_CARGO_TAKEN);
        state.stations[0].add_waiting_cargo(CargoType::Coal, 1);
        let mut train = crate::Vehicle::new(8, VehicleKind::Train, pos, pos);
        train.cargo_type = Some(CargoType::Coal);
        state.vehicles.push(train);

        let mut loaded = false;
        assert!(try_load_from_station_waiting_cargo(
            &mut state,
            0,
            0,
            &mut loaded,
        ));

        assert!(loaded);
        assert_eq!(state.stations[0].cargo_stock.get(CargoType::Coal), 0);
        assert_eq!(
            map_frame(&state, pos),
            2,
            "CargoTaken sólo ocurre al vaciar el cargo"
        );
        assert!(state.newgrf_animated_station_tiles.contains(&pos));
    }

    #[test]
    fn road_stop_cargo_and_vehicle_load_triggers_reach_cb140() {
        let (mut state, pos) = state_with_newgrf_road_stop(
            crate::ROADSTOP_ANIMATION_TRIGGER_NEW_CARGO
                | crate::ROADSTOP_ANIMATION_TRIGGER_CARGO_TAKEN
                | crate::ROADSTOP_ANIMATION_TRIGGER_VEHICLE_LOADS,
        );

        trigger_station_cargo_animation(
            &mut state,
            pos,
            crate::StationAnimationTrigger::NewCargo,
            CargoType::Coal,
        );
        assert_eq!(state.stations[0].road_stop_animation_frame, 1);

        trigger_station_cargo_animation(
            &mut state,
            pos,
            crate::StationAnimationTrigger::CargoTaken,
            CargoType::Coal,
        );
        assert_eq!(state.stations[0].road_stop_animation_frame, 2);

        trigger_station_vehicle_load_animation(&mut state, pos, pos);
        assert_eq!(state.stations[0].road_stop_animation_frame, 5);
    }

    fn map_frame(state: &GameState, coord: TileCoord) -> u8 {
        state.map.get(coord).map_or(0, |tile| tile.m7)
    }
}
