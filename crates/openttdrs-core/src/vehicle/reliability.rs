//! Fiabilidad, servicio y averías del vehículo.

use crate::cargodist::parity::Randomizer;
use crate::vehicle::VehicleKind;

/// Umbral de fiabilidad bajo el cual conviene servicio en depósito.
pub const SERVICING_RELIABILITY_THRESHOLD: u16 = 5_000;
/// Intervalo de revisión por defecto (`OpenTTD` `service_interval` ≈ 150 días).
pub const DEFAULT_SERVICE_INTERVAL_DAYS: u16 = 150;
/// Duración máxima de avería en ticks (`breakdown_delay` hasta 255).
pub const BREAKDOWN_DURATION_TICKS: u32 = 255;
/// Velocidad mínima para acumular riesgo de avería (`vehicle.cpp:1340`).
pub const MIN_SPEED_FOR_BREAKDOWN: u16 = 5;
/// Días de calendario por año (paridad `CalendarTime::DAYS_IN_LEAP_YEAR`).
pub const DAYS_PER_VEHICLE_YEAR: u32 = 366;

/// Tabla `_breakdown_chance[rel >> 10]` (`vehicle.cpp:1303-1312`).
const BREAKDOWN_CHANCE_TABLE: [u8; 64] = [
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 13, 13, 13,
    13, 14, 15, 16, 17, 19, 21, 25, 28, 31, 34, 37, 40, 44, 48, 52, 56, 60, 64, 68, 72, 80, 90,
    100, 110, 120, 130, 140, 150, 170, 190, 210, 230, 250, 250, 250,
];

/// Bonus de fiabilidad efectiva para barcos (`vehicle.cpp:1355`).
const SHIP_RELIABILITY_BONUS: u32 = 0x6666;

pub(crate) fn initial_reliability_for_engine(
    engine_id: u16,
    kind: super::model::VehicleKind,
) -> u16 {
    u16::from(crate::engine::engine_for_vehicle(kind, engine_id).reliability_pct) * 100
}

pub(crate) fn init_vehicle_reliability_from_engine(
    vehicle: &mut super::model::Vehicle,
    engine: &crate::engine::EngineDef,
) {
    vehicle.reliability =
        initial_reliability_for_engine(engine.id, engine.kind);
    vehicle.reliability_spd_dec = engine.reliability_spd_dec;
    vehicle.max_age_days = u32::from(engine.lifelength_years) * DAYS_PER_VEHICLE_YEAR;
}

fn scale_reliability_to_openttd(reliability: u16) -> u32 {
    u32::from(reliability) * 65535 / 10000
}

fn decay_reliability_port(reliability: u16, spd_dec: u16) -> u16 {
    let dec = u32::from(spd_dec) * 10000 / 65535;
    reliability.saturating_sub(u16::try_from(dec).unwrap_or(u16::MAX))
}

fn effective_reliability_for_breakdown(reliability: u16, kind: VehicleKind) -> u32 {
    let mut rel = scale_reliability_to_openttd(reliability);
    if kind == VehicleKind::Ship {
        rel = rel.saturating_add(SHIP_RELIABILITY_BONUS);
    }
    rel.min(65535)
}

fn breakdown_table_index(reliability: u16, kind: VehicleKind) -> usize {
    let rel = effective_reliability_for_breakdown(reliability, kind);
    usize::from((rel >> 10).min(63) as u8)
}

/// `Chance16I(a, b, r)` con los 16 bits bajos de `r`.
fn chance16i(a: u32, b: u32, r: u32) -> bool {
    if b == 0 {
        return false;
    }
  ((u32::from(r as u16) * b + b / 2) >> 16) < a
}

fn extract_bits(value: u32, offset: u32, count: u32) -> u8 {
    let mask = if count >= 32 {
        u32::MAX
    } else {
        (1_u32 << count) - 1
    };
    u8::try_from((value >> offset) & mask).unwrap_or(u8::MAX)
}

impl super::model::Vehicle {
    /// Restaura fiabilidad tras servicio en depósito.
    pub fn service_at_depot(&mut self) {
        let engine_id = self
            .engine_id
            .unwrap_or_else(|| crate::engine::default_engine_id(self.kind));
        self.reliability = initial_reliability_for_engine(engine_id, self.kind);
        self.needs_servicing = false;
        self.breakdown_ctr = 0;
        self.breakdown_delay = 0;
        self.breakdown_chance = 0;
        self.last_service_day =
            crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
    }

    /// ¿Toca revisión? (`NeedsServicing`: intervalo o fiabilidad baja).
    ///
    /// Órdenes `Depot { stop: false }` = «servicio si hace falta» (se saltan si esto es false).
    #[must_use]
    pub fn requires_service(&self) -> bool {
        if self.breakdown_ctr != 0 {
            return false;
        }
        if self.reliability < SERVICING_RELIABILITY_THRESHOLD {
            return true;
        }
        let day = crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
        let interval = u64::from(self.service_interval_days.max(1));
        day.saturating_sub(self.last_service_day) >= interval
    }

    /// ¿El vehículo está parado por avería activa? (`breakdown_ctr == 1`).
    ///
    /// Con `ctr > 2` aún se mueve mientras cuenta atrás hacia la avería; con `ctr == 2`
    /// el tick actual la dispara (`HandleBreakdown`).
    #[must_use]
    pub fn is_broken_down(&self) -> bool {
        self.breakdown_ctr == 1 && self.kind != VehicleKind::Aircraft
    }

    /// Edad del vehículo en días de calendario desde la compra.
    #[must_use]
    pub fn vehicle_age_days(&self, current_tick: u64) -> u64 {
        let age_ticks = current_tick.saturating_sub(self.build_tick);
        age_ticks / u64::from(crate::economy::TICKS_PER_DAY)
    }

    /// `AgeVehicle`: duplica `reliability_spd_dec` en ciertos años tras `max_age`.
    pub fn age_vehicle_calendar_day(&mut self, current_tick: u64) {
        if !self.is_primary_for_aging() {
            return;
        }
        let age_days = self.vehicle_age_days(current_tick);
        let past_max = age_days.saturating_sub(u64::from(self.max_age_days));
        for i in 0_u32..=4_u32 {
            let boundary = u64::from(i) * u64::from(DAYS_PER_VEHICLE_YEAR);
            if past_max == boundary {
                self.reliability_spd_dec = self.reliability_spd_dec.saturating_mul(2);
                break;
            }
        }
    }

    /// Barrido diario de economía: decaimiento de fiabilidad y acumulación de avería.
    pub fn check_vehicle_breakdown(&mut self, rng: &mut Randomizer) {
        if !self.running {
            return;
        }
        self.reliability =
            decay_reliability_port(self.reliability, self.reliability_spd_dec);
        self.needs_servicing = self.requires_service();

        if self.breakdown_ctr != 0 {
            return;
        }
        if self.cur_speed < MIN_SPEED_FOR_BREAKDOWN {
            return;
        }

        let r = rng.next();
        let mut chance = u16::from(self.breakdown_chance) + 1;
        if chance16i(1, 25, r) {
            chance += 25;
        }
        self.breakdown_chance = chance.min(255) as u8;

        let threshold = BREAKDOWN_CHANCE_TABLE[breakdown_table_index(self.reliability, self.kind)];
        if u16::from(threshold) > chance {
            return;
        }

        self.breakdown_ctr = extract_bits(r, 16, 6) + 0x3F;
        self.breakdown_delay = extract_bits(r, 24, 7) + 0x80;
        self.breakdown_chance = 0;
    }

    /// Fases de `HandleBreakdown` durante el movimiento.
    ///
    /// Devuelve `true` si el vehículo acaba de entrar en avería (humo/sonido).
    pub fn handle_breakdown(&mut self, tick: u64) -> bool {
        match self.breakdown_ctr {
            0 => false,
            2 => {
                self.breakdown_ctr = 1;
                if self.kind != VehicleKind::Aircraft {
                    self.cur_speed = 0;
                }
                true
            }
            1 => {
                if self.kind == VehicleKind::Aircraft {
                    return false;
                }
                let half_rate =
                    self.kind == VehicleKind::Train && (tick & 3) != 0;
                if !half_rate && self.breakdown_delay > 0 {
                    self.breakdown_delay -= 1;
                    if self.breakdown_delay == 0 {
                        self.breakdown_ctr = 0;
                    }
                }
                false
            }
            _ => {
                if !self.cargo_loading && !self.cargo_unloading {
                    self.breakdown_ctr -= 1;
                }
                false
            }
        }
    }

    fn is_primary_for_aging(&self) -> bool {
        if self.prev_unit.is_some() {
            return false;
        }
        if self.kind == VehicleKind::Train {
            return self
                .engine_id
                .map(|id| crate::engine::engine_for_vehicle(self.kind, id).is_train_engine())
                .unwrap_or(true);
        }
        true
    }

    /// Edad del vehículo en años de calendario aproximados.
    #[must_use]
    pub fn vehicle_age_years(&self, current_tick: u64) -> u32 {
        let age_ticks = current_tick.saturating_sub(self.build_tick);
        u32::try_from(age_ticks / crate::economy::TICKS_PER_YEAR).unwrap_or(u32::MAX)
    }
}

/// Procesa el barrido diario de fiabilidad/avería para vehículos primarios en marcha.
pub(crate) fn process_vehicle_economy_day(state: &mut crate::GameState, tick: u64) {
    if tick == 0 || !tick.is_multiple_of(u64::from(crate::economy::TICKS_PER_DAY)) {
        return;
    }
    for vehicle in &mut state.vehicles {
        vehicle.sim_tick = tick;
        if vehicle.prev_unit.is_some() {
            continue;
        }
        vehicle.age_vehicle_calendar_day(tick);
        vehicle.check_vehicle_breakdown(&mut state.cargo_rng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::Vehicle;

    #[test]
    fn breakdown_table_uses_scaled_reliability() {
        assert_eq!(breakdown_table_index(10_000, VehicleKind::Bus), 63);
        assert_eq!(breakdown_table_index(1_000, VehicleKind::Bus), 6);
        assert_eq!(
            breakdown_table_index(1_000, VehicleKind::Ship),
            breakdown_table_index(5_000, VehicleKind::Bus)
        );
    }

    #[test]
    fn reliability_decays_by_engine_spd_dec() {
        let mut v = Vehicle::new(1, VehicleKind::Bus, TileCoord::new(0, 0), TileCoord::new(1, 0));
        v.reliability = 5_000;
        v.reliability_spd_dec = 80;
        v.running = true;
        let before = v.reliability;
        v.check_vehicle_breakdown(&mut Randomizer::new(1));
        assert!(v.reliability < before);
    }

    #[test]
    fn reliability_spd_dec_doubles_after_max_age_year_boundary() {
        let mut v = Vehicle::new(1, VehicleKind::Bus, TileCoord::new(0, 0), TileCoord::new(1, 0));
        v.reliability_spd_dec = 80;
        v.max_age_days = DAYS_PER_VEHICLE_YEAR;
        v.build_tick = 0;
        let tick = u64::from(DAYS_PER_VEHICLE_YEAR) * u64::from(crate::economy::TICKS_PER_DAY);
        v.age_vehicle_calendar_day(tick);
        assert_eq!(v.reliability_spd_dec, 160);
    }

    #[test]
    fn breakdown_requires_min_speed_for_chance() {
        let mut v = Vehicle::new(1, VehicleKind::Truck, TileCoord::new(0, 0), TileCoord::new(1, 0));
        v.reliability = 100;
        v.running = true;
        v.cur_speed = 0;
        v.check_vehicle_breakdown(&mut Randomizer::new(99));
        assert_eq!(v.breakdown_chance, 0);
        assert_eq!(v.breakdown_ctr, 0);
    }

    #[test]
    fn handle_breakdown_stops_vehicle_at_phase_two() {
        let mut v = Vehicle::new(1, VehicleKind::Bus, TileCoord::new(0, 0), TileCoord::new(1, 0));
        v.breakdown_ctr = 2;
        v.breakdown_delay = 120;
        v.cur_speed = 40;
        assert!(v.handle_breakdown(0));
        assert_eq!(v.breakdown_ctr, 1);
        assert_eq!(v.cur_speed, 0);
    }

    #[test]
    fn low_reliability_can_trigger_breakdown_with_rng() {
        let mut v = Vehicle::new(7, VehicleKind::Truck, TileCoord::new(0, 0), TileCoord::new(1, 0));
        v.reliability = 100;
        v.running = true;
        v.cur_speed = 50;
        v.breakdown_chance = 250;
        v.check_vehicle_breakdown(&mut Randomizer::new(42));
        assert!(v.breakdown_ctr > 0);
        assert!(v.breakdown_delay >= 0x80);
    }
}
