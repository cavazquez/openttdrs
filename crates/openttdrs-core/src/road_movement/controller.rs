//! `IndividualRoadVehicleController` — un sub-paso de frame/tesela.

use crate::engine::{
    RoadVehicleAccelerationModel, get_advance_distance, road_engine_air_drag, road_max_te_n,
    update_road_vehicle_speed,
};
use crate::map::Map;
use crate::road_movement::bay::{
    bay_direction_at_frame_side, bay_drive_entry_side, bay_stop_frame_side,
};
use crate::road_movement::drive_data::{RDE_NEXT_TILE, road_drive_entry};
use crate::road_movement::overtake::{drive_state_with_overtake_and_side, tick_overtaking};
use crate::road_movement::rvsb::{
    RVC_DEFAULT_START_FRAME, RVC_DRIVE_THROUGH_STOP_FRAME, RVC_TURN_AROUND_START_FRAME,
    RVSB_ENTERED_STOP, RVSB_IN_DEPOT, RVSB_IN_DT_ROAD_STOP, RVSB_IN_ROAD_STOP, RVSB_TRACKDIR_MASK,
    RVSB_USING_SECOND_BAY, RVSB_WORMHOLE, is_bay_road_state, is_drive_through_road_state,
    trackdir_for_entry_exit, trackdir_from_direction,
};
use crate::road_movement::slope::sync_road_slope_speed;
use crate::road_movement::traffic::{
    RoadTrafficIndex, apply_road_veh_close_to, apply_road_veh_close_to_indexed_with_catalog,
    is_road_vehicle_kind,
};
use crate::vehicle::{RoadDepotPhase, Vehicle};

/// Un sub-paso del controlador. `true` = avanzó; `false` = bloqueado.
pub fn individual_road_vehicle_controller(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
) -> bool {
    individual_road_vehicle_controller_side(vehicles, v_idx, map, false)
}

/// Como [`individual_road_vehicle_controller`], con lado de circulación.
pub fn individual_road_vehicle_controller_side(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
) -> bool {
    individual_road_vehicle_controller_side_indexed(vehicles, v_idx, map, drive_on_right, None)
}

/// Variante que consulta el índice vial incremental del tick.
pub fn individual_road_vehicle_controller_side_indexed(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
    traffic: Option<&RoadTrafficIndex>,
) -> bool {
    individual_road_vehicle_controller_side_indexed_with_catalog(
        vehicles,
        v_idx,
        map,
        drive_on_right,
        traffic,
        &[],
    )
}

/// Variante indexada que propaga el catálogo activo a la comprobación de
/// tráfico/adelantamiento y sus callbacks de velocidad.
#[allow(clippy::too_many_lines)]
pub fn individual_road_vehicle_controller_side_indexed_with_catalog(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
    traffic: Option<&RoadTrafficIndex>,
    engine_catalog: &[crate::engine::EngineDef],
) -> bool {
    if vehicles.get(v_idx).is_some_and(|v| v.crashed) {
        return false;
    }

    tick_overtaking(&mut vehicles[v_idx], map);

    let blocked_by_traffic = match traffic {
        Some(traffic) => apply_road_veh_close_to_indexed_with_catalog(
            vehicles,
            v_idx,
            map,
            traffic,
            engine_catalog,
        ),
        None => apply_road_veh_close_to(vehicles, v_idx, map),
    };
    if !is_bay_road_state(vehicles[v_idx].road_state) && blocked_by_traffic {
        return false;
    }

    let v = &mut vehicles[v_idx];
    if matches!(
        v.road_depot_phase,
        RoadDepotPhase::InDepot | RoadDepotPhase::Entering { .. } | RoadDepotPhase::Exiting { .. }
    ) || v.road_state == RVSB_IN_DEPOT
        || v.road_state == RVSB_WORMHOLE
    {
        return false;
    }

    // Mantener trackdir alineado con la dirección de marcha en rectas.
    let expected = trackdir_from_direction(v.direction);
    if !is_bay_road_state(v.road_state)
        && matches!(v.road_state & RVSB_TRACKDIR_MASK, 0 | 1 | 8 | 9)
        && v.road_state & RVSB_TRACKDIR_MASK != expected
    {
        v.road_state = (v.road_state & !RVSB_TRACKDIR_MASK) | expected;
    }

    let state = v.road_state;
    let lookup = drive_state_with_overtake_and_side(state, v.overtaking, drive_on_right);
    let next_frame = v.frame.saturating_add(1);
    let rd = if is_bay_road_state(state) {
        bay_drive_entry_side(state, next_frame, drive_on_right)
    } else {
        road_drive_entry(lookup & 0x1F, next_frame)
    };
    let Some(rd) = rd else {
        // Fin de tabla sin marcador: forzar NEXT_TILE lógico.
        return enter_next_tile(vehicles, v_idx, map, drive_on_right);
    };

    if rd.is_next_tile() {
        return enter_next_tile(vehicles, v_idx, map, drive_on_right);
    }

    if rd.is_turned() {
        let diag = rd.diagdir();
        let dir = match diag {
            0 => crate::vehicle::DIR_NE,
            1 => crate::vehicle::DIR_SE,
            2 => crate::vehicle::DIR_SW,
            _ => crate::vehicle::DIR_NW,
        };
        let v = &mut vehicles[v_idx];
        v.set_direction_with_curve_penalty(
            dir,
            map,
            crate::engine::TrainAccelerationModel::Original,
        );
        v.road_state = trackdir_from_direction(dir);
        v.overtaking = 0;
        v.overtaking_ctr = 0;
        v.frame = RVC_DEFAULT_START_FRAME;
        return true;
    }

    if is_bay_road_state(state) {
        let stop = bay_stop_frame_side(state, drive_on_right).unwrap_or(u8::MAX);
        let entered = state & RVSB_ENTERED_STOP != 0;
        if entered
            && vehicles[v_idx].frame == stop
            && bay_entrance_busy(vehicles, v_idx, vehicles[v_idx].pos, drive_on_right)
        {
            return false;
        }

        vehicles[v_idx].frame = next_frame;
        if !entered && next_frame == stop {
            let v = &mut vehicles[v_idx];
            v.road_state |= RVSB_ENTERED_STOP;
            v.cur_speed = 0;
            v.subspeed = 0;
            v.progress = 0;
            v.advance_destination_after_arrival();
            return true;
        }
        if next_frame != stop
            && let Some(direction) =
                bay_direction_at_frame_side(state, f32::from(next_frame), drive_on_right)
        {
            vehicles[v_idx].set_direction_with_curve_penalty(
                direction,
                map,
                crate::engine::TrainAccelerationModel::Original,
            );
        }
        return true;
    }

    vehicles[v_idx].frame = next_frame;
    if handle_drive_through_stop(&mut vehicles[v_idx], state, next_frame, map) {
        return true;
    }
    let _ = (rd.x, rd.y); // pose visual: frame indexa la tabla
    let _ = RDE_NEXT_TILE;
    true
}

fn enter_next_tile(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
) -> bool {
    let Some(target) = vehicles[v_idx].movement_target() else {
        vehicles[v_idx].cur_speed = 0;
        return false;
    };
    let was_in_bay = is_bay_road_state(vehicles[v_idx].road_state);
    if !was_in_bay && map.is_some_and(|map| crate::station::is_drive_through_road_stop(map, target))
    {
        let inbound = crate::vehicle::direction_from_tile_step(vehicles[v_idx].pos, target);
        vehicles[v_idx].advance_one_tile(map);
        vehicles[v_idx].road_state = RVSB_IN_DT_ROAD_STOP | trackdir_from_direction(inbound);
        vehicles[v_idx].frame = RVC_DEFAULT_START_FRAME;
        vehicles[v_idx].direction = inbound;
        vehicles[v_idx].overtaking = 0;
        vehicles[v_idx].overtaking_ctr = 0;
        return true;
    }
    if !was_in_bay
        && map.is_some_and(|map| {
            target == vehicles[v_idx].dest
                && crate::station::is_connected_bay_road_stop(map, target)
                && crate::station::bay_entry_direction(map, target)
                    == Some(crate::vehicle::direction_from_tile_step(
                        vehicles[v_idx].pos,
                        target,
                    ))
        })
    {
        let Some(far) = allocate_bay(vehicles, v_idx, target, drive_on_right) else {
            return false;
        };
        let inbound = crate::vehicle::direction_from_tile_step(vehicles[v_idx].pos, target);
        vehicles[v_idx].advance_one_tile(map);
        vehicles[v_idx].road_state = RVSB_IN_ROAD_STOP
            | trackdir_from_direction(inbound)
            | if far { 0 } else { RVSB_USING_SECOND_BAY };
        vehicles[v_idx].frame = RVC_DEFAULT_START_FRAME;
        vehicles[v_idx].direction = inbound;
        vehicles[v_idx].overtaking = 0;
        vehicles[v_idx].overtaking_ctr = 0;
        return true;
    }

    let v = &mut vehicles[v_idx];
    let prev_dir = v.direction;
    v.advance_one_tile(map);
    if v.direction != prev_dir || v.movement_target().is_some() {
        let inbound = v.direction;
        let outbound = v.movement_target().map_or(inbound, |next| {
            crate::vehicle::direction_from_tile_step(v.pos, next)
        });
        v.road_state = trackdir_for_entry_exit(inbound, outbound);
    }
    v.frame = RVC_DEFAULT_START_FRAME;
    true
}

fn drive_through_should_stop(v: &Vehicle, map: Option<&Map>) -> bool {
    let Some(map) = map else {
        return false;
    };
    let Some(tile) = map.get(v.pos) else {
        return false;
    };
    if !crate::station::is_drive_through_road_stop(map, v.pos) {
        return false;
    }
    let type_matches = match v.kind {
        crate::vehicle::VehicleKind::Bus => crate::station::station_type_from_m6(tile.m6) == 3,
        crate::vehicle::VehicleKind::Truck => crate::station::station_type_from_m6(tile.m6) == 2,
        crate::vehicle::VehicleKind::Tram => {
            matches!(crate::station::station_type_from_m6(tile.m6), 2 | 3)
        }
        _ => false,
    };
    type_matches
        && v.current_order_ref()
            .is_some_and(|order| order.should_stop_at_station(v.last_station_visited, v.pos))
}

fn handle_drive_through_stop(
    v: &mut Vehicle,
    state: u8,
    next_frame: u8,
    map: Option<&Map>,
) -> bool {
    if !is_drive_through_road_state(state)
        || next_frame != RVC_DRIVE_THROUGH_STOP_FRAME
        || !drive_through_should_stop(v, map)
    {
        return false;
    }
    v.cur_speed = 0;
    v.subspeed = 0;
    v.progress = 0;
    v.advance_destination_after_arrival();
    true
}

fn allocate_bay(
    vehicles: &[Vehicle],
    v_idx: usize,
    station: crate::map::TileCoord,
    drive_on_right: bool,
) -> Option<bool> {
    if bay_entrance_busy(vehicles, v_idx, station, drive_on_right) {
        return None;
    }
    let mut far_used = false;
    let mut near_used = false;
    for (i, other) in vehicles.iter().enumerate() {
        if i == v_idx || other.pos != station || !is_bay_road_state(other.road_state) {
            continue;
        }
        if other.road_state & RVSB_USING_SECOND_BAY != 0 {
            near_used = true;
        } else {
            far_used = true;
        }
    }
    if !far_used {
        Some(true)
    } else if !near_used {
        Some(false)
    } else {
        None
    }
}

fn bay_entrance_busy(
    vehicles: &[Vehicle],
    v_idx: usize,
    station: crate::map::TileCoord,
    drive_on_right: bool,
) -> bool {
    vehicles.iter().enumerate().any(|(i, other)| {
        if i == v_idx || other.pos != station || !is_bay_road_state(other.road_state) {
            return false;
        }
        let Some(stop) = bay_stop_frame_side(other.road_state, drive_on_right) else {
            return false;
        };
        let entered = other.road_state & RVSB_ENTERED_STOP != 0;
        (!entered && other.frame < stop) || (entered && other.frame > stop)
    })
}

fn road_vehicle_has_motion_target(v: &Vehicle) -> bool {
    v.movement_target().is_some()
        || is_bay_road_state(v.road_state) && v.road_state & RVSB_ENTERED_STOP == 0
}

fn tick_reverse_counter(v: &mut Vehicle) -> bool {
    if v.reverse_ctr == 0 {
        return false;
    }
    v.reverse_ctr -= 1;
    if v.frame != RVC_DEFAULT_START_FRAME || v.road_state > RVSB_TRACKDIR_MASK {
        return false;
    }

    let reverse_trackdir = match v.road_state & RVSB_TRACKDIR_MASK {
        0 => 14,
        1 => 15,
        8 => 6,
        9 => 7,
        _ => return false,
    };
    v.road_state = reverse_trackdir;
    v.frame = RVC_TURN_AROUND_START_FRAME;
    v.reverse_ctr = 0;
    v.path.clear();
    true
}

/// Tick completo de roadveh: `UpdateSpeed` + bucle `while j >= adv_spd`.
pub fn road_vehicle_tick(vehicles: &mut [Vehicle], v_idx: usize, map: Option<&Map>) {
    road_vehicle_tick_side(vehicles, v_idx, map, false);
}

/// Como [`road_vehicle_tick`], con lado de circulación (`vehicle.road_side`).
pub fn road_vehicle_tick_side(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
) {
    road_vehicle_tick_side_with_traffic(
        vehicles,
        v_idx,
        map,
        drive_on_right,
        RoadVehicleAccelerationModel::Original,
        None,
        &[],
    );
}

/// Tick de roadveh con búsqueda de tráfico indexada.
pub fn road_vehicle_tick_side_indexed(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
    traffic: &mut RoadTrafficIndex,
) {
    road_vehicle_tick_side_indexed_with_acceleration(
        vehicles,
        v_idx,
        map,
        drive_on_right,
        RoadVehicleAccelerationModel::Original,
        traffic,
    );
}

/// Tick de roadveh indexado con el modelo persistido en `PATS`.
///
/// La variante histórica mantiene `AM_ORIGINAL` para no cambiar los callers
/// unitarios que no cuentan con un `GameState`; el simulador siempre llama a
/// esta función con el setting importado del save.
pub fn road_vehicle_tick_side_indexed_with_acceleration(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
    acceleration_model: RoadVehicleAccelerationModel,
    traffic: &mut RoadTrafficIndex,
) {
    road_vehicle_tick_side_indexed_with_acceleration_and_catalog(
        vehicles,
        v_idx,
        map,
        drive_on_right,
        acceleration_model,
        &[],
        traffic,
    );
}

/// Variante de [`road_vehicle_tick_side_indexed_with_acceleration`] que usa el
/// catálogo activo para resolver motores Action0/Action3 y sus callbacks CB36.
pub fn road_vehicle_tick_side_indexed_with_acceleration_and_catalog(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
    acceleration_model: RoadVehicleAccelerationModel,
    engine_catalog: &[crate::engine::EngineDef],
    traffic: &mut RoadTrafficIndex,
) {
    let previous = vehicles
        .get(v_idx)
        .map_or(crate::TileCoord::new(0, 0), |v| v.pos);
    road_vehicle_tick_side_with_traffic(
        vehicles,
        v_idx,
        map,
        drive_on_right,
        acceleration_model,
        Some(traffic),
        engine_catalog,
    );
    traffic.update_vehicle(vehicles, v_idx, previous);
}

#[allow(clippy::too_many_lines)]
fn road_vehicle_tick_side_with_traffic(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
    drive_on_right: bool,
    acceleration_model: RoadVehicleAccelerationModel,
    traffic: Option<&RoadTrafficIndex>,
    engine_catalog: &[crate::engine::EngineDef],
) {
    if !is_road_vehicle_kind(vehicles[v_idx].kind) {
        return;
    }
    let v = &mut vehicles[v_idx];
    tick_reverse_counter(v);
    if v.crashed {
        v.cur_speed = 0;
        return;
    }
    if !v.running {
        v.cur_speed = 0;
        return;
    }
    if v.cargo_transfer_active() {
        v.cur_speed = 0;
        return;
    }
    v.complete_station_load_window();
    if v.awaiting_load_window {
        v.cur_speed = 0;
        return;
    }
    if v.holding_for_timetable() {
        return;
    }

    // Inicializar state desde dirección.
    if v.road_state != RVSB_IN_DEPOT
        && v.road_state != RVSB_WORMHOLE
        && !is_bay_road_state(v.road_state)
    {
        let td = v.road_state & RVSB_TRACKDIR_MASK;
        if td > 15 {
            v.road_state = trackdir_from_direction(v.direction);
        }
    }

    let max_speed =
        super::slope::current_road_max_speed_with_callbacks_in_catalog(v, map, engine_catalog);

    let engine = crate::newgrf_callback::engine_for_vehicle_catalog(engine_catalog, v);
    let cargo_weight = crate::train_consist::cargo_weight_t(v.cargo, v.cargo_type);
    let weight = crate::newgrf_callback::vehicle_weight_t(engine, v)
        .saturating_add(cargo_weight)
        .max(1);
    let max_te = road_max_te_n(
        weight,
        crate::newgrf_callback::vehicle_tractive_effort(engine, v),
    );
    let power = crate::newgrf_callback::vehicle_power_hp(engine, v);
    let air_drag = road_engine_air_drag(engine);

    if is_bay_road_state(v.road_state)
        && v.road_state & RVSB_ENTERED_STOP != 0
        && v.progress == u8::MAX
        && v.movement_target().is_some()
    {
        v.progress = 0;
    }

    let result = if road_vehicle_has_motion_target(v) {
        update_road_vehicle_speed(
            v.cur_speed,
            v.subspeed,
            v.progress,
            acceleration_model,
            power,
            weight,
            max_te,
            air_drag,
            v.kind,
            max_speed,
            v.overtaking != 0,
            false,
        )
    } else {
        let result = update_road_vehicle_speed(
            v.cur_speed,
            v.subspeed,
            0,
            acceleration_model,
            power,
            weight,
            max_te,
            air_drag,
            v.kind,
            max_speed,
            false,
            true,
        );
        v.cur_speed = result.cur_speed;
        v.subspeed = result.subspeed;
        if v.cur_speed == 0 && v.pos == v.dest && !is_bay_road_state(v.road_state) {
            v.advance_destination_after_arrival();
        }
        return;
    };
    v.cur_speed = result.cur_speed;
    v.subspeed = result.subspeed;
    v.progress = 0;

    if v.cur_speed == 0 {
        return;
    }

    let mut j = result.advance;
    let mut adv_spd = get_advance_distance(v.direction);
    let mut blocked = false;
    while j >= adv_spd {
        j -= adv_spd;
        if !individual_road_vehicle_controller_side_indexed_with_catalog(
            vehicles,
            v_idx,
            map,
            drive_on_right,
            traffic,
            engine_catalog,
        ) {
            blocked = true;
            break;
        }
        if vehicles[v_idx].cur_speed == 0 || !road_vehicle_has_motion_target(&vehicles[v_idx]) {
            break;
        }
        adv_spd = get_advance_distance(vehicles[v_idx].direction);
    }
    if vehicles[v_idx].progress == 0 {
        vehicles[v_idx].progress = if blocked {
            u8::try_from(adv_spd.saturating_sub(1).min(u32::from(u8::MAX))).unwrap_or(u8::MAX)
        } else {
            u8::try_from(j.min(u32::from(u8::MAX))).unwrap_or(u8::MAX)
        };
    }
    if let Some(map) = map {
        sync_road_slope_speed(&mut vehicles[v_idx], map);
    }
}

/// Avance road sin vecinos (tests unitarios de un solo vehículo).
pub fn road_vehicle_step_solo(v: &mut Vehicle, map: Option<&Map>) {
    let mut tmp = vec![v.clone()];
    road_vehicle_tick(&mut tmp, 0, map);
    *v = tmp.remove(0);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::road_movement::bay::bay_stop_frame;

    use super::*;
    use crate::map::{Map, TileCoord, TileKind};
    use crate::road_movement::rvsb::{RVSB_ENTERED_STOP, RVSB_USING_SECOND_BAY};
    use crate::vehicle::{DIR_NE, DIR_SW, VehicleKind};
    use std::collections::VecDeque;

    fn bay_map() -> (Map, TileCoord, TileCoord) {
        let mut map = Map::new_flat(8, 8, 0);
        let stop = TileCoord::new(3, 3);
        let approach = TileCoord::new(4, 3);
        map.set_kind(approach, TileKind::Road).unwrap();
        let mut tile = map.get(stop).unwrap();
        tile.kind = TileKind::Station;
        tile.m5 = 2; // boca hacia SW: approach = station + (1, 0)
        tile.m6 = 3 << 3; // bus stop
        tile.m3 = 0x02;
        map.set_tile(stop, tile).unwrap();
        (map, stop, approach)
    }

    fn drive_through_map(axis_y: bool, station_type: u8) -> (Map, TileCoord, TileCoord, TileCoord) {
        let mut map = Map::new_flat(8, 8, 0);
        let stop = TileCoord::new(3, 3);
        let (approach, exit, orientation, bits) = if axis_y {
            (TileCoord::new(3, 2), TileCoord::new(3, 4), 5, 0x05)
        } else {
            (TileCoord::new(2, 3), TileCoord::new(4, 3), 4, 0x0A)
        };
        map.set_kind(approach, TileKind::Road).unwrap();
        map.set_kind(exit, TileKind::Road).unwrap();
        let mut tile = map.get(stop).unwrap();
        tile.kind = TileKind::Station;
        tile.m5 = orientation;
        tile.m6 = station_type << 3;
        tile.m3 = bits;
        map.set_tile(stop, tile).unwrap();
        (map, stop, approach, exit)
    }

    fn assert_drive_through_cycle(kind: VehicleKind, axis_y: bool, station_type: u8) {
        let (map, stop, approach, exit) = drive_through_map(axis_y, station_type);
        let beyond = if axis_y {
            TileCoord::new(3, 5)
        } else {
            TileCoord::new(5, 3)
        };
        let direction = crate::vehicle::direction_from_tile_step(approach, stop);
        let mut v = Vehicle::new(1, kind, approach, stop);
        v.direction = direction;
        v.road_state = trackdir_from_direction(direction);
        v.frame = 15;
        v.set_station_orders(vec![stop, beyond]);
        v.path = VecDeque::from([stop]);
        let mut vehicles = vec![v];

        assert!(individual_road_vehicle_controller(
            &mut vehicles,
            0,
            Some(&map)
        ));
        assert_eq!(vehicles[0].pos, stop);
        assert!(is_drive_through_road_state(vehicles[0].road_state));

        for _ in 0..RVC_DRIVE_THROUGH_STOP_FRAME {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                0,
                Some(&map)
            ));
        }
        assert_eq!(vehicles[0].frame, RVC_DRIVE_THROUGH_STOP_FRAME);
        assert!(vehicles[0].awaiting_load_window);

        vehicles[0].awaiting_load_window = false;
        vehicles[0].current_order = 1;
        vehicles[0].dest = beyond;
        vehicles[0].path = VecDeque::from([exit, beyond]);
        vehicles[0].progress = 0;
        while vehicles[0].pos == stop {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                0,
                Some(&map)
            ));
        }
        assert_eq!(vehicles[0].pos, exit);
        assert!(!is_drive_through_road_state(vehicles[0].road_state));
        assert_eq!(vehicles[0].direction, direction);
        assert_eq!(vehicles[0].depart_turn, 0);
    }

    fn bus_waiting_at_mouth(id: u32, stop: TileCoord, approach: TileCoord) -> Vehicle {
        let mut v = Vehicle::new(id, VehicleKind::Bus, approach, stop);
        v.direction = DIR_NE;
        v.road_state = 0;
        v.frame = 15;
        v.path = VecDeque::from([stop]);
        v.set_station_orders(vec![stop, TileCoord::new(6, 3)]);
        v.path = VecDeque::from([stop]);
        v
    }

    #[test]
    fn high_speed_advances_multiple_frames_per_tick() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(10, 0),
        );
        v.direction = DIR_SW;
        v.road_state = 8;
        v.frame = 0;
        v.cur_speed = 400;
        v.subspeed = 0;
        v.progress = 0;
        v.path = (1..=10)
            .map(|x| TileCoord::new(x, 0))
            .collect::<VecDeque<_>>();
        let mut frames_seen = 0_u32;
        let start_pos = v.pos;
        for _ in 0..8 {
            let before = v.frame;
            road_vehicle_step_solo(&mut v, None);
            if v.frame != before || v.pos != start_pos {
                frames_seen = frames_seen.saturating_add(1);
            }
        }
        assert!(
            frames_seen >= 2 || v.pos != start_pos,
            "bucle while j>=adv_spd debe consumir varios sub-pasos; frames_seen={frames_seen} frame={} pos={:?}",
            v.frame,
            v.pos
        );
    }

    #[test]
    fn overtaking_uses_accel_512() {
        assert_eq!(crate::road_movement::ROAD_ACCEL_OVERTAKE, 512);
        assert_eq!(crate::engine::ROAD_ACCEL_ORIGINAL, 256);
    }

    #[test]
    fn turn_marker_consumes_a_substep_without_advancing_the_tile() {
        let start = TileCoord::new(0, 0);
        let end = TileCoord::new(1, 0);
        let mut v = Vehicle::new(1, VehicleKind::Bus, start, end);
        v.direction = DIR_NE;
        v.road_state = 6;
        v.frame = 3;
        v.path = VecDeque::from([end]);
        v.cur_speed = 100;
        let mut vehicles = vec![v];

        assert!(individual_road_vehicle_controller(&mut vehicles, 0, None));

        assert_eq!(vehicles[0].pos, start);
        assert_eq!(vehicles[0].direction, DIR_SW);
        assert_eq!(vehicles[0].frame, RVC_DEFAULT_START_FRAME);
        assert_eq!(vehicles[0].cur_speed, 75);
    }

    #[test]
    fn reverse_counter_starts_turnaround_once_at_segment_start() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(5, 0),
        );
        v.road_state = 0;
        v.frame = RVC_DEFAULT_START_FRAME;
        v.reverse_ctr = 180;
        v.path = VecDeque::from([TileCoord::new(1, 0)]);

        assert!(tick_reverse_counter(&mut v));
        assert_eq!(v.reverse_ctr, 0);
        assert_eq!(v.road_state, 14);
        assert_eq!(v.frame, RVC_TURN_AROUND_START_FRAME);
        assert!(v.path.is_empty());
        assert!(!tick_reverse_counter(&mut v));
    }

    #[test]
    fn reverse_counter_waits_while_inside_a_road_stop() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(5, 0),
        );
        v.road_state = RVSB_IN_ROAD_STOP;
        v.reverse_ctr = 2;

        assert!(!tick_reverse_counter(&mut v));
        assert_eq!(v.reverse_ctr, 1);
        assert_eq!(v.road_state, RVSB_IN_ROAD_STOP);
    }

    #[test]
    fn mps_at_56_kmh_needs_37_ticks_for_a_straight_tile() {
        let start = TileCoord::new(0, 0);
        let end = TileCoord::new(1, 0);
        let mut v = Vehicle::new(1, VehicleKind::Bus, start, end);
        v.direction = DIR_SW;
        v.road_state = 8;
        v.frame = 0;
        v.cur_speed = 112;
        v.path = VecDeque::from([end]);

        for _ in 0..36 {
            road_vehicle_step_solo(&mut v, None);
        }
        assert_eq!(v.pos, start);
        road_vehicle_step_solo(&mut v, None);
        assert_eq!(v.pos, end);
    }

    #[test]
    fn bay_arrival_happens_at_stop_frame_not_tile_boundary() {
        let (map, stop, approach) = bay_map();
        let mut vehicles = vec![bus_waiting_at_mouth(1, stop, approach)];

        assert!(individual_road_vehicle_controller(
            &mut vehicles,
            0,
            Some(&map)
        ));
        assert_eq!(vehicles[0].pos, stop);
        assert!(is_bay_road_state(vehicles[0].road_state));
        assert_eq!(vehicles[0].frame, 0);
        assert!(!vehicles[0].awaiting_load_window);

        let stop_frame = bay_stop_frame(vehicles[0].road_state).unwrap();
        for _ in 0..stop_frame {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                0,
                Some(&map)
            ));
        }
        assert_eq!(vehicles[0].frame, stop_frame);
        assert_ne!(vehicles[0].road_state & RVSB_ENTERED_STOP, 0);
        assert!(vehicles[0].awaiting_load_window);
    }

    #[test]
    fn bay_allocates_far_then_near_and_blocks_a_third_bus() {
        let (map, stop, approach) = bay_map();
        let mut vehicles = vec![
            bus_waiting_at_mouth(1, stop, approach),
            bus_waiting_at_mouth(2, stop, approach),
            bus_waiting_at_mouth(3, stop, approach),
        ];

        assert!(individual_road_vehicle_controller(
            &mut vehicles,
            0,
            Some(&map)
        ));
        let first_stop = bay_stop_frame(vehicles[0].road_state).unwrap();
        for _ in 0..first_stop {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                0,
                Some(&map)
            ));
        }
        assert_eq!(vehicles[0].road_state & RVSB_USING_SECOND_BAY, 0);

        assert!(individual_road_vehicle_controller(
            &mut vehicles,
            1,
            Some(&map)
        ));
        assert_ne!(vehicles[1].road_state & RVSB_USING_SECOND_BAY, 0);
        let second_stop = bay_stop_frame(vehicles[1].road_state).unwrap();
        for _ in 0..second_stop {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                1,
                Some(&map)
            ));
        }

        assert!(!individual_road_vehicle_controller(
            &mut vehicles,
            2,
            Some(&map)
        ));
        assert_eq!(vehicles[2].pos, approach);
    }

    #[test]
    fn bay_exit_uses_same_table_without_synthetic_turn() {
        let (map, stop, approach) = bay_map();
        let mut vehicles = vec![bus_waiting_at_mouth(1, stop, approach)];
        assert!(individual_road_vehicle_controller(
            &mut vehicles,
            0,
            Some(&map)
        ));
        let stop_frame = bay_stop_frame(vehicles[0].road_state).unwrap();
        for _ in 0..stop_frame {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                0,
                Some(&map)
            ));
        }
        vehicles[0].awaiting_load_window = false;
        vehicles[0].current_order = 1;
        vehicles[0].dest = TileCoord::new(6, 3);
        vehicles[0].path = VecDeque::from([approach, TileCoord::new(5, 3)]);
        vehicles[0].progress = 0;

        while vehicles[0].pos == stop {
            assert!(individual_road_vehicle_controller(
                &mut vehicles,
                0,
                Some(&map)
            ));
        }
        assert_eq!(vehicles[0].pos, approach);
        assert!(!is_bay_road_state(vehicles[0].road_state));
        assert_eq!(vehicles[0].depart_turn, 0);
    }

    #[test]
    fn bus_uses_horizontal_drive_through_stop() {
        assert_drive_through_cycle(VehicleKind::Bus, false, 3);
    }

    #[test]
    fn truck_uses_vertical_drive_through_stop() {
        assert_drive_through_cycle(VehicleKind::Truck, true, 2);
    }
}
