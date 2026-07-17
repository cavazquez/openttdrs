use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::widget::ImageNode;

use crate::render::{IndustryPreviewCamera, MapPreviewCamera};
use crate::ui::floating_window::{TITLE_CREAM, spawn_floating_window, window_text_font};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

use super::{IndustryPanelCenterButton, IndustryPanelDetails};

const PREVIEW_TEX_W: u32 = 320;
const PREVIEW_TEX_H: u32 = 180;

pub(crate) fn setup_industry_panel(
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
        IndustryPreviewCamera,
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        RenderTarget::from(rt_handle.clone()),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: 2.0,
            ..OrthographicProjection::default_2d()
        }),
    ));

    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        crate::ui::floating_window::FloatingWindowId::Industry,
        "Industria",
        TITLE_CREAM,
        Vec2::new(20.0, 72.0),
        340.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    IndustryPanelCenterButton,
                    Button,
                    Node {
                        width: Val::Px(36.0),
                        height: Val::Px(22.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
                    BorderColor::all(Color::srgb(0.7, 0.62, 0.42)),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new("Loc"),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(Color::srgb(0.92, 0.88, 0.78)),
                    )],
                ));
            });
        panel.spawn((
            Node {
                width: Val::Px(PREVIEW_TEX_W as f32),
                height: Val::Px(PREVIEW_TEX_H as f32),
                ..default()
            },
            ImageNode::new(rt_handle),
            BuildMenuUi,
        ));
        panel.spawn((
            IndustryPanelDetails,
            Text::new("Stock: --"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.76)),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
    });
}
