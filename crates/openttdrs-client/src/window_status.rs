//! Sincronización del título de ventana con estado útil de debug.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy_app::UpdateSet;
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct WindowStatusPlugin;

impl Plugin for WindowStatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_window_title
                .in_set(UpdateSet::Status)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

#[derive(Default)]
pub(crate) struct WindowTitleSync {
    last_scale: f32,
    fps_dt: f32,
    fps_frames: u32,
    last_fps: f32,
}

pub(crate) fn sync_window_title(
    sim: Res<SimWorld>,
    time: Res<Time>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    cam_q: Query<&Projection, (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
    mut state: Local<WindowTitleSync>,
) {
    let scale = cam_q
        .single()
        .ok()
        .and_then(|p| match p {
            Projection::Orthographic(o) => Some(o.scale),
            _ => None,
        })
        .unwrap_or(1.0);

    state.fps_dt += time.delta_secs();
    state.fps_frames += 1;

    let scale_changed = (scale - state.last_scale).abs() > 0.000_5;
    if scale_changed {
        state.last_scale = scale;
    }

    let fps_tick = state.fps_dt >= 1.0;
    if fps_tick {
        state.last_fps = state.fps_frames as f32 / state.fps_dt;
        state.fps_dt = 0.0;
        state.fps_frames = 0;
    }

    if !scale_changed && !fps_tick {
        return;
    }

    let fps = if state.last_fps > 0.0 {
        state.last_fps
    } else {
        60.0
    };

    if let Ok(mut window) = windows.single_mut() {
        let indp_n = sim
            .ottdmap_extras
            .as_ref()
            .map(|e| e.industry_types.len())
            .unwrap_or(0);
        let indp_tag = if indp_n > 0 {
            format!(" - INDP {indp_n}")
        } else {
            String::new()
        };
        window.title = format!(
            "openttdrs - tick {} - cargas {}/{}{indp_tag} - zoom {:.2}x - {:.0} FPS",
            sim.state.tick.get(),
            sim.state.stats.cargo_pickups,
            sim.state.stats.cargo_deliveries,
            scale,
            fps
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::PrimaryWindow;
    use openttdrs_core::OttdmapExtras;

    #[test]
    fn sync_window_title_no_primary_window_is_noop() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(Time::<()>::default());
        world.spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world.run_system_once(sync_window_title).unwrap();
    }

    #[test]
    fn sync_window_title_updates_title_with_camera() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(Time::<()>::default());
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.run_system_once(sync_window_title).unwrap();

        let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let title = q.single(&world).unwrap().title.clone();
        assert!(title.contains("openttdrs - tick"));
    }

    #[test]
    fn sync_window_title_with_extras_and_local_early_return() {
        let sim = SimWorld {
            ottdmap_extras: Some(OttdmapExtras {
                industry_types: vec![(1, 2), (2, 3)],
                ..Default::default()
            }),
            ..SimWorld::default()
        };

        let mut world = World::new();
        world.insert_resource(sim);
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.1));
        world.insert_resource(time);
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.run_system_once(sync_window_title).unwrap();
        let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let title1 = q.single(&world).unwrap().title.clone();
        assert!(title1.contains("INDP 2"));

        // Segunda pasada sin tick de FPS ni cambio de escala: early return.
        world.insert_resource(Time::<()>::default());
        world.run_system_once(sync_window_title).unwrap();
        let title2 = q.single(&world).unwrap().title.clone();
        assert!(title2.contains("INDP 2"));
    }

    #[test]
    fn sync_window_title_non_orthographic_camera_uses_default_scale() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.2));
        world.insert_resource(time);
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((
            PrimaryGameCamera,
            Projection::Perspective(PerspectiveProjection::default()),
        ));

        world.run_system_once(sync_window_title).unwrap();
        let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let title = q.single(&world).unwrap().title.clone();
        assert!(title.contains("zoom 1.00x"));
    }
}
