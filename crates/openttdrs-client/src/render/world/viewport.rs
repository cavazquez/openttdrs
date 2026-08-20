//! Viewport y cámara para world render.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::iso::{ISO_HW, ISO_QH, TILE_HALF_H};
use crate::render::viewport::initial_camera_span_tiles;
use crate::render::viewport::{
    TileViewportBounds, VIEWPORT_MARGIN_TILES, VIEWPORT_REBUILD_LEAD_TILES,
    large_map_viewport_cull_enabled, ortho_visible_tile_bounds,
};

pub(crate) use crate::render::viewport::overview_stride_for_scale;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;

use super::plugin::{MapTileSpawnViewport, RemapMapVisualsPending};

pub(crate) fn resolve_spawn_viewport(
    sim: &SimWorld,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_q: &Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
) -> TileViewportBounds {
    if let Ok((cam_tf, proj)) = cam_q.single() {
        let ortho_scale = if let Projection::Orthographic(o) = proj {
            o.scale
        } else {
            1.0
        };
        return resolve_spawn_viewport_at(sim, windows, cam_tf.translation.truncate(), ortho_scale);
    }
    let (cam_pos, ortho_scale) = initial_map_camera_pose(sim);
    resolve_spawn_viewport_at(sim, windows, cam_pos.truncate(), ortho_scale)
}

pub(crate) fn resolve_spawn_viewport_at(
    sim: &SimWorld,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_translation: Vec2,
    ortho_scale: f32,
) -> TileViewportBounds {
    let (mw, mh) = sim.state.map.dimensions();
    if !large_map_viewport_cull_enabled(mw, mh) {
        return TileViewportBounds::full(mw, mh);
    }
    let (win_w, win_h) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 720.0));
    let visible = ortho_visible_tile_bounds(
        cam_translation,
        ortho_scale,
        win_w,
        win_h,
        mw,
        mh,
        VIEWPORT_MARGIN_TILES,
    );
    // El zoom (`clamp_ortho_scale`) ya garantiza que este rectángulo ≤ MAX_SPAWN_SPAN.
    // No recortar aquí: un AABB isométrico cortado a cuadrado deja franjas vacías en pantalla.
    visible.expand(VIEWPORT_REBUILD_LEAD_TILES, mw, mh)
}

/// En mapas grandes, regenera sprites si la cámara sale del bloque ya instanciado.
pub(crate) fn sync_map_tile_spawn_viewport(
    sim: Res<SimWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut viewport: ResMut<MapTileSpawnViewport>,
) {
    let (mw, mh) = sim.state.map.dimensions();
    if !large_map_viewport_cull_enabled(mw, mh) {
        viewport.bounds = TileViewportBounds::full(mw, mh);
        return;
    }
    let needed = resolve_spawn_viewport(&sim, &windows, &cam_q);
    let ortho_scale = cam_q
        .single()
        .ok()
        .and_then(|(_, proj)| {
            if let Projection::Orthographic(o) = proj {
                Some(o.scale)
            } else {
                None
            }
        })
        .unwrap_or(viewport.last_ortho_scale);
    let scale_changed = (ortho_scale - viewport.last_ortho_scale).abs() > f32::EPSILON;
    if scale_changed || !viewport.bounds.contains(needed) {
        viewport.bounds = needed;
        viewport.last_ortho_scale = ortho_scale;
        pending.pending = true;
        pending.sync_camera = false;
        pending.full = false;
        // El borde del viewport puede cambiar dentro del mismo chunk de 16×16;
        // en ese caso el plan incremental no agrega/quita chunks, pero sí hay
        // que volver a filtrar las etiquetas por el nuevo viewport.
        pending.labels_dirty = true;
    }
}

/// Posición y escala ortho iniciales para un mapa (menú intro o partida).
#[must_use]
pub(crate) fn initial_map_camera_pose(sim: &SimWorld) -> (Vec3, f32) {
    let (mw, mh) = sim.state.map.dimensions();
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;
    let target_tiles_wide = initial_camera_span_tiles(mw, mh, sim.loaded_file);
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);
    (Vec3::new(cam_x, cam_y, 999.9), cam_scale)
}

pub(crate) fn sync_camera_for_sim(
    q_cam: &mut Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    sim: &SimWorld,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;
    let target_tiles_wide = initial_camera_span_tiles(mw, mh, sim.loaded_file);
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);
    let Ok((mut tf, mut proj)) = q_cam.single_mut() else {
        return;
    };
    tf.translation = Vec3::new(cam_x, cam_y, 999.9);
    let Projection::Orthographic(ref mut o) = *proj else {
        return;
    };
    o.scale = cam_scale;
}
