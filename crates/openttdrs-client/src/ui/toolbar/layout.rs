use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::{
    BuildMenuAction, BuildMenuUi, ToolButtonGroup, ToolSelectButton, ToolbarGroup, ToolbarGroupButton,
    ToolbarTooltipTarget, TooltipBox, TooltipText,
};

/// Barra superior compacta tipo toolbar para seleccion rapida de herramienta.
pub(crate) fn setup_top_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(1.0),
                ..default()
            },
            BuildMenuUi,
            GlobalZIndex(2100),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(1.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.17, 0.12, 0.96)),
            BorderColor::all(Color::srgb(0.68, 0.61, 0.42)),
            FocusPolicy::Block,
            BuildMenuUi,
            Interaction::default(),
        ))
        .with_children(|parent| {
            for (i, icon_path, group) in [
                (0_u8, "opengfx/tiles/rail_1005.png", ToolbarGroup::Transport),
                (1, "opengfx/tiles/road_flat_00.png", ToolbarGroup::Build),
                (2, "opengfx/tiles/house_church_build.png", ToolbarGroup::Economy),
                (3, "opengfx/tiles/object_lighthouse.png", ToolbarGroup::Info),
                (4, "opengfx/tiles/object_transmitter.png", ToolbarGroup::Settings),
            ] {
                parent
                    .spawn((
                        Button,
                        group,
                        ToolbarGroupButton,
                        ToolbarTooltipTarget {
                            text: match group {
                                ToolbarGroup::Transport => "Transportes",
                                ToolbarGroup::Build => "Construccion",
                                ToolbarGroup::Economy => "Economia",
                                ToolbarGroup::Info => "Informacion",
                                ToolbarGroup::Settings => "Ajustes",
                            },
                        },
                        BuildMenuUi,
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.33, 0.28, 0.19)),
                        BorderColor::all(Color::srgb(0.64, 0.57, 0.39)),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(asset_server.load::<Image>(icon_path)),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                padding: UiRect::all(Val::Px(0.5)),
                                ..default()
                            },
                        ));
                    });
                if i < 4 {
                    parent.spawn((
                        Node {
                            width: Val::Px(1.0),
                            height: Val::Px(20.0),
                            margin: UiRect::horizontal(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.62, 0.55, 0.38)),
                        BuildMenuUi,
                    ));
                }
            }
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(1.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.18, 0.15, 0.11, 0.95)),
            BorderColor::all(Color::srgb(0.66, 0.6, 0.42)),
            FocusPolicy::Block,
            BuildMenuUi,
            ToolButtonGroup(ToolbarGroup::Build),
            Interaction::default(),
        ))
        .with_children(|buttons| {
            spawn_tool_buttons(
                buttons,
                &asset_server,
                &[
                    ("Road", "opengfx/tiles/road_flat_00.png", BuildMenuAction::Road),
                    ("Station", "opengfx/tiles/truck_stop_ground_0.png", BuildMenuAction::Station),
                    ("Clear", "opengfx/tiles/grass_rough.png", BuildMenuAction::Clear),
                ],
            );
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(1.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.18, 0.15, 0.11, 0.95)),
            BorderColor::all(Color::srgb(0.66, 0.6, 0.42)),
            FocusPolicy::Block,
            BuildMenuUi,
            ToolButtonGroup(ToolbarGroup::Transport),
            Interaction::default(),
        ))
        .with_children(|buttons| {
            spawn_tool_buttons(
                buttons,
                &asset_server,
                &[
                    ("Rail", "opengfx/tiles/rail_1005.png", BuildMenuAction::Rail),
                    ("Road", "opengfx/tiles/road_flat_00.png", BuildMenuAction::Road),
                    ("Clear", "opengfx/tiles/grass_rough.png", BuildMenuAction::Clear),
                ],
            );
        });

        for group in [ToolbarGroup::Economy, ToolbarGroup::Info, ToolbarGroup::Settings] {
            root.spawn((
                Node {
                    display: Display::None,
                    ..default()
                },
                BuildMenuUi,
                ToolButtonGroup(group),
            ));
        }

        root.spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.12, 0.08, 0.97)),
            BorderColor::all(Color::srgb(0.8, 0.72, 0.5)),
            BuildMenuUi,
            TooltipBox,
            children![(
                TooltipText,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            )],
        ));
    });
}

fn spawn_tool_buttons(
    buttons: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    defs: &[(&'static str, &'static str, BuildMenuAction)],
) {
    for (label, icon_path, action) in defs {
        buttons
            .spawn((
                Button,
                *action,
                ToolSelectButton,
                ToolbarTooltipTarget {
                    text: match action {
                        BuildMenuAction::Road => "Construir carretera (1)",
                        BuildMenuAction::Rail => "Construir via ferrea (3)",
                        BuildMenuAction::Station => "Construir estacion (2)",
                        BuildMenuAction::Clear => "Limpiar tesela (C)",
                    },
                },
                BuildMenuUi,
                Node {
                    width: Val::Px(90.0),
                    height: Val::Px(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.28, 0.24, 0.16)),
                BorderColor::all(Color::srgb(0.64, 0.57, 0.39)),
                Interaction::default(),
            ))
            .with_children(|p| {
                p.spawn((
                    ImageNode::new(asset_server.load::<Image>(*icon_path)),
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        margin: UiRect::right(Val::Px(4.0)),
                        ..default()
                    },
                ));
                p.spawn((
                    Text::new(*label),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.12, 0.12, 0.1)),
                ));
            });
    }
}
