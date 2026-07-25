//! Fiabilidad, servicio y averías del vehículo.

/// Umbral de fiabilidad bajo el cual conviene servicio en depósito.
pub const SERVICING_RELIABILITY_THRESHOLD: u16 = 5_000;
/// Intervalo de revisión por defecto (`OpenTTD` `service_interval` ≈ 150 días).
pub const DEFAULT_SERVICE_INTERVAL_DAYS: u16 = 150;
/// Duración de una avería (~3 días de calendario).
pub const BREAKDOWN_DURATION_TICKS: u32 = crate::economy::TICKS_PER_DAY * 3;

pub(crate) fn initial_reliability_for_engine(
    engine_id: u16,
    kind: super::model::VehicleKind,
) -> u16 {
    u16::from(crate::engine::engine_for_vehicle(kind, engine_id).reliability_pct) * 100
}

impl super::model::Vehicle {
    /// Restaura fiabilidad tras servicio en depósito.
    pub fn service_at_depot(&mut self) {
        let engine_id = self
            .engine_id
            .unwrap_or_else(|| crate::engine::default_engine_id(self.kind));
        self.reliability = initial_reliability_for_engine(engine_id, self.kind);
        self.needs_servicing = false;
        self.breakdown_ticks_remaining = 0;
        self.last_service_day =
            crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
    }

    /// ¿Toca revisión? (`NeedsServicing`: intervalo o fiabilidad baja).
    ///
    /// Órdenes `Depot { stop: false }` = «servicio si hace falta» (se saltan si esto es false).
    #[must_use]
    pub fn requires_service(&self) -> bool {
        if self.breakdown_ticks_remaining > 0 {
            return false;
        }
        if self.reliability < SERVICING_RELIABILITY_THRESHOLD {
            return true;
        }
        let day = crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
        let interval = u64::from(self.service_interval_days.max(1));
        day.saturating_sub(self.last_service_day) >= interval
    }

    /// Comprueba avería durante el movimiento; devuelve `true` si acaba de averiarse.
    pub fn check_breakdown(&mut self, tick: u64) -> bool {
        if self.breakdown_ticks_remaining > 0 {
            self.breakdown_ticks_remaining = self.breakdown_ticks_remaining.saturating_sub(1);
            self.cur_speed = 0;
            return false;
        }
        if !self.running || self.cur_speed == 0 {
            return false;
        }
        if tick.is_multiple_of(256) {
            self.reliability = self.reliability.saturating_sub(10);
            self.needs_servicing = self.requires_service();
        }
        if self.reliability >= 4_000 {
            return false;
        }
        let chance = (tick.wrapping_mul(u64::from(self.id.wrapping_add(1))) % 256) as u32;
        if chance != 0 {
            return false;
        }
        self.breakdown_ticks_remaining = BREAKDOWN_DURATION_TICKS;
        self.cur_speed = 0;
        true
    }

    /// Edad del vehículo en años de calendario aproximados.
    #[must_use]
    pub fn vehicle_age_years(&self, current_tick: u64) -> u32 {
        let age_ticks = current_tick.saturating_sub(self.build_tick);
        u32::try_from(age_ticks / crate::economy::TICKS_PER_YEAR).unwrap_or(u32::MAX)
    }
}
