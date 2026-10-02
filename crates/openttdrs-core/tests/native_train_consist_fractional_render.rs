//! Continuous chain presentation toward native GetNewVehiclePos candidates before entry rebinding.
#![allow(clippy::expect_used, clippy::cast_precision_loss)]
use openttdrs_core::{Map, TileCoord, TileKind, Vehicle, VehicleKind, VehiclePose};
fn exit_step(track: u8, enter: u8) -> (i32, i32) {
    let (a, b) =
        openttdrs_core::train_movement::rail_track_sides(track).expect("native connected track");
    let inbound = ((enter - 1) / 2 + 2) & 3;
    let exit = if a == inbound { b } else { a };
    [(-1, 0), (0, 1), (1, 0), (0, -1)][usize::from(exit)]
}

fn fixture_chain(fields: &[i32]) -> (Map, Vec<Vehicle>) {
    let track = u8::try_from(fields[1]).expect("track");
    let enter = u8::try_from(fields[2]).expect("enter");
    let next_track = u8::try_from(fields[3]).expect("next track");
    let next_enter = u8::try_from(fields[4]).expect("next enter");
    let first = TileCoord::new(4, 4);
    let (dx, dy) = [(-1, 0), (0, 1), (1, 0), (0, -1)][usize::from((enter - 1) / 2)];
    let prefix: Vec<_> = (1..=3)
        .map(|n| TileCoord::new(4 - dx * n, 4 - dy * n))
        .collect();
    let (nx, ny) = exit_step(track, enter);
    let next = TileCoord::new(4 + nx, 4 + ny);
    let (nx, ny) = exit_step(next_track, next_enter);
    let after = TileCoord::new(next.x + nx, next.y + ny);
    let mut map = Map::new_flat(10, 10, 0);
    let prefix_track = if enter & 2 == 0 { 1 } else { 2 };
    for (pos, bit) in prefix
        .iter()
        .map(|&p| (p, prefix_track))
        .chain([(first, track), (next, next_track)])
    {
        map.set_kind(pos, TileKind::Rail).expect("rail fixture");
        map.set_mapt_m5(pos, 0, bit).expect("track fixture");
    }
    let head_pos = if fields[5] == 3 { first } else { next };
    let mut vehicles: Vec<_> = (0..3)
        .map(|n| {
            let mut v = Vehicle::new(n + 1, VehicleKind::Train, head_pos, after);
            v.unit_length = u8::try_from(fields[8 + n as usize]).expect("native length");
            v.prev_unit = (n > 0).then_some(n);
            v.next_unit = (n < 2).then_some(n + 2);
            v.running = true;
            v
        })
        .collect();
    let head = &mut vehicles[0];
    head.rail_pixel = u8::try_from(fields[6]).expect("head pixel");
    head.direction = u8::try_from(fields[7]).expect("head physical heading");
    if fields[5] == 3 {
        head.path.extend([next, after]);
    } else {
        head.path.push_back(after);
        head.rail_tile_history.push_back(first);
    }
    head.rail_tile_history.extend(prefix);
    (map, vehicles)
}

#[test]
fn all_units_share_the_heads_native_step_count_and_fraction() {
    let fixture = include_str!("fixtures/parity/native-train-consist-fractional-render.csv");
    let mut count = 0;
    let mut errors = 0;
    let mut discrete_follower_errors = 0;
    let mut examples = Vec::new();
    for line in fixture.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        let inputs: Vec<i32> = fields[..12]
            .iter()
            .map(|s| s.parse().expect("native input"))
            .collect();
        let (map, mut vehicles) = fixture_chain(&inputs);
        vehicles[0].cur_speed = 512;
        vehicles[0].progress = fields[20].parse().expect("native head remainder");
        openttdrs_core::propagate_consist_unit_poses(&mut vehicles, 1);
        let unit: usize = fields[11].parse().expect("unit");
        let alpha = fields[21].parse::<f32>().expect("render budget") / 768.0;
        let head_pose = openttdrs_core::extrapolate_vehicle_pose(&vehicles[0], alpha);
        let poses = openttdrs_core::train_consist::consist_render_poses_indexed(
            &vehicles,
            &[0, 1, 2],
            head_pose,
        );
        let pose = poses[unit];
        let local = openttdrs_core::vehicle_subtile_at_with_map(&vehicles[unit], pose, Some(&map));
        let x = pose.pos.x as f32 * 16.0 + local.0;
        let y = pose.pos.y as f32 * 16.0 + local.1;
        let direction =
            openttdrs_core::vehicle_render_direction_at_with_map(&vehicles[unit], pose, Some(&map));
        let route_good = unit == 0
            || pose.train_route
                == Some((
                    fields[18].parse().expect("native entry"),
                    fields[19].parse().expect("native exit"),
                ));
        let discrete = VehiclePose::from_vehicle(&vehicles[unit]);
        let ds = openttdrs_core::vehicle_subtile_at_with_map(&vehicles[unit], discrete, Some(&map));
        discrete_follower_errors += usize::from(
            unit > 0
                && ((discrete.pos.x as f32 * 16.0 + ds.0
                    - fields[15].parse::<f32>().expect("native x"))
                .abs()
                    >= 0.0001
                    || (discrete.pos.y as f32 * 16.0 + ds.1
                        - fields[16].parse::<f32>().expect("native y"))
                    .abs()
                        >= 0.0001),
        );
        let good = route_good
            && pose.pos
                == TileCoord::new(
                    fields[12].parse().expect("x tile"),
                    fields[13].parse().expect("y tile"),
                )
            && (x - fields[15].parse::<f32>().expect("native x")).abs() < 0.0001
            && (y - fields[16].parse::<f32>().expect("native y")).abs() < 0.0001
            && direction == fields[17].parse::<u8>().expect("native direction");
        errors += usize::from(!good);
        if !good && examples.len() < 8 {
            examples.push(format!(
                "{line}: pose={pose:?}, world=({x},{y}), direction={direction}"
            ));
        }
        count += 1;
    }
    println!(
        "{count} native chain presentations; {discrete_follower_errors} discrete follower mismatches"
    );
    assert_eq!(count, 115200);
    assert_eq!(errors, 0, "first mismatches: {examples:?}");
}
