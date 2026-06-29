use bevy::prelude::*;

use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::{
    DragBuildState, OrderEditState, ToolbarCloseButton, ToolbarState, UiToolState,
};

pub(crate) fn close_toolbar_panel_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
        order_state.clear();
    }
}

pub(crate) fn close_toolbar_button_interaction(
    q: Query<&Interaction, (Changed<Interaction>, With<ToolbarCloseButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
        order_state.clear();
    }
}
