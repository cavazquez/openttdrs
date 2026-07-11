//! Navegación tipada y popover reutilizable de la toolbar.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::state::ingame_lifecycle::InGameUi;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::{BuildMenuUi, DragBuildState, ToolbarState, UiToolState};

const MENU_BG: Color = Color::srgb(0.22, 0.18, 0.12);
const MENU_BORDER: Color = Color::srgb(0.68, 0.61, 0.42);
const ENTRY_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const ENTRY_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);

/// Destinos navegables desde toolbar/menús.
///
/// Solo se añaden variantes cuando existe un consumidor real para evitar
/// repetir el problema de ventanas registradas pero inalcanzables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiRoute {
    TownDirectory,
}

/// Petición tipada para abrir una superficie UI.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct OpenUiRoute(pub(crate) UiRoute);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarMenuKind {
    World,
}

#[derive(Resource, Default)]
pub(crate) struct ToolbarMenuState {
    pub(crate) open: Option<ToolbarMenuKind>,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct ToolbarNavigationButton(pub(crate) ToolbarMenuKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct ToolbarMenuRoot(ToolbarMenuKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct ToolbarMenuEntry(UiRoute);

/// Botón textual de navegación global dentro de la barra superior.
pub(crate) fn spawn_world_navigation_button(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Button,
        ToolbarNavigationButton(ToolbarMenuKind::World),
        RelativeCursorPosition::default(),
        BuildMenuUi,
        Node {
            width: Val::Px(72.0),
            height: Val::Px(48.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.33, 0.28, 0.19)),
        BorderColor::all(MENU_BORDER),
        Interaction::default(),
        children![(
            Text::new("Mundo"),
            TextFont {
                font_size: FontSize::Rem(0.8),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.78)),
        )],
    ));
}

/// Popover de navegación. Es hijo del root centrado de toolbar y aparece justo
/// debajo de la fila principal; futuras categorías reutilizan el mismo patrón.
pub(crate) fn spawn_toolbar_navigation_menus(
    root: &mut ChildSpawnerCommands,
    _asset_server: &AssetServer,
) {
    root.spawn((
        ToolbarMenuRoot(ToolbarMenuKind::World),
        RelativeCursorPosition::default(),
        InGameUi,
        BuildMenuUi,
        Node {
            width: Val::Px(220.0),
            padding: UiRect::all(Val::Px(4.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(MENU_BG),
        BorderColor::all(MENU_BORDER),
        Interaction::default(),
        GlobalZIndex(2150),
    ))
    .with_children(|menu| {
        spawn_menu_entry(menu, "Directorio de pueblos", UiRoute::TownDirectory);
    });
}

fn spawn_menu_entry(parent: &mut ChildSpawnerCommands, label: &str, route: UiRoute) {
    parent.spawn((
        Button,
        ToolbarMenuEntry(route),
        BuildMenuUi,
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(26.0),
            padding: UiRect::horizontal(Val::Px(7.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(ENTRY_BG),
        BorderColor::all(Color::srgb(0.48, 0.41, 0.28)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(0.72),
                ..default()
            },
            TextColor(Color::srgb(0.94, 0.90, 0.76)),
        )],
    ));
}

pub(crate) fn handle_toolbar_navigation_button(
    buttons: Query<(&Interaction, &ToolbarNavigationButton), (Changed<Interaction>, With<Button>)>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        menu_state.open = if menu_state.open == Some(button.0) {
            None
        } else {
            Some(button.0)
        };
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

pub(crate) fn handle_toolbar_menu_entries(
    entries: Query<(&Interaction, &ToolbarMenuEntry), (Changed<Interaction>, With<Button>)>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut routes: MessageWriter<OpenUiRoute>,
) {
    for (interaction, entry) in &entries {
        if *interaction != Interaction::Pressed {
            continue;
        }
        routes.write(OpenUiRoute(entry.0));
        menu_state.open = None;
    }
}

pub(crate) fn sync_toolbar_navigation_menu(
    menu_state: Res<ToolbarMenuState>,
    mut roots: Query<(&ToolbarMenuRoot, &mut Node)>,
    mut buttons: Query<
        (
            &ToolbarNavigationButton,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut entries: Query<(&Interaction, &mut BackgroundColor), With<ToolbarMenuEntry>>,
) {
    for (root, mut node) in &mut roots {
        node.display = if menu_state.open == Some(root.0) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (button, interaction, mut bg, mut border) in &mut buttons {
        let active = menu_state.open == Some(button.0);
        *bg = if active {
            BackgroundColor(Color::srgb(0.62, 0.54, 0.34))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.42, 0.36, 0.24))
        } else {
            BackgroundColor(Color::srgb(0.33, 0.28, 0.19))
        };
        *border = BorderColor::all(if active {
            Color::srgb(0.86, 0.76, 0.5)
        } else {
            MENU_BORDER
        });
    }
    for (interaction, mut bg) in &mut entries {
        *bg = if *interaction == Interaction::Hovered {
            BackgroundColor(ENTRY_HOVER)
        } else {
            BackgroundColor(ENTRY_BG)
        };
    }
}

/// Cierra el popover al hacer clic fuera de él y de sus botones.
pub(crate) fn dismiss_toolbar_menu_on_outside_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut menu_state: ResMut<ToolbarMenuState>,
    menu_roots: Query<(&ToolbarMenuRoot, &RelativeCursorPosition)>,
    buttons: Query<(&ToolbarNavigationButton, &RelativeCursorPosition)>,
) {
    let Some(open) = menu_state.open else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let over_menu = menu_roots
        .iter()
        .any(|(root, cursor)| root.0 == open && cursor.normalized.is_some());
    let over_button = buttons
        .iter()
        .any(|(button, cursor)| button.0 == open && cursor.normalized.is_some());
    if !over_menu && !over_button {
        menu_state.open = None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn menu_button_toggles_and_cancels_active_tool() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState::default());
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState {
            active_tool: Some(crate::ui::BuildMenuAction::Road),
        });
        world.insert_resource(DragBuildState {
            armed: true,
            ..default()
        });
        world.spawn((
            Button,
            ToolbarNavigationButton(ToolbarMenuKind::World),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_toolbar_navigation_button)
            .unwrap();
        assert_eq!(
            world.resource::<ToolbarMenuState>().open,
            Some(ToolbarMenuKind::World)
        );
        assert!(world.resource::<UiToolState>().active_tool.is_none());
        assert!(!world.resource::<DragBuildState>().armed);
    }

    #[test]
    fn outside_click_closes_open_menu() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(ToolbarMenuKind::World),
        });
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world
            .run_system_once(dismiss_toolbar_menu_on_outside_click)
            .unwrap();
        assert!(world.resource::<ToolbarMenuState>().open.is_none());
    }
}
