//! Lógica de movimiento del vehículo: step, progress, dirección, velocidad.

use crate::engine::{
    ROAD_ACCEL_ORIGINAL, accelerate_train_speed, decelerate_road_speed, decelerate_train_speed,
    progress_step_for_speed, train_acceleration, update_road_speed,
};
use crate::map::{Map, TileCoord, slope_pixel_z};
use crate::rail_type::rail_type_from_tile;
use crate::train_movement::{ACCEL_SLOWDOWN, affect_speed_by_z_change, is_45_degree_turn};

/// Paso sub-tile de referencia (bus MPS en diagonal). Ver [`crate::REFERENCE_PROGRESS_STEP`].
pub const VEHICLE_PROGRESS_STEP: u8 = crate::engine::REFERENCE_PROGRESS_STEP;

/// Sprite cardinal intermedio al girar 90° entre dos diagonales.
#[must_use]
const fn turn_cardinal_direction(
    entry: super::model::VehicleDirection,
    exit: super::model::VehicleDirection,
) -> super::model::VehicleDirection {
    use super::model::{DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W};
    match (entry, exit) {
        (DIR_NE, DIR_SE) | (DIR_SE, DIR_NE) => DIR_E,
        (DIR_SE, DIR_SW) | (DIR_SW, DIR_SE) => DIR_S,
        (DIR_SW, DIR_NW | DIR_NE) | (DIR_NW | DIR_NE, DIR_SW) => DIR_W,
        (DIR_NW, DIR_NE | DIR_SE) | (DIR_NE | DIR_SE, DIR_NW) => DIR_N,
        _ => entry,
    }
}

impl super::model::Vehicle {
    /// Dirección del paso en curso (eje del carril / vía).
    #[must_use]
    pub fn movement_direction(&self) -> super::model::VehicleDirection {
        let Some(next) = self.movement_target() else {
            return self.direction;
        };
        super::direction_from_tile_step(self.pos, next)
    }

    /// Avance sub-tile por tick según motor y dirección.
    #[must_use]
    pub fn progress_step(&self) -> u8 {
        progress_step_for_speed(self.effective_speed(), self.movement_direction())
    }

    /// Ticks de sim estimados para cruzar una tesela en la dirección actual.
    #[must_use]
    pub fn ticks_per_tile(&self) -> u32 {
        let step = self.progress_step().max(1);
        255_u32.div_ceil(u32::from(step))
    }

    /// Como `OpenTTD` `GetImage`: semi-lleno/lleno cambia sprite en bus/camión.
    #[must_use]
    pub fn uses_loaded_road_sprite(&self) -> bool {
        if self.cargo < self.capacity / 2 {
            return false;
        }
        matches!(
            self.kind,
            super::model::VehicleKind::Bus
                | super::model::VehicleKind::Truck
                | super::model::VehicleKind::Tram
                | super::model::VehicleKind::Ship
                | super::model::VehicleKind::Aircraft
        )
    }

    /// Dirección de sprite para render (8 vías; cardinales en la mitad de giros).
    #[must_use]
    pub fn render_direction(&self) -> super::model::VehicleDirection {
        let Some(next) = self.movement_target() else {
            return self.direction;
        };
        let entry = super::direction_from_tile_step(self.pos, next);
        if self.progress < 128 {
            return entry;
        }
        if let Some(&after) = self.path.get(1) {
            let exit = super::direction_from_tile_step(next, after);
            if exit != entry {
                return turn_cardinal_direction(entry, exit);
            }
        }
        entry
    }

    /// Siguiente tesela hacia la que avanza (path BFS o paso Manhattan).
    #[must_use]
    pub fn movement_target(&self) -> Option<TileCoord> {
        if !self.running {
            return None;
        }
        if let Some(&next) = self.path.front() {
            return Some(next);
        }
        if self.pos == self.dest {
            return None;
        }
        // Un tren o barco nunca avanza fuera de la red: sin camino no se mueve.
        if matches!(
            self.kind,
            super::model::VehicleKind::Train
                | super::model::VehicleKind::Ship
                | super::model::VehicleKind::Aircraft
        ) {
            return None;
        }
        if !self.orders.is_empty() {
            return None;
        }
        let dx = self.dest.x - self.pos.x;
        let dy = self.dest.y - self.pos.y;
        if dx == 0 && dy == 0 {
            return None;
        }
        Some(if dx != 0 {
            TileCoord::new(self.pos.x + dx.signum(), self.pos.y)
        } else {
            TileCoord::new(self.pos.x, self.pos.y + dy.signum())
        })
    }

    /// Avanza un tick de sim: sub-tile y, al completar 255, la tesela siguiente.
    pub fn step(&mut self) {
        self.step_with_map(None);
    }

    /// Como [`Self::step`], aplicando límites de velocidad del mapa (puentes).
    pub fn step_with_map(&mut self, map: Option<&Map>) {
        if !self.running {
            self.update_movement_speed(map);
            self.progress = 0;
            return;
        }

        self.resolve_conditional_orders();

        if self.holding_for_timetable() {
            self.update_movement_speed(map);
            return;
        }

        // Carga/descarga gradual: no mover hasta cerrar la transferencia.
        if self.cargo_transfer_active() {
            self.cur_speed = 0;
            self.progress = 255;
            return;
        }

        // Cierra la ventana de carga abierta en la llegada del tick anterior.
        // En `sim_step` las fases de carga/descarga corren antes que el
        // movimiento, así que a esta altura ya tuvieron su oportunidad: si
        // actuaron, la orden avanzó y la bandera se limpió; si no, la salida
        // de la parada se decide ahora.
        self.complete_station_load_window();

        self.update_movement_speed(map);

        if self.kind == super::model::VehicleKind::Train {
            self.apply_immediate_train_turnaround(map);
        }

        if self.movement_target().is_none() {
            if self.cur_speed == 0 && self.pos == self.dest {
                self.advance_destination_after_arrival();
            }
            return;
        }

        if self.cur_speed == 0 {
            return;
        }

        if self.depart_turn > 0 {
            let step = u16::from(self.progress_step().max(1));
            let next = u16::from(self.depart_turn) + step;
            if next < 255 {
                if let Ok(t) = u8::try_from(next) {
                    self.depart_turn = t;
                }
            } else {
                self.depart_turn = 0;
                self.progress = 0;
                if let Some(next) = self.movement_target() {
                    self.set_direction_with_curve_penalty(
                        super::direction_from_tile_step(self.pos, next),
                        map,
                    );
                }
            }
            return;
        }

        if self.progress == 255 && self.needs_depart_turnaround() {
            self.depart_turn = 1;
            return;
        }

        let step = u16::from(self.progress_step());
        let next = u16::from(self.progress) + step;
        if next < 255 {
            if let Ok(progress) = u8::try_from(next) {
                self.progress = progress;
            }
            if let Some(map) = map {
                self.sync_train_slope_speed(map);
            }
            return;
        }
        let mut remaining = next;
        loop {
            remaining = remaining.saturating_sub(255);
            self.progress = 0;
            self.advance_one_tile(map);
            if remaining < 255 {
                // Si `advance_destination_after_arrival` ancló en 255, no pisar con el resto.
                if self.progress != 255
                    && let Ok(progress) = u8::try_from(remaining)
                {
                    self.progress = progress;
                }
                if let Some(map) = map {
                    self.sync_train_slope_speed(map);
                }
                return;
            }
            if self.movement_target().is_none() {
                if let Some(map) = map {
                    self.sync_train_slope_speed(map);
                }
                return;
            }
        }
    }

    /// Máximo de teselas recordadas para huella PBS / consist.
    const RAIL_HISTORY_CAP: usize = 32;

    fn push_rail_tile_history(&mut self, left: TileCoord) {
        if self.kind != super::model::VehicleKind::Train {
            return;
        }
        if self.rail_tile_history.front() != Some(&left) {
            self.rail_tile_history.push_front(left);
        }
        while self.rail_tile_history.len() > Self::RAIL_HISTORY_CAP {
            self.rail_tile_history.pop_back();
        }
    }

    pub(super) fn advance_one_tile(&mut self, map: Option<&Map>) {
        if let Some(next) = self.path.pop_front() {
            self.update_direction_step(self.pos, next, map);
            if self.orders.is_empty() {
                self.origin = self.pos;
            }
            let left = self.pos;
            self.pos = next;
            self.push_rail_tile_history(left);
            if self.pos == self.dest {
                self.advance_destination_after_arrival();
            }
        } else if self.pos == self.dest {
            self.advance_destination_after_arrival();
        } else {
            if matches!(
                self.kind,
                super::model::VehicleKind::Train
                    | super::model::VehicleKind::Ship
                    | super::model::VehicleKind::Aircraft
            ) || !self.orders.is_empty()
            {
                return;
            }
            let dx = self.dest.x - self.pos.x;
            let dy = self.dest.y - self.pos.y;
            let previous = self.pos;
            if dx != 0 {
                self.pos.x += dx.signum();
            } else if dy != 0 {
                self.pos.y += dy.signum();
            }
            if self.pos != previous {
                self.update_direction_step(previous, self.pos, map);
            }
            if self.orders.is_empty() && self.pos != previous {
                self.origin = previous;
            }
            if self.pos == self.dest && !self.orders.is_empty() {
                self.advance_destination_after_arrival();
            }
        }
        if let Some(map) = map {
            self.sync_train_slope_speed(map);
        }
    }

    /// `UpdateInclination` + `AffectSpeedByZChange` (`ground_vehicle.hpp` / `train_cmd.cpp`).
    ///
    /// Usa Z en píxeles (`GetSlopePixelZ` ≈ base·8 + partial) en la sub-tesela actual.
    pub(super) fn sync_train_slope_speed(&mut self, map: &Map) {
        if self.kind != super::model::VehicleKind::Train || !self.is_consist_head() {
            return;
        }
        let (sub_x, sub_y) = crate::road_movement::vehicle_subtile(self);
        let new_z = slope_pixel_z(map, self.pos, sub_x, sub_y);
        let Some(old_z) = self.z_pos else {
            self.z_pos = Some(new_z);
            return;
        };
        let z_diff = new_z - old_z;
        self.z_pos = Some(new_z);
        if z_diff == 0 {
            return;
        }
        let rail_idx = map
            .get(self.pos)
            .map_or(0, |t| rail_type_from_tile(t).accel_table_index());
        let mut max_speed = self.effective_engine().max_speed;
        if let Some(bridge_cap) = crate::bridge_spec::bridge_max_speed_for_tile(map, self.pos) {
            max_speed = max_speed.min(bridge_cap);
        }
        self.cur_speed = affect_speed_by_z_change(self.cur_speed, z_diff, rail_idx, max_speed);
    }

    fn update_direction_step(&mut self, from: TileCoord, to: TileCoord, map: Option<&Map>) {
        self.set_direction_with_curve_penalty(super::direction_from_tile_step(from, to), map);
    }

    /// Cambia `direction` aplicando penalización de curva del modelo original:
    /// carretera `v->cur_speed -= v->cur_speed >> 2` (`roadveh_cmd.cpp:1481`);
    /// tren `_accel_slowdown` (`train_cmd.cpp:3564-3568`, locomotora).
    pub(super) fn set_direction_with_curve_penalty(
        &mut self,
        new_dir: super::model::VehicleDirection,
        map: Option<&Map>,
    ) {
        if new_dir != self.direction {
            match self.kind {
                super::model::VehicleKind::Train => {
                    // Índice por railtype: normal/eléctrico=0, mono=1, maglev=2.
                    let rail_idx = map
                        .and_then(|m| m.get(self.pos))
                        .map_or(0, |t| rail_type_from_tile(t).accel_table_index())
                        .min(ACCEL_SLOWDOWN.len() - 1);
                    let params = &ACCEL_SLOWDOWN[rail_idx];
                    let turn = if is_45_degree_turn(self.direction, new_dir) {
                        params.small_turn
                    } else {
                        params.large_turn
                    };
                    let penalty = (u32::from(turn) * u32::from(self.cur_speed)) >> 8;
                    self.cur_speed = self
                        .cur_speed
                        .saturating_sub(u16::try_from(penalty).unwrap_or(0));
                }
                super::model::VehicleKind::Bus
                | super::model::VehicleKind::Truck
                | super::model::VehicleKind::Tram
                | super::model::VehicleKind::Ship
                | super::model::VehicleKind::Aircraft => {
                    self.cur_speed -= self.cur_speed >> 2;
                }
            }
        }
        self.direction = new_dir;
    }

    fn update_movement_speed(&mut self, map: Option<&Map>) {
        let engine = self.effective_engine();
        let mut max_speed = engine.max_speed;
        if let Some(map) = map
            && let Some(bridge_cap) = crate::bridge_spec::bridge_max_speed_for_tile(map, self.pos)
        {
            max_speed = max_speed.min(bridge_cap);
        }
        let (power, weight) = if self.kind == super::model::VehicleKind::Train
            && (self.cached_power_hp > 0 || self.cached_weight_t > 0)
        {
            (
                self.cached_power_hp.max(engine.power_hp),
                self.cached_weight_t.max(engine.weight_t),
            )
        } else {
            (engine.power_hp, engine.weight_t)
        };
        if self.running && self.movement_target().is_some() {
            let (cur, sub) = if self.kind == super::model::VehicleKind::Train {
                accelerate_train_speed(self.cur_speed, self.subspeed, power, weight, max_speed)
            } else {
                update_road_speed(
                    self.cur_speed,
                    self.subspeed,
                    ROAD_ACCEL_ORIGINAL,
                    0,
                    max_speed,
                )
            };
            self.cur_speed = cur;
            self.subspeed = sub;
        } else {
            let (cur, sub) = if self.kind == super::model::VehicleKind::Train {
                let accel = train_acceleration(power, weight);
                decelerate_train_speed(self.cur_speed, self.subspeed, accel)
            } else {
                decelerate_road_speed(self.cur_speed, self.subspeed)
            };
            self.cur_speed = cur;
            self.subspeed = sub;
        }
        if self.cur_speed > max_speed {
            self.cur_speed = max_speed;
        }
    }

    fn advance_destination_after_arrival(&mut self) {
        self.path.clear();
        self.depart_turn = 0;
        if self.orders.is_empty() {
            self.progress = 0;
            return;
        }
        let early = self.travel_early_wait_ticks();
        if early > 0 {
            self.timetable_wait_remaining = early.max(1);
            self.timetable_wait_kind = super::model::TimetableWaitKind::TravelEarly;
            self.progress = 255;
            return;
        }
        self.finish_arrival_processing();
    }

    /// Salida con sentido opuesto al de llegada (giro animado en parada bus/camión).
    #[must_use]
    pub(crate) fn needs_depart_turnaround(&self) -> bool {
        if matches!(
            self.kind,
            super::model::VehicleKind::Train
                | super::model::VehicleKind::Ship
                | super::model::VehicleKind::Aircraft
        ) {
            return false;
        }
        let Some(next) = self.movement_target() else {
            return false;
        };
        let outbound = super::direction_from_tile_step(self.pos, next);
        outbound == super::reverse_direction(self.direction)
    }

    /// Tren: invierte el rumbo en el acto si la siguiente tesela exige sentido opuesto.
    fn apply_immediate_train_turnaround(&mut self, map: Option<&Map>) {
        let Some(next) = self.movement_target() else {
            return;
        };
        let outbound = super::direction_from_tile_step(self.pos, next);
        if outbound != super::reverse_direction(self.direction) {
            return;
        }
        self.set_direction_with_curve_penalty(outbound, map);
        self.depart_turn = 0;
        if self.progress == 255 {
            self.progress = 0;
        }
    }

    fn holding_for_timetable(&self) -> bool {
        self.timetable_active && self.timetable_wait_remaining > 0
    }

    pub(crate) fn resolve_conditional_orders(&mut self) {
        const MAX_STEPS: usize = 64;
        for _ in 0..MAX_STEPS {
            let Some(order) = self.orders.get(self.current_order).copied() else {
                break;
            };
            if !order.is_conditional() {
                break;
            }
            self.current_order = order.evaluate_conditional(self);
            self.path.clear();
            self.progress = 0;
        }
    }

    /// Cierra la ventana de carga abierta en la llegada (inicio del `step`
    /// siguiente). Si las fases de carga/descarga actuaron, ya avanzaron la
    /// orden (`advance_after_loading`/`_unloading`) y aquí no queda nada.
    pub(super) fn complete_station_load_window(&mut self) {
        if !self.awaiting_load_window {
            return;
        }
        // Carga/descarga gradual: mantener la ventana abierta mientras haya transferencia.
        if self.cargo_transfer_active() {
            self.progress = 255;
            return;
        }
        self.awaiting_load_window = false;
        if !self.orders.is_empty() && self.pos == self.dest && self.progress == 255 {
            self.finish_arrival_after_load_window();
        }
    }

    pub(super) fn finish_arrival_processing(&mut self) {
        // Llegada a una orden de estación: abre una «ventana de carga» de un
        // tick (análogo a `Vehicle::BeginLoading` de OpenTTD) para que la fase
        // de carga/descarga de `sim_step` actúe antes de avanzar la orden.
        // `sim_step::finish_station_load_windows` la cierra tras esa fase.
        if !self.awaiting_load_window
            && matches!(
                self.orders.get(self.current_order),
                Some(crate::vehicle::order::VehicleOrder::Station { .. })
            )
        {
            self.awaiting_load_window = true;
            self.progress = 255;
            return;
        }
        self.finish_arrival_after_load_window();
    }

    pub(super) fn finish_arrival_after_load_window(&mut self) {
        if self.cargo_transfer_active() {
            self.progress = 255;
            return;
        }
        if self.cargo > 0
            && !self
                .orders
                .get(self.current_order)
                .is_some_and(|o| o.no_unload())
        {
            self.progress = 255;
            return;
        }
        self.sanitize_current_order();
        let Some(order) = self.current_order_ref().copied() else {
            return;
        };
        let pass_through = order.is_pass_through();
        if order.is_depot() {
            let halt = order.depot_stops();
            let needs = self.requires_service();
            if halt || needs {
                if let Some(cargo) = order.depot_refit_cargo() {
                    self.pending_depot_order_refit = Some(cargo);
                }
                self.service_at_depot();
            }
            if halt {
                self.running = false;
                self.progress = 255;
                return;
            }
            self.do_advance_after_arrival(true);
            return;
        }
        if order.full_load() && self.cargo < self.capacity {
            self.progress = 255;
            return;
        }
        if self.schedule_timetable_wait(super::model::TimetableWaitKind::AfterArrival) {
            self.progress = 255;
            return;
        }
        self.do_advance_after_arrival(pass_through);
    }

    fn travel_early_wait_ticks(&self) -> u32 {
        if !self.timetable_active {
            return 0;
        }
        let travel = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.travel_ticks());
        if travel == 0 || self.timetable_leg_start_tick == 0 {
            return 0;
        }
        let elapsed = self.sim_tick.saturating_sub(self.timetable_leg_start_tick);
        if elapsed >= u64::from(travel) {
            return 0;
        }
        u32::try_from(u64::from(travel).saturating_sub(elapsed)).unwrap_or(1)
    }
}
