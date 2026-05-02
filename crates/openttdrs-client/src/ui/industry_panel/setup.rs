use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::widget::ImageNode;
use bevy::ui::{FocusPolicy, GlobalZIndex};

use crate::render::IndustryPreviewCamera;
use crate::ui::toolbar::BuildMenuUi;

use super::{
    IndustryPanelCloseButton, IndustryPanelDetails, IndustryPanelRoot, IndustryPanelTitle,
};

const PREVIEW_TEX_W: u32 = 320;
const PREVIEW_TEX_H: u32 = 180;
const UI_FONT: &str = "static/fonts/DejaVuSansMono.ttf";

pub(crate) fn setup_industry_panel(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let image = Image::new_target_texture(
        PREVIEW_TEX_W,
        PREVIEW_TEX_H,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    let rt_handle = images.add(image);
    let ui_font = asset_server.load::<Font>(UI_FONT);

    commands.spawn((
        Camera2d,
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

    commands
        .spawn((
            IndustryPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(72.0),
                width: Val::Px(340.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.14, 0.11, 0.08, 0.97)),
            BorderColor::all(Color::srgb(0.78, 0.7, 0.48)),
            GlobalZIndex(2200),
            Visibility::Hidden,
            BuildMenuUi,
            FocusPolicy::Block,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                row.spawn((
                    IndustryPanelTitle,
                    Text::new("Industria"),
                    TextFont {
                        font: ui_font.clone(),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.8)),
                    BuildMenuUi,
                ));
                row.spawn((
                    IndustryPanelCloseButton,
                    Button,
                    Node {
                        width: Val::Px(28.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.42, 0.36, 0.24)),
                    BorderColor::all(Color::srgb(0.7, 0.62, 0.42)),
                    Interaction::default(),
                    BuildMenuUi,
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("✕"),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.92, 0.88, 0.78)),
                    ));
                });
            });
            p.spawn((
                Node {
                    width: Val::Px(PREVIEW_TEX_W as f32),
                    height: Val::Px(PREVIEW_TEX_H as f32),
                    ..default()
                },
                ImageNode::new(rt_handle),
                BuildMenuUi,
            ));
            p.spawn((
                IndustryPanelDetails,
                Text::new("Stock: --"),
                TextFont {
                    font: ui_font,
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.88, 0.76)),
                BuildMenuUi,
            ));
        });
}
