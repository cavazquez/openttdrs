//! Isolated native departure branch; occupancy is adapted, speed ticks omitted.
#![allow(clippy::expect_used)]

use openttdrs_core::road_movement::individual_road_vehicle_controller_side;
use openttdrs_core::{
    TileCoord, Vehicle, VehicleKind, extrapolate_vehicle_pose, vehicle_render_direction_at,
    vehicle_subtile_at,
};

#[test]
fn bay_exit_busy_entrance_matches_native_speed_and_stationary_pose() {
    let mut cases = 0;
    let mut held_poses = 0;
    for kind in [VehicleKind::Bus, VehicleKind::Truck] {
        for line in include_str!("fixtures/parity/native-road-bay-busy-exit.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<u16> = line
                .split(',')
                .map(|field| field.parse().expect("native numeric field"))
                .collect();
            let [
                table,
                busy,
                calls,
                input_frame,
                input_x,
                input_y,
                input_direction,
                input_speed,
                input_subspeed,
                input_progress,
                frame,
                x,
                y,
                direction,
                speed,
                subspeed,
                progress,
                entered,
                result,
            ] = fields[..]
            else {
                panic!("native exit row requires nineteen fields");
            };
            let mut v = Vehicle::new(1, kind, TileCoord::new(0, 0), TileCoord::new(0, 0));
            v.running = true;
            v.road_pos_valid = true;
            v.road_state = u8::try_from((table & 0x2F) | 4).expect("native bay state");
            v.frame = u8::try_from(input_frame).expect("native stop frame");
            v.road_x = i32::from(input_x);
            v.road_y = i32::from(input_y);
            v.direction = u8::try_from(input_direction).expect("native heading");
            let (dx, dy) = match input_direction {
                1 => (-1, 0),
                3 => (0, 1),
                5 => (1, 0),
                7 => (0, -1),
                _ => panic!("native bay departure heading must be diagonal"),
            };
            v.dest = TileCoord::new(dx, dy);
            v.path.push_back(v.dest);
            v.cur_speed = input_speed;
            v.subspeed = u8::try_from(input_subspeed).expect("native subspeed");
            v.progress = u8::try_from(input_progress).expect("native remainder");
            let mut vehicles = vec![v];
            if busy != 0 {
                // Adapt the native occupancy bit with another vehicle on the
                // inbound part of the bay. Its own controller is not run.
                let mut blocker = Vehicle::new(2, kind, TileCoord::new(0, 0), TileCoord::new(0, 0));
                blocker.running = true;
                blocker.road_state = u8::try_from(table & 0x2F).expect("native bay state");
                blocker.frame = 0;
                vehicles.push(blocker);
            }
            let mut actual_result = false;
            for _ in 0..calls {
                actual_result =
                    individual_road_vehicle_controller_side(&mut vehicles, 0, None, table >= 48);
            }
            let actual = &vehicles[0];
            assert_eq!(actual_result, result != 0, "{line}, {kind:?}");
            assert_eq!(
                (
                    actual.cur_speed,
                    u16::from(actual.subspeed),
                    u16::from(actual.progress)
                ),
                (speed, subspeed, progress),
                "{line}, {kind:?}"
            );
            assert_eq!(
                (
                    u16::from(actual.frame),
                    actual.road_x,
                    actual.road_y,
                    u16::from(actual.direction)
                ),
                (frame, i32::from(x), i32::from(y), direction),
                "{line}"
            );
            assert_eq!(actual.road_state & 4 != 0, entered != 0, "{line}");
            if busy != 0 {
                for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let pose =
                        extrapolate_vehicle_pose(actual, alpha).with_drive_on_right(table >= 48);
                    let (actual_x, actual_y) = vehicle_subtile_at(actual, pose);
                    assert!(
                        (actual_x - f32::from(x)).abs() < 0.0001
                            && (actual_y - f32::from(y)).abs() < 0.0001,
                        "{line}, alpha={alpha}: ({actual_x},{actual_y})"
                    );
                    assert_eq!(
                        u16::from(vehicle_render_direction_at(actual, pose)),
                        direction,
                        "{line}, alpha={alpha}"
                    );
                    held_poses += 1;
                }
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 7680);
    assert_eq!(held_poses, 30_720);
}
