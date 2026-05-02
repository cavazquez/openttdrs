//! Sistema de cámara: movimiento WASD con inercia, arrastre con botón derecho y zoom.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy_app::UpdateSet;
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::ClientScreen;

/// Paneo con botón derecho: factor × `OrthographicProjection::scale` × delta en píxeles.
const PAN_RMB_SCALE: f32 = 1.35;
/// Zoom con teclado (+/-): fracción de `scale` por segundo al mantener pulsado.
const ZOOM_KEY_RATE: f32 = 3.5;
/// Zoom con rueda: multiplicador por unidad de `scroll.delta.y`.
const ZOOM_WHEEL_SENS: f32 = 0.23;
/// Velocidad de aceleración WASD (unidades/s²).
const WASD_ACCEL: f32 = 2000.0;
/// Factor de desaceleración por fricción (fracción de velocidad que se pierde por segundo).
/// 1.0 = para instantáneamente; valores menores (~10-15) dan inercia suave.
const WASD_FRICTION: f32 = 12.0;
/// Velocidad máxima WASD (unidades de mundo/s, relativa a scale=1).
const WASD_MAX_SPEED: f32 = 600.0;

/// Velocidad de la cámara (inercia WASD).
#[derive(Resource, Default)]
pub struct CameraVelocity(pub Vec2);

pub(crate) struct CameraControlPlugin;

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraVelocity>().add_systems(
            Update,
            move_camera
                .in_set(UpdateSet::Camera)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Mueve la cámara con WASD (con inercia), arrastre con botón derecho y rueda del ratón.
#[allow(clippy::too_many_arguments)] // firma dictada por el sistema ECS de Bevy
pub fn move_camera(
    time: Res<Time>,
    kbd: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    mut vel: ResMut<CameraVelocity>,
) {
    let Ok((mut transform, mut projection)) = cam_q.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref mut proj) = *projection else {
        return;
    };

    let dt = time.delta_secs();

    // Arrastre con botón derecho (inmediato, sin inercia)
    if mouse.pressed(MouseButton::Right) && motion.delta != Vec2::ZERO {
        let s = proj.scale * PAN_RMB_SCALE;
        transform.translation.x -= motion.delta.x * s;
        transform.translation.y += motion.delta.y * s;
        vel.0 = Vec2::ZERO;
    }

    // WASD: acumular dirección deseada
    let mut dir = Vec2::ZERO;
    if kbd.pressed(KeyCode::KeyW) || kbd.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    let save_combo = kbd.pressed(KeyCode::ControlLeft) || kbd.pressed(KeyCode::ControlRight);
    if (kbd.pressed(KeyCode::KeyS) && !save_combo) || kbd.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if kbd.pressed(KeyCode::KeyA) || kbd.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if kbd.pressed(KeyCode::KeyD) || kbd.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }

    let max_speed = WASD_MAX_SPEED * proj.scale;
    if dir != Vec2::ZERO {
        vel.0 += dir.normalize() * WASD_ACCEL * proj.scale * dt;
        if vel.0.length() > max_speed {
            vel.0 = vel.0.normalize() * max_speed;
        }
    }

    // Fricción: desacelera aunque no haya tecla presionada
    let friction = (1.0 - WASD_FRICTION * dt).max(0.0);
    vel.0 *= friction;
    if vel.0.length() < 0.5 {
        vel.0 = Vec2::ZERO;
    }

    transform.translation.x += vel.0.x * dt;
    transform.translation.y += vel.0.y * dt;

    // Zoom con teclado
    let z = ZOOM_KEY_RATE * dt;
    if kbd.pressed(KeyCode::Equal) || kbd.pressed(KeyCode::NumpadAdd) {
        proj.scale = (proj.scale * (1.0 - z)).max(0.25);
    }
    if kbd.pressed(KeyCode::Minus) || kbd.pressed(KeyCode::NumpadSubtract) {
        proj.scale = (proj.scale * (1.0 + z)).min(20.0);
    }

    // Zoom con rueda del ratón hacia la posición del cursor
    if scroll.delta.y.abs() > 0.0 {
        let Ok(window) = windows.single() else {
            return;
        };
        let Some(cursor_pos) = window.cursor_position() else {
            return;
        };

        let window_size = Vec2::new(window.width(), window.height());
        let cursor_offset = cursor_pos - window_size / 2.0;
        let cursor_offset_world = Vec2::new(cursor_offset.x, -cursor_offset.y);

        let world_pos = Vec2::new(transform.translation.x, transform.translation.y)
            + cursor_offset_world * proj.scale;

        let old_scale = proj.scale;
        let new_scale = (old_scale * (1.0 - scroll.delta.y * ZOOM_WHEEL_SENS)).clamp(0.25, 20.0);
        proj.scale = new_scale;

        let new_cam_pos = world_pos - cursor_offset_world * new_scale;
        transform.translation.x = new_cam_pos.x;
        transform.translation.y = new_cam_pos.y;
    }
}

/// Valor para HUD / título: **aumento aparente** respecto a `orthographic_scale = 1`.
/// En Bevy, [`OrthographicProjection::scale`] alto cubre más mundo en pantalla (sensación de alejado);
/// su recíproco se comporta como un “×” de acercar en sentido coloquial (más grande = más cerca).
#[must_use]
pub(crate) fn zoom_display_magnification(orthographic_scale: f32) -> f32 {
    orthographic_scale.recip()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::{PrimaryWindow, WindowResolution};

    #[test]
    fn move_camera_without_camera_query_is_noop() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(AccumulatedMouseMotion::default());
        world.insert_resource(AccumulatedMouseScroll::default());
        world.insert_resource(CameraVelocity::default());
        world.run_system_once(move_camera).unwrap();
    }

    #[test]
    fn move_camera_handles_keyboard_drag_and_scroll() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        world.insert_resource(time);

        let mut kbd = ButtonInput::<KeyCode>::default();
        kbd.press(KeyCode::KeyW);
        kbd.press(KeyCode::Equal);
        world.insert_resource(kbd);

        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Right);
        world.insert_resource(mouse);

        world.insert_resource(AccumulatedMouseMotion {
            delta: Vec2::new(8.0, -4.0),
        });
        world.insert_resource(AccumulatedMouseScroll {
            delta: Vec2::new(0.0, 1.0),
            ..default()
        });
        world.insert_resource(CameraVelocity::default());

        world.spawn((
            Window {
                resolution: WindowResolution::new(1280, 720),
                ..default()
            },
            PrimaryWindow,
        ));
        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.run_system_once(move_camera).unwrap();
    }

    #[test]
    fn move_camera_handles_non_ortho_and_ctrl_s_combo() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(30));
        world.insert_resource(time);

        let mut kbd = ButtonInput::<KeyCode>::default();
        kbd.press(KeyCode::ControlLeft);
        kbd.press(KeyCode::KeyS);
        kbd.press(KeyCode::Minus);
        world.insert_resource(kbd);
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(AccumulatedMouseMotion::default());
        world.insert_resource(AccumulatedMouseScroll::default());
        world.insert_resource(CameraVelocity::default());

        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        world.run_system_once(move_camera).unwrap();
    }

    #[test]
    fn zoom_display_magnification_is_reciprocal_of_ortho_scale() {
        assert!((zoom_display_magnification(1.0) - 1.0).abs() < 0.001);
        assert!((zoom_display_magnification(0.25) - 4.0).abs() < 0.001);
        assert!((zoom_display_magnification(20.0) - 0.05).abs() < 0.001);
    }

    #[test]
    fn move_camera_scroll_without_window_returns_after_zoom_branch() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        world.insert_resource(time);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(AccumulatedMouseMotion::default());
        world.insert_resource(AccumulatedMouseScroll {
            delta: Vec2::new(0.0, 1.0),
            ..default()
        });
        world.insert_resource(CameraVelocity::default());

        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.run_system_once(move_camera).unwrap();
    }
}
