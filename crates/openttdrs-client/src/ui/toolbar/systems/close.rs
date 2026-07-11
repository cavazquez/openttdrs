use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::state::{ClientScreen, OrderPickState, SuspendedGameSession, order_pick_active};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, close_top_visible_floating_window,
};
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::main_menu::return_to_main_menu;
use crate::ui::navigation::ToolbarMenuState;
use crate::ui::save_window::SaveWindowState;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::{
    DragBuildState, MinimapLayerState, OrderEditState, RoadTypeEscapeConsumed, ToolbarCloseButton,
    ToolbarState, UiToolState,
};

#[derive(SystemParam)]
pub(crate) struct InGameEscOverlays<'w> {
    navigation_menu: Option<ResMut<'w, ToolbarMenuState>>,
    minimap_layers: Option<ResMut<'w, MinimapLayerState>>,
    road_type_escape: Res<'w, RoadTypeEscapeConsumed>,
}

/// Hay herramienta, panel o modo de colocación activo que Esc debe cancelar primero.
fn ingame_placement_busy(
    save_window: &SaveWindowState,
    tool_state: &UiToolState,
    toolbar_state: &ToolbarState,
    pick_state: &State<OrderPickState>,
    order_state: &OrderEditState,
    industry_panel: &IndustryPanelState,
) -> bool {
    save_window.open
        || tool_state.active_tool.is_some()
        || toolbar_state.active_group.is_some()
        || order_pick_active(pick_state)
        || order_state.vehicle_id.is_some()
        || industry_panel.open
}

fn cancel_ingame_placement(
    toolbar_state: &mut ToolbarState,
    tool_state: &mut UiToolState,
    drag_state: &mut DragBuildState,
    order_state: &mut OrderEditState,
    next_pick: &mut NextState<OrderPickState>,
    industry_panel: &mut IndustryPanelState,
    station_state: &mut crate::ui::toolbar::StationBuildState,
) {
    toolbar_state.active_group = None;
    tool_state.active_tool = None;
    station_state.join_keep = None;
    cancel_placement(drag_state);
    order_state.clear();
    next_pick.set(OrderPickState::Idle);
    industry_panel.open = false;
    industry_panel.focus_tile = None;
}

/// **Esc** cancela herramienta/panel activo; cierra ventanas flotantes; si no hay ninguno, vuelve al menú.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_ingame_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    save_window: Res<SaveWindowState>,
    pick_state: Res<State<OrderPickState>>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut industry_panel: ResMut<IndustryPanelState>,
    mut station_state: ResMut<crate::ui::toolbar::StationBuildState>,
    mut windows_q: Query<(&FloatingWindow, &GlobalZIndex, &mut Visibility)>,
    mut closed: MessageWriter<FloatingWindowClosed>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    mut overlays: InGameEscOverlays,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if overlays.road_type_escape.0 {
        return;
    }
    if let Some(menu) = overlays.navigation_menu.as_deref_mut()
        && menu.open.take().is_some()
    {
        menu.focus = None;
        return;
    }
    if let Some(layers) = overlays.minimap_layers.as_deref_mut()
        && layers.expanded
    {
        layers.expanded = false;
        return;
    }
    if ingame_placement_busy(
        &save_window,
        &tool_state,
        &toolbar_state,
        &pick_state,
        &order_state,
        &industry_panel,
    ) {
        cancel_ingame_placement(
            &mut toolbar_state,
            &mut tool_state,
            &mut drag_state,
            &mut order_state,
            &mut next_pick,
            &mut industry_panel,
            &mut station_state,
        );
        return;
    }
    if close_top_visible_floating_window(&mut windows_q, &mut closed) {
        return;
    }
    return_to_main_menu(&mut next_screen, &mut suspended);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn close_toolbar_button_interaction(
    q: Query<&Interaction, (Changed<Interaction>, With<ToolbarCloseButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut industry_panel: ResMut<IndustryPanelState>,
    mut station_state: ResMut<crate::ui::toolbar::StationBuildState>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        cancel_ingame_placement(
            &mut toolbar_state,
            &mut tool_state,
            &mut drag_state,
            &mut order_state,
            &mut next_pick,
            &mut industry_panel,
            &mut station_state,
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    use crate::state::{ClientScreen, insert_test_order_pick_state};

    fn escape_test_world() -> World {
        let mut world = World::new();
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(DragBuildState::default());
        world.insert_resource(OrderEditState::default());
        world.insert_resource(IndustryPanelState::default());
        world.insert_resource(crate::ui::toolbar::StationBuildState::default());
        world.insert_resource(crate::ui::toolbar::RoadTypeEscapeConsumed::default());
        world.insert_resource(SuspendedGameSession::default());
        world.insert_resource(NextState::<ClientScreen>::default());
        world.insert_resource(State::new(ClientScreen::InGame));
        insert_test_order_pick_state(&mut world);
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world
    }

    #[test]
    fn escape_with_active_tool_clears_tool_without_leaving_game() {
        let mut world = escape_test_world();
        world.resource_mut::<UiToolState>().active_tool = Some(crate::ui::BuildMenuAction::Rail);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.run_system_once(handle_ingame_escape).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
        assert!(matches!(
            world.resource::<NextState<ClientScreen>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn escape_closes_navigation_menu_before_leaving_game() {
        let mut world = escape_test_world();
        world.insert_resource(ToolbarMenuState {
            open: Some(crate::ui::menu::MenuId::World),
            ..Default::default()
        });
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.run_system_once(handle_ingame_escape).unwrap();
        assert!(world.resource::<ToolbarMenuState>().open.is_none());
        assert!(matches!(
            world.resource::<NextState<ClientScreen>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn escape_collapses_expanded_minimap_before_leaving_game() {
        let mut world = escape_test_world();
        world.insert_resource(MinimapLayerState {
            expanded: true,
            ..Default::default()
        });
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.run_system_once(handle_ingame_escape).unwrap();
        assert!(!world.resource::<MinimapLayerState>().expanded);
        assert!(matches!(
            world.resource::<NextState<ClientScreen>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn escape_without_active_tool_returns_to_main_menu() {
        let mut world = escape_test_world();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.run_system_once(handle_ingame_escape).unwrap();
        assert!(matches!(
            world.resource::<NextState<ClientScreen>>(),
            NextState::Pending(ClientScreen::MainMenu)
                | NextState::PendingIfNeq(ClientScreen::MainMenu)
        ));
        assert!(world.resource::<SuspendedGameSession>().active);
    }
}
