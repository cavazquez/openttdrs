//! Ordinary drive-table steps against pinned native direction/turn fragments.
//! Traffic, tile transitions, bays, reversals and speed integration are excluded.
#![allow(clippy::expect_used)]

use openttdrs_core::road_movement::individual_road_vehicle_controller_side;
use openttdrs_core::{
    TileCoord, Vehicle, VehicleKind, VehiclePose, vehicle_render_direction_at, vehicle_subtile_at,
};

#[derive(Clone, Copy, Debug)]
struct NativeStep {
    table: u8,
    substep: u16,
    frame: u8,
    x: i32,
    y: i32,
    direction: u8,
    speed: u16,
    turn_only: bool,
}

fn steps() -> Vec<NativeStep> {
    include_str!("fixtures/parity/native-road-vehicle-turn-direction.csv")
        .lines()
        .skip(1)
        .map(|line| {
            let values: Vec<u16> = line
                .split(',')
                .map(|value| value.parse().expect("native numeric field"))
                .collect();
            let [table, substep, frame, x, y, direction, speed, turn_only] = values[..] else {
                panic!("native road row requires eight columns");
            };
            NativeStep {
                table: u8::try_from(table).expect("native table index"),
                substep,
                frame: u8::try_from(frame).expect("native frame"),
                x: i32::from(x),
                y: i32::from(y),
                direction: u8::try_from(direction).expect("native direction"),
                speed,
                turn_only: turn_only != 0,
            }
        })
        .collect()
}

fn vehicle_at(step: NativeStep) -> Vehicle {
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    );
    v.running = true;
    v.path.push_back(TileCoord::new(1, 0));
    v.road_state = step.table & 15;
    v.frame = step.frame;
    v.road_x = step.x;
    v.road_y = step.y;
    v.road_pos_valid = true;
    v.direction = step.direction;
    v.cur_speed = step.speed;
    v.progress = 0;
    v
}

#[test]
fn road_controller_matches_native_ordinary_turn_steps_on_both_sides() {
    let rows = steps();
    assert_eq!(rows.len(), 352);
    let mut vehicle = None;
    let mut tables = 0;
    for step in rows {
        if step.substep == 0 {
            vehicle = Some(vehicle_at(step));
            tables += 1;
        } else {
            let v = vehicle.as_mut().expect("native initial step");
            let old_frame = v.frame;
            assert!(individual_road_vehicle_controller_side(
                std::slice::from_mut(v),
                0,
                None,
                step.table >= 16,
            ));
            assert_eq!(v.frame == old_frame, step.turn_only, "{step:?}");
        }
        let v = vehicle.as_ref().expect("native initial step");
        assert_eq!(
            (v.frame, v.road_x, v.road_y, v.direction, v.cur_speed),
            (step.frame, step.x, step.y, step.direction, step.speed),
            "{step:?}"
        );
    }
    assert_eq!(tables, 24);
}

#[test]
fn road_sprite_direction_matches_native_before_and_after_turn_delay() {
    for step in steps() {
        let v = vehicle_at(step);
        let pose = VehiclePose::from_vehicle(&v).with_drive_on_right(step.table >= 16);
        assert_eq!(
            vehicle_render_direction_at(&v, pose),
            step.direction,
            "native orientation at {step:?}"
        );
    }
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn road_render_budget_matches_recorded_native_stationary_and_moving_steps() {
    let states: std::collections::BTreeMap<_, _> = steps()
        .into_iter()
        .map(|step| ((step.table, step.substep), step))
        .collect();
    let fixture = include_str!("fixtures/parity/native-road-render-budget.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        let table: u8 = fields[0].parse().expect("native table");
        let substep: u16 = fields[1].parse().expect("native substep");
        let budget: f32 = fields[2].parse().expect("native movement budget");
        let x: f32 = fields[3].parse().expect("native interpolated x");
        let y: f32 = fields[4].parse().expect("native interpolated y");
        let direction: u8 = fields[5].parse().expect("native direction");
        for kind in [VehicleKind::Bus, VehicleKind::Truck, VehicleKind::Tram] {
            let mut v = vehicle_at(states[&(table, substep)]);
            v.kind = kind;
            let mut pose = VehiclePose::from_vehicle(&v).with_drive_on_right(table >= 16);
            pose.road_frame_f = f32::from(v.frame)
                + budget / openttdrs_core::get_advance_distance(v.direction) as f32;
            let (actual_x, actual_y) = vehicle_subtile_at(&v, pose);
            assert!(
                (actual_x - x).abs() < 0.0001 && (actual_y - y).abs() < 0.0001,
                "{kind:?}, {line}: actual=({actual_x}, {actual_y})"
            );
            assert_eq!(
                vehicle_render_direction_at(&v, pose),
                direction,
                "{kind:?}, {line}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 3520);
}
