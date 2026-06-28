use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::{MapSizePreset, PopulationDensity};
use crate::ui::font::UiFontRole;

use super::labels::climate_label;
use super::{
    MainMenuClimateButton, MainMenuDensityButton, MainMenuDensityTarget, MainMenuMapSizeButton,
    MainMenuStartYearButton, MainMenuStartingMoneyButton, MainMenuToggle,
};

pub(super) fn density_button(
    density: PopulationDensity,
    target: MainMenuDensityTarget,
) -> impl Bundle {
    (
        Button,
        MainMenuDensityButton(density, target),
        Node {
            width: Val::Px(88.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        Interaction::default(),
        children![(
            Text::new(density.menu_label()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn starting_money_button(amount: i64) -> impl Bundle {
    let label = if amount >= 1_000_000 {
        format!("{}M", amount / 1_000_000)
    } else if amount >= 1_000 {
        format!("{}k", amount / 1_000)
    } else {
        amount.to_string()
    };
    (
        Button,
        MainMenuStartingMoneyButton(amount),
        Node {
            width: Val::Px(72.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn map_size_button(size: MapSizePreset) -> impl Bundle {
    (
        Button,
        MainMenuMapSizeButton(size),
        Node {
            width: Val::Px(92.0),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
        Interaction::default(),
        children![(
            Text::new(size.menu_label()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.74)),
        )],
    )
}

pub(super) fn start_year_button(year: u32) -> impl Bundle {
    (
        Button,
        MainMenuStartYearButton(year),
        Node {
            width: Val::Px(46.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        Interaction::default(),
        children![(
            Text::new(year.to_string()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn seed_adjust_button(marker: impl Component, label: &str) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(36.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
        BorderColor::all(Color::srgb(0.55, 0.5, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn climate_button(climate: Climate) -> impl Bundle {
    (
        Button,
        MainMenuClimateButton(climate),
        Node {
            width: Val::Px(100.0),
            height: Val::Px(32.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
        Interaction::default(),
        children![(
            Text::new(climate_label(climate)),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.74)),
        )],
    )
}

pub(super) fn toggle_button(toggle: MainMenuToggle, label: &'static str) -> impl Bundle {
    (
        Button,
        toggle,
        Node {
            width: Val::Px(360.0),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
        BorderColor::all(Color::srgb(0.55, 0.5, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn primary_button(marker: impl Component, label: &str, height: f32) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        BorderColor::all(Color::srgb(0.7, 0.66, 0.5)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Hud.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.8)),
        )],
    )
}

pub(super) fn secondary_button(marker: impl Component, label: &str, height: f32) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.4)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.91, 0.88, 0.76)),
        )],
    )
}

pub(super) fn option_button_bg(selected: bool, interaction: Interaction) -> BackgroundColor {
    if selected {
        BackgroundColor(Color::srgb(0.48, 0.42, 0.28))
    } else if interaction == Interaction::Hovered {
        BackgroundColor(Color::srgb(0.36, 0.32, 0.22))
    } else {
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2))
    }
}

pub(super) fn toggle_button_bg(on: bool, interaction: Interaction) -> BackgroundColor {
    if on {
        BackgroundColor(Color::srgb(0.38, 0.44, 0.32))
    } else if interaction == Interaction::Hovered {
        BackgroundColor(Color::srgb(0.3, 0.28, 0.2))
    } else {
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17))
    }
}

pub(super) fn seed_button_bg(interaction: Interaction) -> BackgroundColor {
    match interaction {
        Interaction::Hovered => BackgroundColor(Color::srgb(0.32, 0.3, 0.22)),
        Interaction::Pressed => BackgroundColor(Color::srgb(0.38, 0.34, 0.24)),
        Interaction::None => BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
    }
}

pub(super) fn hover_primary(interaction: &Interaction, bg: &mut BackgroundColor) {
    match *interaction {
        Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.46, 0.42, 0.3)),
        Interaction::None => *bg = BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        Interaction::Pressed => {}
    }
}

pub(super) fn hover_secondary(interaction: &Interaction, bg: &mut BackgroundColor) {
    match *interaction {
        Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.34, 0.3, 0.22)),
        Interaction::None => *bg = BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        Interaction::Pressed => {}
    }
}
