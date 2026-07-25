//! Horarios por orden (paridad reducida con `OpenTTD` timetable).

use crate::vehicle::{Vehicle, VehicleOrder};

/// Presets de espera en parada (ticks de simulación; ~37 Hz → 30 ≈ 0,8 s).
pub const WAIT_PRESETS: [u32; 5] = [0, 30, 60, 120, 300];

/// Presets de tiempo mínimo de viaje entre órdenes.
pub const TRAVEL_PRESETS: [u32; 5] = [0, 60, 120, 240, 600];

/// Segundos de simulación mostrados en la GUI (`Ticks::TICKS_PER_SECOND`).
pub const TICKS_PER_SECOND: u32 = 37;

#[must_use]
pub const fn cycle_preset(presets: &[u32], current: u32) -> u32 {
    let mut i = 0;
    while i < presets.len() {
        if presets[i] == current {
            let next = (i + 1) % presets.len();
            return presets[next];
        }
        i += 1;
    }
    presets[0]
}

#[must_use]
pub fn cycle_wait_ticks(current: u32) -> u32 {
    cycle_preset(&WAIT_PRESETS, current)
}

#[must_use]
pub fn cycle_travel_ticks(current: u32) -> u32 {
    cycle_preset(&TRAVEL_PRESETS, current)
}

/// Redondea hacia arriba al segundo de sim (como `UpdateVehicleTimetable` en `timetable_cmd.cpp`).
#[must_use]
pub fn round_timetable_ticks(ticks: u32) -> u32 {
    if ticks == 0 {
        return 0;
    }
    ticks
        .div_ceil(TICKS_PER_SECOND)
        .saturating_mul(TICKS_PER_SECOND)
        .max(TICKS_PER_SECOND)
}

impl Vehicle {
    /// Avanza el cronómetro de la orden actual (`Vehicle::current_order_time`).
    pub(crate) fn tick_timetable_clock(&mut self) {
        if self.timetable_active {
            self.current_order_time = self.current_order_time.saturating_add(1);
        }
    }

    /// Port reducido de `UpdateVehicleTimetable` (`timetable_cmd.cpp:466-572`).
    pub(crate) fn update_vehicle_timetable(&mut self, travelling: bool) {
        let time_taken = self.current_order_time;
        self.current_order_time = 0;

        if !self.timetable_active {
            return;
        }
        let Some(order) = self.orders.get(self.current_order).copied() else {
            return;
        };
        if order.is_conditional() {
            return;
        }

        let first_manual = 0usize;
        let just_started =
            travelling && self.current_order == first_manual && !self.timetable_started;

        if just_started {
            if self.timetable_start != 0 {
                self.timetable_lateness = i32::try_from(
                    self.sim_tick
                        .saturating_sub(u64::from(self.timetable_start)),
                )
                .unwrap_or(i32::MAX);
                self.timetable_start = 0;
            }
            self.timetable_started = true;
        }

        if !self.timetable_started {
            return;
        }

        if just_started {
            return;
        }

        let timetabled = if travelling {
            order.travel_ticks()
        } else {
            order.wait_ticks()
        };

        if self.timetable_autofill {
            let rounded = round_timetable_ticks(time_taken.max(1));
            let idx = self.current_order;
            if let Some(o) = self.orders.get_mut(idx) {
                if travelling {
                    *o = o.with_travel_ticks(rounded);
                } else if let Some(updated) = o.with_wait_ticks(rounded) {
                    *o = updated;
                }
            }
            if travelling && self.current_order == first_manual {
                self.timetable_autofill = false;
            }
            return;
        }

        if timetabled == 0 && (travelling || self.timetable_lateness >= 0) {
            return;
        }

        let delta = i32::try_from(timetabled).unwrap_or(i32::MAX)
            - i32::try_from(time_taken).unwrap_or(i32::MAX);
        self.timetable_lateness = self.timetable_lateness.saturating_sub(delta);

        if self.timetable_lateness > i32::try_from(timetabled).unwrap_or(i32::MAX) {
            let cycle = self.orders.iter().map(|o| o.cycle_ticks()).sum::<u32>();
            if cycle > 0 {
                let cycle_i = i32::try_from(cycle).unwrap_or(i32::MAX);
                if self.timetable_lateness > cycle_i {
                    self.timetable_lateness %= cycle_i;
                }
            }
        }
    }
}

impl VehicleOrder {
    /// Duración de un ciclo de horario (wait + travel por orden).
    #[must_use]
    pub const fn cycle_ticks(self) -> u32 {
        self.wait_ticks().saturating_add(self.travel_ticks())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileCoord;

    #[test]
    fn cycle_wait_wraps_to_zero() {
        assert_eq!(cycle_wait_ticks(0), 30);
        assert_eq!(cycle_wait_ticks(300), 0);
    }

    #[test]
    fn round_timetable_ticks_ceil_to_second() {
        assert_eq!(round_timetable_ticks(0), 0);
        assert_eq!(round_timetable_ticks(1), TICKS_PER_SECOND);
        assert_eq!(round_timetable_ticks(TICKS_PER_SECOND), TICKS_PER_SECOND);
        assert_eq!(
            round_timetable_ticks(TICKS_PER_SECOND + 1),
            TICKS_PER_SECOND * 2
        );
    }

    #[test]
    fn timetable_clock_increments_current_order_time() {
        let mut v = Vehicle::new(
            1,
            crate::vehicle::VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.timetable_active = true;
        v.tick_timetable_clock();
        assert_eq!(v.current_order_time, 1);
    }

    #[test]
    fn autofill_sets_travel_ticks_on_arrival() {
        let mut v = Vehicle::new(
            1,
            crate::vehicle::VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.timetable_active = true;
        v.timetable_autofill = true;
        v.timetable_started = true;
        v.orders = vec![VehicleOrder::station(TileCoord::new(1, 0))];
        v.current_order_time = 50;
        v.update_vehicle_timetable(true);
        assert_eq!(v.orders[0].travel_ticks(), TICKS_PER_SECOND * 2);
        assert!(!v.timetable_autofill);
    }

    #[test]
    fn lateness_increases_when_late() {
        let mut v = Vehicle::new(
            1,
            crate::vehicle::VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.timetable_active = true;
        v.timetable_started = true;
        let mut order = VehicleOrder::station(TileCoord::new(1, 0));
        order = order.with_travel_ticks(60);
        v.orders = vec![order];
        v.current_order_time = 100;
        v.update_vehicle_timetable(true);
        assert_eq!(v.timetable_lateness, 40);
    }

    use crate::vehicle::OrderNonStop;

    #[test]
    fn non_stop_enum_default_is_non_stop_destination() {
        assert_eq!(OrderNonStop::default(), OrderNonStop::NonStopDestination);
    }
}
