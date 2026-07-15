//! Salida de depósito ferroviario (`CheckTrainStayInDepot`).
//!
//! Paridad de `train_cmd.cpp`: espera ~37 ticks y chequea señal/reserva en la
//! boca antes de permitir el primer avance. Tras autorizar la salida se marca
//! `depot_leave_cleared` (equivalente a dejar de estar en `Track::Depot` aunque
//! la tesela siga siendo el depósito hasta el cruce).

use crate::depot::rail_depot_entrance_tile;
use crate::map::{Map, TileCoord, TileKind};
use crate::train_consist::consist_unit_ids;
use crate::vehicle::{Vehicle, VehicleKind};

/// Ticks de espera en depósito antes de intentar salir (`CheckTrainStayInDepot`).
pub const TRAIN_DEPOT_LEAVE_WAIT_TICKS: u32 = 37;

/// `true` si el tren debe permanecer en el depósito este tick (sin avanzar).
pub fn tick_train_stay_in_depot(map: &Map, vehicles: &mut [Vehicle], index: usize) -> bool {
    let Some(vehicle) = vehicles.get(index) else {
        return false;
    };
    if vehicle.kind != VehicleKind::Train || !vehicle.running || !vehicle.is_consist_head() {
        return false;
    }
    if map.get_kind(vehicle.pos) != Some(TileKind::RailDepot) {
        // Fuera del depósito: limpiar gate para la próxima entrada.
        if vehicle.depot_leave_cleared
            && let Some(v) = vehicles.get_mut(index)
        {
            v.depot_leave_cleared = false;
        }
        return false;
    }
    if vehicle.depot_leave_cleared {
        return false;
    }

    let head_id = vehicle.id;
    let head_pos = vehicle.pos;
    let force = vehicle.force_proceed;
    let power = vehicle.cached_power_hp;
    let engine_power = vehicle
        .engine_id
        .and_then(crate::engine::engine_by_id)
        .map_or(0, |e| e.power_hp);

    let unit_ids = consist_unit_ids(vehicles, head_id);
    for id in &unit_ids {
        let Some(u) = vehicles.iter().find(|v| v.id == *id) else {
            return false;
        };
        if u.pos != head_pos || map.get_kind(u.pos) != Some(TileKind::RailDepot) {
            return false;
        }
    }

    if power == 0 && engine_power == 0 {
        if let Some(v) = vehicles.get_mut(index) {
            v.running = false;
            v.cur_speed = 0;
        }
        return true;
    }

    let exit_blocked = depot_exit_blocked(map, vehicles, index);

    let Some(vehicle) = vehicles.get_mut(index) else {
        return false;
    };

    if force {
        vehicle.wait_counter = 0;
        if exit_blocked {
            vehicle.cur_speed = 0;
            return true;
        }
        vehicle.depot_leave_cleared = true;
        return false;
    }

    vehicle.wait_counter = vehicle.wait_counter.saturating_add(1);
    if vehicle.wait_counter < TRAIN_DEPOT_LEAVE_WAIT_TICKS {
        vehicle.cur_speed = 0;
        return true;
    }
    vehicle.wait_counter = 0;

    if exit_blocked {
        vehicle.cur_speed = 0;
        return true;
    }
    vehicle.depot_leave_cleared = true;
    false
}

fn depot_exit_blocked(map: &Map, vehicles: &[Vehicle], index: usize) -> bool {
    let Some(vehicle) = vehicles.get(index) else {
        return false;
    };
    if crate::rail_signals::train_blocked_by_signal(map, vehicles, vehicle)
        || crate::rail_signals::train_blocked_by_traffic(map, vehicles, vehicle)
    {
        return true;
    }
    let Some(entrance) = rail_depot_entrance_tile(map, vehicle.pos) else {
        return false;
    };
    foreign_reservation_on_tile(vehicles, vehicle.id, entrance)
}

fn foreign_reservation_on_tile(vehicles: &[Vehicle], self_id: u32, tile: TileCoord) -> bool {
    vehicles.iter().any(|v| {
        v.id != self_id
            && v.kind == VehicleKind::Train
            && v.is_consist_head()
            && v.reserved_steps.iter().any(|s| s.tile == tile)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::command::{Command, apply_command};
    use crate::engine::ENGINE_TRAIN_GINZU_A4;

    #[test]
    fn train_waits_37_ticks_before_leaving_depot() {
        let mut s = GameState::new(16, 16);
        s.economy.money = 1_000_000;
        for x in 2..=10_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        }
        let depot = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_TRAIN_GINZU_A4),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        apply_command(
            &mut s,
            &Command::SetVehicleOrders(id, vec![TileCoord::new(10, 4)]),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        for tick in 1..TRAIN_DEPOT_LEAVE_WAIT_TICKS {
            s.step();
            let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
            assert_eq!(v.pos, depot, "aún en depósito en tick {tick}");
            assert_eq!(v.wait_counter, tick);
            assert!(!v.depot_leave_cleared);
        }
        let mut left = false;
        for _ in 0..64 {
            s.step();
            let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
            if v.pos != depot || v.progress > 0 || v.cur_speed > 0 {
                left = true;
                break;
            }
        }
        assert!(left, "debe salir del depósito tras la espera de 37 ticks");
    }
}
