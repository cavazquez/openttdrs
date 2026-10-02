//! Ordinary boundary positions and costs from pinned native movement fragments.
#![allow(clippy::expect_used)]
#![allow(clippy::cast_precision_loss)]

use openttdrs_core::{
    Map, TileCoord, TileKind, Vehicle, VehicleKind, extrapolate_vehicle_pose,
    vehicle_render_direction_at_with_map, vehicle_subtile_at_with_map,
};

#[test]
fn train_boundary_extrapolation_matches_native_track_entries_and_costs() {
    let fixture = include_str!("fixtures/parity/native-train-track-transition.csv");
    let steps = [(-1, 0), (0, 1), (1, 0), (0, -1)];
    let exit_step = |track, enter: u8| {
        let (a, b) = openttdrs_core::train_movement::rail_track_sides(track)
            .expect("native connected track");
        let inbound = ((enter - 1) / 2 + 2) & 3;
        let exit = if a == inbound { b } else { a };
        steps[usize::from(exit)]
    };
    let mut count = 0;
    let mut mismatches = Vec::new();
    let mut short_mismatches = 0;
    for line in fixture.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        let track: u8 = fields[0].parse().expect("native track");
        let enter: u8 = fields[1].parse().expect("native entrance");
        let next_track: u8 = fields[2].parse().expect("next native track");
        let next_enter: u8 = fields[3].parse().expect("next native entrance");
        let pixel: u8 = fields[4].parse().expect("native rail pixel");
        let remainder: u8 = fields[5].parse().expect("native remainder");
        let budget: f32 = fields[6].parse().expect("render advance budget");
        let tile_x: i32 = fields[7].parse().expect("native tile x");
        let tile_y: i32 = fields[8].parse().expect("native tile y");
        let path_index: usize = fields[9].parse().expect("native route index");
        let x: f32 = fields[10].parse().expect("native world x");
        let y: f32 = fields[11].parse().expect("native world y");
        let direction: u8 = fields[12].parse().expect("native physical heading");
        let tile = TileCoord::new(2, 2);
        let (dx, dy) = steps[usize::from((enter - 1) / 2)];
        let (out_x, out_y) = exit_step(track, enter);
        let next = TileCoord::new(tile.x + out_x, tile.y + out_y);
        let (out_x, out_y) = exit_step(next_track, next_enter);
        let after = TileCoord::new(next.x + out_x, next.y + out_y);
        let mut map = Map::new_flat(5, 5, 0);
        for (pos, bit) in [(tile, track), (next, next_track)] {
            map.set_kind(pos, TileKind::Rail).expect("rail fixture");
            map.set_mapt_m5(pos, 0, bit).expect("track fixture");
        }
        let mut vehicle = Vehicle::new(1, VehicleKind::Train, tile, after);
        vehicle.running = true;
        vehicle.cur_speed = 512; // GetAdvanceSpeed * two controller passes = 768.
        vehicle.direction =
            openttdrs_core::train_movement::train_render_dir_on_track(enter, track, 0.0)
                .expect("native initial heading");
        vehicle.rail_pixel = pixel;
        vehicle.progress = remainder;
        vehicle
            .rail_tile_history
            .push_front(TileCoord::new(tile.x - dx, tile.y - dy));
        vehicle.path.extend([next, after]);
        let pose = extrapolate_vehicle_pose(&vehicle, budget / 768.0);
        let local = vehicle_subtile_at_with_map(&vehicle, pose, Some(&map));
        let actual = (
            pose.pos.x as f32 * 16.0 + local.0,
            pose.pos.y as f32 * 16.0 + local.1,
        );
        let actual_direction = vehicle_render_direction_at_with_map(&vehicle, pose, Some(&map));
        if pose.pos != TileCoord::new(tile_x, tile_y)
            || pose.path_index != path_index
            || (actual.0 - x).abs() >= 0.0001
            || (actual.1 - y).abs() >= 0.0001
            || actual_direction != direction
        {
            if pixel == 7 {
                short_mismatches += 1;
            }
            mismatches.push(format!(
                "{line}: actual={actual:?}, direction={actual_direction}, tile={:?}, index={}",
                pose.pos, pose.path_index
            ));
        }
        count += 1;
    }
    assert_eq!(count, 1440);
    assert!(
        mismatches.is_empty(),
        "{} mismatches ({short_mismatches} from short tracks), first samples: {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(8)]
    );
}
