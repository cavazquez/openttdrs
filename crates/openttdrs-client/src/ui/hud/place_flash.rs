//! Destello breve al colocar construcción (p. ej. señal ferroviaria).

use bevy::prelude::*;

use crate::render::MapVisualLayer;

#[derive(Component)]
pub(crate) struct BuildPlaceFlashSprite {
    timer: Timer,
}

/// Encola un destello en coordenadas de mundo (pantalla).
pub(crate) fn enqueue_build_place_flash(feedback: &mut super::HudBuildFeedback, world: Vec3) {
    feedback.pending_place_flash = Some(world);
}

/// Crea el sprite de destello tras un clic de construcción exitoso.
pub(crate) fn spawn_build_place_flash(
    mut commands: Commands,
    mut feedback: ResMut<super::HudBuildFeedback>,
) {
    let Some(pos) = feedback.pending_place_flash.take() else {
        return;
    };
    commands.spawn((
        MapVisualLayer,
        BuildPlaceFlashSprite {
            timer: Timer::from_seconds(0.38, TimerMode::Once),
        },
        Sprite {
            color: Color::srgba(0.92, 1.0, 0.82, 0.82),
            custom_size: Some(Vec2::new(14.0, 14.0)),
            ..default()
        },
        Transform::from_translation(pos).with_scale(Vec3::splat(1.2)),
    ));
}

pub(crate) fn animate_build_place_flash(
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &mut Transform,
        &mut Sprite,
        &mut BuildPlaceFlashSprite,
    )>,
    mut commands: Commands,
) {
    for (entity, mut tf, mut sprite, mut flash) in &mut q {
        flash.timer.tick(time.delta());
        let t = flash.timer.fraction();
        let scale = 1.2 + t * 0.9;
        tf.scale = Vec3::splat(scale);
        sprite.color.set_alpha(0.82 * (1.0 - t));
        if flash.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
