use bevy::prelude::*;

use crate::ui::toolbar::BuildMenuUi;

use super::{
    MinimapCell, MinimapRoot, MinimapViewport, MINIMAP_BOTTOM, MINIMAP_CELL, MINIMAP_COLS,
    MINIMAP_PAD, MINIMAP_RIGHT, MINIMAP_ROWS,
};

pub(crate) fn setup_minimap(mut commands: Commands) {
    let root = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(MINIMAP_RIGHT),
                bottom: Val::Px(MINIMAP_BOTTOM),
                width: Val::Px(MINIMAP_COLS as f32 * MINIMAP_CELL + 12.0),
                height: Val::Px(MINIMAP_ROWS as f32 * MINIMAP_CELL + 12.0),
                padding: UiRect::all(Val::Px(MINIMAP_PAD)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.04, 0.82)),
            BorderColor::all(Color::srgb(0.55, 0.5, 0.34)),
            BuildMenuUi,
            MinimapRoot,
            Interaction::default(),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        for row in 0..MINIMAP_ROWS {
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(0.0),
                ..default()
            })
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
    });
}
