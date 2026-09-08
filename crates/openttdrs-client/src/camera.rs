//! Sistema de cámara: movimiento WASD con inercia, arrastre con botón derecho y zoom.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::iso::{HEIGHT_PX, ISO_QH, iso};
use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, clamp_ortho_scale, large_map_viewport_cull_enabled,
};
use crate::state::{ClientScreen, SimWorld};

/// Paneo con botón derecho: factor × `OrthographicProjection::scale` × delta en píxeles.
const PAN_RMB_SCALE: f32 = 1.35;
/// Zoom con teclado (+/-): fracción de `scale` por segundo al mantener pulsado.
const ZOOM_KEY_RATE: f32 = 3.5;
/// Zoom con rueda: multiplicador por unidad de `scroll.delta.y`.
const ZOOM_WHEEL_SENS: f32 = 0.23;
/// Niveles discretos de `ZoomLevel` en OpenTTD, expresados como
/// [`OrthographicProjection::scale`].
///
/// OpenTTD define `In4x`, `In2x`, `Normal`, `Out2x`, `Out4x` y `Out8x`.
/// En nuestra cámara ortográfica la escala crece al alejarse, de modo que se
/// corresponden con 0.25, 0.5, 1, 2, 4 y 8 respectivamente.
const OPENTTD_FIXED_ORTHO_SCALES: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
/// Velocidad de aceleración WASD (unidades/s²).
const WASD_ACCEL: f32 = 2000.0;
/// Factor de desaceleración por fricción (fracción de velocidad que se pierde por segundo).
/// 1.0 = para instantáneamente; valores menores (~10-15) dan inercia suave.
const WASD_FRICTION: f32 = 12.0;
/// Velocidad máxima WASD (unidades de mundo/s, relativa a scale=1).
const WASD_MAX_SPEED: f32 = 600.0;
/// Cota de un frame de presentación para no convertir una suspensión del SO en
/// un salto de cámara. Antes la cámara leía `Time<Virtual>`, cuyo máximo se
/// ajusta a un tick de OpenTTD; conservar una cota equivalente sólo afecta
/// pausas/stalls anómalos, no la velocidad de simulación.
const MAX_PRESENTATION_DELTA_SECS: f32 = 0.03;

/// Velocidad de la cámara (inercia WASD).
#[derive(Resource, Default)]
pub struct CameraVelocity(pub Vec2);

/// Política de zoom de la cámara principal.
///
/// El modo fijo es el valor inicial y sigue los niveles discretos de OpenTTD.
/// En `Out4x`/`Out8x` los viewports que caben en el presupuesto conservan sus
/// sprites detallados; sólo los recortes realmente grandes usan un resumen de
/// protección. El modo libre conserva el comportamiento continuo previo.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ZoomMode {
    Free,
    #[default]
    Fixed,
}

impl ZoomMode {
    #[must_use]
    pub(crate) const fn toggled(self) -> Self {
        match self {
            Self::Free => Self::Fixed,
            Self::Fixed => Self::Free,
        }
    }

    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Free => "libre",
            Self::Fixed => "fijo OpenTTD",
        }
    }
}

/// Petición de salto de cámara (p. ej. clic en noticia); scroll suave ~300 ms.
#[derive(Resource, Default)]
pub struct CameraFocusRequest {
    pub target: Option<Vec2>,
    /// Destino activo del lerp (viewport.cpp `ClampSmoothScroll`).
    smooth_target: Option<Vec2>,
}

#[must_use]
pub fn tile_camera_world_pos(map: &Map, coord: TileCoord) -> Vec2 {
    let height = map.get(coord).map_or(0, |tile| tile.height);
    let pos = iso(coord.x, coord.y);
    // La cámara de OpenTTD se desplaza al centro geométrico del tile:
    // `TileX * 16 + 8`, `TileY * 16 + 8`. Su proyección equivale a media
    // altura lógica de 16 px. `TILE_HALF_H` (15,5 px) es, en cambio, el
    // ancla visual del sprite 8bpp de terreno; usarlo aquí corría la captura
    // limpia un píxel tras el redondeo. Mantener ambos conceptos separados
    // preserva el anclaje de sprites y alinea el viewport con OpenTTD.
    Vec2::new(pos.x, pos.y - ISO_QH + f32::from(height) * HEIGHT_PX)
}

pub(crate) struct CameraControlPlugin;

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraVelocity>()
            .init_resource::<ZoomMode>()
            .init_resource::<CameraFocusRequest>()
            .add_systems(
                Update,
                (
                    apply_camera_focus_request,
                    move_camera.after(apply_camera_focus_request),
                )
                    .chain()
                    .in_set(UpdateSet::Camera)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

/// Niveles de zoom fijo que siguen siendo seguros para el viewport actual.
fn available_fixed_zoom_levels(
    _window_width: f32,
    _window_height: f32,
    _large_map_cull: bool,
) -> &'static [f32] {
    // Out4x/Out8x siguen siendo seguros: los viewports dentro del presupuesto
    // conservan detalle y los que lo exceden usan el resumen de protección.
    // Por eso culling no los elimina de la secuencia fija.
    &OPENTTD_FIXED_ORTHO_SCALES
}

/// Ajusta una escala libre al nivel OpenTTD permitido más cercano.
#[must_use]
pub(crate) fn snap_fixed_ortho_scale(
    scale: f32,
    window_width: f32,
    window_height: f32,
    large_map_cull: bool,
) -> f32 {
    let target = scale.clamp(
        crate::render::MIN_ORTHO_SCALE,
        crate::render::ABSOLUTE_MAX_ORTHO_SCALE,
    );
    available_fixed_zoom_levels(window_width, window_height, large_map_cull)
        .iter()
        .copied()
        .min_by(|left, right| (left - target).abs().total_cmp(&(right - target).abs()))
        .unwrap_or(OPENTTD_FIXED_ORTHO_SCALES[0])
}

/// Aplica un paso de zoom respetando la política seleccionada.
///
/// En modo libre conserva los factores históricos de los botones/atajos. En
/// modo fijo avanza exactamente un nivel OpenTTD, incluyendo `Out8x`.
#[must_use]
pub(crate) fn zoom_step_scale(
    scale: f32,
    zoom_in: bool,
    mode: ZoomMode,
    window_width: f32,
    window_height: f32,
    large_map_cull: bool,
) -> f32 {
    if mode == ZoomMode::Free {
        let factor = if zoom_in { 0.85 } else { 1.15 };
        return clamp_ortho_scale(scale * factor, window_width, window_height, large_map_cull);
    }

    let levels = available_fixed_zoom_levels(window_width, window_height, large_map_cull);
    let snapped = snap_fixed_ortho_scale(scale, window_width, window_height, large_map_cull);
    let current = levels
        .iter()
        .position(|candidate| *candidate == snapped)
        .unwrap_or(0);
    let next = if zoom_in {
        current.saturating_sub(1)
    } else {
        (current + 1).min(levels.len() - 1)
    };
    levels[next]
}

fn apply_camera_focus_request(
    time: Res<Time<Real>>,
    mut request: ResMut<CameraFocusRequest>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut vel: ResMut<CameraVelocity>,
) {
    if let Some(target) = request.target.take() {
        request.smooth_target = Some(target);
    }
    let Some(target) = request.smooth_target else {
        return;
    };
    let Ok(mut transform) = cam_q.single_mut() else {
        return;
    };
    let current = Vec2::new(transform.translation.x, transform.translation.y);
    let dt = presentation_delta_secs(&time);
    let lerp = (dt / 0.3).clamp(0.0, 1.0);
    let next = current.lerp(target, lerp);
    transform.translation.x = next.x;
    transform.translation.y = next.y;
    vel.0 = Vec2::ZERO;
    if next.distance_squared(target) < 4.0 {
        request.smooth_target = None;
    }
}

/// Mueve la cámara con WASD (con inercia), arrastre con botón derecho y rueda del ratón.
#[allow(clippy::too_many_arguments)] // firma dictada por el sistema ECS de Bevy
pub fn move_camera(
    time: Res<Time<Real>>,
    kbd: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    sim: Res<SimWorld>,
    zoom_mode: Option<Res<ZoomMode>>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    mut vel: ResMut<CameraVelocity>,
) {
    let Ok((mut transform, mut projection)) = cam_q.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref mut proj) = *projection else {
        return;
    };
    let zoom_mode = zoom_mode.map_or(ZoomMode::Fixed, |mode| *mode);

    let (mw, mh) = sim.state.map.dimensions();
    let large_cull = large_map_viewport_cull_enabled(mw, mh);
    let (win_w, win_h) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 720.0));

    let dt = presentation_delta_secs(&time);

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

    // Zoom con teclado. En modo libre se conserva el ajuste continuo; el
    // atajo configurable `+`/`-` se atiende también en `handle_zoom_hotkeys`.
    // En modo fijo ese atajo da un solo paso discreto allí; aquí solo se
    // mantienen los equivalentes del teclado numérico.
    if zoom_mode == ZoomMode::Free {
        let z = ZOOM_KEY_RATE * dt;
        if kbd.pressed(KeyCode::Equal) || kbd.pressed(KeyCode::NumpadAdd) {
            proj.scale = clamp_ortho_scale(proj.scale * (1.0 - z), win_w, win_h, large_cull);
        }
        if kbd.pressed(KeyCode::Minus) || kbd.pressed(KeyCode::NumpadSubtract) {
            proj.scale = clamp_ortho_scale(proj.scale * (1.0 + z), win_w, win_h, large_cull);
        }
    } else if kbd.just_pressed(KeyCode::NumpadAdd) {
        proj.scale = zoom_step_scale(proj.scale, true, zoom_mode, win_w, win_h, large_cull);
    } else if kbd.just_pressed(KeyCode::NumpadSubtract) {
        proj.scale = zoom_step_scale(proj.scale, false, zoom_mode, win_w, win_h, large_cull);
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
        let new_scale = if zoom_mode == ZoomMode::Free {
            clamp_ortho_scale(
                old_scale * (1.0 - scroll.delta.y * ZOOM_WHEEL_SENS),
                window.width(),
                window.height(),
                large_cull,
            )
        } else {
            // Una rueda/touchpad puede acumular más de una unidad en un frame.
            // Cada unidad avanza un nivel, con un límite defensivo de seis
            // pasos (la cantidad total de niveles OpenTTD).
            let steps = scroll.delta.y.abs().ceil().clamp(1.0, 6.0) as usize;
            (0..steps).fold(old_scale, |next, _| {
                zoom_step_scale(
                    next,
                    scroll.delta.y > 0.0,
                    zoom_mode,
                    window.width(),
                    window.height(),
                    large_cull,
                )
            })
        };
        proj.scale = new_scale;

        let new_cam_pos = world_pos - cursor_offset_world * new_scale;
        transform.translation.x = new_cam_pos.x;
        transform.translation.y = new_cam_pos.y;
    } else {
        // Mantener escala dentro del tope si cambió el tamaño de mapa / ventana.
        proj.scale = if zoom_mode == ZoomMode::Fixed {
            snap_fixed_ortho_scale(proj.scale, win_w, win_h, large_cull)
        } else {
            clamp_ortho_scale(proj.scale, win_w, win_h, large_cull)
        };
    }
}

/// El reloj de presentación no se pausa ni se escala junto con la simulación.
///
/// OpenTTD actualiza su viewport desde `steady_clock` en `UpdateWindows`, por
/// fuera del game tick. La cota conserva la protección previa ante un frame
/// extraordinariamente largo sin reintroducir dependencia de `Time<Virtual>`.
fn presentation_delta_secs(time: &Time<Real>) -> f32 {
    time.delta_secs().min(MAX_PRESENTATION_DELTA_SECS)
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
    use std::time::Duration;

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::state::app::StatesPlugin;
    use bevy::window::{PrimaryWindow, WindowResolution};

    use crate::render::{RemapMapVisualsPending, VehicleIndex};
    use crate::settings::ClientPreferences;
    use crate::simulation::SimulationPlugin;
    use crate::state::{ClientScreen, SimRunState};
    use crate::ui::SimHudControls;

    /// Ejecuta los plugins de producción con los tres relojes que instala
    /// `TimePlugin`. El helper avanza `Virtual` usando la función real de
    /// Bevy, para poder probar pausa y multiplicadores sin convertir el
    /// reloj genérico en un doble artificial.
    fn camera_schedule_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<ClientScreen>()
            .add_sub_state::<SimRunState>()
            .insert_resource(Time::<()>::default())
            .insert_resource(Time::<Real>::default())
            .insert_resource(Time::<Virtual>::default())
            .insert_resource(Time::<Fixed>::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(ButtonInput::<MouseButton>::default())
            .insert_resource(AccumulatedMouseMotion::default())
            .insert_resource(AccumulatedMouseScroll::default())
            .insert_resource(SimWorld::default())
            .insert_resource(VehicleIndex::default())
            .insert_resource(RemapMapVisualsPending::default())
            .insert_resource(ClientPreferences::default())
            .insert_resource(SimHudControls::default())
            .add_plugins((SimulationPlugin, CameraControlPlugin));
        let camera = app
            .world_mut()
            .spawn((
                PrimaryGameCamera,
                Transform::default(),
                Projection::Orthographic(OrthographicProjection::default_2d()),
            ))
            .id();
        app.world_mut()
            .resource_mut::<NextState<ClientScreen>>()
            .set(ClientScreen::InGame);
        app.update();
        (app, camera)
    }

    fn advance_presentation_and_virtual_time(app: &mut App, delta: Duration) {
        let mut real = *app.world().resource::<Time<Real>>();
        real.advance_by(delta);
        let mut virtual_time = *app.world().resource::<Time<Virtual>>();
        let mut game_time = *app.world().resource::<Time<()>>();
        bevy::time::update_virtual_time(&mut game_time, &mut virtual_time, &real);
        app.world_mut().insert_resource(real);
        app.world_mut().insert_resource(virtual_time);
        app.world_mut().insert_resource(game_time);
    }

    #[test]
    fn move_camera_without_camera_query_is_noop() {
        let mut world = World::new();
        world.insert_resource(Time::<Real>::default());
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(AccumulatedMouseMotion::default());
        world.insert_resource(AccumulatedMouseScroll::default());
        world.insert_resource(CameraVelocity::default());
        world.insert_resource(SimWorld::default());
        world.run_system_once(move_camera).unwrap();
    }

    #[test]
    fn move_camera_handles_keyboard_drag_and_scroll() {
        let mut world = World::new();
        let mut time = Time::<Real>::default();
        time.advance_by(Duration::from_millis(16));
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
        world.insert_resource(SimWorld::default());

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
        let mut time = Time::<Real>::default();
        time.advance_by(Duration::from_millis(30));
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
        world.insert_resource(SimWorld::default());

        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        world.run_system_once(move_camera).unwrap();
    }

    #[test]
    fn camera_moves_and_focuses_with_real_time_while_simulation_is_paused() {
        let (mut app, camera) = camera_schedule_app();
        app.world_mut()
            .resource_mut::<NextState<SimRunState>>()
            .set(SimRunState::Paused);
        app.update();
        assert_eq!(
            *app.world().resource::<State<SimRunState>>().get(),
            SimRunState::Paused
        );
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.world_mut().resource_mut::<CameraFocusRequest>().target = Some(Vec2::new(120.0, 0.0));
        let tick_before = app.world().resource::<SimWorld>().state.tick.get();

        advance_presentation_and_virtual_time(&mut app, Duration::from_millis(16));
        assert_eq!(app.world().resource::<Time<Real>>().delta_secs(), 0.016);
        assert_eq!(app.world().resource::<Time<Virtual>>().delta_secs(), 0.0);
        app.update();

        let transform = app.world().get::<Transform>(camera).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "el foco avanza con reloj real"
        );
        assert!(transform.translation.y > 0.0, "WASD avanza con reloj real");
        assert_eq!(
            app.world().resource::<SimWorld>().state.tick.get(),
            tick_before,
            "mover la cámara pausada no avanza la simulación"
        );
        assert_eq!(
            *app.world().resource::<State<SimRunState>>().get(),
            SimRunState::Paused
        );
    }

    #[test]
    fn camera_response_is_independent_of_virtual_simulation_speed() {
        let move_once = |speed: f32| {
            let (mut app, camera) = camera_schedule_app();
            app.world_mut().resource_mut::<SimHudControls>().sim_speed = speed;
            // `sync_sim_time_controls` aplica exactamente el multiplicador que
            // usa producción antes de que Bevy derive Virtual desde Real.
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyW);
            advance_presentation_and_virtual_time(&mut app, Duration::from_millis(16));
            let virtual_delta = app.world().resource::<Time<Virtual>>().delta_secs();
            app.update();
            (
                app.world().get::<Transform>(camera).unwrap().translation.y,
                virtual_delta,
            )
        };

        let (at_1x, virtual_at_1x) = move_once(1.0);
        let (at_4x, virtual_at_4x) = move_once(4.0);
        let (at_8x, virtual_at_8x) = move_once(8.0);

        assert!(virtual_at_1x < virtual_at_4x && virtual_at_4x < virtual_at_8x);
        assert!((at_1x - at_4x).abs() < f32::EPSILON);
        assert!((at_1x - at_8x).abs() < f32::EPSILON);
    }

    #[test]
    fn presentation_delta_is_bounded_after_a_long_system_stall() {
        let mut time = Time::<Real>::default();
        time.advance_by(Duration::from_secs(1));
        assert_eq!(presentation_delta_secs(&time), MAX_PRESENTATION_DELTA_SECS);
    }

    #[test]
    fn zoom_display_magnification_is_reciprocal_of_ortho_scale() {
        assert!((zoom_display_magnification(1.0) - 1.0).abs() < 0.001);
        assert!((zoom_display_magnification(0.25) - 4.0).abs() < 0.001);
        assert!((zoom_display_magnification(20.0) - 0.05).abs() < 0.001);
    }

    #[test]
    fn tile_camera_uses_geometric_tile_center_not_sprite_anchor() {
        let map = Map::new_flat(8, 8, 3);
        let coord = TileCoord::new(2, 5);

        let actual = tile_camera_world_pos(&map, coord);
        let top = iso(coord.x, coord.y);
        let expected = Vec2::new(top.x, top.y - ISO_QH + 3.0 * HEIGHT_PX);

        assert_eq!(actual, expected);
        // La diferencia de medio píxel respecto al ancla visual del sprite
        // es intencional: el scroll de OpenTTD centra el rombo lógico 64×32.
        assert_eq!(actual.y, top.y - 16.0 + 24.0);
    }

    #[test]
    fn zoom_mode_defaults_to_fixed_openttd() {
        assert_eq!(ZoomMode::default(), ZoomMode::Fixed);
    }

    #[test]
    fn fixed_zoom_uses_all_openttd_levels_on_large_maps() {
        assert_eq!(snap_fixed_ortho_scale(0.78, 1280.0, 720.0, false), 1.0);
        assert_eq!(
            zoom_step_scale(1.0, true, ZoomMode::Fixed, 1280.0, 720.0, false),
            0.5
        );
        assert_eq!(
            zoom_step_scale(1.0, false, ZoomMode::Fixed, 1280.0, 720.0, false),
            2.0
        );

        // Out4x sigue disponible con culling: los viewports dentro del
        // presupuesto conservan sprites completos y el resto usa el resumen.
        assert_eq!(
            zoom_step_scale(2.0, false, ZoomMode::Fixed, 1280.0, 720.0, true),
            4.0
        );
    }

    #[test]
    fn fixed_zoom_matrix_matches_all_openttd_levels_and_hud_magnification() {
        // OpenTTD exposes seis niveles: In4x, In2x, Normal, Out2x, Out4x y
        // Out8x. La escala ortográfica es la inversa de la magnificación que
        // se muestra en el título/HUD.
        let levels = [0.25_f32, 0.5, 1.0, 2.0, 4.0, 8.0];
        let expected_magnification = [4.0_f32, 2.0, 1.0, 0.5, 0.25, 0.125];

        for (index, (&scale, &magnification)) in
            levels.iter().zip(expected_magnification.iter()).enumerate()
        {
            assert_eq!(snap_fixed_ortho_scale(scale, 1280.0, 720.0, true), scale);
            assert!((zoom_display_magnification(scale) - magnification).abs() < 0.001);

            if index > 0 {
                assert_eq!(
                    zoom_step_scale(
                        levels[index - 1],
                        false,
                        ZoomMode::Fixed,
                        1280.0,
                        720.0,
                        true,
                    ),
                    scale
                );
            }
            if index + 1 < levels.len() {
                assert_eq!(
                    zoom_step_scale(scale, false, ZoomMode::Fixed, 1280.0, 720.0, true,),
                    levels[index + 1]
                );
            }
        }
    }

    #[test]
    fn move_camera_scroll_without_window_returns_after_zoom_branch() {
        let mut world = World::new();
        let mut time = Time::<Real>::default();
        time.advance_by(Duration::from_millis(16));
        world.insert_resource(time);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(AccumulatedMouseMotion::default());
        world.insert_resource(AccumulatedMouseScroll {
            delta: Vec2::new(0.0, 1.0),
            ..default()
        });
        world.insert_resource(CameraVelocity::default());
        world.insert_resource(SimWorld::default());

        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.run_system_once(move_camera).unwrap();
    }
}
