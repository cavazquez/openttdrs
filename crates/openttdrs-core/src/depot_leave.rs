//! Salida de depósito ferroviario (`CheckTrainStayInDepot`).
//!
//! Paridad de `train_cmd.cpp`: espera ~37 ticks, reserva de depósito (`m5` bit 4),
//! `TryPathReserve` antes de salir, reentrada por orden al mismo depósito y
//! activación escalonada de unidades (`TicksToLeaveDepot`).
//!
//! `depot_leave_cleared == false` es el proxy de `Track::Depot` en cada unidad.

use crate::depot::{
    has_depot_reservation, rail_depot_entrance_tile, rail_depot_mouth_dir, set_depot_reservation,
};
use crate::fleet_index::FleetIndex;
use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinding_settings::PathfindingSettings;
use crate::train_movement::{
    DELTACOORD_LEAVE_OFFSET, FRACTCOORDS_ENTER, calc_next_vehicle_offset, diag_dir_index,
    train_depot_facing, train_depot_subtile,
};
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

/// Ticks de espera en depósito antes de intentar salir (`CheckTrainStayInDepot`).
pub const TRAIN_DEPOT_LEAVE_WAIT_TICKS: u32 = 37;

/// `true` si el tren debe permanecer en el depósito este tick (sin avanzar).
#[allow(clippy::too_many_lines)]
pub fn tick_train_stay_in_depot(
    map: &mut Map,
    vehicles: &mut [Vehicle],
    index: usize,
    settings: PathfindingSettings,
) -> bool {
    let mut fleet = FleetIndex::default();
    fleet.rebuild(vehicles);
    tick_train_stay_in_depot_indexed(map, vehicles, index, settings, &fleet, &[])
}

/// Variante para el ciclo de simulación que reutiliza el `FleetIndex` del tick.
#[allow(clippy::too_many_lines)]
pub(crate) fn tick_train_stay_in_depot_indexed(
    map: &mut Map,
    vehicles: &mut [Vehicle],
    index: usize,
    settings: PathfindingSettings,
    fleet: &FleetIndex,
    engine_catalog: &[crate::engine::EngineDef],
) -> bool {
    let Some(vehicle) = vehicles.get(index) else {
        return false;
    };
    if vehicle.kind != VehicleKind::Train || !vehicle.running || !vehicle.is_consist_head() {
        return false;
    }

    let head_id = vehicle.id;
    let head_pos = vehicle.pos;

    if map.get_kind(head_pos) != Some(TileKind::RailDepot) {
        // Cabeza ya en vía: seguir activando vagones que queden en Track::Depot.
        activate_depot_leave_units_indexed(map, vehicles, head_id, fleet);
        maybe_clear_depot_reservation_after_exit_indexed(map, vehicles, fleet, head_id, head_pos);
        // Reiniciar gate solo en unidades que ya abandonaron la tesela depósito.
        for &id in fleet.consist(head_id) {
            if let Some(slot) = fleet.slot(id)
                && let Some(v) = vehicles.get_mut(slot)
                && map.get_kind(v.pos) != Some(TileKind::RailDepot)
            {
                v.depot_leave_cleared = false;
            }
        }
        return false;
    }

    // Ya autorizado: activar followers escalonados y no bloquear el avance.
    if vehicle.depot_leave_cleared {
        activate_depot_leave_units_indexed(map, vehicles, head_id, fleet);
        return false;
    }

    let force = vehicle.force_proceed;
    let power = vehicle.cached_power_hp;
    let engine_power = vehicle
        .engine_id
        .and_then(crate::engine::engine_by_id)
        .map_or(0, |e| e.power_hp);

    let unit_ids = fleet.consist(head_id);
    for &id in unit_ids {
        let Some(slot) = fleet.slot(id) else {
            return false;
        };
        let Some(u) = vehicles.get(slot) else {
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

    let exit_blocked = depot_exit_blocked_indexed(map, vehicles, index, fleet, engine_catalog);

    if !force {
        let Some(vehicle) = vehicles.get_mut(index) else {
            return false;
        };
        vehicle.wait_counter = vehicle.wait_counter.saturating_add(1);
        if vehicle.wait_counter < TRAIN_DEPOT_LEAVE_WAIT_TICKS {
            vehicle.cur_speed = 0;
            return true;
        }
        vehicle.wait_counter = 0;

        if is_waiting_for_unbunching(vehicle) {
            vehicle.cur_speed = 0;
            return true;
        }

        if has_depot_reservation(map, head_pos) || exit_blocked {
            vehicle.cur_speed = 0;
            return true;
        }
    } else if exit_blocked {
        if let Some(v) = vehicles.get_mut(index) {
            v.wait_counter = 0;
            v.cur_speed = 0;
        }
        return true;
    } else if let Some(v) = vehicles.get_mut(index) {
        v.wait_counter = 0;
    }

    // Reentrada: orden al mismo depósito → servicio y permanecer.
    let same_depot_order = vehicles
        .get(index)
        .and_then(Vehicle::current_order_ref)
        .is_some_and(|o| matches!(o, VehicleOrder::Depot { depot, .. } if *depot == head_pos));
    if same_depot_order {
        if !has_depot_reservation(map, head_pos)
            && let Some(v) = vehicles.get_mut(index)
        {
            v.service_at_depot();
        }
        if let Some(v) = vehicles.get_mut(index) {
            v.cur_speed = 0;
        }
        return true;
    }

    let reserved_ok = crate::rail_pbs::try_path_reserve(map, vehicles, index, !force, settings);
    if !reserved_ok && !force {
        if let Some(v) = vehicles.get_mut(index) {
            v.cur_speed = 0;
        }
        return true;
    }

    let _ = set_depot_reservation(map, head_pos, true);
    if let Some(v) = vehicles.get_mut(index) {
        v.service_at_depot();
        v.depot_leave_cleared = true;
        v.cur_speed = 0;
        v.pbs_stuck = false;
    }
    leave_unbunching_depot(vehicles, index);
    activate_depot_leave_units_indexed(map, vehicles, head_id, fleet);
    false
}

/// ¿La orden de depósito previa (o actual) pide unbunch y aún no toca salir?
#[must_use]
fn is_waiting_for_unbunching(vehicle: &Vehicle) -> bool {
    // OpenTTD: sin lista compartida no hay unbunch.
    if vehicle.shared_order_id.is_none() || vehicle.orders.len() <= 1 {
        return false;
    }
    if !previous_or_current_order_is_unbunching(vehicle) {
        return false;
    }
    vehicle.depot_unbunching_next_departure > vehicle.sim_tick
}

#[must_use]
fn previous_or_current_order_is_unbunching(vehicle: &Vehicle) -> bool {
    if vehicle.orders.is_empty() {
        return false;
    }
    let n = vehicle.orders.len();
    let cur = vehicle.current_order.min(n - 1);
    let prev = if cur == 0 { n - 1 } else { cur - 1 };
    vehicle.orders[cur].depot_unbunch() || vehicle.orders[prev].depot_unbunch()
}

/// Programa la separación de salidas entre vehículos con órdenes compartidas.
fn leave_unbunching_depot(vehicles: &mut [Vehicle], index: usize) {
    let Some(vehicle) = vehicles.get(index) else {
        return;
    };
    if vehicle.shared_order_id.is_none() || !previous_or_current_order_is_unbunching(vehicle) {
        return;
    }
    let tick = vehicle.sim_tick;
    let shared = vehicle.shared_order_id;
    let head_id = vehicle.id;

    // Actualizar round-trip del que sale.
    if let Some(v) = vehicles.get_mut(index) {
        if v.depot_unbunching_last_departure > 0 {
            let elapsed = tick.saturating_sub(v.depot_unbunching_last_departure);
            v.round_trip_time = u32::try_from(elapsed.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
        }
        v.depot_unbunching_last_departure = tick;
        v.timetable_lateness = 0;
    }

    let peers: Vec<usize> = vehicles
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.is_consist_head()
                && v.running
                && (v.id == head_id || (shared.is_some() && v.shared_order_id == shared))
        })
        .map(|(i, _)| i)
        .collect();
    let num = peers.len().max(1);
    let total_travel: u64 = peers
        .iter()
        .filter_map(|&i| vehicles.get(i).map(|v| u64::from(v.round_trip_time)))
        .sum();
    let separation = (total_travel / (num as u64) / (num as u64)).max(1);
    let next_departure = tick.saturating_add(separation);
    for i in peers {
        if let Some(v) = vehicles.get_mut(i) {
            v.depot_unbunching_next_departure = next_departure;
        }
    }
}

/// Activa followers aún en `Track::Depot` cuando `TicksToLeaveDepot(prev) <= 0`.
pub fn activate_depot_leave_units(map: &Map, vehicles: &mut [Vehicle], head_id: u32) {
    let mut fleet = FleetIndex::default();
    fleet.rebuild(vehicles);
    activate_depot_leave_units_indexed(map, vehicles, head_id, &fleet);
}

/// Variante que reutiliza la topología de consists ya preparada por el tick.
pub(crate) fn activate_depot_leave_units_indexed(
    map: &Map,
    vehicles: &mut [Vehicle],
    head_id: u32,
    fleet: &FleetIndex,
) {
    let ids = fleet.consist(head_id);
    if ids.len() < 2 {
        return;
    }
    for i in 0..ids.len().saturating_sub(1) {
        let prev_id = ids[i];
        let next_id = ids[i + 1];
        let Some(prev_slot) = fleet.slot(prev_id) else {
            continue;
        };
        let Some(next_slot) = fleet.slot(next_id) else {
            continue;
        };
        let Some(prev) = vehicles.get(prev_slot) else {
            continue;
        };
        let Some(next) = vehicles.get(next_slot) else {
            continue;
        };
        let next_cleared = next.depot_leave_cleared;
        let next_pos = next.pos;
        let next_length = next.unit_length;
        let prev_cleared = prev.depot_leave_cleared;
        let prev_pos = prev.pos;
        let prev_length = prev.unit_length;
        if next_cleared || map.get_kind(next_pos) != Some(TileKind::RailDepot) {
            continue;
        }
        // La unidad previa ya salió de Track::Depot (flag o tesela de vía).
        let prev_ready = prev_cleared || map.get_kind(prev_pos) != Some(TileKind::RailDepot);
        if !prev_ready {
            continue;
        }
        let mouth = rail_depot_mouth_dir(map, next_pos).unwrap_or(0);
        let (x, y) = unit_depot_fract(map, prev, mouth);
        let ticks =
            ticks_to_leave_depot(mouth, x, y, prev_length.max(1), next_length.max(1), false);
        if ticks <= 0
            && let Some(n) = vehicles.get_mut(next_slot)
        {
            n.depot_leave_cleared = true;
        }
    }
}

/// `TicksToLeaveDepot` (`rail_cmd.cpp:2999`).
#[must_use]
pub fn ticks_to_leave_depot(
    mouth_dir: u8,
    x_fract: u8,
    y_fract: u8,
    self_length: u8,
    next_length: u8,
    driving_backwards: bool,
) -> i32 {
    let length = i32::from(calc_next_vehicle_offset(
        self_length,
        next_length,
        driving_backwards,
    )) + 1;
    let idx = diag_dir_index(train_depot_facing(mouth_dir));
    let (ex, ey) = FRACTCOORDS_ENTER[idx];
    let x = i32::from(x_fract);
    let y = i32::from(y_fract);
    match mouth_dir & 0x03 {
        0 => x - (i32::from(ex) - length),    // NE
        1 => -(y - (i32::from(ey) + length)), // SE
        2 => -(x - (i32::from(ex) + length)), // SW
        _ => y - (i32::from(ey) - length),    // NW
    }
}

fn unit_depot_fract(map: &Map, unit: &Vehicle, mouth: u8) -> (u8, u8) {
    let idx = diag_dir_index(train_depot_facing(mouth));
    if map.get_kind(unit.pos) == Some(TileKind::RailDepot) {
        let (x, y) = train_depot_subtile(mouth, f32::from(unit.progress));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let xi = x.round().clamp(0.0, 15.0) as u8;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let yi = y.round().clamp(0.0, 15.0) as u8;
        return (xi, yi);
    }
    // Fuera del depósito: avance past enter a lo largo de `_deltacoord_leaveoffset`.
    let (ex, ey) = FRACTCOORDS_ENTER[idx];
    let (dx, dy) = DELTACOORD_LEAVE_OFFSET[idx];
    let adv = i32::from(unit.rail_pixel.min(15));
    let x = (i32::from(ex) + adv * i32::from(dx)).clamp(0, 15);
    let y = (i32::from(ey) + adv * i32::from(dy)).clamp(0, 15);
    (u8::try_from(x).unwrap_or(0), u8::try_from(y).unwrap_or(0))
}

fn maybe_clear_depot_reservation_after_exit_indexed(
    map: &mut Map,
    vehicles: &[Vehicle],
    fleet: &FleetIndex,
    head_id: u32,
    head_pos: TileCoord,
) {
    // Si ningún miembro del consist queda en un depósito, liberar el bit de la
    // tesela de depósito que aún figure reservada y no tenga otro tren saliendo.
    let any_in_depot = fleet.consist(head_id).iter().any(|&id| {
        fleet
            .slot(id)
            .and_then(|slot| vehicles.get(slot))
            .is_some_and(|v| map.get_kind(v.pos) == Some(TileKind::RailDepot))
    });
    if any_in_depot {
        return;
    }
    // Cuando la cola termina de salir, la cabeza ya puede estar varias teselas
    // adelante. Buscar sólo sus vecinos dejaba el bit del depósito pegado para
    // siempre y bloqueaba todos los consists apilados. `origin` y el historial
    // conservan la boca realmente abandonada hasta mucho después de liberar la
    // última unidad.
    let Some(head) = fleet.slot(head_id).and_then(|slot| vehicles.get(slot)) else {
        return;
    };
    let mut candidates: Vec<TileCoord> = head
        .rail_tile_history
        .iter()
        .copied()
        .chain(std::iter::once(head.origin))
        .collect();
    candidates.extend(
        [(-1_i32, 0), (1, 0), (0, -1), (0, 1), (0, 0)]
            .map(|(dx, dy)| TileCoord::new(head_pos.x + dx, head_pos.y + dy)),
    );
    candidates.sort_unstable_by_key(|tile| (tile.y, tile.x));
    candidates.dedup();
    for c in candidates {
        if map.get_kind(c) != Some(TileKind::RailDepot) || !has_depot_reservation(map, c) {
            continue;
        }
        let other_holds = vehicles.iter().any(|v| {
            v.id != head_id
                && v.kind == VehicleKind::Train
                && v.is_consist_head()
                && v.depot_leave_cleared
                && v.pos == c
        });
        if !other_holds {
            let _ = set_depot_reservation(map, c, false);
        }
    }
}

fn depot_exit_blocked_indexed(
    map: &Map,
    vehicles: &[Vehicle],
    index: usize,
    fleet: &FleetIndex,
    engine_catalog: &[crate::engine::EngineDef],
) -> bool {
    let Some(vehicle) = vehicles.get(index) else {
        return false;
    };
    if crate::rail_signals::train_blocked_by_signal_with_catalog(
        map,
        vehicles,
        vehicle,
        engine_catalog,
    ) || crate::rail_signals::train_blocked_by_traffic_indexed(map, vehicles, vehicle, fleet)
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
    use crate::command::{Command, apply_command};
    use crate::depot::has_depot_reservation;
    use crate::engine::ENGINE_TRAIN_GINZU_A4;
    use crate::test_fixtures::SandboxMap;
    use crate::vehicle::VehicleOrder;

    #[test]
    fn train_waits_37_ticks_before_leaving_depot() {
        let mut s = SandboxMap::flat_rich(16, 16, 1);
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
            assert!(!has_depot_reservation(&s.map, depot));
        }
        let mut left = false;
        for _ in 0..64 {
            s.step();
            let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
            if v.depot_leave_cleared || v.pos != depot || v.progress > 0 || v.cur_speed > 0 {
                left = true;
                assert!(has_depot_reservation(&s.map, depot) || v.pos != depot);
                break;
            }
        }
        assert!(left, "debe salir del depósito tras la espera de 37 ticks");
    }

    #[test]
    fn second_train_waits_while_depot_reserved() {
        let mut s = SandboxMap::flat_rich(12, 12, 1);
        for x in 2..=8_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        }
        let depot = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_TRAIN_GINZU_A4),
        )
        .unwrap();
        let id1 = s.vehicles[0].id;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_TRAIN_GINZU_A4),
        )
        .unwrap();
        let id2 = s.vehicles[1].id;
        let orders = vec![TileCoord::new(8, 4)];
        apply_command(&mut s, &Command::SetVehicleOrders(id1, orders.clone())).unwrap();
        apply_command(&mut s, &Command::SetVehicleOrders(id2, orders)).unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id1)).unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id2)).unwrap();

        let mut saw_exclusive = false;
        let mut both_left = false;
        for _ in 0..20_000 {
            s.step();
            let v1 = s.vehicles.iter().find(|v| v.id == id1).unwrap();
            let v2 = s.vehicles.iter().find(|v| v.id == id2).unwrap();
            if v1.depot_leave_cleared && v2.pos == depot && !v2.depot_leave_cleared {
                saw_exclusive = true;
            }
            if v1.pos != depot && v2.pos != depot {
                both_left = true;
                break;
            }
        }
        assert!(
            saw_exclusive,
            "el segundo tren debe esperar mientras el primero reserva el depósito"
        );
        assert!(both_left, "ambos trenes deben salir de forma secuencial");
    }

    #[test]
    fn same_depot_order_reenters_and_services() {
        let mut s = SandboxMap::flat_rich(12, 12, 1);
        for x in 2..=8_i32 {
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
        s.vehicles[0].reliability = 100;
        s.vehicles[0].orders = vec![VehicleOrder::Depot {
            depot,
            stop: false,
            wait_ticks: 0,
            travel_ticks: 0,
            refit_cargo: None,
            unbunch: false,
        }];
        s.vehicles[0].current_order = 0;
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        for _ in 0..(TRAIN_DEPOT_LEAVE_WAIT_TICKS + 10) {
            s.step();
        }
        let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
        assert_eq!(v.pos, depot);
        assert!(!v.depot_leave_cleared);
        assert!(v.reliability > 100, "debe haber hecho service_at_depot");
    }

    #[test]
    fn consist_followers_activate_after_head_leaves() {
        use crate::engine::ENGINE_WAGON_PASSENGER;
        use crate::train_consist::consist_unit_ids;

        let mut s = SandboxMap::flat_rich(16, 16, 1);
        for x in 2..=12_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        }
        let depot = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_TRAIN_GINZU_A4),
        )
        .unwrap();
        let head = s.vehicles[0].id;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_WAGON_PASSENGER),
        )
        .unwrap();
        let w1 = s.vehicles[1].id;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_WAGON_PASSENGER),
        )
        .unwrap();
        let w2 = s.vehicles[2].id;
        apply_command(
            &mut s,
            &Command::AttachWagonToConsist {
                head_id: head,
                wagon_id: w1,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::AttachWagonToConsist {
                head_id: head,
                wagon_id: w2,
            },
        )
        .unwrap();
        assert_eq!(
            consist_unit_ids(&s.vehicles, head).len(),
            3,
            "consist tras attach"
        );
        apply_command(
            &mut s,
            &Command::SetVehicleOrders(head, vec![TileCoord::new(12, 4)]),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(head)).unwrap();

        let mut saw_head_only = false;
        let mut saw_wagon_activate = false;
        for tick in 0..2_000 {
            s.step();
            let ids = consist_unit_ids(&s.vehicles, head);
            assert_eq!(
                ids.len(),
                3,
                "consist roto en tick {tick}; vehicles={}",
                s.vehicles.len()
            );
            let units: Vec<_> = ids
                .iter()
                .map(|&id| s.vehicles.iter().find(|v| v.id == id).unwrap())
                .collect();
            if units[0].depot_leave_cleared && !units[1].depot_leave_cleared {
                saw_head_only = true;
            }
            if units[1].depot_leave_cleared || units[2].depot_leave_cleared {
                saw_wagon_activate = true;
                break;
            }
        }
        assert!(
            saw_head_only,
            "la cabeza debe autorizar leave antes que los vagones"
        );
        assert!(
            saw_wagon_activate,
            "al menos un follower debe activarse vía TicksToLeaveDepot"
        );
    }

    #[test]
    fn consist_clears_depot_reservation_after_tail_leaves_far_behind_head() {
        use crate::vehicle::Vehicle;

        let mut s = SandboxMap::flat_rich(20, 12, 1);
        let depot = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(5, 4))).unwrap();
        apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
        assert!(set_depot_reservation(&mut s.map, depot, true));

        let mut head = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(12, 4),
            TileCoord::new(16, 4),
        );
        head.origin = depot;
        head.next_unit = Some(2);
        head.rail_tile_history.push_back(depot);
        let mut wagon = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(11, 4),
            TileCoord::new(16, 4),
        );
        wagon.prev_unit = Some(1);
        let vehicles = vec![head, wagon];

        let mut fleet = FleetIndex::default();
        fleet.rebuild(&vehicles);
        maybe_clear_depot_reservation_after_exit_indexed(
            &mut s.map,
            &vehicles,
            &fleet,
            1,
            TileCoord::new(12, 4),
        );
        assert!(
            !has_depot_reservation(&s.map, depot),
            "la reserva debe liberarse aunque la cabeza ya no sea vecina"
        );
    }

    #[test]
    fn force_proceed_leaves_without_waiting_full_timer() {
        let mut s = SandboxMap::flat_rich(16, 16, 1);
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
        // Forzar salida antes de 37 ticks.
        for _ in 0..5 {
            s.step();
        }
        apply_command(&mut s, &Command::ForceVehicleProceed(id)).unwrap();
        let mut left = false;
        for _ in 0..8 {
            s.step();
            let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
            if v.depot_leave_cleared {
                left = true;
                break;
            }
        }
        assert!(
            left,
            "force_proceed debe autorizar leave sin esperar 37 ticks"
        );
    }

    #[test]
    fn ticks_to_leave_depot_at_behind_is_positive() {
        // NW mouth=3, behind=(8,15), enter=(8,10), length=9 → 15-(10-9)=14
        let t = ticks_to_leave_depot(3, 8, 15, 8, 8, false);
        assert!(t > 0, "ticks={t}");
        let (bx, by) = crate::train_movement::FRACTCOORDS_BEHIND[3];
        assert_eq!((bx, by), (8, 15));
    }

    #[test]
    fn ticks_to_leave_depot_past_enter_can_be_non_positive() {
        // NW: y - (enter.y - length); y=1, enter=10, length=9 → 1-(10-9)=0
        let t = ticks_to_leave_depot(3, 8, 1, 8, 8, false);
        assert!(t <= 0, "ticks={t}");
    }
}
