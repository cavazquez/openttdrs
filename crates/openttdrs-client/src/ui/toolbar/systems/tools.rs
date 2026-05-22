use bevy::prelude::*;

use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::order_panel::start_order_destination_pick;
use crate::ui::toolbar::{
    BuildMenuAction, DragBuildState, OrderEditState, ToolSelectButton, UiToolState,
};

/// El boton del menu selecciona la herramienta activa para aplicar en el mapa.
#[allow(clippy::type_complexity)]
pub(crate) fn build_menu_interaction(
    mut q: Query<(&Interaction, &BuildMenuAction), (Changed<Interaction>, With<Button>)>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        tool_state.active_tool = Some(*action);
        cancel_placement(&mut drag_state);
        if *action == BuildMenuAction::Orders {
            start_order_destination_pick(&mut order_state);
        } else {
            order_state.picking_destination = false;
        }
    }
}

/// Resalta el boton de herramienta actualmente activo.
pub(crate) fn update_tool_button_visuals(
    tool_state: Res<UiToolState>,
    mut q: Query<
        (
            &BuildMenuAction,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<ToolSelectButton>,
    >,
) {
    for (action, interaction, mut bg, mut border) in &mut q {
        let is_active = tool_state
            .active_tool
            .is_some_and(|active| active == *action);
        *bg = if is_active && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.76, 0.67, 0.42))
        } else if is_active && *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.68, 0.59, 0.37))
        } else if is_active {
            BackgroundColor(Color::srgb(0.6, 0.52, 0.33))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.4, 0.34, 0.23))
        } else {
            BackgroundColor(Color::srgb(0.28, 0.24, 0.16))
        };
        *border = if is_active {
            BorderColor::all(Color::srgb(0.84, 0.74, 0.5))
        } else {
            BorderColor::all(Color::srgb(0.64, 0.57, 0.39))
        };
    }
}
