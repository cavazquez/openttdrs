//! AIR-POSE: el sprite sigue el rumbo físico también fuera del FTA.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeSet;

use openttdrs_core::{
    AirportSpecId, CargoType, Command, GameState, OrderLoadType, OrderNonStop, OrderUnloadType,
    TileCoord, Vehicle, VehicleKind, VehicleOrder, apply_command, extrapolate_vehicle_pose,
    vehicle_render_direction_at_with_map, vehicle_sprite_direction_at,
    vehicle_sprite_direction_at_with_map,
};

fn air_service() -> GameState {
    let mut state = GameState::new(64, 64);
    state.world_seed = 0x5940_0064;
    state.tick = openttdrs_core::news::tick_for_calendar_year(1950);
    state.sync_timers_from_tick();
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;
    let origins = [TileCoord::new(4, 4), TileCoord::new(42, 42)];
    for origin in origins {
        apply_command(
            &mut state,
            &Command::PlaceAirportArea {
                origin,
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        )
        .unwrap();
    }
    let anchors = origins.map(|origin| {
        state
            .stations
            .iter()
            .find(|s| s.covers_tile(origin))
            .unwrap()
            .pos
    });
    for station in &mut state.stations {
        station.add_waiting_cargo(CargoType::Passengers, 500);
    }
    apply_command(
        &mut state,
        &Command::BuildVehicleAtDepot(anchors[0], openttdrs_core::ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    let orders = anchors
        .into_iter()
        .map(|anchor| {
            VehicleOrder::station_with_types(
                anchor,
                OrderLoadType::LoadIfPossible,
                OrderUnloadType::UnloadIfPossible,
                OrderNonStop::NonStopDestination,
            )
        })
        .collect();
    apply_command(&mut state, &Command::SetVehicleOrderList(1, orders)).unwrap();
    apply_command(&mut state, &Command::ToggleVehicleRunning(1)).unwrap();
    state
}

#[test]
fn aircraft_sprite_keeps_physical_heading_through_complete_flights() {
    let mut state = air_service();
    let mut directions = BTreeSet::new();
    let mut left_airport = false;
    let mut returned_to_fta = false;
    let mut cruise_turns = 0;
    let mut previous = state.vehicles[0].direction;
    for elapsed in 1..=10_000 {
        state.step();
        let plane = state.vehicles.iter().find(|v| v.id == 1).unwrap();
        directions.insert(plane.direction);
        if !plane.airport_fta_active {
            left_airport = true;
            cruise_turns += usize::from(plane.direction != previous);
        } else if left_airport {
            returned_to_fta = true;
        }
        previous = plane.direction;
        for alpha in [0.0, 0.5, 1.0] {
            let pose = extrapolate_vehicle_pose(plane, alpha);
            for map in [None, Some(&state.map)] {
                assert_eq!(
                    vehicle_sprite_direction_at_with_map(plane, pose, map),
                    plane.direction,
                    "tick {elapsed}, alpha {alpha}, FTA {}: el path no puede reemplazar el rumbo físico",
                    plane.airport_fta_active,
                );
            }
        }
    }
    assert!(
        left_airport && returned_to_fta,
        "el ciclo debe salir y volver al FTA"
    );
    assert!(cruise_turns > 0, "debe haber giros durante el vuelo libre");
    assert!(directions.len() >= 4, "no basta un avión detenido o recto");
    assert!(state.stats.cargo_units_final_delivered > 0);
}

#[test]
fn all_aircraft_headings_preserve_physical_direction_and_newgrf_mirroring() {
    let mut plane = Vehicle::new(
        1,
        VehicleKind::Aircraft,
        TileCoord::new(5, 5),
        TileCoord::new(10, 5),
    );
    plane.path = [TileCoord::new(6, 5), TileCoord::new(6, 6)].into();
    plane.cur_speed = 200;
    for active in [false, true] {
        plane.airport_fta_active = active;
        for running in [false, true] {
            plane.running = running;
            for direction in 0..8 {
                plane.direction = direction;
                for progress in [0, 127, 128, 255] {
                    plane.progress = progress;
                    for alpha in [0.0, 0.5, 1.0] {
                        let pose = extrapolate_vehicle_pose(&plane, alpha);
                        for mirrored in [false, true] {
                            plane.newgrf_mirrored = mirrored;
                            assert_eq!(
                                vehicle_render_direction_at_with_map(&plane, pose, None),
                                direction
                            );
                            let expected = if mirrored {
                                (direction + 4) & 7
                            } else {
                                direction
                            };
                            assert_eq!(vehicle_sprite_direction_at(&plane, pose), expected);
                        }
                    }
                }
            }
        }
    }
}
