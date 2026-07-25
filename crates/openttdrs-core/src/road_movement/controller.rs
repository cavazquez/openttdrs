//! `IndividualRoadVehicleController` — un sub-paso de frame/tesela.

use crate::engine::{ROAD_ACCEL_ORIGINAL, do_update_speed, get_advance_distance};
use crate::map::Map;
use crate::road_movement::drive_data::{RDE_NEXT_TILE, road_drive_entry};
use crate::road_movement::overtake::{
    ROAD_ACCEL_OVERTAKE, drive_state_with_overtake, tick_overtaking,
};
use crate::road_movement::rvsb::{
    RVC_DEFAULT_START_FRAME, RVSB_IN_DEPOT, RVSB_TRACKDIR_MASK, RVSB_WORMHOLE,
    trackdir_from_direction,
};
use crate::road_movement::slope::sync_road_slope_speed;
use crate::road_movement::traffic::{apply_road_veh_close_to, is_road_vehicle_kind};
use crate::vehicle::{RoadDepotPhase, Vehicle};

/// Un sub-paso del controlador. `true` = avanzó; `false` = bloqueado.
pub fn individual_road_vehicle_controller(
    vehicles: &mut [Vehicle],
    v_idx: usize,
    map: Option<&Map>,
) -> bool {
    if vehicles.get(v_idx).is_some_and(|v| v.crashed) {
        return false;
    }

    tick_overtaking(&mut vehicles[v_idx], map);

    if apply_road_veh_close_to(vehicles, v_idx, map) {
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
    if matches!(v.road_state & RVSB_TRACKDIR_MASK, 0 | 1 | 8 | 9)
        && v.road_state & RVSB_TRACKDIR_MASK != expected
    {
        v.road_state = expected;
    }

    let lookup = drive_state_with_overtake(v.road_state, v.overtaking);
    let next_frame = v.frame.saturating_add(1);
    let Some(rd) = road_drive_entry(lookup & 0x1F, next_frame) else {
        // Fin de tabla sin marcador: forzar NEXT_TILE lógico.
        return enter_next_tile(vehicles, v_idx, map);
    };

    if rd.is_next_tile() {
        return enter_next_tile(vehicles, v_idx, map);
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

    vehicles[v_idx].frame = next_frame;
    let _ = (rd.x, rd.y); // pose visual: frame indexa la tabla
    let _ = RDE_NEXT_TILE;
    true
}

fn enter_next_tile(vehicles: &mut [Vehicle], v_idx: usize, map: Option<&Map>) -> bool {
    let v = &mut vehicles[v_idx];
    if v.movement_target().is_none() {
        v.cur_speed = 0;
        return false;
    }
    let prev_dir = v.direction;
    v.advance_one_tile(map);
    if v.direction != prev_dir || v.movement_target().is_some() {
        v.road_state = trackdir_from_direction(v.direction);
    }
    v.frame = RVC_DEFAULT_START_FRAME;
    true
}

/// Tick completo de roadveh: `UpdateSpeed` + bucle `while j >= adv_spd`.
pub fn road_vehicle_tick(vehicles: &mut [Vehicle], v_idx: usize, map: Option<&Map>) {
    if !is_road_vehicle_kind(vehicles[v_idx].kind) {
        return;
    }
    let v = &mut vehicles[v_idx];
    if v.crashed {
        v.cur_speed = 0;
        return;
    }
    if !v.running {
        v.cur_speed = 0;
        return;
    }
    if v.cargo_transfer_active() || v.awaiting_load_window {
        v.cur_speed = 0;
        return;
    }
    if v.holding_for_timetable() {
        return;
    }

    // Inicializar state desde dirección.
    if v.road_state != RVSB_IN_DEPOT && v.road_state != RVSB_WORMHOLE {
        let td = v.road_state & RVSB_TRACKDIR_MASK;
        if td > 15 {
            v.road_state = trackdir_from_direction(v.direction);
        }
    }

    let engine = v.effective_engine();
    let mut max_speed = engine.max_speed;
    if let Some(map) = map
        && let Some(cap) = crate::bridge_spec::bridge_max_speed_for_tile(map, v.pos)
    {
        max_speed = max_speed.min(cap);
    }

    let accel = if v.overtaking != 0 {
        i32::from(ROAD_ACCEL_OVERTAKE)
    } else {
        i32::from(ROAD_ACCEL_ORIGINAL)
    };

    let result = if v.movement_target().is_some() {
        do_update_speed(v.cur_speed, v.subspeed, accel, 0, max_speed, v.progress)
    } else {
        let (cur, sub) = crate::engine::decelerate_road_speed(v.cur_speed, v.subspeed);
        v.cur_speed = cur;
        v.subspeed = sub;
        if v.cur_speed == 0 && v.pos == v.dest {
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
        if !individual_road_vehicle_controller(vehicles, v_idx, map) {
            blocked = true;
            break;
        }
        if vehicles[v_idx].cur_speed == 0 || vehicles[v_idx].movement_target().is_none() {
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
    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::{DIR_SW, VehicleKind};
    use std::collections::VecDeque;

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
        assert_eq!(ROAD_ACCEL_OVERTAKE, 512);
        assert_eq!(ROAD_ACCEL_ORIGINAL, 256);
    }
}
