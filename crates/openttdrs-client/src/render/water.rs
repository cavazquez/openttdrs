//! Animación visual del agua renderizada.

use bevy::math::{Affine3A, Rect};
use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::iso::ISO_HW;
use crate::render::WaterTile;
use crate::state::ClientScreen;

pub(crate) struct WaterAnimationPlugin;

impl Plugin for WaterAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_water
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Anima agua con ciclos discretos para aproximar la paleta animada de OpenTTD.
///
/// En OpenTTD clásico el agua se mueve ciclando índices de paleta:
/// - dark water: ciclo de 5 entradas
/// - glitter water: ciclo de 15 colores, muestreado de 3 en 3
///
/// Este cliente usa sprites RGBA (no indexados), por eso emulamos ese efecto
/// modulando brillo/tinte en pasos discretos sincronizados.
pub(crate) fn animate_water(
    time: Res<Time>,
    cam_q: Query<(&GlobalTransform, &Projection), With<Camera2d>>,
    mut query: Query<(&WaterTile, &GlobalTransform, &mut Sprite)>,
) {
    const DARK_CYCLE: [f32; 5] = [0.92, 0.95, 0.98, 1.01, 1.04];
    const GLITTER_CYCLE: [f32; 15] = [
        0.00, 0.02, 0.05, 0.01, 0.03, 0.07, 0.02, 0.00, 0.04, 0.08, 0.03, 0.01, 0.06, 0.02, 0.00,
    ];
    let dark_tick = ((time.elapsed_secs() * 3.0) as usize) % DARK_CYCLE.len();
    let glitter_tick = (((time.elapsed_secs() * 3.0) as usize) * 3) % GLITTER_CYCLE.len();

    let cull: Option<(Affine3A, Rect)> = cam_q.iter().next().and_then(|(cam_gt, proj)| {
        let Projection::Orthographic(ortho) = proj else {
            return None;
        };
        Some((cam_gt.affine().inverse(), ortho.area))
    });
    let margin = ISO_HW * 4.0;

    for (water, wg, mut sprite) in &mut query {
        if let Some((world_to_view, area)) = cull.as_ref() {
            let wpos = wg.translation();
            let local = world_to_view.transform_point3(wpos);
            if local.x < area.min.x - margin
                || local.x > area.max.x + margin
                || local.y < area.min.y - margin
                || local.y > area.max.y + margin
            {
                continue;
            }
        }
        let dark_idx = (dark_tick + water.dark_phase as usize) % DARK_CYCLE.len();
        let glitter_idx = (glitter_tick + water.glitter_phase as usize) % GLITTER_CYCLE.len();
        let dark = DARK_CYCLE[dark_idx];
        let glitter = GLITTER_CYCLE[glitter_idx];

        let v = (dark + glitter * 0.40).clamp(0.88, 1.08);
        sprite.color = Color::srgb(v * 0.95, v * 0.99, v * 1.03);
    }
}
