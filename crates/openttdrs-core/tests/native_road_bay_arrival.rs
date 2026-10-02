//! Native turn-before-arrival and stop-frame trigger; service effects are stubbed.
#![allow(clippy::expect_used)]

use openttdrs_core::road_movement::individual_road_vehicle_controller_side;
use openttdrs_core::{TileCoord, Vehicle, VehicleKind, VehicleOrder};

#[test]
fn bay_controller_completes_native_turn_before_loading() {
    let mut cases = 0;
    for kind in [VehicleKind::Bus, VehicleKind::Truck] {
        let mut vehicle = None;
        let mut arrivals = 0;
        for line in include_str!("fixtures/parity/native-road-bay-arrival.csv")
            .lines()
            .skip(1)
        {
            let values: Vec<u16> = line
                .split(',')
                .map(|value| value.parse().expect("native numeric field"))
                .collect();
            let [
                table,
                substep,
                frame,
                x,
                y,
                direction,
                speed,
                entered,
                result,
                loading,
            ] = values[..]
            else {
                panic!("native bay arrival row requires ten columns");
            };
            if substep == 0 {
                let mut v = Vehicle::new(1, kind, TileCoord::new(0, 0), TileCoord::new(0, 0));
                v.running = true;
                v.orders = vec![VehicleOrder::station(v.pos)];
                v.road_state = u8::try_from(table & 0x2F).expect("native bay state");
                v.frame = 0;
                v.road_x = i32::from(x);
                v.road_y = i32::from(y);
                v.road_pos_valid = true;
                v.direction = u8::try_from(direction).expect("native direction");
                v.cur_speed = speed;
                vehicle = Some(v);
            } else {
                let v = vehicle.as_mut().expect("initial native state");
                assert_eq!(
                    individual_road_vehicle_controller_side(
                        std::slice::from_mut(v),
                        0,
                        None,
                        table >= 48,
                    ),
                    result != 0,
                    "controller result at {line}, {kind:?}"
                );
            }
            let v = vehicle.as_ref().expect("initial native state");
            assert_eq!(
                (
                    u16::from(v.frame),
                    v.road_x,
                    v.road_y,
                    u16::from(v.direction),
                    v.cur_speed
                ),
                (frame, i32::from(x), i32::from(y), direction, speed),
                "physical state at {line}, {kind:?}"
            );
            assert_eq!(v.road_state & (1 << 2) != 0, entered != 0, "{line}");
            assert_eq!(v.awaiting_load_window, loading != 0, "{line}");
            arrivals += usize::from(loading != 0);
            cases += 1;
        }
        assert_eq!(arrivals, 16);
    }
    assert_eq!(cases, 864);
}
