//! Sub-tesela de vehículos en carretera/vía (`table/roadveh_movement.h`).

pub mod bay;
pub mod controller;
mod curves;
pub mod depot;
pub mod drive_data;
pub mod overtake;
pub mod pose;
mod render_pose;
pub mod rvsb;
pub mod slope;
pub mod traffic;

// Re-exportar tipos y funciones públicas principales
pub use bay::{BayStationTable, bay_station_table, bay_station_table_for_side, parked_inside_bay};
pub use controller::{
    individual_road_vehicle_controller, individual_road_vehicle_controller_side,
    individual_road_vehicle_controller_side_indexed, road_vehicle_step_solo, road_vehicle_tick,
    road_vehicle_tick_side, road_vehicle_tick_side_indexed,
    road_vehicle_tick_side_indexed_with_acceleration,
    road_vehicle_tick_side_indexed_with_acceleration_and_catalog,
};
pub use curves::{straight_subtile, train_straight_subtile, turn_curve_points};
pub use depot::{
    ROAD_DEPOT_ENTRY_STOP, ROAD_DEPOT_EXIT_START, ROAD_DEPOT_PROGRESS_STEP, road_depot_direction,
    road_depot_entry_direction, road_depot_exit_direction, road_depot_subtile,
};
pub use drive_data::{RDE_NEXT_TILE, RDE_TURNED, RoadDriveEntry, road_drive_entry};
pub use overtake::{
    ROAD_ACCEL_OVERTAKE, RV_OVERTAKE_TIMEOUT, drive_state_with_overtake,
    drive_state_with_overtake_and_side, road_veh_check_overtake,
};
pub use pose::{
    VehiclePose, extrapolate_vehicle_pose, retreat_vehicle_pose, retreat_vehicle_pose_distance,
    vehicle_render_progress,
};
pub use render_pose::{
    road_turn_entry_exit, train_subtile_direction, vehicle_render_direction,
    vehicle_render_direction_at, vehicle_render_direction_at_with_map, vehicle_sprite_direction_at,
    vehicle_sprite_direction_at_with_map, vehicle_subtile, vehicle_subtile_at,
    vehicle_subtile_at_with_map, vehicle_subtile_with_progress,
};
pub use rvsb::RVSB_DRIVE_SIDE;
pub use rvsb::{RVSB_IN_DEPOT, RVSB_TRACKDIR_MASK, trackdir_from_direction};
pub use slope::{
    current_road_max_speed_with_callbacks, current_road_max_speed_with_callbacks_in_catalog,
    road_z_pos_affect_speed, sync_road_slope_speed,
};
pub use traffic::{
    BLOCKED_CTR_LIMIT, RoadTrafficIndex, apply_road_veh_close_to, apply_road_veh_close_to_indexed,
    road_veh_find_close_to, road_veh_find_close_to_indexed,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;

    use crate::map::TileCoord;
    use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleDirection, VehicleKind};

    use super::*;

    fn ne_to_se_turn_vehicle() -> Vehicle {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(0, 2),
        );
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(0, 2)]);
        v
    }

    #[test]
    fn detects_ne_to_se_turn() {
        let v = ne_to_se_turn_vehicle();
        assert_eq!(road_turn_entry_exit(&v), Some((DIR_NE, DIR_SE)));
    }

    #[test]
    fn retreat_pose_uses_multiple_road_history_tiles() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(4, 2),
            TileCoord::new(8, 2),
        );
        v.progress = 128;
        v.path = VecDeque::from([TileCoord::new(5, 2)]);
        v.road_tile_history = VecDeque::from([
            TileCoord::new(3, 2),
            TileCoord::new(2, 2),
            TileCoord::new(1, 2),
        ]);

        let pose = retreat_vehicle_pose_distance(&v, VehiclePose::from_vehicle(&v), 512);

        assert_eq!(pose.pos, TileCoord::new(2, 2));
        assert_eq!(pose.progress, 126);
    }

    #[test]
    fn articulated_mirror_reverses_sprite_direction_without_changing_motion() {
        let mut v = Vehicle::new(
            2,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(3, 1),
        );
        v.path = VecDeque::from([TileCoord::new(2, 1)]);
        v.direction = crate::vehicle::DIR_E;
        let pose = VehiclePose::from_vehicle(&v);
        let motion = vehicle_render_direction_at(&v, pose);
        v.newgrf_mirrored = true;

        assert_eq!(vehicle_render_direction_at(&v, pose), motion);
        assert_eq!(
            vehicle_sprite_direction_at(&v, pose),
            crate::vehicle::reverse_direction(motion)
        );
    }

    #[test]
    fn articulated_mirror_and_road_history_survive_json_roundtrip() {
        let mut v = Vehicle::new(
            3,
            VehicleKind::Bus,
            TileCoord::new(2, 2),
            TileCoord::new(4, 2),
        );
        v.newgrf_articulated = true;
        v.newgrf_mirrored = true;
        v.road_tile_history = VecDeque::from([TileCoord::new(1, 2)]);

        let restored: Vehicle = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();

        assert!(restored.newgrf_mirrored);
        assert_eq!(restored.road_tile_history, v.road_tile_history);
    }

    #[test]
    fn turn_curve_endpoints_match_openrtd_data() {
        let v = ne_to_se_turn_vehicle();
        let (entry, exit) = road_turn_entry_exit(&v).unwrap();
        let curve = turn_curve_points(entry, exit).unwrap();
        let start = curves::sample_curve(curve, 0.0);
        let end = curves::sample_curve(curve, 255.0);
        assert_eq!(start, curve[0]);
        assert_eq!(end, curve[curve.len() - 1]);
    }

    #[test]
    fn straight_tile_uses_real_road_frame() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        v.path = VecDeque::from([
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
        ]);
        v.direction = DIR_SW;
        v.road_state = 8;
        v.frame = 11;
        v.progress = 0;
        assert!(road_turn_entry_exit(&v).is_none());
        let (x, y) = vehicle_subtile(&v);
        assert_eq!((x, y), (11.0, 9.0));
    }

    #[test]
    fn train_uses_center_track_not_road_lanes() {
        const TRAIN_TRACK_CENTER: f32 = 8.0;
        let (tx, ty) = train_straight_subtile(DIR_SW, 128.0);
        let (rx, ry) = straight_subtile(DIR_SW, 128.0);
        assert!(
            (ty - TRAIN_TRACK_CENTER).abs() < 0.1,
            "eje horizontal por el centro de la vía"
        );
        assert!((ty - ry).abs() > 0.5, "no usa carril de carretera (y={ry})");
        assert!(tx > 0.0 && tx < 15.0, "avance a lo largo de x (tx={tx})");
        let _ = rx;
    }

    #[test]
    fn train_motion_remainder_advances_between_rail_pixels() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(2, 2),
            TileCoord::new(3, 2),
        );
        v.direction = DIR_SW;
        v.rail_pixel = 8;
        v.progress = 96;
        let pose = VehiclePose::from_vehicle(&v);
        assert!((pose.progress_f - 135.46875).abs() < f32::EPSILON);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn train_crosses_tile_boundary_without_visual_jump() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        for x in 2..=4 {
            let c = TileCoord::new(x, 2);
            map.set_kind(c, crate::map::TileKind::Rail).unwrap();
            map.set_mapt_m5(c, 0, 0x01).unwrap();
        }
        let mut v = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(2, 2),
            TileCoord::new(4, 2),
        );
        v.path = VecDeque::from([TileCoord::new(3, 2), TileCoord::new(4, 2)]);
        v.direction = DIR_SW;
        v.running = true;
        v.cur_speed = v.effective_engine().max_speed;
        v.rail_pixel = 15;
        v.progress = 191;

        let before = VehiclePose::from_vehicle(&v);
        let before_sub = vehicle_subtile_at_with_map(&v, before, Some(&map));
        let after = extrapolate_vehicle_pose(&v, 1.0);
        let after_sub = vehicle_subtile_at_with_map(&v, after, Some(&map));
        let before_world_x = before.pos.x as f32 * 16.0 + before_sub.0;
        let after_world_x = after.pos.x as f32 * 16.0 + after_sub.0;
        assert_eq!(after.pos, TileCoord::new(3, 2));
        assert!(
            after_world_x >= before_world_x && after_world_x - before_world_x < 2.0,
            "cruce continuo: {before_world_x} → {after_world_x}"
        );
    }

    #[test]
    fn train_render_follows_route_track_at_switch() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let junction = TileCoord::new(3, 2);
        map.set_kind(junction, crate::map::TileKind::Rail).unwrap();
        map.set_mapt_m5(junction, 0, 0x01 | 0x20).unwrap(); // X + RIGHT
        let mut v = Vehicle::new(0, VehicleKind::Train, junction, TileCoord::new(3, 3));
        v.path = VecDeque::from([TileCoord::new(3, 3)]);
        v.rail_tile_history.push_front(TileCoord::new(2, 2));
        v.direction = DIR_SW;
        v.running = true;
        let mut pose = VehiclePose::from_vehicle(&v);
        pose.progress = 255;
        pose.progress_f = 255.0;
        let sub = vehicle_subtile_at_with_map(&v, pose, Some(&map));
        assert_eq!(sub, (7.0, 15.0), "debe tomar RIGHT, no seguir por X");
    }

    #[test]
    fn stopped_road_vehicle_keeps_its_authoritative_frame() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(15, 3),
            TileCoord::new(15, 3),
        );
        v.direction = DIR_NW;
        v.road_state = 9;
        v.frame = 15;
        v.progress = 255; // sentinel de llegada; no es posición visual
        let parked = vehicle_subtile(&v);
        assert_eq!(parked, (9.0, 0.0));
    }

    #[test]
    fn extrapolate_advances_real_road_frame_between_sim_ticks() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(5, 6),
            TileCoord::new(6, 6),
        );
        v.path = VecDeque::from([TileCoord::new(6, 6)]);
        v.set_cruise_speed();
        v.progress = 230;
        let before = VehiclePose::from_vehicle(&v);
        let pose = extrapolate_vehicle_pose(&v, 1.0);
        assert_eq!(pose.pos, v.pos, "la pose no inventa un cambio de tesela");
        assert!(pose.road_frame_f > before.road_frame_f);
        assert_eq!(pose.depart_turn, 0);
    }

    /// Camión con orden en la bahía donde está parado (entró rumbo NW).
    fn parked_in_bay_vehicle() -> Vehicle {
        let bay = TileCoord::new(4, 5);
        let mut v = Vehicle::new(0, VehicleKind::Truck, bay, bay);
        v.direction = DIR_NW;
        v.progress = 255;
        v.set_station_orders(vec![bay, TileCoord::new(10, 5)]);
        v.progress = 255;
        v
    }

    #[test]
    fn parked_in_bay_sits_on_stop_frame_of_rv_station_table() {
        let v = parked_in_bay_vehicle();
        let table = bay_station_table(DIR_NW, true).unwrap();
        assert_eq!(
            vehicle_subtile(&v),
            table.points[table.stop],
            "detenido en el stop frame de `_rv_station_left_se_far` (9,5)"
        );
    }

    #[test]
    fn bay_entry_follows_rv_station_table_from_mouth_to_stop() {
        let mut v = parked_in_bay_vehicle();
        let table = bay_station_table(DIR_NW, true).unwrap();
        v.progress = 0;
        assert_eq!(vehicle_subtile(&v), table.points[0], "entra por la boca");
        v.progress = 255;
        assert_eq!(vehicle_subtile(&v), table.points[table.stop]);
    }

    #[test]
    fn bay_exit_retraces_loop_back_to_mouth() {
        let mut v = parked_in_bay_vehicle();
        let table = bay_station_table(DIR_NW, true).unwrap();
        // Tras el giro: rumbo de salida SE hacia la carretera de acceso.
        v.direction = DIR_SE;
        v.path = VecDeque::from([TileCoord::new(4, 6)]);
        v.progress = 0;
        assert_eq!(
            vehicle_subtile(&v),
            table.points[table.stop],
            "la salida arranca en el punto de parada"
        );
        v.progress = 255;
        assert_eq!(
            vehicle_subtile(&v),
            *table.points.last().unwrap(),
            "y termina en la boca (5,15)"
        );
    }

    #[test]
    fn bay_sprite_direction_follows_loop_not_logical_heading() {
        let mut v = parked_in_bay_vehicle();
        // Mitad de la entrada SE-far: tramo transversal del lazo (x decrece →
        // componente NE), distinto del rumbo lógico NW de entrada.
        v.progress = 40;
        let pose = VehiclePose::from_vehicle(&v);
        let dir = render_pose::vehicle_render_direction_at(&v, pose);
        assert_ne!(dir, DIR_SE, "no debe usar el rumbo de salida");
        let table = bay_station_table(DIR_NW, true).unwrap();
        let (x0, _) = table.points[0];
        let (x1, _) = table.points[7];
        assert!(x1 < x0, "el tramo inicial del lazo se mueve hacia -x (NE)");
    }

    #[test]
    fn fta_aircraft_sprite_follows_actual_subpixel_heading() {
        let mut aircraft = Vehicle::new(
            77,
            VehicleKind::Aircraft,
            TileCoord::new(5, 5),
            TileCoord::new(10, 5),
        );
        aircraft.running = true;
        aircraft.airport_fta_active = true;
        aircraft.airport_subpos_valid = true;
        aircraft.direction = DIR_NW;

        let pose = VehiclePose::from_vehicle(&aircraft);
        assert_eq!(
            vehicle_render_direction_at(&aircraft, pose),
            DIR_NW,
            "la orden global apunta al este, pero el sprite debe seguir el giro FTA al noroeste"
        );
    }

    #[test]
    fn turn_midpoint_differs_from_tile_center() {
        let mut v = ne_to_se_turn_vehicle();
        v.road_state = 3; // TRACKDIR_LOWER_E: NE -> SE
        v.frame = 8;
        v.progress = 0;
        let (x, y) = vehicle_subtile(&v);
        // Punto medio de la curva NE→SE (~(7,8)), no centro del rombo.
        assert!(x > 5.0 && x < 10.0);
        assert!(y > 6.0 && y < 11.0);
    }

    #[test]
    fn bay_state_renders_far_and_near_stop_frames_from_controller() {
        let bay = TileCoord::new(4, 5);
        let mut far = Vehicle::new(1, VehicleKind::Bus, bay, bay);
        far.direction = DIR_NW;
        far.road_state = rvsb::RVSB_IN_ROAD_STOP | 9 | rvsb::RVSB_ENTERED_STOP;
        far.frame = bay::bay_stop_frame(far.road_state).unwrap();
        let mut near = far.clone();
        near.id = 2;
        near.road_state |= rvsb::RVSB_USING_SECOND_BAY;
        near.frame = bay::bay_stop_frame(near.road_state).unwrap();

        let far_table = bay_station_table(DIR_NW, true).unwrap();
        let near_table = bay_station_table(DIR_NW, false).unwrap();
        assert_eq!(vehicle_subtile(&far), far_table.points[far_table.stop]);
        assert_eq!(vehicle_subtile(&near), near_table.points[near_table.stop]);
        assert_ne!(vehicle_subtile(&far), vehicle_subtile(&near));
    }

    /// #227: las ocho tablas `_rv_station_right_*` coinciden con `OpenTTD` 15.3
    /// (stop frame y primer punto del lazo).
    #[test]
    fn bay_right_tables_match_openttd_stop_and_mouth() {
        let cases: [(VehicleDirection, bool, (f32, f32), usize); 8] = [
            (DIR_NE, true, (15.0, 9.0), 16),
            (DIR_NE, false, (15.0, 9.0), 12),
            (DIR_SE, true, (9.0, 0.0), 16),
            (DIR_SE, false, (9.0, 0.0), 12),
            (DIR_SW, true, (0.0, 5.0), 15),
            (DIR_SW, false, (0.0, 5.0), 11),
            (DIR_NW, true, (5.0, 15.0), 15),
            (DIR_NW, false, (5.0, 15.0), 11),
        ];
        for (inbound, far, mouth, stop) in cases {
            let left = bay_station_table(inbound, far).expect("left");
            let right = bay_station_table_for_side(inbound, far, true).expect("right");
            assert_eq!(right.points[0], mouth, "mouth {inbound:?} far={far}");
            assert_eq!(right.stop, stop, "stop {inbound:?} far={far}");
            assert_ne!(
                left.points, right.points,
                "tabla right distinta de left ({inbound:?} far={far})"
            );
            let state = rvsb::RVSB_IN_ROAD_STOP
                | match inbound {
                    DIR_NE => 0,
                    DIR_SE => 1,
                    DIR_SW => 8,
                    DIR_NW => 9,
                    _ => unreachable!(),
                }
                | if far { 0 } else { rvsb::RVSB_USING_SECOND_BAY };
            assert_eq!(
                bay::bay_stop_frame_side(state, true),
                Some(u8::try_from(stop).unwrap())
            );
        }
    }

    #[test]
    fn bay_right_render_uses_right_stop_point() {
        let bay = TileCoord::new(4, 5);
        let mut v = Vehicle::new(1, VehicleKind::Bus, bay, bay);
        v.direction = DIR_NW;
        v.road_state = rvsb::RVSB_IN_ROAD_STOP | 9 | rvsb::RVSB_ENTERED_STOP;
        let stop = bay::bay_stop_frame_side(v.road_state, true).unwrap();
        v.frame = stop;
        let pose = VehiclePose::from_vehicle(&v).with_drive_on_right(true);
        let right = bay_station_table_for_side(DIR_NW, true, true).unwrap();
        assert_eq!(
            vehicle_subtile_at(&v, pose),
            right.points[right.stop],
            "render right en stop frame"
        );
    }
}
