//! Spawn de anclas y popovers desde [`MenuSpec`].

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::state::ingame_lifecycle::InGameUi;
use crate::ui::floating_window::{WINDOW_TEXT, window_text_font};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

use super::model::{
    MenuAction, MenuEntryKind, MenuEntrySpec, MenuId, MenuSpec, all_toolbar_menu_specs,
};

pub(crate) const ANCHOR_W: f32 = 78.0;
pub(crate) const ANCHOR_H: f32 = 48.0;
pub(crate) const MENU_MIN_WIDTH: f32 = 228.0;

/// Fondo del popover (alineado con scroll de listas).
pub(crate) const MENU_BG: Color = Color::srgb(0.22, 0.18, 0.12);
/// Borde exterior oscuro (marco tipo ventana).
pub(crate) const MENU_OUTER_BORDER: Color = Color::srgb(0.13, 0.10, 0.07);
/// Borde interior / anclas idle.
pub(crate) const MENU_BORDER: Color = Color::srgb(0.68, 0.61, 0.42);
pub(crate) const ENTRY_BG: Color = Color::srgb(0.36, 0.31, 0.21);
pub(crate) const ENTRY_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
pub(crate) const ENTRY_DISABLED: Color = Color::srgb(0.26, 0.23, 0.17);
/// Estado checked (toggle activo), sin confundir con foco teclado.
pub(crate) const ENTRY_CHECKED: Color = Color::srgb(0.50, 0.44, 0.28);
pub(crate) const ENTRY_FOCUS_BORDER: Color = Color::srgb(0.92, 0.82, 0.48);
pub(crate) const ENTRY_BORDER: Color = Color::srgb(0.48, 0.41, 0.28);
pub(crate) const ENTRY_TEXT: Color = WINDOW_TEXT;
pub(crate) const ENTRY_TEXT_DISABLED: Color = Color::srgb(0.55, 0.52, 0.45);
pub(crate) const HOTKEY_TEXT: Color = Color::srgb(0.72, 0.68, 0.55);
pub(crate) const DIVIDER: Color = Color::srgb(0.50, 0.44, 0.30);

#[derive(Component, Clone, Copy)]
pub(crate) struct ToolbarNavigationButton(pub(crate) MenuId);

#[derive(Component, Clone, Copy)]
pub(crate) struct ToolbarMenuRoot(pub(crate) MenuId);

#[derive(Component, Clone, Copy)]
pub(crate) struct ToolbarMenuEntry {
    pub menu: MenuId,
    pub action: MenuAction,
    pub enabled: bool,
    pub checkable: bool,
}

#[derive(Component)]
pub(crate) struct ToolbarMenuEntryLabel;

#[derive(Component)]
pub(crate) struct ToolbarMenuEntryCheck;

#[derive(Component)]
pub(crate) struct ToolbarMenuEntryHotkey;

#[derive(Component)]
pub(crate) struct ToolbarMenuDivider;

/// Ancla + popover anidado debajo del botón (sin offsets mágicos globales).
pub(crate) fn spawn_menu_anchor_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    id: MenuId,
) {
    let Some(spec) = all_toolbar_menu_specs().iter().find(|s| s.id == id) else {
        return;
    };
    parent
        .spawn((
            Node {
                width: Val::Px(ANCHOR_W),
                height: Val::Px(ANCHOR_H),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BuildMenuUi,
            ZIndex(1),
        ))
        .with_children(|wrap| {
            wrap.spawn((
                Button,
                ToolbarNavigationButton(id),
                RelativeCursorPosition::default(),
                BuildMenuUi,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.33, 0.28, 0.19)),
                BorderColor::all(MENU_BORDER),
                Interaction::default(),
                children![(
                    Text::new(id.label()),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(ENTRY_TEXT),
                )],
            ));
            spawn_menu_from_spec(wrap, asset_server, spec);
        });
}

pub(crate) fn spawn_menu_from_spec(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    spec: &MenuSpec,
) {
    let align_end = matches!(spec.id, MenuId::Fleet | MenuId::Economy);
    parent
        .spawn((
            ToolbarMenuRoot(spec.id),
            RelativeCursorPosition::default(),
            InGameUi,
            BuildMenuUi,
            Node {
                min_width: Val::Px(MENU_MIN_WIDTH),
                padding: UiRect::all(Val::Px(5.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                position_type: PositionType::Absolute,
                top: Val::Px(ANCHOR_H + 2.0),
                left: if align_end { Val::Auto } else { Val::Px(0.0) },
                right: if align_end { Val::Px(0.0) } else { Val::Auto },
                ..default()
            },
            BackgroundColor(MENU_BG),
            BorderColor::all(MENU_OUTER_BORDER),
            Interaction::default(),
            GlobalZIndex(2150),
        ))
        .with_children(|menu| {
            for entry in spec.entries {
                match entry.kind {
                    MenuEntryKind::Divider => spawn_divider(menu),
                    MenuEntryKind::Action => spawn_menu_entry(menu, asset_server, spec.id, entry),
                }
            }
        });
}

fn spawn_divider(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        ToolbarMenuDivider,
        BuildMenuUi,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(DIVIDER),
    ));
}

fn spawn_menu_entry(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    menu: MenuId,
    entry: &MenuEntrySpec,
) {
    let Some(action) = entry.action else {
        return;
    };
    let text_color = if entry.enabled {
        ENTRY_TEXT
    } else {
        ENTRY_TEXT_DISABLED
    };
    parent
        .spawn((
            Button,
            ToolbarMenuEntry {
                menu,
                action,
                enabled: entry.enabled,
                checkable: entry.checkable,
            },
            BuildMenuUi,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(28.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                column_gap: Val::Px(6.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if entry.enabled {
                ENTRY_BG
            } else {
                ENTRY_DISABLED
            }),
            BorderColor::all(ENTRY_BORDER),
            Interaction::default(),
        ))
        .with_children(|row| {
            row.spawn((
                ToolbarMenuEntryCheck,
                Text::new(if entry.checkable { " " } else { "" }),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(text_color),
                Node {
                    width: Val::Px(14.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            row.spawn((
                ToolbarMenuEntryLabel,
                Text::new(entry.label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(text_color),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            if let Some(hk) = entry.hotkey {
                row.spawn((
                    ToolbarMenuEntryHotkey,
                    Text::new(hk),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(if entry.enabled {
                        HOTKEY_TEXT
                    } else {
                        ENTRY_TEXT_DISABLED
                    }),
                ));
            }
        });
}
