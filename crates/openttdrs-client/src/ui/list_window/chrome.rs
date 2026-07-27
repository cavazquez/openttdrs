//! Chrome visual compartido: chips de sort, scroll y filas.

use bevy::prelude::*;

use crate::ui::floating_window::{WINDOW_TEXT, window_text_font};
use crate::ui::font::UiFontRole;
use crate::ui::scrollbar::spawn_classic_scroll_area;
use crate::ui::toolbar::BuildMenuUi;

pub(crate) const LIST_BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
pub(crate) const LIST_BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
pub(crate) const LIST_BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
pub(crate) const LIST_BTN_BORDER: Color = Color::srgb(0.58, 0.50, 0.33);
pub(crate) const LIST_DEFAULT_HEIGHT: f32 = 330.0;
pub(crate) const LIST_ROW_MIN_HEIGHT: f32 = 28.0;

/// Chip de ordenación con marcador de componente `M`.
pub(crate) fn spawn_list_sort_button<M: Component>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    marker: M,
    min_width: f32,
) {
    parent.spawn((
        Button,
        marker,
        Node {
            min_width: Val::Px(min_width),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(LIST_BTN_BG),
        BorderColor::all(LIST_BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
}

/// Campo de búsqueda editable (marcador `M`).
pub(crate) fn spawn_list_filter_input<M: Component>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    marker: M,
    placeholder: &str,
) {
    parent.spawn((
        marker,
        bevy::text::EditableText::new(""),
        Text::new(placeholder),
        window_text_font(asset_server, UiFontRole::Caption),
        TextColor(Color::srgb(0.75, 0.72, 0.62)),
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            margin: UiRect::bottom(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.18, 0.15, 0.10)),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        BuildMenuUi,
    ));
}

/// Área con scroll + raíz de filas (`list_root_marker`).
pub(crate) fn spawn_list_scroll_area(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    list_root_marker: impl Bundle,
    height: f32,
) {
    spawn_classic_scroll_area(
        parent,
        asset_server,
        list_root_marker,
        height,
        Color::srgb(0.22, 0.18, 0.12),
        Color::srgb(0.45, 0.39, 0.27),
    );
}

/// Color de chip: activo / hover / idle.
#[must_use]
pub(crate) fn list_chip_bg(active: bool, interaction: Interaction) -> BackgroundColor {
    if active {
        BackgroundColor(LIST_BTN_ACTIVE)
    } else if interaction == Interaction::Hovered {
        BackgroundColor(LIST_BTN_HOVER)
    } else {
        BackgroundColor(LIST_BTN_BG)
    }
}

/// Colorea chips de sort: activo / hover / idle.
pub(crate) fn sync_list_sort_colors<M: Component + Copy + PartialEq>(
    buttons: &mut Query<(&M, &Interaction, &mut BackgroundColor), With<Button>>,
    active: M,
) {
    for (button, interaction, mut bg) in buttons.iter_mut() {
        *bg = list_chip_bg(*button == active, *interaction);
    }
}

pub(crate) fn clear_list_children(
    commands: &mut Commands,
    list_root: Entity,
    children_q: &Query<&Children>,
) {
    if let Ok(children) = children_q.get(list_root) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}

pub(crate) fn spawn_list_empty_label(
    list: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    message: &str,
) {
    list.spawn((
        Text::new(message),
        window_text_font(asset_server, UiFontRole::Caption),
        TextColor(WINDOW_TEXT),
        Node {
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
    ));
}

/// Fila clicable estándar de directorio.
pub(crate) fn spawn_list_row_button(
    list: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: String,
    row_marker: impl Bundle,
    selected: bool,
) {
    list.spawn((
        Button,
        row_marker,
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(LIST_ROW_MIN_HEIGHT),
            padding: UiRect::horizontal(Val::Px(7.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(if selected {
            LIST_BTN_ACTIVE
        } else {
            LIST_BTN_BG
        }),
        BorderColor::all(Color::srgb(0.50, 0.44, 0.30)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
}
