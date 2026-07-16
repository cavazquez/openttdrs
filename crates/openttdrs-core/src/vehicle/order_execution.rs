//! Ejecución de órdenes, avance tras carga/descarga, horarios.

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
            self.path.clear();
            self.no_network_route_to_order = false;
            self.sync_order_destination(map);
        }
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
        if self.orders[self.current_order].full_load() && self.cargo < self.capacity {
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
        self.current_order = (self.current_order + 1) % self.orders.len();
        self.origin = self.pos;
        self.timetable_leg_start_tick = self.sim_tick;
        if self.kind != super::model::VehicleKind::Train {
            self.dest = self.orders[self.current_order].destination();
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

    fn record_timetable_autofill_sample(&mut self, wait: u32, travel: u32) {
        if !self.timetable_autofill {
            return;
        }
        let idx = self.current_order;
        if self.timetable_autofill_samples.len() <= idx {
            self.timetable_autofill_samples.resize(idx + 1, (0, 0));
        }
        let (w, t) = &mut self.timetable_autofill_samples[idx];
        *w = u32::midpoint(*w, wait);
        *t = u32::midpoint(*t, travel);
        if let Some(order) = self.orders.get_mut(idx) {
            *order = order.with_travel_ticks(*t);
            if let Some(updated) = order.with_wait_ticks(*w) {
                *order = updated;
            }
        }
    }

    fn update_timetable_lateness_on_wait_end(&mut self, planned_wait: u32) {
        if !self.timetable_active {
            return;
        }
        let delta = i32::try_from(planned_wait).unwrap_or(i32::MAX);
        self.timetable_lateness = self.timetable_lateness.saturating_sub(delta);
    }

    pub(crate) fn complete_timetable_wait(&mut self) {
        let kind = self.timetable_wait_kind;
        let planned = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.wait_ticks());
        self.timetable_wait_kind = super::model::TimetableWaitKind::None;
        if kind != super::model::TimetableWaitKind::None && planned > 0 {
            self.update_timetable_lateness_on_wait_end(planned);
            let travel = self
                .orders
                .get(self.current_order)
                .map_or(0, |o| o.travel_ticks());
            self.record_timetable_autofill_sample(planned, travel);
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
                let pass_through = self.orders[self.current_order].is_pass_through();
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
        self.resolve_conditional_orders();
        self.skip_inapplicable_service_depot_orders();
        if self.orders.is_empty() {
            return;
        }
        let order = self.orders[self.current_order];
        if order.is_conditional() {
            return;
        }
        self.dest = if self.kind == super::model::VehicleKind::Aircraft {
            match order {
                crate::vehicle::order::VehicleOrder::Station { station, .. } => {
                    // Prefer apron/loading if the hangar ancla está en un footprint.
                    crate::airport::airport_loading_tile_at(map, station)
                }
                _ => crate::station::resolve_order_destination(map, self.kind, order),
            }
        } else {
            crate::station::resolve_order_destination(map, self.kind, order)
        };
    }

    /// Salta órdenes «servicio si hace falta» cuando no toca revisión.
    fn skip_inapplicable_service_depot_orders(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        let max = self.orders.len();
        for _ in 0..max {
            let Some(order) = self.orders.get(self.current_order).copied() else {
                break;
            };
            if matches!(
                order,
                crate::vehicle::order::VehicleOrder::Depot { stop: false, .. }
            ) && !self.requires_service()
            {
                self.current_order = (self.current_order + 1) % self.orders.len();
                self.path.clear();
                self.progress = 0;
                continue;
            }
            break;
        }
        self.resolve_conditional_orders();
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
