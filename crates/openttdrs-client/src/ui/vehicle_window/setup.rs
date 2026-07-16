//! Inicialización de la ventana de vista del vehículo (setup, spawn helpers).

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::text::EditableText;
use bevy::ui::widget::ImageNode;

use crate::render::MapPreviewCamera;
use crate::ui::floating_window::{
    FloatingWindowId, TITLE_CRIMSON, WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::{BuildMenuUi, ToolbarTooltipTarget};

use super::{
    BTN_BG, BTN_BORDER, CONSIST_STRIP_MAX_UNITS, CONSIST_UNIT_SPRITE_H, CONSIST_UNIT_SPRITE_W,
    ICON_BTN, ICON_IMG, PLACEHOLDER_SPRITE, PREVIEW_SCALE, PREVIEW_TEX_H, PREVIEW_TEX_W,
    STATUS_STOPPED, VehicleConsistUnitSprite, VehicleWindowButton, VehicleWindowPreviewCamera,
    VehicleWindowRefitOnly, VehicleWindowRenameButton, VehicleWindowRenameInput,
    VehicleWindowRenameRow, VehicleWindowStatusText, VehicleWindowToggleText,
    VehicleWindowTrainOnly,
};

pub(crate) fn setup_vehicle_window(
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
        VehicleWindowPreviewCamera,
        Camera {
            order: -3,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        RenderTarget::from(rt_handle.clone()),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: PREVIEW_SCALE,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // Vista compacta OpenTTD (#174): preview + status + fila de iconos.
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Vehicle,
        "Vehículo",
        TITLE_CRIMSON,
        Vec2::new(720.0, 148.0),
        280.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            ImageNode::new(rt_handle),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(PREVIEW_TEX_H as f32),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.13, 0.10, 0.07)),
            BuildMenuUi,
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(CONSIST_UNIT_SPRITE_H + 2.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                margin: UiRect::bottom(Val::Px(2.0)),
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|strip| {
                for unit_idx in 0..CONSIST_STRIP_MAX_UNITS {
                    strip.spawn((
                        VehicleConsistUnitSprite { unit_idx },
                        ImageNode::new(asset_server.load::<Image>(PLACEHOLDER_SPRITE)),
                        Node {
                            width: Val::Px(CONSIST_UNIT_SPRITE_W),
                            height: Val::Px(CONSIST_UNIT_SPRITE_H),
                            display: Display::None,
                            ..default()
                        },
                    ));
                }
            });
        panel.spawn((
            VehicleWindowStatusText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(STATUS_STOPPED),
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(16.0),
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
        ));
        panel
            .spawn((
                VehicleWindowRenameRow,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    display: Display::None,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                row.spawn((
                    VehicleWindowRenameInput,
                    EditableText::new(""),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(22.0),
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(BTN_BORDER),
                ));
                spawn_rename_action(
                    row,
                    asset_server,
                    VehicleWindowRenameButton::Apply,
                    "Guardar",
                );
                spawn_rename_action(
                    row,
                    asset_server,
                    VehicleWindowRenameButton::Cancel,
                    "Cancelar",
                );
            });
        // Toolbar de iconos (#174).
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|row| {
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::ToggleRunning,
                    "text:▶",
                    "Iniciar / Detener",
                    true,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::Orders,
                    "assets/opengfx/tiles/toolbar_rail_station.png",
                    "Órdenes",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::Timetable,
                    "text:⏱",
                    "Horario",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::GotoDepot,
                    "assets/opengfx/tiles/toolbar_rail_depot.png",
                    "Enviar al depósito",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::CenterOrder,
                    "assets/opengfx/tiles/toolbar_rail_waypoint.png",
                    "Ir a orden activa",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::CenterCamera,
                    "assets/opengfx/tiles/ui_terraform_up.png",
                    "Centrar cámara",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::Details,
                    "assets/opengfx/tiles/ui_settings.png",
                    "Detalles",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::Rename,
                    "text:Aa",
                    "Renombrar",
                    false,
                );
            });
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    margin: UiRect::top(Val::Px(2.0)),
                    ..default()
                },
                VehicleWindowTrainOnly,
                BuildMenuUi,
            ))
            .with_children(|row| {
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::TurnAround,
                    "text:↺",
                    "Dar la vuelta",
                    false,
                );
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::ForceProceed,
                    "text:⏭",
                    "Forzar paso",
                    false,
                );
            });
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    display: Display::None,
                    margin: UiRect::top(Val::Px(2.0)),
                    ..default()
                },
                VehicleWindowRefitOnly,
                BuildMenuUi,
            ))
            .with_children(|row| {
                spawn_vehicle_icon(
                    row,
                    asset_server,
                    VehicleWindowButton::Refit,
                    "text:⚙",
                    "Refit carga",
                    false,
                );
            });
    });
}

fn spawn_rename_action(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: VehicleWindowRenameButton,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: Val::Px(66.0),
                height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            ));
        });
}

fn spawn_vehicle_icon(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: VehicleWindowButton,
    icon: &str,
    tip: &'static str,
    is_toggle: bool,
) {
    parent
        .spawn((
            Button,
            action,
            ToolbarTooltipTarget { text: tip },
            Node {
                width: Val::Px(ICON_BTN),
                height: Val::Px(ICON_BTN),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            if let Some(label) = icon.strip_prefix("text:") {
                let mut text = btn.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(ICON_IMG + 2.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.92, 0.78)),
                ));
                if is_toggle {
                    text.insert(VehicleWindowToggleText);
                }
            } else {
                btn.spawn((
                    ImageNode::new(asset_server.load::<Image>(icon.to_string())),
                    Node {
                        width: Val::Px(ICON_IMG),
                        height: Val::Px(ICON_IMG),
                        ..default()
                    },
                ));
            }
        });
}
