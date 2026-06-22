use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::widget::ImageNode;
use bevy::ui::{FocusPolicy, GlobalZIndex};

use crate::render::{MapPreviewCamera, VehiclePreviewCamera};
use crate::ui::toolbar::{BuildMenuUi, OrderPanelButton, OrderPanelRoot, OrderPanelTitle};

use super::{ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

const PREVIEW_TEX_W: u32 = 320;
const PREVIEW_TEX_H: u32 = 180;
const UI_FONT: &str = "static/fonts/DejaVuSansMono.ttf";

pub(crate) fn setup_order_panel(
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
        MapPreviewCamera,
        VehiclePreviewCamera,
        Camera {
            order: -2,
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
            OrderPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(72.0),
                width: Val::Px(340.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.13, 0.1, 0.07, 0.97)),
            BorderColor::all(Color::srgb(0.75, 0.67, 0.45)),
            GlobalZIndex(2200),
            Visibility::Hidden,
            BuildMenuUi,
            FocusPolicy::Block,
        ))
        .with_children(|panel| {
            panel
                .spawn((
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
                        OrderPanelTitle,
                        Text::new("Vehículo"),
                        TextFont {
                            font: ui_font.clone().into(),
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.92, 0.8)),
                        BuildMenuUi,
                    ));
                    row.spawn((
                        OrderPanelButton::Close,
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
                                font: ui_font.clone().into(),
                                font_size: FontSize::Px(13.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.88, 0.78)),
                        ));
                    });
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
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|list| {
                    for slot in 0..ORDER_PANEL_ROWS {
                        spawn_order_panel_row(list, slot);
                    }
                });
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_order_button(row, OrderPanelButton::PickDestOnMap, "Agregar destino");
                        spawn_order_button(row, OrderPanelButton::ToggleRunning, "Iniciar/Detener");
                        spawn_order_button(row, OrderPanelButton::ClearLast, "Quitar última");
                        spawn_order_button(row, OrderPanelButton::ClearAll, "Vaciar lista");
                    });
                });
        });
}

fn spawn_order_panel_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent.spawn((
        OrderPanelRow { slot },
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
        BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
        BuildMenuUi,
        children![(
            OrderPanelRowText { slot },
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_order_button(
    parent: &mut ChildSpawnerCommands,
    action: OrderPanelButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            min_width: Val::Px(78.0),
            padding: UiRect::horizontal(Val::Px(4.0)),
            height: Val::Px(24.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}
