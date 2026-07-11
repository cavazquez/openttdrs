//! Sistemas de menú: toggle, entradas, sync, Esc/teclado y clic externo.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::settings::ClientPreferences;
use crate::ui::display_options_window::DisplayOptionsWindowState;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::hud::SimHudControls;
use crate::ui::navigation::OpenUiRoute;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::{DragBuildState, MinimapLayerState, ToolbarState, UiToolState};

use super::chrome::{
    ENTRY_BG, ENTRY_BORDER, ENTRY_CHECKED, ENTRY_DISABLED, ENTRY_FOCUS_BORDER, ENTRY_HOVER,
    MENU_BORDER, ToolbarMenuEntry, ToolbarMenuEntryCheck, ToolbarMenuRoot, ToolbarNavigationButton,
};
use super::model::{MenuAction, MenuClientAction, MenuId};

#[derive(Resource, Default)]
pub(crate) struct ToolbarMenuState {
    pub(crate) open: Option<MenuId>,
    /// Índice de foco teclado entre entradas *enabled* del menú abierto.
    pub(crate) focus: Option<usize>,
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
        menu_state.focus = None;
        tool_state.block_map_click = true;
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_toolbar_menu_entries(
    entries: Query<(&Interaction, &ToolbarMenuEntry), (Changed<Interaction>, With<Button>)>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut tool_state: ResMut<UiToolState>,
    mut routes: MessageWriter<OpenUiRoute>,
    mut prefs: ResMut<ClientPreferences>,
    mut hud: ResMut<SimHudControls>,
    mut minimap: ResMut<MinimapLayerState>,
    mut display_options: ResMut<DisplayOptionsWindowState>,
    mut extra_viewport: ResMut<ExtraViewportWindowState>,
) {
    for (interaction, entry) in &entries {
        if *interaction != Interaction::Pressed || !entry.enabled {
            continue;
        }
        apply_menu_action(
            entry.action,
            &mut routes,
            &mut prefs,
            &mut hud,
            &mut minimap,
            &mut display_options,
            &mut extra_viewport,
        );
        menu_state.open = None;
        menu_state.focus = None;
        tool_state.block_map_click = true;
    }
}

fn apply_menu_action(
    action: MenuAction,
    routes: &mut MessageWriter<OpenUiRoute>,
    prefs: &mut ClientPreferences,
    hud: &mut SimHudControls,
    minimap: &mut MinimapLayerState,
    display_options: &mut DisplayOptionsWindowState,
    extra_viewport: &mut ExtraViewportWindowState,
) {
    match action {
        MenuAction::Route(route) => {
            routes.write(OpenUiRoute(route));
        }
        MenuAction::Client(MenuClientAction::ToggleMinimap) => {
            prefs.minimap_visible = !prefs.minimap_visible;
            hud.minimap_visible = prefs.minimap_visible;
        }
        MenuAction::Client(MenuClientAction::ExpandMinimap) => {
            minimap.expanded = !minimap.expanded;
            if minimap.expanded {
                prefs.minimap_visible = true;
                hud.minimap_visible = true;
            }
        }
        MenuAction::Client(MenuClientAction::OpenDisplayOptions) => {
            display_options.open = true;
        }
        MenuAction::Client(MenuClientAction::OpenExtraViewport) => {
            extra_viewport.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_toolbar_navigation_menu(
    menu_state: Res<ToolbarMenuState>,
    prefs: Res<ClientPreferences>,
    minimap: Res<MinimapLayerState>,
    mut visuals: ParamSet<(
        Query<(&ToolbarMenuRoot, &mut Node)>,
        Query<
            (
                &ToolbarNavigationButton,
                &Interaction,
                &mut BackgroundColor,
                &mut BorderColor,
            ),
            (
                With<Button>,
                With<ToolbarNavigationButton>,
                Without<ToolbarMenuEntry>,
            ),
        >,
        Query<
            (
                Entity,
                &ToolbarMenuEntry,
                &Interaction,
                &mut BackgroundColor,
                &mut BorderColor,
            ),
            (With<Button>, Without<ToolbarNavigationButton>),
        >,
        Query<&mut Text, With<ToolbarMenuEntryCheck>>,
    )>,
    children_q: Query<&Children>,
) {
    {
        let mut roots = visuals.p0();
        for (root, mut node) in &mut roots {
            node.display = if menu_state.open == Some(root.0) {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    {
        let mut buttons = visuals.p1();
        for (button, interaction, mut bg, mut border) in &mut buttons {
            let active = menu_state.open == Some(button.0);
            *bg = if active && *interaction == Interaction::Pressed {
                BackgroundColor(Color::srgb(0.78, 0.68, 0.43))
            } else if active && *interaction == Interaction::Hovered {
                BackgroundColor(Color::srgb(0.70, 0.61, 0.38))
            } else if active {
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
    }

    let open = menu_state.open;
    let mut enabled_index = 0usize;
    let mut check_updates: Vec<(Entity, bool)> = Vec::new();
    {
        let mut entries = visuals.p2();
        for (entity, entry, interaction, mut bg, mut border) in &mut entries {
            if open != Some(entry.menu) {
                continue;
            }
            let checked = entry.checkable && menu_entry_checked(entry.action, &prefs, &minimap);
            let is_focus = entry.enabled && menu_state.focus == Some(enabled_index);
            if entry.enabled {
                enabled_index = enabled_index.saturating_add(1);
            }

            *bg = if !entry.enabled {
                BackgroundColor(ENTRY_DISABLED)
            } else if *interaction == Interaction::Hovered {
                BackgroundColor(ENTRY_HOVER)
            } else if checked {
                BackgroundColor(ENTRY_CHECKED)
            } else {
                BackgroundColor(ENTRY_BG)
            };
            *border = BorderColor::all(if is_focus {
                ENTRY_FOCUS_BORDER
            } else {
                ENTRY_BORDER
            });

            if entry.checkable {
                check_updates.push((entity, checked));
            }
        }
    }

    let mut checks = visuals.p3();
    for (entity, checked) in check_updates {
        let Ok(children) = children_q.get(entity) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(mut text) = checks.get_mut(child) {
                **text = if checked { "✓" } else { " " }.to_string();
            }
        }
    }
}

fn menu_entry_checked(
    action: MenuAction,
    prefs: &ClientPreferences,
    minimap: &MinimapLayerState,
) -> bool {
    match action {
        MenuAction::Client(MenuClientAction::ToggleMinimap) => prefs.minimap_visible,
        MenuAction::Client(MenuClientAction::ExpandMinimap) => minimap.expanded,
        _ => false,
    }
}

pub(crate) fn dismiss_toolbar_menu_on_outside_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut tool_state: ResMut<UiToolState>,
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
        menu_state.focus = None;
        tool_state.block_map_click = true;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_toolbar_menu_keyboard(
    mut key_events: MessageReader<KeyboardInput>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut tool_state: ResMut<UiToolState>,
    entries: Query<&ToolbarMenuEntry>,
    mut routes: MessageWriter<OpenUiRoute>,
    mut prefs: ResMut<ClientPreferences>,
    mut hud: ResMut<SimHudControls>,
    mut minimap: ResMut<MinimapLayerState>,
    mut display_options: ResMut<DisplayOptionsWindowState>,
    mut extra_viewport: ResMut<ExtraViewportWindowState>,
) {
    let Some(open) = menu_state.open else {
        key_events.clear();
        return;
    };
    let enabled: Vec<MenuAction> = entries
        .iter()
        .filter(|e| e.menu == open && e.enabled)
        .map(|e| e.action)
        .collect();
    if enabled.is_empty() {
        key_events.clear();
        return;
    }
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Escape => {
                menu_state.open = None;
                menu_state.focus = None;
                tool_state.block_map_click = true;
            }
            Key::ArrowDown => {
                let next = menu_state.focus.map_or(0, |i| (i + 1) % enabled.len());
                menu_state.focus = Some(next);
            }
            Key::ArrowUp => {
                let next = menu_state.focus.map_or(enabled.len() - 1, |i| {
                    if i == 0 { enabled.len() - 1 } else { i - 1 }
                });
                menu_state.focus = Some(next);
            }
            Key::Enter => {
                if let Some(i) = menu_state.focus
                    && let Some(action) = enabled.get(i).copied()
                {
                    apply_menu_action(
                        action,
                        &mut routes,
                        &mut prefs,
                        &mut hud,
                        &mut minimap,
                        &mut display_options,
                        &mut extra_viewport,
                    );
                    menu_state.open = None;
                    menu_state.focus = None;
                    tool_state.block_map_click = true;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ui::navigation::UiRoute;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn sync_menu_queries_are_disjoint() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Map),
            focus: Some(0),
        });
        world.insert_resource(ClientPreferences {
            minimap_visible: false,
            ..Default::default()
        });
        world.insert_resource(MinimapLayerState::default());
        world.spawn((
            Button,
            ToolbarNavigationButton(MenuId::Map),
            Interaction::Hovered,
            BackgroundColor(ENTRY_BG),
            BorderColor::all(ENTRY_BORDER),
        ));
        let entry = world
            .spawn((
                Button,
                ToolbarMenuEntry {
                    menu: MenuId::Map,
                    action: MenuAction::Client(MenuClientAction::ToggleMinimap),
                    enabled: true,
                    checkable: true,
                },
                Interaction::None,
                BackgroundColor(ENTRY_BG),
                BorderColor::all(ENTRY_BORDER),
            ))
            .id();
        let check = world.spawn((ToolbarMenuEntryCheck, Text::new(" "))).id();
        world.entity_mut(entry).add_children(&[check]);
        world.spawn((
            ToolbarMenuRoot(MenuId::Map),
            Node {
                display: Display::None,
                ..default()
            },
        ));
        world.run_system_once(sync_toolbar_navigation_menu).unwrap();
        assert_eq!(
            world.entity(check).get::<Text>().map(|t| t.as_str()),
            Some(" ")
        );
    }

    #[test]
    fn menu_button_toggles_and_cancels_active_tool() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState::default());
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState {
            active_tool: Some(crate::ui::BuildMenuAction::Road),
            ..Default::default()
        });
        world.insert_resource(DragBuildState {
            armed: true,
            ..default()
        });
        world.spawn((
            Button,
            ToolbarNavigationButton(MenuId::World),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_toolbar_navigation_button)
            .unwrap();
        assert_eq!(
            world.resource::<ToolbarMenuState>().open,
            Some(MenuId::World)
        );
        assert!(world.resource::<UiToolState>().active_tool.is_none());
        assert!(!world.resource::<DragBuildState>().armed);
        assert!(world.resource::<UiToolState>().block_map_click);
    }

    #[test]
    fn outside_click_closes_open_menu() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::World),
            ..default()
        });
        world.insert_resource(UiToolState::default());
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world
            .run_system_once(dismiss_toolbar_menu_on_outside_click)
            .unwrap();
        assert!(world.resource::<ToolbarMenuState>().open.is_none());
        assert!(world.resource::<UiToolState>().block_map_click);
    }

    #[test]
    fn escape_closes_open_menu() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Map),
            focus: Some(0),
        });
        world.insert_resource(UiToolState::default());
        world.init_resource::<Messages<OpenUiRoute>>();
        world.init_resource::<Messages<KeyboardInput>>();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(MinimapLayerState::default());
        world.insert_resource(DisplayOptionsWindowState::default());
        world.insert_resource(ExtraViewportWindowState::default());
        world.spawn(ToolbarMenuEntry {
            menu: MenuId::Map,
            action: MenuAction::Client(MenuClientAction::ToggleMinimap),
            enabled: true,
            checkable: true,
        });
        world.write_message(KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        world.run_system_once(handle_toolbar_menu_keyboard).unwrap();
        assert!(world.resource::<ToolbarMenuState>().open.is_none());
        assert!(world.resource::<ToolbarMenuState>().focus.is_none());
        assert!(world.resource::<UiToolState>().block_map_click);
    }

    #[test]
    fn disabled_entry_does_not_fire_route() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Industries),
            ..default()
        });
        world.insert_resource(UiToolState::default());
        world.init_resource::<Messages<OpenUiRoute>>();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(MinimapLayerState::default());
        world.insert_resource(DisplayOptionsWindowState::default());
        world.insert_resource(ExtraViewportWindowState::default());
        world.spawn((
            Button,
            ToolbarMenuEntry {
                menu: MenuId::Industries,
                action: MenuAction::Route(UiRoute::LinkGraph),
                enabled: false,
                checkable: false,
            },
            Interaction::Pressed,
        ));
        world.run_system_once(handle_toolbar_menu_entries).unwrap();
        assert!(world.resource::<ToolbarMenuState>().open.is_some());
    }
}
