//! Sistemas de menú: toggle, entradas, sync, Esc/teclado y clic externo.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::settings::ClientPreferences;
use crate::ui::display_options_window::DisplayOptionsWindowState;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::hotkeys::{UiCommandId, UiHotkeys};
use crate::ui::hud::SimHudControls;
use crate::ui::navigation::OpenUiRoute;
use crate::ui::toolbar::{MinimapLayerState, UiToolState};

use super::chrome::{
    ENTRY_BG, ENTRY_BORDER, ENTRY_CHECKED, ENTRY_DISABLED, ENTRY_FOCUS_BORDER, ENTRY_HOVER,
    MENU_BORDER, ToolbarMenuEntry, ToolbarMenuEntryCheck, ToolbarMenuRoot, ToolbarNavigationButton,
};
use super::model::{MenuAction, MenuClientAction, MenuId, ToolbarContext};

pub(crate) fn refresh_toolbar_context(
    sim: Option<Res<crate::state::SimWorld>>,
    network: Option<Res<crate::network::NetworkRuntime>>,
    mut context: ResMut<ToolbarContext>,
) {
    let Some(sim) = sim else {
        return;
    };
    *context = ToolbarContext {
        has_companies: !sim.state.companies.is_empty(),
        has_goals: !sim.state.gs.goals.is_empty(),
        has_story: !sim.state.gs.story_pages.is_empty(),
        can_control_simulation: network
            .is_none_or(|network| network.role() != crate::network::NetworkRole::Client),
    };
}

#[derive(Resource, Default)]
pub(crate) struct ToolbarMenuState {
    pub(crate) open: Option<MenuId>,
    /// Índice de foco teclado entre entradas *enabled* del menú abierto.
    pub(crate) focus: Option<usize>,
    /// El botón izquierdo sigue pulsado desde el ancla (press/drag/release).
    pub(crate) pointer_capture: bool,
}

pub(crate) fn handle_toolbar_navigation_button(
    buttons: Query<(&Interaction, &ToolbarNavigationButton), (Changed<Interaction>, With<Button>)>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut tool_state: ResMut<UiToolState>,
    prefs: Res<ClientPreferences>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Hovered && menu_state.pointer_capture {
            menu_state.open = Some(button.0);
            menu_state.focus = None;
            continue;
        }
        if *interaction != Interaction::Pressed {
            continue;
        }
        menu_state.open = if menu_state.open == Some(button.0) {
            None
        } else {
            Some(button.0)
        };
        menu_state.focus = None;
        menu_state.pointer_capture = prefs.toolbar_dropdown_autoselect && menu_state.open.is_some();
        // Navigation is orthogonal to map placement. Looking at a directory or
        // graph must not discard an armed drag or the currently selected tool.
        // We only consume the pointer press that opened/closed the popover so it
        // cannot leak through to the map in the same frame.
        tool_state.block_map_click = true;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_toolbar_menu_entries(
    entries: Query<(&Interaction, &ToolbarMenuEntry), With<Button>>,
    mouse: Res<ButtonInput<MouseButton>>,
    context: Res<ToolbarContext>,
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
        let click_select = *interaction == Interaction::Pressed && !menu_state.pointer_capture;
        let drag_release_select = menu_state.pointer_capture
            && prefs.toolbar_dropdown_autoselect
            && mouse.just_released(MouseButton::Left)
            && *interaction == Interaction::Hovered
            && menu_state.open == Some(entry.menu);
        if (!click_select && !drag_release_select)
            || !entry.enabled
            || !context.allows(entry.availability)
        {
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
        menu_state.pointer_capture = false;
        tool_state.block_map_click = true;
        return;
    }
    if mouse.just_released(MouseButton::Left) {
        menu_state.pointer_capture = false;
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
    context: Res<ToolbarContext>,
    hotkeys: Option<Res<UiHotkeys>>,
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
        Query<
            &mut TextColor,
            Or<(
                With<super::chrome::ToolbarMenuEntryLabel>,
                With<ToolbarMenuEntryCheck>,
                With<super::chrome::ToolbarMenuEntryHotkey>,
            )>,
        >,
        Query<(&super::chrome::ToolbarMenuEntryHotkey, &mut Text)>,
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
    let mut text_updates: Vec<(Entity, bool)> = Vec::new();
    {
        let mut entries = visuals.p2();
        for (entity, entry, interaction, mut bg, mut border) in &mut entries {
            if open != Some(entry.menu) {
                continue;
            }
            let enabled = entry.enabled && context.allows(entry.availability);
            let checked = entry.checkable && menu_entry_checked(entry.action, &prefs, &minimap);
            let is_focus = enabled && menu_state.focus == Some(enabled_index);
            if enabled {
                enabled_index = enabled_index.saturating_add(1);
            }

            *bg = if !enabled {
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
            text_updates.push((entity, enabled));
        }
    }

    {
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

    {
        let mut text_colors = visuals.p4();
        for (entity, enabled) in text_updates {
            let Ok(children) = children_q.get(entity) else {
                continue;
            };
            for child in children.iter() {
                if let Ok(mut color) = text_colors.get_mut(child) {
                    *color = TextColor(if enabled {
                        super::chrome::ENTRY_TEXT
                    } else {
                        super::chrome::ENTRY_TEXT_DISABLED
                    });
                }
            }
        }
    }

    let mut hotkey_texts = visuals.p5();
    for (marker, mut text) in &mut hotkey_texts {
        let label = command_for_menu_action(marker.0)
            .and_then(|command| hotkeys.as_deref()?.label(command))
            .unwrap_or_default();
        if **text != label {
            **text = label;
        }
    }
}

fn command_for_menu_action(action: MenuAction) -> Option<UiCommandId> {
    use crate::ui::navigation::UiRoute;
    Some(match action {
        MenuAction::Client(MenuClientAction::ToggleMinimap) => UiCommandId::SmallMap,
        MenuAction::Client(MenuClientAction::OpenExtraViewport) => UiCommandId::ExtraViewport,
        MenuAction::Route(UiRoute::Towns) => UiCommandId::TownDirectory,
        MenuAction::Route(UiRoute::Stations) => UiCommandId::StationList,
        MenuAction::Route(UiRoute::Industries) => UiCommandId::IndustryDirectory,
        MenuAction::Route(UiRoute::Finances) => UiCommandId::Finances,
        MenuAction::Route(UiRoute::League) => UiCommandId::League,
        MenuAction::Route(UiRoute::SoundMusic) => UiCommandId::Music,
        MenuAction::Route(UiRoute::Help) => UiCommandId::Help,
        MenuAction::Route(UiRoute::Cheats) => UiCommandId::Cheats,
        MenuAction::Route(UiRoute::SaveGame | UiRoute::LoadGame) => UiCommandId::SaveLoad,
        _ => return None,
    })
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
        menu_state.pointer_capture = false;
        tool_state.block_map_click = true;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_toolbar_menu_keyboard(
    mut key_events: MessageReader<KeyboardInput>,
    mut menu_state: ResMut<ToolbarMenuState>,
    mut tool_state: ResMut<UiToolState>,
    entries: Query<&ToolbarMenuEntry>,
    context: Res<ToolbarContext>,
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
        .filter(|e| e.menu == open && e.enabled && context.allows(e.availability))
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
                menu_state.pointer_capture = false;
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
                    menu_state.pointer_capture = false;
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
    use crate::ui::toolbar::DragBuildState;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn sync_menu_queries_are_disjoint() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Map),
            focus: Some(0),
            pointer_capture: false,
        });
        world.insert_resource(ClientPreferences {
            minimap_visible: false,
            ..Default::default()
        });
        world.insert_resource(MinimapLayerState::default());
        world.insert_resource(ToolbarContext::default());
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
                    availability: super::super::model::MenuAvailability::Always,
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
    fn menu_button_toggles_without_cancelling_active_placement() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState::default());
        world.insert_resource(ClientPreferences::default());
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
        assert_eq!(
            world.resource::<UiToolState>().active_tool,
            Some(crate::ui::BuildMenuAction::Road)
        );
        assert!(world.resource::<DragBuildState>().armed);
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
            pointer_capture: false,
        });
        world.insert_resource(UiToolState::default());
        world.init_resource::<Messages<OpenUiRoute>>();
        world.init_resource::<Messages<KeyboardInput>>();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(MinimapLayerState::default());
        world.insert_resource(ToolbarContext::default());
        world.insert_resource(DisplayOptionsWindowState::default());
        world.insert_resource(ExtraViewportWindowState::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.spawn(ToolbarMenuEntry {
            menu: MenuId::Map,
            action: MenuAction::Client(MenuClientAction::ToggleMinimap),
            enabled: true,
            availability: super::super::model::MenuAvailability::Always,
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
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(ToolbarContext::default());
        world.spawn((
            Button,
            ToolbarMenuEntry {
                menu: MenuId::Industries,
                action: MenuAction::Route(UiRoute::LinkGraph),
                enabled: false,
                availability: super::super::model::MenuAvailability::Always,
                checkable: false,
            },
            Interaction::Pressed,
        ));
        world.run_system_once(handle_toolbar_menu_entries).unwrap();
        assert!(world.resource::<ToolbarMenuState>().open.is_some());
    }

    #[test]
    fn unavailable_entry_does_not_fire_route() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Economy),
            ..default()
        });
        world.insert_resource(UiToolState::default());
        world.init_resource::<Messages<OpenUiRoute>>();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(MinimapLayerState::default());
        world.insert_resource(DisplayOptionsWindowState::default());
        world.insert_resource(ExtraViewportWindowState::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(ToolbarContext {
            has_goals: false,
            ..default()
        });
        world.spawn((
            Button,
            ToolbarMenuEntry {
                menu: MenuId::Economy,
                action: MenuAction::Route(UiRoute::Goals),
                enabled: true,
                availability: super::super::model::MenuAvailability::HasGoals,
                checkable: false,
            },
            Interaction::Pressed,
        ));

        world.run_system_once(handle_toolbar_menu_entries).unwrap();

        assert_eq!(
            world.resource::<ToolbarMenuState>().open,
            Some(MenuId::Economy)
        );
        assert!(world.resource::<Messages<OpenUiRoute>>().is_empty());
    }

    #[test]
    fn captured_pointer_switches_to_hovered_anchor() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Map),
            pointer_capture: true,
            ..default()
        });
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(UiToolState::default());
        world.spawn((
            Button,
            ToolbarNavigationButton(MenuId::World),
            Interaction::Hovered,
        ));

        world
            .run_system_once(handle_toolbar_navigation_button)
            .unwrap();

        assert_eq!(
            world.resource::<ToolbarMenuState>().open,
            Some(MenuId::World)
        );
        assert!(world.resource::<ToolbarMenuState>().pointer_capture);
    }

    #[test]
    fn autoselect_executes_hovered_entry_on_pointer_release() {
        let mut world = World::new();
        world.insert_resource(ToolbarMenuState {
            open: Some(MenuId::Map),
            pointer_capture: true,
            ..default()
        });
        world.insert_resource(UiToolState::default());
        world.init_resource::<Messages<OpenUiRoute>>();
        world.insert_resource(ClientPreferences {
            minimap_visible: false,
            toolbar_dropdown_autoselect: true,
            ..default()
        });
        world.insert_resource(SimHudControls::default());
        world.insert_resource(MinimapLayerState::default());
        world.insert_resource(DisplayOptionsWindowState::default());
        world.insert_resource(ExtraViewportWindowState::default());
        world.insert_resource(ToolbarContext::default());
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        mouse.clear();
        mouse.release(MouseButton::Left);
        world.insert_resource(mouse);
        world.spawn((
            Button,
            ToolbarMenuEntry {
                menu: MenuId::Map,
                action: MenuAction::Client(MenuClientAction::ToggleMinimap),
                enabled: true,
                availability: super::super::model::MenuAvailability::Always,
                checkable: true,
            },
            Interaction::Hovered,
        ));

        world.run_system_once(handle_toolbar_menu_entries).unwrap();

        assert!(world.resource::<ClientPreferences>().minimap_visible);
        assert!(world.resource::<ToolbarMenuState>().open.is_none());
        assert!(!world.resource::<ToolbarMenuState>().pointer_capture);
    }
}
