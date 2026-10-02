//! In-tile coordinates against native entry and pixel-step bodies.
#![allow(clippy::expect_used)]

use openttdrs_core::{
    Map, TileCoord, TileKind, Vehicle, VehicleKind, VehiclePose,
    vehicle_render_direction_at_with_map, vehicle_subtile_at_with_map,
};

#[test]
fn train_render_pixels_and_remainders_match_native_track_entries() {
    let fixture = include_str!("fixtures/parity/native-train-subtile-motion.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        let track: u8 = fields[0].parse().expect("native track");
        let enter: u8 = fields[1].parse().expect("native entrance direction");
        let pixel: u8 = fields[2].parse().expect("native rail pixel");
        let remainder: u8 = fields[3].parse().expect("native remainder");
        let x: f32 = fields[5].parse().expect("native interpolated x");
        let y: f32 = fields[6].parse().expect("native interpolated y");
        let direction: u8 = fields[7].parse().expect("native physical heading");
        let tile = TileCoord::new(1, 1);
        let steps = [(-1, 0), (0, 1), (1, 0), (0, -1)];
        let entry_index = (enter - 1) / 2;
        let (dx, dy) = steps[usize::from(entry_index)];
        let (a, b) = openttdrs_core::train_movement::rail_track_sides(track)
            .expect("native connected track");
        let inbound_side = (entry_index + 2) & 3;
        let exit = if a == inbound_side { b } else { a };
        let (out_x, out_y) = steps[usize::from(exit)];
        let next = TileCoord::new(tile.x + out_x, tile.y + out_y);
        let mut map = Map::new_flat(3, 3, 0);
        map.set_kind(tile, TileKind::Rail).expect("rail fixture");
        map.set_mapt_m5(tile, 0, track)
            .expect("native track fixture");
        let mut vehicle = Vehicle::new(1, VehicleKind::Train, tile, next);
        vehicle.running = true;
        vehicle.cur_speed = 112;
        vehicle.direction = direction;
        vehicle.rail_pixel = pixel;
        vehicle.progress = remainder;
        vehicle
            .rail_tile_history
            .push_front(TileCoord::new(tile.x - dx, tile.y - dy));
        vehicle.path.push_back(next);
        let pose = VehiclePose::from_vehicle(&vehicle);
        let actual = vehicle_subtile_at_with_map(&vehicle, pose, Some(&map));
        assert!(
            (actual.0 - x).abs() < 0.0001 && (actual.1 - y).abs() < 0.0001,
            "{line}: actual={actual:?}"
        );
        assert_eq!(
            vehicle_render_direction_at_with_map(&vehicle, pose, Some(&map)),
            direction,
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 512);
}
