//! ExtraViewport: cámara secundaria en ventana flotante (sigue a la principal).

use bevy::camera::RenderTarget;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

const PREVIEW_TEX_W: u32 = 320;
const PREVIEW_TEX_H: u32 = 200;
const PREVIEW_SCALE: f32 = 1.35;

#[derive(Resource, Default)]
pub(crate) struct ExtraViewportWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct ExtraViewportCamera;

#[derive(Component)]
pub(crate) struct ExtraViewportHintText;

pub(crate) fn setup_extra_viewport_window(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let asset_server = &*asset_server;
    let image = Image::new_target_texture(
        PREVIEW_TEX_W,
        PREVIEW_TEX_H,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    let rt_handle = images.add(image);

    commands.spawn((
        Camera2d,
        MapPreviewCamera,
        ExtraViewportCamera,
        Camera {
            order: -4,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.18, 0.28, 0.36)),
            ..default()
        },
        RenderTarget::from(rt_handle.clone()),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: PREVIEW_SCALE,
            ..OrthographicProjection::default_2d()
        }),
    ));

    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::ExtraViewport,
        "Vista extra",
        TITLE_BROWN,
        Vec2::new(360.0, 100.0),
        340.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            ExtraViewportHintText,
            Text::new("Sigue la cámara principal (zoom más alejado)."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel.spawn((
            ImageNode::new(rt_handle),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(PREVIEW_TEX_H as f32),
                margin: UiRect::top(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.13, 0.10, 0.07)),
            BuildMenuUi,
        ));
    });
}

pub(crate) fn sync_extra_viewport_window(
    state: Res<ExtraViewportWindowState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut cam_q: Query<
        (&mut Camera, &mut Transform),
        (With<ExtraViewportCamera>, Without<PrimaryGameCamera>),
    >,
    primary_q: Query<&Transform, (With<PrimaryGameCamera>, Without<ExtraViewportCamera>)>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::ExtraViewport)
    else {
        return;
    };
    let Ok((mut camera, mut cam_tf)) = cam_q.single_mut() else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        camera.is_active = false;
        return;
    }
    *vis = Visibility::Visible;
    camera.is_active = true;
    if let Ok(primary) = primary_q.single() {
        cam_tf.translation.x = primary.translation.x;
        cam_tf.translation.y = primary.translation.y;
        cam_tf.translation.z = primary.translation.z;
    }
}

pub(crate) fn extra_viewport_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<ExtraViewportWindowState>,
    mut cam_q: Query<&mut Camera, With<ExtraViewportCamera>>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::ExtraViewport {
            state.open = false;
            if let Ok(mut camera) = cam_q.single_mut() {
                camera.is_active = false;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn closing_extra_viewport_deactivates_camera() {
        let mut world = World::new();
        world.init_resource::<ExtraViewportWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.resource_mut::<ExtraViewportWindowState>().open = true;
        world.spawn((
            ExtraViewportCamera,
            Camera {
                is_active: true,
                ..default()
            },
        ));
        world.write_message(FloatingWindowClosed(FloatingWindowId::ExtraViewport));
        world
            .run_system_once(extra_viewport_window_on_closed)
            .unwrap();
        assert!(!world.resource::<ExtraViewportWindowState>().open);
        let cam = world
            .query_filtered::<&Camera, With<ExtraViewportCamera>>()
            .single(&world)
            .unwrap();
        assert!(!cam.is_active);
    }
}
