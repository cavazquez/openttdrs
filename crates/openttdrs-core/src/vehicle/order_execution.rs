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
                self.apply_order_destination(map, order);
                true
            }
            crate::vehicle::order::VehicleOrder::Depot { stop: false, .. } => {
                // Servicio opcional: saltar si no hace falta.
                if !self.needs_servicing {
                    self.increment_real_order_index();
                    return self.update_order_dest(map, conditional_depth + 1);
                }
                self.apply_order_destination(map, order);
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
                    if let Some(nearest) =
                        crate::depot::nearest_depot_tile(map, self.pos, self.kind)
                    {
                        self.dest = nearest;
                        return true;
                    }
                    self.increment_real_order_index();
                    return self.update_order_dest(map, conditional_depth + 1);
                }
                self.apply_order_destination(map, order);
                true
            }
            crate::vehicle::order::VehicleOrder::Conditional { .. } => {
                let next = order.evaluate_conditional(self);
                self.cur_implicit_order_index = next;
                self.current_order = next;
                self.update_real_order_index();
                self.current_order_time = 0;
                self.update_order_dest(map, conditional_depth + 1)
            }
        }
    }

    fn apply_order_destination(
        &mut self,
        map: &crate::map::Map,
        order: crate::vehicle::order::VehicleOrder,
    ) {
        if self.kind == super::model::VehicleKind::Aircraft && self.awaiting_load_window {
            // FTA: `dest` es el stand exacto, que no necesariamente coincide
            // con el ancla de la orden ni con la primera tesela de terminal.
            return;
        }
        if self.kind == super::model::VehicleKind::Train
            && !self.path.is_empty()
            && let crate::vehicle::order::VehicleOrder::Station { station, .. } = order
            && crate::station::rail_station_platform_tiles(map, station).contains(&self.dest)
        {
            return;
        }
        self.dest = if self.kind == super::model::VehicleKind::Aircraft {
            match order {
                crate::vehicle::order::VehicleOrder::Station { station, .. } => {
                    crate::airport::airport_loading_tile_at(map, station)
                }
                _ => {
                    crate::station::resolve_order_destination_from(map, self.kind, order, self.pos)
                }
            }
        } else {
            crate::station::resolve_order_destination_from(map, self.kind, order, self.pos)
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
            self.progress = 255;
            return;
        }
        self.do_advance_after_unloading();
    }

    fn do_advance_after_unloading(&mut self) {
        self.path.clear();
        self.depart_turn = 0;
        self.progress = 255;
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
            self.progress = 255;
            return;
        }
        self.do_advance_after_loading();
    }

    fn do_advance_after_loading(&mut self) {
        self.path.clear();
        self.depart_turn = 0;
        self.progress = 255;
        self.advance_to_next_order();
    }

    pub(super) fn advance_to_next_order(&mut self) {
        self.awaiting_load_window = false;
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
        true
    }

    #[allow(clippy::unused_self)]
    fn record_timetable_autofill_sample(&mut self, _wait: u32, _travel: u32) {
        // Autofill completo en `timetable::Vehicle::update_vehicle_timetable`.
    }

    pub(crate) fn complete_timetable_wait(&mut self) {
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
                self.finish_arrival_processing();
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

    pub(crate) fn tick_timetable_wait(&mut self) {
        if self.timetable_wait_remaining == 0 {
            return;
        }
        self.timetable_wait_remaining = self.timetable_wait_remaining.saturating_sub(1);
        if self.timetable_wait_remaining == 0 {
            self.complete_timetable_wait();
        }
    }

    /// Actualiza `dest` según la orden actual (vía adyacente para estaciones de tren).
    pub fn sync_order_destination(&mut self, map: &crate::map::Map) {
        if self.orders.is_empty() {
            return;
        }
        self.sanitize_current_order();
        self.update_real_order_index();
        let _ = self.update_order_dest(map, 0);
    }

    pub(super) fn do_advance_after_arrival(&mut self, pass_through: bool) {
        if pass_through {
            self.progress = 0;
        } else {
            self.progress = 255;
        }
        self.advance_to_next_order();
    }
}
