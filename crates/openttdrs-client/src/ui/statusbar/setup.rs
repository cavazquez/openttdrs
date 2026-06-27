use bevy::prelude::*;

use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

use super::{
    STATUS_BAR_HEIGHT, STATUS_BAR_Z, StatusBarCenterButton, StatusBarDefaultText,
    StatusBarReminderDot, StatusBarRoot, StatusBarTickerText,
};

const BAR_BG: Color = Color::srgb(0.38, 0.38, 0.38);
const BAR_BORDER: Color = Color::srgb(0.22, 0.22, 0.22);
const TEXT_LIGHT: Color = Color::srgb(0.96, 0.96, 0.92);
const TICKER_BLUE: Color = Color::srgb(0.55, 0.78, 0.98);

pub(crate) fn setup_status_bar(mut commands: Commands) {
    commands
        .spawn((
            StatusBarRoot,
            BuildMenuUi,
            GlobalZIndex(STATUS_BAR_Z),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Px(STATUS_BAR_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            BorderColor::all(BAR_BORDER),
        ))
        .with_children(|bar| {
            spawn_date_panel(bar, "1 ene 1950");
            bar.spawn((
                Button,
                StatusBarCenterButton,
                BuildMenuUi,
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(80.0),
                    overflow: Overflow::clip(),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::axes(Val::Px(1.0), Val::Px(0.0)),
                    ..default()
                },
                BackgroundColor(BAR_BG),
                BorderColor::all(BAR_BORDER),
                Interaction::default(),
            ))
            .with_children(|center| {
                center.spawn((
                    StatusBarDefaultText,
                    Text::new(super::COMPANY_DISPLAY_NAME),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                        ..default()
                    },
                    TextColor(TEXT_LIGHT),
                ));
                center.spawn((
                    StatusBarTickerText,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(100.0),
                        top: Val::Px(0.0),
                        ..default()
                    },
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                        ..default()
                    },
                    TextColor(TICKER_BLUE),
                    Visibility::Hidden,
                ));
                center.spawn((
                    StatusBarReminderDot,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(6.0),
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.92, 0.18, 0.12)),
                    Visibility::Hidden,
                ));
            });
            spawn_money_panel(bar, "$100000");
        });
}

fn spawn_date_panel(parent: &mut ChildSpawnerCommands, label: &str) {
    parent
        .spawn((
            Button,
            super::StatusBarDateButton,
            BuildMenuUi,
            Node {
                width: Val::Px(148.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            BorderColor::all(BAR_BORDER),
            Interaction::default(),
        ))
        .with_children(|panel| {
            panel.spawn((
                super::StatusBarDateText,
                Text::new(label),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                    ..default()
                },
                TextColor(TEXT_LIGHT),
            ));
        });
}

fn spawn_money_panel(parent: &mut ChildSpawnerCommands, label: &str) {
    parent
        .spawn((
            Button,
            super::StatusBarMoneyButton,
            BuildMenuUi,
            Node {
                width: Val::Px(148.0),
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            BorderColor::all(BAR_BORDER),
            Interaction::default(),
        ))
        .with_children(|panel| {
            panel.spawn((
                super::StatusBarMoneyText,
                Text::new(label),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                    ..default()
                },
                TextColor(TEXT_LIGHT),
            ));
        });
}
