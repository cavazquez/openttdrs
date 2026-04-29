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
                top: Val::Px(12.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
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
                column_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.22, 0.2, 0.16, 0.95)),
            BorderColor::all(Color::srgb(0.55, 0.5, 0.36)),
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
                            width: Val::Px(22.0),
                            height: Val::Px(22.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.36, 0.33, 0.24)),
                        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(asset_server.load::<Image>(icon_path)),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                ..default()
                            },
                        ));
                    });
                if i < 4 {
                    parent.spawn((
                        Node {
                            width: Val::Px(1.0),
                            height: Val::Px(18.0),
                            margin: UiRect::horizontal(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.5, 0.46, 0.34)),
                        BuildMenuUi,
                    ));
                }
            }
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.18, 0.14, 0.94)),
            BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
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
                column_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.18, 0.14, 0.94)),
            BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
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

        for (label, group) in [
            ("Economia: pronto", ToolbarGroup::Economy),
            ("Info: pronto", ToolbarGroup::Info),
            ("Ajustes: pronto", ToolbarGroup::Settings),
        ] {
            root.spawn((
                Node {
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.18, 0.14, 0.94)),
                BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
                FocusPolicy::Block,
                BuildMenuUi,
                ToolButtonGroup(group),
                children![(
                    Text::new(label),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.86, 0.72)),
                )],
            ));
        }

        root.spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.11, 0.08, 0.95)),
            BorderColor::all(Color::srgb(0.76, 0.7, 0.52)),
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
                    width: Val::Px(86.0),
                    height: Val::Px(22.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.28, 0.2)),
                BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
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
