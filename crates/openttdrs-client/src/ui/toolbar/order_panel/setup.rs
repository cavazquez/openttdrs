use bevy::prelude::*;

use crate::ui::toolbar::{BuildMenuUi, OrderPanelButton, OrderPanelRoot, OrderPanelText};

use super::{ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

pub(crate) fn setup_order_panel(mut commands: Commands) {
    commands
        .spawn((
            OrderPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(320.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.13, 0.1, 0.07, 0.95)),
            BorderColor::all(Color::srgb(0.75, 0.67, 0.45)),
            Visibility::Hidden,
            BuildMenuUi,
        ))
        .with_children(|panel| {
            panel.spawn((
                OrderPanelText,
                Text::new("Vehículo"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
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
                        spawn_order_button(row, OrderPanelButton::Sell, "Vender");
                        spawn_order_button(row, OrderPanelButton::ClearLast, "Quitar última");
                        spawn_order_button(row, OrderPanelButton::ClearAll, "Vaciar lista");
                        spawn_order_button(row, OrderPanelButton::Close, "Cerrar");
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
                font_size: 11.0,
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
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}
