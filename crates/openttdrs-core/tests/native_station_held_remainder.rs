//! Isolated native loading guards; service/order maintenance and departure omitted.
#![allow(clippy::expect_used)]

use openttdrs_core::{TileCoord, Vehicle, VehicleKind, VehicleOrder};

#[test]
fn physical_road_transfer_preserves_native_remainder_for_repeated_holds() {
    let mut cases = 0;
    for kind in [VehicleKind::Bus, VehicleKind::Truck] {
        for unloading in [false, true] {
            for line in include_str!("fixtures/parity/native-road-bay-held-remainder.csv")
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
                    _,
                    _,
                    _,
                    speed,
                    subspeed,
                    progress,
                    entered,
                    loading,
                    calls,
                ] = values[..]
                else {
                    panic!("native road hold requires fourteen fields");
                };
                assert_eq!(loading, 1);
                let mut v = Vehicle::new(1, kind, TileCoord::new(0, 0), TileCoord::new(0, 0));
                v.running = true;
                v.orders = vec![VehicleOrder::station(v.pos)];
                v.awaiting_load_window = true;
                v.cargo_loading = !unloading;
                v.cargo_unloading = unloading;
                v.road_state =
                    u8::try_from((table & 0x2F) | (entered << 2)).expect("native bay state");
                v.frame = u8::try_from(frame).expect("native frame");
                v.road_x = i32::from(x);
                v.road_y = i32::from(y);
                v.road_pos_valid = true;
                v.direction = u8::try_from(direction).expect("native direction");
                v.cur_speed = speed;
                v.subspeed = u8::try_from(subspeed).expect("native subspeed");
                v.progress = u8::try_from(progress).expect("native remainder");
                for _ in 0..calls {
                    v.step();
                }
                assert_eq!(
                    (v.cur_speed, u16::from(v.subspeed), u16::from(v.progress)),
                    (speed, subspeed, progress),
                    "{line}, {kind:?}, unloading={unloading}"
                );
                assert_eq!(
                    (
                        u16::from(v.frame),
                        v.road_x,
                        v.road_y,
                        u16::from(v.direction)
                    ),
                    (frame, i32::from(x), i32::from(y), direction),
                    "{line}"
                );
                assert_eq!(v.road_state & (1 << 2) != 0, entered != 0, "{line}");
                assert!(v.awaiting_load_window, "{line}");
                assert_eq!(v.current_order, 0, "{line}");
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 13_056);
}

#[test]
fn train_transfer_preserves_native_remainder_for_repeated_holds() {
    let mut cases = 0;
    for unloading in [false, true] {
        for line in include_str!("fixtures/parity/native-train-station-held-remainder.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            assert_eq!(fields.len(), 12);
            assert_eq!(fields[8], "2", "native OT_LOADING guard");
            assert_eq!(fields[9], "1", "native guard retains position");
            let pixel: u8 = fields[2].parse().expect("native pixel");
            let remainder: u8 = fields[3].parse().expect("native remainder");
            let direction: u8 = fields[7].parse().expect("native direction");
            let subspeed: u8 = fields[10].parse().expect("native subspeed");
            let calls: u16 = fields[11].parse().expect("native held calls");
            let mut v = Vehicle::new(
                1,
                VehicleKind::Train,
                TileCoord::new(1, 1),
                TileCoord::new(1, 1),
            );
            v.running = true;
            v.orders = vec![VehicleOrder::station(v.pos)];
            v.awaiting_load_window = true;
            v.cargo_loading = !unloading;
            v.cargo_unloading = unloading;
            v.cur_speed = 0;
            v.subspeed = subspeed;
            v.progress = remainder;
            v.rail_pixel = pixel;
            v.direction = direction;
            for _ in 0..calls {
                v.step();
            }
            assert_eq!(
                (
                    v.cur_speed,
                    v.subspeed,
                    v.progress,
                    v.rail_pixel,
                    v.direction
                ),
                (0, subspeed, remainder, pixel, direction),
                "{line}, unloading={unloading}"
            );
            assert_eq!(v.pos, TileCoord::new(1, 1), "{line}");
            assert!(v.awaiting_load_window, "{line}");
            assert_eq!(v.current_order, 0, "{line}");
            cases += 1;
        }
    }
    assert_eq!(cases, 12_288);
}
