//! Ejecución de órdenes, avance tras carga/descarga, horarios.
//!
//! Incluye `ProcessOrders` / `UpdateOrderDest` (P2.18) e inserción de `OT_IMPLICIT` (P2.17).

impl super::model::Vehicle {
    /// Añade una orden al final sin reiniciar `current_order` (salvo lista vacía).
    pub fn append_order(
        &mut self,
        order: crate::vehicle::order::VehicleOrder,
        map: &crate::map::Map,
    ) {
        let was_empty = self.orders.is_empty();
        self.orders.push(order);
        if was_empty {
            self.current_order = 0;
            self.cur_implicit_order_index = 0;
            self.path.clear();
            self.no_network_route_to_order = false;
            self.sync_order_destination(map);
        }
    }

    /// Inserta `OT_IMPLICIT` al visitar una estación no programada (`BeginLoading`).
    pub(crate) fn maybe_insert_implicit_order(&mut self, station: crate::map::TileCoord) {
        self.last_station_visited = Some(station);
        if self.orders.is_empty() {
            return;
        }
        // Solo vehículos terrestres generan órdenes implícitas.
        if !matches!(
            self.kind,
            super::model::VehicleKind::Train
                | super::model::VehicleKind::Truck
                | super::model::VehicleKind::Bus
                | super::model::VehicleKind::Tram
        ) {
            return;
        }
        self.sanitize_current_order();

        // Destino programado: alinear índices implícito y real.
        if let Some(order) = self.orders.get(self.current_order)
            && matches!(
                order,
                crate::vehicle::order::VehicleOrder::Station { station: s, implicit: false, .. }
                    if *s == station
            )
        {
            self.cur_implicit_order_index = self.current_order;
            return;
        }

        // Ya hay implícita/estación en el índice implícito para esta parada.
        if let Some(order) = self.orders.get(self.cur_implicit_order_index)
            && matches!(
                order,
                crate::vehicle::order::VehicleOrder::Station { station: s, .. } if *s == station
            )
        {
            return;
        }

        // Evitar duplicados consecutivos.
        let prev_idx = if self.cur_implicit_order_index > 0 {
            self.cur_implicit_order_index - 1
        } else if self.orders.len() > 1 {
            self.orders.len() - 1
        } else {
            self.cur_implicit_order_index
        };
        if let Some(prev) = self.orders.get(prev_idx)
            && matches!(
                prev,
                crate::vehicle::order::VehicleOrder::Station { station: s, .. } if *s == station
            )
        {
            return;
        }

        let insert_at = self.cur_implicit_order_index.min(self.orders.len());
        self.orders.insert(
            insert_at,
            crate::vehicle::order::VehicleOrder::implicit(station),
        );
        // Tras insertar en `cur_implicit`, el índice apunta a la nueva; el real se desplaza.
        if self.current_order >= insert_at {
            self.current_order += 1;
        }
        self.cur_implicit_order_index = insert_at;
        self.sanitize_current_order();
    }

    /// `ProcessOrders` — avanza / resuelve destino si la orden actual cambió.
    ///
    /// Devuelve `true` si el vehículo puede invertir (tren al salir de estación).
    pub fn process_orders(&mut self, map: &crate::map::Map) -> bool {
        if self.orders.is_empty() {
            return false;
        }
        if self.cargo_loading || self.cargo_unloading || self.awaiting_load_window {
            return false;
        }

        let may_reverse = self.path.is_empty() && self.progress == 255;
        self.sanitize_current_order();
        self.update_real_order_index();

        let Some(order) = self.orders.get(self.current_order).copied() else {
            return false;
        };
        // Lista solo de implícitas: no hay destino manual.
        if order.is_implicit()
            && !self.orders.iter().any(|o| {
                matches!(
                    o,
                    crate::vehicle::order::VehicleOrder::Station {
                        implicit: false,
                        ..
                    } | crate::vehicle::order::VehicleOrder::Depot { .. }
                        | crate::vehicle::order::VehicleOrder::Waypoint { .. }
                        | crate::vehicle::order::VehicleOrder::Tile(_)
                )
            })
        {
            return false;
        }

        let updated = self.update_order_dest(map, 0);
        updated && may_reverse
    }

    /// Avanza `cur_real_order_index` hasta la siguiente orden no implícita.
    pub(crate) fn update_real_order_index(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        self.sanitize_current_order();
        let n = self.orders.len();
        for _ in 0..n {
            if !self
                .orders
                .get(self.current_order)
                .is_some_and(|o| o.is_implicit())
            {
                break;
            }
            // Si solo hay implícitas, conservar el índice.
            if self.orders.iter().all(|o| o.is_implicit()) {
                break;
            }
            self.current_order = (self.current_order + 1) % n;
        }
    }

    /// `UpdateOrderDest` — resuelve destino (estación, depósito, condicional, waypoint).
    pub fn update_order_dest(&mut self, map: &crate::map::Map, conditional_depth: usize) -> bool {
        self.update_order_dest_with_stations(map, &[], conditional_depth)
    }

    /// Variante de `UpdateOrderDest` que conserva la identidad física de las
    /// estaciones al elegir el destino naval.
    pub fn update_order_dest_with_stations(
        &mut self,
        map: &crate::map::Map,
        stations: &[crate::station::Station],
        conditional_depth: usize,
    ) -> bool {
        self.update_order_dest_with_stations_impl(map, stations, conditional_depth, None)
    }

    fn update_order_dest_with_stations_impl(
        &mut self,
        map: &crate::map::Map,
        stations: &[crate::station::Station],
        conditional_depth: usize,
        mut depot_index: Option<&mut crate::depot::DepotSpatialIndex>,
    ) -> bool {
        if self.orders.is_empty() {
            return false;
        }
        if conditional_depth > self.orders.len() {
            return false;
        }
        self.sanitize_current_order();
        let Some(order) = self.orders.get(self.current_order).copied() else {
            return false;
        };

        match order {
            crate::vehicle::order::VehicleOrder::Station { .. }
            | crate::vehicle::order::VehicleOrder::Waypoint { .. }
            | crate::vehicle::order::VehicleOrder::Tile(_) => {
                self.apply_order_destination_with_stations(map, stations, order);
                true
            }
            crate::vehicle::order::VehicleOrder::Depot { stop: false, .. } => {
                // Servicio opcional: saltar si no hace falta.
                if !self.needs_servicing {
                    self.increment_real_order_index();
                    return self.update_order_dest_with_stations_impl(
                        map,
                        stations,
                        conditional_depth + 1,
                        depot_index,
                    );
                }
                self.apply_order_destination_with_stations(map, stations, order);
                true
            }
            crate::vehicle::order::VehicleOrder::Depot {
                depot, stop: true, ..
            } => {
                // Depósito concreto; si no existe en mapa, buscar el más cercano.
                if map.get(depot).is_none()
                    || !matches!(
                        map.get_kind(depot),
                        Some(
                            crate::map::TileKind::RailDepot
                                | crate::map::TileKind::RoadDepot
                                | crate::map::TileKind::ShipDepot
                                | crate::map::TileKind::Airport
                        )
                    )
                {
                    let nearest = if let Some(index) = depot_index.as_deref_mut() {
                        crate::depot::nearest_depot_tile_indexed(map, self.pos, self.kind, index)
                    } else {
                        crate::depot::nearest_depot_tile(map, self.pos, self.kind)
                    };
                    if let Some(nearest) = nearest {
                        self.dest = nearest;
                        return true;
                    }
                    self.increment_real_order_index();
                    return self.update_order_dest_with_stations_impl(
                        map,
                        stations,
                        conditional_depth + 1,
                        depot_index,
                    );
                }
                self.apply_order_destination_with_stations(map, stations, order);
                true
            }
            crate::vehicle::order::VehicleOrder::Conditional { .. } => {
                let next = order.evaluate_conditional(self);
                self.cur_implicit_order_index = next;
                self.current_order = next;
                self.update_real_order_index();
                self.current_order_time = 0;
                self.update_order_dest_with_stations_impl(
                    map,
                    stations,
                    conditional_depth + 1,
                    depot_index,
                )
            }
        }
    }

    fn apply_order_destination_with_stations(
        &mut self,
        map: &crate::map::Map,
        stations: &[crate::station::Station],
        order: crate::vehicle::order::VehicleOrder,
    ) {
        if matches!(
            self.kind,
            super::model::VehicleKind::Train | super::model::VehicleKind::Aircraft
        ) && self.awaiting_load_window
        {
            // FTA: `dest` es el stand exacto, que no necesariamente coincide
            // con el ancla de la orden ni con la primera tesela de terminal.
            // En trenes, `TrainEnterStation` conserva el tile de plataforma
            // mientras `OT_LOADING` espera su primera fase de carga.
            return;
        }
        if self.kind == super::model::VehicleKind::Train
            && !self.path.is_empty()
            && let crate::vehicle::order::VehicleOrder::Station { station, .. } = order
            && crate::station::rail_station_platform_tiles(map, station).contains(&self.dest)
        {
            return;
        }
        if self.kind == super::model::VehicleKind::Train {
            self.train_station_arrival_target = None;
        }
        self.dest = if self.kind == super::model::VehicleKind::Aircraft {
            match order {
                crate::vehicle::order::VehicleOrder::Station { station, .. } => {
                    crate::airport::airport_loading_tile_at(map, station)
                }
                _ => crate::station::resolve_order_destination_from_with_stations(
                    map, stations, self.kind, order, self.pos,
                ),
            }
        } else {
            crate::station::resolve_order_destination_from_with_stations(
                map, stations, self.kind, order, self.pos,
            )
        };
    }

    pub(crate) fn increment_real_order_index(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        self.current_order = (self.current_order + 1) % self.orders.len();
        self.sanitize_current_order();
        self.update_real_order_index();
    }

    /// Tras descargar en la parada actual, pasar a la siguiente orden antes de la fase de carga.
    pub(crate) fn advance_after_unloading(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        if self.schedule_timetable_wait(super::model::TimetableWaitKind::AfterUnload) {
            return;
        }
        self.do_advance_after_unloading();
    }

    fn do_advance_after_unloading(&mut self) {
        self.mark_train_station_departure_hold();
        self.path.clear();
        self.depart_turn = 0;
        if !self.retains_station_movement_fractions() {
            self.progress = 255;
        }
        self.mark_station_departure();
        self.advance_to_next_order();
    }

    /// Tras cargar en la parada actual, pasar a la siguiente orden aunque haya carga a bordo.
    pub(crate) fn advance_after_loading(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        self.sanitize_current_order();
        if let Some(order) = self.current_order_ref()
            && order.should_wait_for_loading(self.cargo, self.capacity)
        {
            return;
        }
        if self.schedule_timetable_wait(super::model::TimetableWaitKind::AfterLoad) {
            return;
        }
        self.do_advance_after_loading();
    }

    /// Avanza después de cargar un consist cuya terminación ya fue evaluada
    /// sobre todas sus unidades.
    ///
    /// `Vehicle::capacity` de la cabeza de un tren puede ser la capacidad
    /// agregada del consist, mientras que `FullLoad` se decide por la
    /// capacidad local de cada unidad. La fase de carga hace esa evaluación
    /// antes de llamar aquí; repetir el chequeo escalar de la cabeza haría que
    /// una locomotora sin bodega mantuviera la orden para siempre.
    pub(crate) fn advance_after_consist_loading(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        self.sanitize_current_order();
        if self.schedule_timetable_wait(super::model::TimetableWaitKind::AfterLoad) {
            return;
        }
        self.do_advance_after_loading();
    }

    fn do_advance_after_loading(&mut self) {
        self.mark_train_station_departure_hold();
        self.path.clear();
        self.depart_turn = 0;
        if !self.retains_station_movement_fractions() {
            self.progress = 255;
        }
        self.mark_station_departure();
        self.advance_to_next_order();
    }

    pub(super) fn advance_to_next_order(&mut self) {
        self.awaiting_load_window = false;
        self.train_station_arrival_target = None;
        if self.orders.is_empty() {
            return;
        }
        self.increment_real_order_index();
        self.cur_implicit_order_index = self.current_order;
        self.origin = self.pos;
        self.timetable_leg_start_tick = self.sim_tick;
        self.current_order_time = 0;
        if self.kind != super::model::VehicleKind::Train
            && let Some(order) = self.current_order_ref()
        {
            self.dest = order.destination();
        }
    }

    /// Registra una salida de estación para que `sim_step` ejecute CB140 antes
    /// de abandonar la plataforma o `RoadStop`. No se persiste: se consume
    /// durante el mismo tick de la simulación.
    fn mark_station_departure(&mut self) {
        if (self.kind == super::model::VehicleKind::Train && self.is_consist_head())
            || matches!(
                self.kind,
                super::model::VehicleKind::Bus
                    | super::model::VehicleKind::Truck
                    | super::model::VehicleKind::Tram
            )
        {
            self.station_departure_pending = true;
        }
    }

    /// Consume el evento de salida que producen las rutas de carga, descarga,
    /// espera de horario o cierre de `BeginLoading`.
    #[must_use]
    pub(crate) fn take_station_departure(&mut self) -> bool {
        std::mem::take(&mut self.station_departure_pending)
    }

    pub(super) fn schedule_timetable_wait(
        &mut self,
        kind: super::model::TimetableWaitKind,
    ) -> bool {
        if !self.timetable_active {
            return false;
        }
        let wait = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.wait_ticks());
        if wait == 0 {
            return false;
        }
        self.timetable_wait_remaining = wait;
        self.timetable_wait_kind = kind;
        if self.is_station_service_timetable_wait() {
            self.hold_station_movement();
        } else {
            self.progress = 255;
        }
        true
    }

    #[allow(clippy::unused_self)]
    fn record_timetable_autofill_sample(&mut self, _wait: u32, _travel: u32) {
        // Autofill completo en `timetable::Vehicle::update_vehicle_timetable`.
    }

    #[allow(dead_code)]
    pub(crate) fn complete_timetable_wait(&mut self) {
        self.complete_timetable_wait_with_catalog(&[]);
    }

    pub(crate) fn complete_timetable_wait_with_catalog(
        &mut self,
        engine_catalog: &[crate::engine::EngineDef],
    ) {
        let kind = self.timetable_wait_kind;
        let planned = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.wait_ticks());
        self.timetable_wait_kind = super::model::TimetableWaitKind::None;
        if kind != super::model::TimetableWaitKind::None && planned > 0 {
            if kind != super::model::TimetableWaitKind::TravelEarly {
                self.update_vehicle_timetable(false);
            }
            self.record_timetable_autofill_sample(planned, 0);
        }
        match kind {
            super::model::TimetableWaitKind::None => {}
            super::model::TimetableWaitKind::TravelEarly => {
                if self.timetable_active {
                    self.timetable_lateness = self.timetable_lateness.saturating_add(1);
                }
                self.finish_arrival_processing_with_catalog(engine_catalog);
            }
            super::model::TimetableWaitKind::AfterArrival => {
                self.sanitize_current_order();
                let pass_through = self
                    .current_order_ref()
                    .is_some_and(|o| o.is_pass_through());
                self.do_advance_after_arrival(pass_through);
            }
            super::model::TimetableWaitKind::AfterUnload => self.do_advance_after_unloading(),
            super::model::TimetableWaitKind::AfterLoad => self.do_advance_after_loading(),
        }
        self.resolve_conditional_orders();
    }

    #[allow(dead_code)]
    pub(crate) fn tick_timetable_wait(&mut self) {
        self.tick_timetable_wait_with_catalog(&[]);
    }

    pub(crate) fn tick_timetable_wait_with_catalog(
        &mut self,
        engine_catalog: &[crate::engine::EngineDef],
    ) {
        if self.timetable_wait_remaining == 0 {
            return;
        }
        self.timetable_wait_remaining = self.timetable_wait_remaining.saturating_sub(1);
        if self.timetable_wait_remaining == 0 {
            self.complete_timetable_wait_with_catalog(engine_catalog);
        }
    }

    /// Actualiza `dest` según la orden actual (vía adyacente para estaciones de tren).
    pub fn sync_order_destination(&mut self, map: &crate::map::Map) {
        self.sync_order_destination_with_stations(map, &[]);
    }

    /// Actualiza `dest` usando el catálogo lógico de estaciones del `GameState`.
    pub fn sync_order_destination_with_stations(
        &mut self,
        map: &crate::map::Map,
        stations: &[crate::station::Station],
    ) {
        self.sync_order_destination_impl(map, stations, None);
    }

    /// Shares depot candidates within a batch that does not mutate the map.
    /// Callers create a fresh index for each batch so deleted or new depots
    /// are visible without retaining a chosen destination across ticks.
    pub(crate) fn sync_order_destination_with_depot_index(
        &mut self,
        map: &crate::map::Map,
        stations: &[crate::station::Station],
        index: &mut crate::depot::DepotSpatialIndex,
    ) {
        self.sync_order_destination_impl(map, stations, Some(index));
    }

    fn sync_order_destination_impl(
        &mut self,
        map: &crate::map::Map,
        stations: &[crate::station::Station],
        depot_index: Option<&mut crate::depot::DepotSpatialIndex>,
    ) {
        if self.orders.is_empty() {
            return;
        }
        self.sanitize_current_order();
        self.update_real_order_index();
        let _ = self.update_order_dest_with_stations_impl(map, stations, 0, depot_index);
    }

    pub(super) fn do_advance_after_arrival(&mut self, pass_through: bool) {
        self.mark_train_station_departure_hold();
        // Native LeaveStation keeps the handler's movement fractions.
        // Synthetic movement retains its endpoint normalization.
        if !self.retains_station_movement_fractions() {
            self.progress = if pass_through { 0 } else { 255 };
        }
        self.mark_station_departure();
        self.advance_to_next_order();
    }

    fn mark_train_station_departure_hold(&mut self) {
        if self.kind == super::model::VehicleKind::Train
            && self
                .current_order_ref()
                .is_some_and(|order| order.is_station_like())
        {
            self.train_station_departure_hold = true;
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::depot::DepotSpatialIndex;
    use crate::map::{Map, TileKind};
    use crate::vehicle::order::OrderConditionKind;
    use crate::{Command, GameState, TileCoord, Vehicle, VehicleKind, VehicleOrder, apply_command};

    #[test]
    fn station_departure_paths_keep_native_movement_fractions() {
        use crate::vehicle::{OrderLoadType, OrderNonStop, OrderUnloadType};

        let mut cases = 0;
        for kind in [VehicleKind::Train, VehicleKind::Bus, VehicleKind::Truck] {
            for line in
                include_str!("../../tests/fixtures/parity/native-station-departure-remainder.csv")
                    .lines()
                    .skip(1)
            {
                let fields: Vec<u16> = line
                    .split(',')
                    .map(|field| field.parse().expect("native numeric field"))
                    .collect();
                let native_train = fields[0] == 0;
                if (kind == VehicleKind::Train) != native_train {
                    continue;
                }
                assert_eq!(fields[11], 4, "native OT_LEAVESTATION");
                assert_eq!(fields[12], 0, "native station registration removed");
                for phase in 0..4 {
                    let at = TileCoord::new(0, 0);
                    let mut v = Vehicle::new(1, kind, at, at);
                    v.running = true;
                    v.awaiting_load_window = true;
                    v.crashed = fields[1] != 0;
                    let mut order = VehicleOrder::station(at);
                    if let VehicleOrder::Station {
                        load_type,
                        unload_type,
                        non_stop,
                        ..
                    } = &mut order
                    {
                        *load_type = if fields[3] == 0 {
                            OrderLoadType::LoadIfPossible
                        } else {
                            OrderLoadType::NoLoad
                        };
                        *unload_type = if fields[4] == 0 {
                            OrderUnloadType::UnloadIfPossible
                        } else {
                            OrderUnloadType::NoUnload
                        };
                        *non_stop = if fields[2] == 0 {
                            OrderNonStop::StopAtIntermediate
                        } else {
                            OrderNonStop::NonStopDestination
                        };
                    }
                    v.orders = vec![order, VehicleOrder::station(TileCoord::new(0, 1))];
                    v.cur_speed = 0;
                    v.progress = u8::try_from(fields[6]).expect("native input progress");
                    v.subspeed = u8::try_from(fields[7]).expect("native input subspeed");
                    v.rail_pixel = 4;
                    if !native_train {
                        v.road_pos_valid = true;
                        v.road_state = crate::road_movement::rvsb::RVSB_IN_ROAD_STOP
                            | crate::road_movement::rvsb::RVSB_ENTERED_STOP;
                        v.frame = 20;
                        v.road_x = 5;
                        v.road_y = 6;
                        v.direction = crate::vehicle::DIR_SE;
                    }
                    match phase {
                        0 => v.advance_after_loading(),
                        1 => v.advance_after_consist_loading(),
                        2 => v.advance_after_unloading(),
                        _ => v.finish_arrival_after_load_window(),
                    }
                    assert_eq!(
                        (v.cur_speed, u16::from(v.progress), u16::from(v.subspeed)),
                        (fields[8], fields[9], fields[10]),
                        "{line}, {kind:?}, phase={phase}"
                    );
                    assert_eq!(v.current_order, 1, "completed service at {line}");
                    assert!(!v.awaiting_load_window, "completed service at {line}");
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 8064);
    }

    fn depot_map() -> Map {
        let mut state = GameState::new(12, 12);
        for (kind, coords) in [
            (TileKind::RoadDepot, [(3, 2), (2, 3)]),
            (TileKind::RailDepot, [(7, 2), (6, 3)]),
            (TileKind::Airport, [(3, 10), (2, 11)]),
        ] {
            for (x, y) in coords {
                let coord = TileCoord::new(x, y);
                state.map.set_kind(coord, kind).expect("depot kind");
                if kind == TileKind::Airport {
                    let mut tile = state.map.get(coord).expect("hangar");
                    tile.m5 = 1;
                    state.map.set_tile(coord, tile).expect("hangar piece");
                }
            }
        }
        let apron = TileCoord::new(2, 2);
        state
            .map
            .set_kind(apron, TileKind::Airport)
            .expect("airport");
        let mut tile = state.map.get(apron).expect("apron");
        tile.m5 = 2;
        state.map.set_tile(apron, tile).expect("apron piece");
        for x in [3, 7] {
            for dx in -1..=1 {
                state
                    .map
                    .set_kind(TileCoord::new(x + dx, 7), TileKind::Water)
                    .expect("water");
            }
            apply_command(
                &mut state,
                &Command::PlaceShipDepotDir(TileCoord::new(x, 7), 2),
            )
            .expect("ship depot");
        }
        state.map
    }

    fn order_vehicle(kind: VehicleKind, mode: u8, from: TileCoord) -> Vehicle {
        let missing = TileCoord::new(0, 0);
        let existing = match kind {
            VehicleKind::Train => TileCoord::new(7, 2),
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => TileCoord::new(3, 2),
            VehicleKind::Ship => TileCoord::new(3, 7),
            VehicleKind::Aircraft => TileCoord::new(3, 10),
        };
        let mut vehicle = Vehicle::new(7, kind, from, TileCoord::new(11, 11));
        vehicle.current_order_time = 57;
        vehicle.orders = vec![VehicleOrder::depot(missing), VehicleOrder::Tile(existing)];
        match mode {
            0 => vehicle.orders.clear(),
            1 => vehicle.orders[0] = VehicleOrder::depot(existing),
            3 => vehicle.orders[0] = VehicleOrder::depot(TileCoord::new(-5, 19)),
            4 | 5 => {
                vehicle.orders[0] = VehicleOrder::depot_pass_through(missing);
                vehicle.needs_servicing = mode == 5;
            }
            6 => vehicle.orders.insert(
                0,
                VehicleOrder::conditional(OrderConditionKind::Unconditionally, 0, 1),
            ),
            7 => {
                vehicle.orders = vec![VehicleOrder::conditional(
                    OrderConditionKind::Unconditionally,
                    0,
                    0,
                )];
            }
            8 => vehicle.orders = vec![VehicleOrder::implicit(TileCoord::new(11, 11))],
            9 => vehicle.current_order = 98,
            10 => vehicle.awaiting_load_window = true,
            11 => {
                // Existing wrong-kind depots retain the scalar acceptance rule.
                vehicle.orders[0] = VehicleOrder::depot(TileCoord::new(2, 2));
            }
            _ => {}
        }
        vehicle
    }

    fn compare_batch(map: &Map, vehicles: &mut [Vehicle]) -> u64 {
        let mut index = DepotSpatialIndex::default();
        for vehicle in vehicles {
            let mut expected = vehicle.clone();
            expected.sync_order_destination_with_stations(map, &[]);
            vehicle.sync_order_destination_with_depot_index(map, &[], &mut index);
            assert_eq!(
                serde_json::to_value(&*vehicle).expect("indexed vehicle"),
                serde_json::to_value(expected).expect("scalar vehicle")
            );
        }
        index.full_map_scans()
    }

    #[test]
    fn batch_depot_resolution_preserves_scalar_order_mutations() {
        let map = depot_map();
        let kinds = [
            VehicleKind::Train,
            VehicleKind::Bus,
            VehicleKind::Truck,
            VehicleKind::Tram,
            VehicleKind::Ship,
            VehicleKind::Aircraft,
        ];
        for mode in 0..12 {
            for from in [
                TileCoord::new(2, 2),
                TileCoord::new(9, 2),
                TileCoord::new(3, 8),
            ] {
                let mut vehicles: Vec<_> = kinds
                    .iter()
                    .map(|&kind| order_vehicle(kind, mode, from))
                    .collect();
                let scans = compare_batch(&map, &mut vehicles);
                assert_eq!(
                    scans,
                    u64::from(matches!(mode, 2 | 3 | 6 | 9 | 10)),
                    "mode {mode}"
                );
            }
        }
        let mut tie = order_vehicle(VehicleKind::Bus, 2, TileCoord::new(2, 2));
        assert_eq!(compare_batch(&map, std::slice::from_mut(&mut tie)), 1);
        assert_eq!(tie.dest, TileCoord::new(3, 2));
    }

    #[test]
    fn fresh_batch_observes_deleted_depots_and_current_vehicle_positions() {
        let mut map = Map::new_flat(256, 256, 0);
        let old = TileCoord::new(20, 20);
        let far = TileCoord::new(200, 200);
        let new = TileCoord::new(40, 40);
        for depot in [old, far] {
            map.set_kind(depot, TileKind::RoadDepot)
                .expect("road depot");
        }
        let mut vehicles: Vec<_> = (0..32)
            .map(|id| {
                let mut vehicle = Vehicle::new(id, VehicleKind::Truck, TileCoord::new(10, 10), old);
                vehicle.orders = vec![
                    VehicleOrder::depot(TileCoord::new(500, 500)),
                    VehicleOrder::Tile(TileCoord::new(250, 250)),
                ];
                vehicle
            })
            .collect();
        assert_eq!(compare_batch(&map, &mut vehicles), 1);
        assert!(vehicles.iter().all(|v| v.dest == old));

        map.set_kind(old, TileKind::Grass)
            .expect("remove old depot");
        map.set_kind(new, TileKind::RoadDepot).expect("new depot");
        vehicles[0].pos = TileCoord::new(210, 210);
        assert_eq!(compare_batch(&map, &mut vehicles), 1);
        assert_eq!(vehicles[0].dest, far);
        assert!(vehicles[1..].iter().all(|v| v.dest == new));

        for depot in [new, far] {
            map.set_kind(depot, TileKind::Grass)
                .expect("remove all depots");
        }
        assert_eq!(compare_batch(&map, &mut vehicles), 1);
        assert!(
            vehicles
                .iter()
                .all(|v| v.current_order == 1 && v.dest == TileCoord::new(250, 250))
        );
    }
}
