//! Optional vehicle-following camera for reproducible live route checks.
//! `OPENTTDRS_FOLLOW_VEHICLE=<id>` selects an existing unit without editing its
//! orders, map or simulation. The normal client installs no extra system.

use super::*;
use crate::render::{
    VehicleSprite, vehicle_pose_for_construction, vehicle_sprite_pos_at_with_catalog,
};
use crate::simulation::SimClock;
use std::io::{BufWriter, Write};

#[derive(Resource)]
struct FollowVehicle(u32);

#[derive(Resource)]
struct FollowTrace {
    output: BufWriter<std::fs::File>,
    frame: u32,
}

pub(super) fn install(app: &mut App) {
    let Ok(raw) = std::env::var("OPENTTDRS_FOLLOW_VEHICLE") else {
        return;
    };
    let Ok(id) = raw.parse() else {
        warn!("OPENTTDRS_FOLLOW_VEHICLE debe ser un id de vehículo");
        return;
    };
    app.insert_resource(FollowVehicle(id)).add_systems(
        Update,
        follow_vehicle
            .after(move_camera)
            .in_set(UpdateSet::Camera)
            .run_if(in_state(ClientScreen::InGame)),
    );
    app.add_systems(
        Update,
        follow_rendered_vehicle
            .after(crate::bevy_app::UpdateSet::RenderRefresh)
            .after(follow_vehicle)
            .run_if(in_state(ClientScreen::InGame)),
    );
    if let Some(path) = std::env::var_os("OPENTTDRS_FOLLOW_TRACE_OUT") {
        let result = std::fs::File::create(path).and_then(|file| {
            let mut output = BufWriter::new(file);
            writeln!(output, "frame,tick,x,y,direction,rail_pixel,order,loading,camera_x,camera_y,sprite_x,sprite_y,scale,kind,cargo,cargo_loading,cargo_unloading,running,road_x,road_y,road_frame,road_state,last_station_x,last_station_y,cargo_income,delivered_units,vehicle_id,in_depot,service_generation")?;
            Ok(output)
        });
        match result {
            Ok(output) => {
                app.insert_resource(FollowTrace { output, frame: 0 })
                    .add_systems(Last, trace_follow.run_if(in_state(ClientScreen::InGame)));
            }
            Err(error) => warn!("No se puede escribir la traza de seguimiento: {error}"),
        }
    }
}

// The rendered anchor can differ from the catalogue fallback for NewGRF
// layers. Refine after rendering has resolved that layer's real offsets.
fn follow_rendered_vehicle(
    selected: Res<FollowVehicle>,
    sprites: Query<(&VehicleSprite, &Transform, &Visibility), Without<PrimaryGameCamera>>,
    mut cameras: Query<
        &mut Transform,
        (
            With<PrimaryGameCamera>,
            Without<VehicleSprite>,
            Without<MapPreviewCamera>,
        ),
    >,
) {
    let Some((_, sprite, _)) = sprites
        .iter()
        .find(|(id, _, visibility)| id.0 == selected.0 && **visibility != Visibility::Hidden)
    else {
        return;
    };
    if let Ok(mut camera) = cameras.single_mut() {
        camera.translation.x = sprite.translation.x;
        camera.translation.y = sprite.translation.y;
    }
}

fn trace_follow(
    selected: Res<FollowVehicle>,
    sim: Res<SimWorld>,
    cameras: Query<(&Transform, &Projection), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    sprites: Query<(&VehicleSprite, &Transform, &Visibility)>,
    mut trace: ResMut<FollowTrace>,
) {
    let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == selected.0) else {
        return;
    };
    let Ok((camera, Projection::Orthographic(projection))) = cameras.single() else {
        return;
    };
    let sprite = sprites
        .iter()
        .find(|(id, _, visibility)| id.0 == selected.0 && **visibility != Visibility::Hidden)
        .map(|(_, transform, _)| transform.translation);
    let sprite_x = sprite.map_or(f32::NAN, |p| p.x);
    let sprite_y = sprite.map_or(f32::NAN, |p| p.y);
    let frame = trace.frame;
    let station = vehicle
        .last_station_visited
        .unwrap_or(openttdrs_core::TileCoord::new(-1, -1));
    let company = &sim.state.companies[vehicle.owner.index()];
    let result = writeln!(
        trace.output,
        "{frame},{},{},{},{},{},{},{},{},{},{sprite_x},{sprite_y},{},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        sim.state.tick.get(),
        vehicle.pos.x,
        vehicle.pos.y,
        vehicle.direction,
        vehicle.rail_pixel,
        vehicle.current_order,
        vehicle.awaiting_load_window,
        camera.translation.x,
        camera.translation.y,
        projection.scale,
        vehicle.kind,
        vehicle.cargo,
        vehicle.cargo_loading,
        vehicle.cargo_unloading,
        vehicle.running,
        vehicle.road_x,
        vehicle.road_y,
        vehicle.frame,
        vehicle.road_state,
        station.x,
        station.y,
        company.cargo_income_earned,
        sim.state.stats.cargo_units_final_delivered,
        vehicle.id,
        openttdrs_core::refit::vehicle_is_in_depot(&sim.state.map, vehicle),
        vehicle.depot_service_generation()
    );
    trace.frame = trace.frame.wrapping_add(1);
    if let Err(error) = result.and_then(|()| trace.output.flush()) {
        warn!("Traza de seguimiento: {error}");
    }
}

fn follow_vehicle(
    selected: Res<FollowVehicle>,
    sim: Res<SimWorld>,
    clock: Res<SimClock>,
    mut velocity: ResMut<CameraVelocity>,
    mut focus: ResMut<CameraFocusRequest>,
    mut cameras: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == selected.0) else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let pose = vehicle_pose_for_construction(vehicle, clock.tick_alpha, sim.state.construction);
    let position = vehicle_sprite_pos_at_with_catalog(
        vehicle,
        &sim.state.map,
        pose,
        Some(&sim.state.engine_catalog),
    );
    camera.translation.x = position.x;
    camera.translation.y = position.y;
    velocity.0 = Vec2::ZERO;
    focus.target = None;
    focus.smooth_target = None;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn follows_visible_rendered_anchor_and_retains_fallback_in_tunnel() {
        let mut app = App::new();
        app.insert_resource(FollowVehicle(2902))
            .add_systems(Update, follow_rendered_vehicle);
        let camera = app
            .world_mut()
            .spawn((PrimaryGameCamera, Transform::from_xyz(1.0, 2.0, 999.0)))
            .id();
        app.world_mut().spawn((
            VehicleSprite(404),
            Transform::from_xyz(999.0, 999.0, 0.0),
            Visibility::Visible,
        ));
        let head = app
            .world_mut()
            .spawn((
                VehicleSprite(2902),
                Transform::from_xyz(25.0, 50.0, 1.0),
                Visibility::Hidden,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(1.0, 2.0, 999.0)
        );
        app.world_mut().entity_mut(head).insert(Visibility::Visible);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(25.0, 50.0, 999.0)
        );
        app.world_mut()
            .entity_mut(head)
            .insert(Transform::from_xyz(-80.0, 500.0, 2.0));
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(-80.0, 500.0, 999.0)
        );
    }

    #[test]
    fn follows_moving_and_stopped_vehicle_at_every_zoom_without_changing_zoom_or_depth() {
        let mut app = App::new();
        let mut state = GameState::new(32, 32);
        let mut train = Vehicle::new(
            2902,
            VehicleKind::Train,
            TileCoord::new(5, 5),
            TileCoord::new(15, 5),
        );
        train.running = true;
        state.vehicles.push(train);
        app.insert_resource(SimWorld {
            state,
            loaded_file: true,
            ottdmap_extras: None,
        })
        .insert_resource(FollowVehicle(2902))
        .insert_resource(SimClock { tick_alpha: 0.5 })
        .init_resource::<CameraVelocity>()
        .init_resource::<CameraFocusRequest>()
        .add_systems(Update, follow_vehicle);
        let camera = app
            .world_mut()
            .spawn((
                PrimaryGameCamera,
                Transform::from_xyz(1.0, 2.0, 999.0),
                Projection::Orthographic(OrthographicProjection::default_2d()),
            ))
            .id();
        for scale in OPENTTD_FIXED_ORTHO_SCALES {
            if let Projection::Orthographic(projection) =
                &mut *app.world_mut().get_mut::<Projection>(camera).unwrap()
            {
                projection.scale = scale;
            }
            for (x, running) in [(6, true), (12, true), (15, false)] {
                app.world_mut().resource_mut::<SimWorld>().state.vehicles[0].pos =
                    TileCoord::new(x, 5);
                app.world_mut().resource_mut::<SimWorld>().state.vehicles[0].running = running;
                app.world_mut().resource_mut::<CameraVelocity>().0 = Vec2::splat(100.0);
                app.update();
                let sim = app.world().resource::<SimWorld>();
                let vehicle = &sim.state.vehicles[0];
                let pose = vehicle_pose_for_construction(vehicle, 0.5, sim.state.construction);
                let expected = vehicle_sprite_pos_at_with_catalog(
                    vehicle,
                    &sim.state.map,
                    pose,
                    Some(&sim.state.engine_catalog),
                );
                let transform = app.world().get::<Transform>(camera).unwrap();
                assert_eq!(transform.translation.truncate(), expected.truncate());
                assert_eq!(transform.translation.z, 999.0);
                assert_eq!(app.world().resource::<CameraVelocity>().0, Vec2::ZERO);
                let Projection::Orthographic(projection) =
                    app.world().get::<Projection>(camera).unwrap()
                else {
                    panic!("orthographic camera")
                };
                assert_eq!(projection.scale, scale);
            }
        }
    }

    #[test]
    fn absent_vehicle_keeps_camera_position() {
        let mut app = App::new();
        app.insert_resource(FollowVehicle(404))
            .insert_resource(SimWorld {
                state: GameState::new(8, 8),
                loaded_file: true,
                ottdmap_extras: None,
            })
            .init_resource::<SimClock>()
            .init_resource::<CameraVelocity>()
            .init_resource::<CameraFocusRequest>()
            .add_systems(Update, follow_vehicle);
        let initial = Transform::from_xyz(10.0, 20.0, 30.0);
        let camera = app.world_mut().spawn((PrimaryGameCamera, initial)).id();
        app.update();
        assert_eq!(*app.world().get::<Transform>(camera).unwrap(), initial);
    }
}
