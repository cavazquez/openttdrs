use crate::ui::interaction::{UiButton, UiInteraction};
use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::{PopulationDensity, TerrainRoughness};
use crate::ui::font::UiFontRole;
use crate::ui::hud::UiClickBeep;

use super::labels::climate_label;
use super::{
    MainMenuClimateButton, MainMenuDensityButton, MainMenuDensityTarget, MainMenuDynamicText,
    MainMenuLocalizedText, MainMenuMapSizeButton, MainMenuRoughnessButton, MainMenuStartYearButton,
    MainMenuStartingMoneyButton, MainMenuToggle,
};

pub(super) fn density_button(
    density: PopulationDensity,
    target: MainMenuDensityTarget,
) -> impl Bundle {
    (
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
        children![(
            MainMenuDynamicText::Density(density),
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
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
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

pub(super) fn roughness_button(roughness: TerrainRoughness) -> impl Bundle {
    (
        UiButton,
        UiClickBeep,
        MainMenuRoughnessButton(roughness),
        Node {
            width: Val::Px(96.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        UiInteraction::default(),
        children![(
            MainMenuDynamicText::Roughness(roughness),
            Text::new(roughness.menu_label()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn map_size_button(btn: MainMenuMapSizeButton) -> impl Bundle {
    let (label, width) = match btn {
        MainMenuMapSizeButton::Compact => ("Demo 64×64".to_string(), 116.0),
        MainMenuMapSizeButton::Width(axis) | MainMenuMapSizeButton::Height(axis) => {
            (axis.menu_label().to_string(), 56.0)
        }
    };
    (
        UiButton,
        UiClickBeep,
        btn,
        Node {
            width: Val::Px(width),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
        UiInteraction::default(),
        children![(
            Text::new(label),
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
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
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
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
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
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
        children![(
            MainMenuDynamicText::Climate(climate),
            Text::new(climate_label(climate)),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.74)),
        )],
    )
}

pub(super) fn toggle_button(
    toggle: MainMenuToggle,
    label: &'static str,
    width: f32,
) -> impl Bundle {
    (
        UiButton,
        UiClickBeep,
        toggle,
        Node {
            width: Val::Px(width),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
        BorderColor::all(Color::srgb(0.55, 0.5, 0.38)),
        UiInteraction::default(),
        children![(
            MainMenuLocalizedText(label),
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

pub(super) fn primary_button(
    marker: impl Component,
    label: &'static str,
    height: f32,
) -> impl Bundle {
    (
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
        children![(
            MainMenuLocalizedText(label),
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Hud.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.8)),
        )],
    )
}

pub(super) fn secondary_button(
    marker: impl Component,
    label: &'static str,
    height: f32,
) -> impl Bundle {
    (
        UiButton,
        UiClickBeep,
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
        UiInteraction::default(),
        children![(
            MainMenuLocalizedText(label),
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.91, 0.88, 0.76)),
        )],
    )
}

pub(super) fn option_button_bg(selected: bool, interaction: UiInteraction) -> BackgroundColor {
    if selected {
        BackgroundColor(Color::srgb(0.48, 0.42, 0.28))
    } else if interaction == UiInteraction::Hovered {
        BackgroundColor(Color::srgb(0.36, 0.32, 0.22))
    } else {
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2))
    }
}

pub(super) fn toggle_button_bg(on: bool, interaction: UiInteraction) -> BackgroundColor {
    if on {
        BackgroundColor(Color::srgb(0.38, 0.44, 0.32))
    } else if interaction == UiInteraction::Hovered {
        BackgroundColor(Color::srgb(0.3, 0.28, 0.2))
    } else {
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17))
    }
}

pub(super) fn seed_button_bg(interaction: UiInteraction) -> BackgroundColor {
    match interaction {
        UiInteraction::Hovered => BackgroundColor(Color::srgb(0.32, 0.3, 0.22)),
        UiInteraction::Pressed => BackgroundColor(Color::srgb(0.38, 0.34, 0.24)),
        UiInteraction::None => BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
    }
}

pub(super) fn hover_primary(interaction: &UiInteraction, bg: &mut BackgroundColor) {
    match *interaction {
        UiInteraction::Hovered => *bg = BackgroundColor(Color::srgb(0.46, 0.42, 0.3)),
        UiInteraction::None => *bg = BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        UiInteraction::Pressed => {}
    }
}

pub(super) fn hover_secondary(interaction: &UiInteraction, bg: &mut BackgroundColor) {
    match *interaction {
        UiInteraction::Hovered => *bg = BackgroundColor(Color::srgb(0.34, 0.3, 0.22)),
        UiInteraction::None => *bg = BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        UiInteraction::Pressed => {}
    }
}
