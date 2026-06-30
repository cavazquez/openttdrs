//! Interpolación suave de sprites fantasma hacia su objetivo.

use bevy::prelude::*;

/// Objetivo de posición; el sistema [`lerp_ghost_previews`] acerca el [`Transform`].
#[derive(Component, Clone, Copy)]
pub(crate) struct GhostLerp {
    pub target: Vec3,
    pub speed: f32,
}

pub(crate) const GHOST_LERP_SPEED: f32 = 18.0;

pub(crate) fn lerp_ghost_previews(time: Res<Time>, mut q: Query<(&mut Transform, &GhostLerp)>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut tf, lerp) in &mut q {
        let step = (lerp.speed * dt).clamp(0.0, 1.0);
        tf.translation = tf.translation.lerp(lerp.target, step);
    }
}
