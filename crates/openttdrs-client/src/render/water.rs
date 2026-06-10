//! Animación visual del agua renderizada.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::viewport::ortho_world_cull_margin;
use crate::render::{MapPreviewCamera, PrimaryGameCamera, WaterTile};
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

/// Pasos del ciclo «dark water» de OpenTTD (5 entradas de paleta).
const DARK_CYCLE: [f32; 5] = [0.90, 0.94, 0.98, 1.02, 1.05];

/// Pasos del ciclo «glitter» (15 entradas; en el original se avanza de 3 en 3).
const GLITTER_CYCLE: [f32; 15] = [
    0.00, 0.03, 0.07, 0.02, 0.05, 0.10, 0.04, 0.00, 0.06, 0.12, 0.05, 0.02, 0.09, 0.04, 0.00,
];

/// Velocidad del ciclo oscuro (pasos por segundo).
const DARK_STEPS_PER_SEC: f32 = 2.0;

/// Cada cuántos pasos oscuros avanza un «bloque» del glitter (OpenTTD: 3 en 3).
const GLITTER_DARK_RATIO: f32 = 3.0;

#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Interpola en un ciclo cerrado (evita saltos bruscos entre el último y el primer paso).
fn sample_cyclic_lerp(table: &[f32], continuous_phase: f32) -> f32 {
    let n = table.len();
    if n == 0 {
        return 1.0;
    }
    let phase = continuous_phase.rem_euclid(n as f32);
    let idx = phase.floor() as usize % n;
    let next = (idx + 1) % n;
    let t = smoothstep(phase.fract());
    table[idx] + (table[next] - table[idx]) * t
}

/// Color del sprite de agua para un instante y fases por tesela (puro, testeable).
#[must_use]
pub(crate) fn water_sprite_color(dark_phase: u8, glitter_phase: u8, elapsed_secs: f32) -> Color {
    let dark_cont = elapsed_secs * DARK_STEPS_PER_SEC + f32::from(dark_phase);
    let dark = sample_cyclic_lerp(&DARK_CYCLE, dark_cont);

    let glitter_cont =
        dark_cont / GLITTER_DARK_RATIO + f32::from(glitter_phase % GLITTER_CYCLE.len() as u8);
    let glitter = sample_cyclic_lerp(&GLITTER_CYCLE, glitter_cont);

    let v = (dark + glitter * 0.58).clamp(0.86, 1.12);
    let sparkle = (glitter * 1.4).min(0.14);
    Color::srgb(
        v * (0.92 - sparkle * 0.25),
        v * (0.97 + sparkle * 0.15),
        v * (1.04 + sparkle * 0.45),
    )
}

/// Anima agua con ciclos discretos para aproximar la paleta animada de OpenTTD.
///
/// En OpenTTD clásico el agua se mueve ciclando índices de paleta:
/// - dark water: ciclo de 5 entradas
/// - glitter water: ciclo de 15 colores, muestreado de 3 en 3
///
/// Este cliente usa sprites RGBA (no indexados), por eso emulamos ese efecto
/// modulando brillo/tinte con interpolación suave entre pasos.
pub(crate) fn animate_water(
    time: Res<Time>,
    cam_q: Query<
        (&GlobalTransform, &Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    mut query: Query<(&WaterTile, &GlobalTransform, &mut Sprite)>,
) {
    let elapsed = time.elapsed_secs();

    let (cull, cull_margin) = cam_q.iter().next().map_or((None, 0.0), |(cam_gt, proj)| {
        let Projection::Orthographic(ortho) = proj else {
            return (None, 0.0);
        };
        (
            Some((cam_gt.affine().inverse(), ortho.area)),
            ortho_world_cull_margin(ortho.scale),
        )
    });
    for (water, wg, mut sprite) in &mut query {
        if let Some((world_to_view, area)) = cull.as_ref() {
            let margin = cull_margin;
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
        sprite.color = water_sprite_color(water.dark_phase, water.glitter_phase, elapsed);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn water_color_varies_over_time() {
        let c0 = water_sprite_color(0, 0, 0.0);
        let c1 = water_sprite_color(0, 0, 0.65);
        assert_ne!(c0, c1);
    }

    #[test]
    fn water_phases_desync_colors_at_same_time() {
        let a = water_sprite_color(0, 0, 1.2);
        let b = water_sprite_color(2, 7, 1.2);
        assert_ne!(a, b);
    }

    #[test]
    fn cyclic_lerp_wraps_without_discontinuity() {
        let v_end = sample_cyclic_lerp(&DARK_CYCLE, 4.99);
        let v_start = sample_cyclic_lerp(&DARK_CYCLE, 0.01);
        assert!((v_end - v_start).abs() < 0.15);
    }

    #[test]
    fn animate_water_without_camera_still_updates_color() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(400));
        world.insert_resource(time);
        world.spawn((
            WaterTile {
                dark_phase: 1,
                glitter_phase: 2,
            },
            GlobalTransform::IDENTITY,
            Sprite::default(),
        ));

        world.run_system_once(animate_water).unwrap();

        let mut q = world.query::<&Sprite>();
        let color = q.single(&world).unwrap().color;
        assert_ne!(color, Color::WHITE);
    }

    #[test]
    fn animate_water_culls_far_entities_with_camera() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(600));
        world.insert_resource(time);

        world.spawn((
            PrimaryGameCamera,
            GlobalTransform::IDENTITY,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world.spawn((
            WaterTile {
                dark_phase: 0,
                glitter_phase: 0,
            },
            GlobalTransform::from_translation(Vec3::new(100_000.0, 100_000.0, 0.0)),
            Sprite::default(),
        ));

        world.run_system_once(animate_water).unwrap();
        let mut q = world.query::<&Sprite>();
        let color = q.single(&world).unwrap().color;
        assert_eq!(color, Color::WHITE);
    }
}
