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
