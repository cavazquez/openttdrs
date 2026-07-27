use bevy::prelude::*;

use crate::ui::toolbar::BuildMenuUi;

use super::{
    MINIMAP_BOTTOM, MINIMAP_CELL, MINIMAP_COLS, MINIMAP_CONTROLS_H, MINIMAP_PAD, MINIMAP_RIGHT,
    MINIMAP_ROWS, MinimapCell, MinimapGrid, MinimapGridRow, MinimapLayerState, MinimapLayerToggle,
    MinimapLegendText, MinimapRoot, MinimapViewport,
};

pub(crate) fn setup_minimap(mut commands: Commands) {
    let root = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(MINIMAP_RIGHT),
                bottom: Val::Px(MINIMAP_BOTTOM),
                width: Val::Px(MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0),
                height: Val::Px(
                    MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0 + MINIMAP_CONTROLS_H,
                ),
                padding: UiRect::all(Val::Px(MINIMAP_PAD)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.04, 0.82)),
            BorderColor::all(Color::srgb(0.55, 0.5, 0.34)),
            BuildMenuUi,
            MinimapRoot,
            Interaction::default(),
            GlobalZIndex(1200),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        root.spawn((
            MinimapGrid,
            Node {
                width: Val::Px(MINIMAP_COLS as f32 * MINIMAP_CELL),
                height: Val::Px(MINIMAP_ROWS as f32 * MINIMAP_CELL),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(0.0),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|grid| {
            for row in 0..MINIMAP_ROWS {
                grid.spawn((
                    MinimapGridRow,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(MINIMAP_CELL),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(0.0),
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .with_children(|line| {
                    for col in 0..MINIMAP_COLS {
                        line.spawn((
                            MinimapCell { col, row },
                            Node {
                                width: Val::Px(MINIMAP_CELL),
                                height: Val::Px(MINIMAP_CELL),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.12, 0.2, 0.09)),
                            Interaction::default(),
                            BuildMenuUi,
                        ));
                    }
                });
            }
        });
        root.spawn((
            MinimapViewport,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(MINIMAP_PAD),
                top: Val::Px(MINIMAP_PAD),
                width: Val::Px(12.0),
                height: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgb(1.0, 1.0, 0.9)),
        ));
        root.spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(MINIMAP_CONTROLS_H - 4.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(3.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            spawn_layer_toggle(row, MinimapLayerToggle::Industries, "Ind");
            spawn_layer_toggle(row, MinimapLayerToggle::Owners, "Due");
            spawn_layer_toggle(row, MinimapLayerToggle::Vehicles, "Veh");
            spawn_layer_toggle(row, MinimapLayerToggle::Expand, "Ampliar");
            row.spawn((
                MinimapLegendText,
                Text::new("capas"),
                TextFont {
                    font_size: FontSize::Rem(0.55),
                    ..default()
                },
                TextColor(Color::srgb(0.82, 0.78, 0.62)),
                BuildMenuUi,
            ));
        });
    });
}

fn spawn_layer_toggle(parent: &mut ChildSpawnerCommands, toggle: MinimapLayerToggle, label: &str) {
    parent.spawn((
        Button,
        toggle,
        BuildMenuUi,
        Node {
            min_width: Val::Px(28.0),
            height: Val::Px(16.0),
            padding: UiRect::horizontal(Val::Px(3.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(0.5),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.78)),
        )],
    ));
}

pub(crate) fn handle_minimap_layer_buttons(
    buttons: Query<(&Interaction, &MinimapLayerToggle), (Changed<Interaction>, With<Button>)>,
    mut layers: ResMut<MinimapLayerState>,
) {
    for (interaction, toggle) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *toggle {
            MinimapLayerToggle::Industries => layers.industries = !layers.industries,
            MinimapLayerToggle::Owners => layers.owners = !layers.owners,
            MinimapLayerToggle::Vehicles => layers.vehicles = !layers.vehicles,
            MinimapLayerToggle::Expand => layers.expanded = !layers.expanded,
        }
    }
}
