//! Sistema de cámara: movimiento WASD, arrastre con botón derecho y zoom.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Paneo con botón derecho: factor × `OrthographicProjection::scale` × delta en píxeles.
const PAN_RMB_SCALE: f32 = 1.05;
/// Zoom con teclado (+/-): fracción de `scale` por segundo al mantener pulsado.
const ZOOM_KEY_RATE: f32 = 3.5;
/// Zoom con rueda: multiplicador por unidad de `scroll.delta.y`.
const ZOOM_WHEEL_SENS: f32 = 0.16;

/// Mueve la cámara con WASD, arrastre con botón derecho y rueda del ratón.
pub fn move_camera(
    time: Res<Time>,
    kbd: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = cam_q.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref mut proj) = *projection else {
        return;
    };

    let speed = 300.0 * proj.scale * time.delta_secs();

    // Arrastre con botón derecho
    if mouse.pressed(MouseButton::Right) && motion.delta != Vec2::ZERO {
        let s = proj.scale * PAN_RMB_SCALE;
        transform.translation.x -= motion.delta.x * s;
        transform.translation.y += motion.delta.y * s;
    }

    // Movimiento con teclado
    if kbd.pressed(KeyCode::KeyW) || kbd.pressed(KeyCode::ArrowUp) {
        transform.translation.y += speed;
    }
    if kbd.pressed(KeyCode::KeyS) || kbd.pressed(KeyCode::ArrowDown) {
        transform.translation.y -= speed;
    }
    if kbd.pressed(KeyCode::KeyA) || kbd.pressed(KeyCode::ArrowLeft) {
        transform.translation.x -= speed;
    }
    if kbd.pressed(KeyCode::KeyD) || kbd.pressed(KeyCode::ArrowRight) {
        transform.translation.x += speed;
    }

    // Zoom con teclado
    let z = ZOOM_KEY_RATE * time.delta_secs();
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
