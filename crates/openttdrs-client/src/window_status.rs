//! Sincronización del título de ventana con estado útil de debug.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy_app::UpdateSet;
use crate::camera::zoom_display_magnification;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct WindowStatusPlugin;

impl Plugin for WindowStatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
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
    last_tick: u64,
    last_pickups: u64,
    last_deliveries: u64,
    last_indp_n: usize,
    last_fps: u32,
}

pub(crate) fn sync_window_title(
    sim: Res<SimWorld>,
    diagnostics: Res<DiagnosticsStore>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    cam_q: Query<&Projection, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
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

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(bevy::diagnostic::Diagnostic::smoothed)
        .map(|f| f.round() as u32)
        .unwrap_or(0);

    let tick = sim.state.tick.get();
    let pickups = sim.state.stats.cargo_pickups;
    let deliveries = sim.state.stats.cargo_deliveries;
    let indp_n = sim
        .ottdmap_extras
        .as_ref()
        .map(|e| e.industry_types.len())
        .unwrap_or(0);

    let scale_changed = (scale - state.last_scale).abs() > 0.000_5;
    let stats_changed = tick != state.last_tick
        || pickups != state.last_pickups
        || deliveries != state.last_deliveries
        || indp_n != state.last_indp_n;
    let fps_changed = fps != state.last_fps;

    if !scale_changed && !stats_changed && !fps_changed {
        return;
    }

    state.last_scale = scale;
    state.last_tick = tick;
    state.last_pickups = pickups;
    state.last_deliveries = deliveries;
    state.last_indp_n = indp_n;
    state.last_fps = fps;

    if let Ok(mut window) = windows.single_mut() {
        let indp_tag = if indp_n > 0 {
            format!(" - INDP {indp_n}")
        } else {
            String::new()
        };
        window.title = format!(
            "openttdrs - tick {tick} - cargas {pickups}/{deliveries}{indp_tag} - zoom {:.2}x - {fps} FPS",
            zoom_display_magnification(scale),
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::window::PrimaryWindow;
    use openttdrs_core::OttdmapExtras;

    fn window_title_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        app.insert_resource(SimWorld::default());
        app.add_systems(Update, sync_window_title);
        app
    }

    #[test]
    fn sync_window_title_no_primary_window_is_noop() {
        let mut app = window_title_test_app();
        app.world_mut().spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        app.update();
    }

    #[test]
    fn sync_window_title_updates_title_with_camera() {
        let mut app = window_title_test_app();
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut().spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        app.update();

        let world = app.world_mut();
        let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let title = q.single(world).unwrap().title.clone();
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

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        app.insert_resource(sim);
        app.add_systems(Update, sync_window_title);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut().spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        app.update();
        let title1 = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
            q.single(world).unwrap().title.clone()
        };
        assert!(title1.contains("INDP 2"));

        // Segunda pasada sin cambios: early return.
        app.update();
        let title2 = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
            q.single(world).unwrap().title.clone()
        };
        assert_eq!(title1, title2);
    }

    #[test]
    fn sync_window_title_non_orthographic_camera_uses_default_scale() {
        let mut app = window_title_test_app();
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut().spawn((
            PrimaryGameCamera,
            Projection::Perspective(PerspectiveProjection::default()),
        ));

        app.update();
        let world = app.world_mut();
        let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let title = q.single(world).unwrap().title.clone();
        assert!(title.contains("zoom 1.00x"));
    }

    #[test]
    fn sync_window_title_without_camera_query_uses_default_scale() {
        let mut app = window_title_test_app();
        app.world_mut().spawn((Window::default(), PrimaryWindow));

        app.update();
        let world = app.world_mut();
        let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let title = q.single(world).unwrap().title.clone();
        assert!(title.contains("zoom 1.00x"));
    }
}
