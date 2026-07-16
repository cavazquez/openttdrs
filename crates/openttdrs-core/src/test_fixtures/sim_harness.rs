//! Stepping de simulación para tests (#152).
//!
//! Unifica espera/timeout/diagnóstico; no define escenarios ni aserciones de dominio.

use crate::game_state::GameState;
use crate::vehicle::Vehicle;

/// Harness concreto de stepping (sin DSL genérico).
pub struct SimHarness;

impl SimHarness {
    /// Avanza la simulación hasta `max_ticks` o hasta que `predicate` sea verdadera.
    ///
    /// Cada iteración hace un `step` y luego evalúa la condición (igual que los
    /// loops históricos `for _ in 0..N { step; if cond { break } }`).
    ///
    /// # Panics
    ///
    /// Si se agota el presupuesto de ticks sin cumplir la condición.
    pub fn step_until(
        state: &mut GameState,
        max_ticks: u32,
        condition: &str,
        mut predicate: impl FnMut(&GameState) -> bool,
    ) {
        for _tick in 1..=max_ticks {
            state.step();
            if predicate(state) {
                return;
            }
        }
        panic!("timeout after {max_ticks} ticks waiting for: {condition}");
    }

    /// Espera a que `vehicles[vehicle_index].cargo == want`.
    ///
    /// # Panics
    ///
    /// Si el índice no existe o se agota `max_ticks`.
    pub fn until_vehicle_cargo(
        state: &mut GameState,
        vehicle_index: usize,
        want: u32,
        max_ticks: u32,
    ) {
        let condition = format!("vehicles[{vehicle_index}].cargo == {want}");
        Self::step_until(state, max_ticks, &condition, |s| {
            s.vehicles
                .get(vehicle_index)
                .is_some_and(|v| v.cargo == want)
        });
    }

    /// Avanza `tiles` cambios de tesela de algún vehículo del estado.
    ///
    /// # Panics
    ///
    /// Si en algún tile no hay movimiento dentro del presupuesto de ticks.
    pub fn advance_vehicle_tiles(state: &mut GameState, tiles: u32) {
        for _ in 0..tiles {
            advance_vehicle_one_tile(state);
        }
    }
}

fn advance_vehicle_one_tile(state: &mut GameState) {
    let max_ticks = state
        .vehicles
        .iter()
        .map(Vehicle::ticks_per_tile)
        .max()
        .unwrap_or(5)
        * 2;
    for _tick in 1..=max_ticks {
        let before: Vec<_> = state.vehicles.iter().map(|v| v.pos).collect();
        state.step();
        for (vehicle, prev) in state.vehicles.iter().zip(&before) {
            if vehicle.pos != *prev {
                return;
            }
        }
    }
    panic!("timeout after {max_ticks} ticks waiting for: any vehicle changed tile");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::{Vehicle, VehicleKind};

    #[test]
    fn step_until_stops_when_predicate_holds() {
        let mut state = GameState::new(4, 4);
        SimHarness::step_until(&mut state, 10, "tick >= 3", |s| s.tick.get() >= 3);
        assert_eq!(state.tick.get(), 3);
    }

    #[test]
    #[should_panic(expected = "timeout after 2 ticks waiting for: never")]
    fn step_until_reports_condition_on_timeout() {
        let mut state = GameState::new(4, 4);
        SimHarness::step_until(&mut state, 2, "never", |_| false);
    }

    #[test]
    fn until_vehicle_cargo_waits_for_target() {
        let mut state = GameState::new(4, 4);
        state.vehicles.push(Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        ));
        state.vehicles[0].cargo = 7;
        SimHarness::until_vehicle_cargo(&mut state, 0, 7, 1);
        assert_eq!(state.vehicles[0].cargo, 7);
    }
}
