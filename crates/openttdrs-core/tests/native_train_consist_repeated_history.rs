//! Native pixel geometry for a six-unit chain revisiting a rail junction.
#![allow(clippy::expect_used, clippy::cast_precision_loss)]
use openttdrs_core::{Map, TileCoord, TileKind, Vehicle, VehicleKind, VehiclePose};
const ROUTE: [(i32, i32); 15] = [
    (7, 4),
    (6, 4),
    (5, 4),
    (4, 4),
    (3, 4),
    (3, 5),
    (4, 5),
    (4, 4),
    (3, 4),
    (3, 5),
    (4, 5),
    (4, 4),
    (3, 4),
    (3, 5),
    (4, 5),
];
fn chain(fields: &[&str], render: bool) -> (Map, Vec<Vehicle>) {
    let index: usize = fields[0].parse().expect("head occurrence");
    let (x, y) = ROUTE[index];
    let pos = TileCoord::new(x, y);
    let mut vs: Vec<_> = (0..6)
        .map(|i| {
            let mut v = Vehicle::new(i + 1, VehicleKind::Train, pos, pos);
            v.unit_length = 8;
            v.prev_unit = (i > 0).then_some(i);
            v.next_unit = (i < 5).then_some(i + 2);
            v.running = true;
            v
        })
        .collect();
    vs[0].rail_pixel = fields[1].parse().expect("head pixel");
    vs[0].direction = fields[2].parse().expect("physical heading");
    if render {
        vs[0].cur_speed = 512;
        vs[0].progress = fields[13].parse().expect("head remainder");
    }
    vs[0].path.extend(
        ROUTE[index + 1..]
            .iter()
            .map(|&(x, y)| TileCoord::new(x, y)),
    );
    vs[0].rail_tile_history.extend(
        ROUTE[..index]
            .iter()
            .rev()
            .map(|&(x, y)| TileCoord::new(x, y)),
    );
    let mut map = Map::new_flat(10, 10, 0);
    // All pieces at the junction are available; the recorded route selects
    // the native piece for each occurrence, rather than priority by track bit.
    for (x, y) in ROUTE {
        let tile = TileCoord::new(x, y);
        map.set_kind(tile, TileKind::Rail).expect("rail");
        map.set_mapt_m5(tile, 0, 63).expect("junction tracks");
    }
    (map, vs)
}
fn check_fixture(render: bool) {
    let fixture = if render {
        include_str!("fixtures/parity/native-train-consist-repeated-history-render.csv")
    } else {
        include_str!("fixtures/parity/native-train-consist-repeated-history.csv")
    };
    let mut errors = 0;
    let mut samples = 0;
    let mut examples = Vec::new();
    for line in fixture.lines().skip(1) {
        let f: Vec<_> = line.split(',').collect();
        let (map, mut vs) = chain(&f, render);
        let physical = openttdrs_core::consist_unit_poses(&vs, 1);
        openttdrs_core::propagate_consist_unit_poses(&mut vs, 1);
        let unit: usize = f[3].parse().expect("unit");
        let (pose, route) = if render {
            let alpha = f[14].parse::<f32>().expect("budget") / 768.0;
            let head_pose = openttdrs_core::extrapolate_vehicle_pose(&vs[0], alpha);
            let poses = openttdrs_core::train_consist::consist_render_poses_indexed(
                &vs,
                &[0, 1, 2, 3, 4, 5],
                head_pose,
            );
            let pose = poses[unit];
            let route = pose.train_route;
            if unit > 0 {
                assert!(route.is_some(), "follower projected route metadata");
            }
            (pose, route)
        } else {
            (
                VehiclePose::from_vehicle(&vs[unit]),
                Some((
                    physical[unit].direction,
                    physical[unit].curve_prev_direction,
                )),
            )
        };
        let (sx, sy) = openttdrs_core::vehicle_subtile_at_with_map(&vs[unit], pose, Some(&map));
        let x = pose.pos.x as f32 * 16.0 + sx;
        let y = pose.pos.y as f32 * 16.0 + sy;
        let direction =
            openttdrs_core::vehicle_render_direction_at_with_map(&vs[unit], pose, Some(&map));
        assert_eq!(f[12], "0", "native centre spacing invariant");
        let route_good = render && unit == 0
            || route
                == Some((
                    f[10].parse().expect("native entry"),
                    f[11].parse().expect("native exit"),
                ));
        let good = route_good
            && pose.pos
                == TileCoord::new(f[4].parse().expect("tile x"), f[5].parse().expect("tile y"))
            && (x - f[7].parse::<f32>().expect("native x")).abs() < 0.0001
            && (y - f[8].parse::<f32>().expect("native y")).abs() < 0.0001
            && direction == f[9].parse::<u8>().expect("native heading")
            && (render
                || physical[unit].rail_pixel == f[6].parse::<u8>().expect("native rail pixel"));
        if !good {
            if examples.len() < 6 {
                examples.push(format!(
                    "{line}: {pose:?}, ({x},{y}), dir={direction}, route={route:?}"
                ));
            }
            errors += 1;
        }
        samples += 1;
    }
    assert_eq!(samples, if render { 4800 } else { 240 });
    assert_eq!(errors, 0, "first mismatches: {examples:?}");
}
#[test]
fn physical_followers_distinguish_repeated_junction_occurrences() {
    check_fixture(false);
}
#[test]
fn interpolated_followers_distinguish_repeated_junction_occurrences() {
    check_fixture(true);
}
