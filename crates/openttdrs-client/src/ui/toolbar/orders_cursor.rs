//! Cursor en forma de cruz cuando la herramienta Órdenes está activa y hay vehículo seleccionado.

use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use super::{BuildMenuAction, BuildMenuUi, OrderEditState, UiToolState};

pub(crate) fn sync_orders_pick_cursor(
    tool_state: Res<UiToolState>,
    order_state: Res<OrderEditState>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
    mut commands: Commands,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut prev: Local<Option<bool>>,
) {
    let over_menu = menu_pointer.iter().any(|i| *i != Interaction::None);
    let active = tool_state.active_tool == Some(BuildMenuAction::Orders)
        && order_state.vehicle_id.is_some()
        && !over_menu;
    if *prev == Some(active) {
        return;
    }
    *prev = Some(active);
    let Ok(window) = windows.single() else {
        return;
    };
    if active {
        commands
            .entity(window)
            .insert(CursorIcon::from(SystemCursorIcon::Crosshair));
    } else {
        commands.entity(window).remove::<CursorIcon>();
    }
}
