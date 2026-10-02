//! Single-vehicle loading-entry tick; native service/traffic effects are stubs.
#![allow(clippy::expect_used)]
use openttdrs_core::road_movement::road_vehicle_tick_side;
use openttdrs_core::{
    TileCoord, Vehicle, VehicleKind, VehicleOrder, extrapolate_vehicle_pose,
    vehicle_render_direction_at, vehicle_subtile_at,
};

#[test]
fn bay_loading_entry_tick_retains_native_fractional_speed_and_progress() {
    let mut cases = 0;
    for kind in [VehicleKind::Bus, VehicleKind::Truck] {
        for line in include_str!("fixtures/parity/native-road-bay-loading-remainder.csv")
            .lines()
            .skip(1)
        {
            let values: Vec<u16> = line
                .split(',')
                .map(|value| value.parse().expect("native numeric field"))
                .collect();
            let [
                table,
                frame,
                x,
                y,
                direction,
                input_speed,
                input_subspeed,
                input_progress,
                speed,
                subspeed,
                progress,
                entered,
                loading,
            ] = values[..]
            else {
                panic!("native bay tick row requires thirteen fields");
            };
            let mut v = Vehicle::new(1, kind, TileCoord::new(0, 0), TileCoord::new(0, 0));
            v.running = true;
            v.orders = vec![VehicleOrder::station(v.pos)];
            v.road_state = u8::try_from(table & 0x2F).expect("native bay state");
            v.frame = u8::try_from(frame).expect("native frame");
            v.road_x = i32::from(x);
            v.road_y = i32::from(y);
            v.road_pos_valid = true;
            v.direction = u8::try_from(direction).expect("native direction");
            v.cur_speed = input_speed;
            v.subspeed = u8::try_from(input_subspeed).expect("native subspeed");
            v.progress = u8::try_from(input_progress).expect("native progress");
            v.cached_max_track_speed = 112;
            road_vehicle_tick_side(std::slice::from_mut(&mut v), 0, None, table >= 48);
            assert_eq!(
                (v.cur_speed, u16::from(v.subspeed), u16::from(v.progress)),
                (speed, subspeed, progress),
                "native fractional state at {line}, {kind:?}"
            );
            assert_eq!(
                (
                    u16::from(v.frame),
                    v.road_x,
                    v.road_y,
                    u16::from(v.direction)
                ),
                (frame, i32::from(x), i32::from(y), direction),
                "native physical state at {line}"
            );
            assert_eq!(v.road_state & (1 << 2) != 0, entered != 0, "{line}");
            assert_eq!(v.awaiting_load_window, loading != 0, "{line}");
            if loading != 0 {
                for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let pose = extrapolate_vehicle_pose(&v, alpha).with_drive_on_right(table >= 48);
                    let (actual_x, actual_y) = vehicle_subtile_at(&v, pose);
                    assert!(
                        (actual_x - f32::from(x)).abs() < 0.0001,
                        "loading x at {line}"
                    );
                    assert!(
                        (actual_y - f32::from(y)).abs() < 0.0001,
                        "loading y at {line}"
                    );
                    assert_eq!(
                        u16::from(vehicle_render_direction_at(&v, pose)),
                        direction,
                        "loading direction at {line}"
                    );
                }
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 3456);
}
