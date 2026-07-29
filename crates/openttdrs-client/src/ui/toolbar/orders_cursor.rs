//! Cursor al elegir destinos en el mapa (modo «Agregar destino»).

use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use crate::state::{OrderPickState, SimWorld, order_pick_active};
use crate::ui::hud::HoveredTileCoord;
use crate::ui::toolbar::build_input::orders::order_pick_valid;
use crate::ui::toolbar::{BuildMenuAction, BuildMenuUi, OrderEditState, UiToolState};

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_orders_pick_cursor(
    tool_state: Res<UiToolState>,
    order_state: Res<OrderEditState>,
    pick_state: Res<State<OrderPickState>>,
    hovered: Res<HoveredTileCoord>,
    sim: Res<SimWorld>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
    mut commands: Commands,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut prev: Local<Option<(bool, bool)>>,
) {
    let over_menu = menu_pointer.iter().any(|i| *i != Interaction::None);
    let picking =
        order_pick_active(&pick_state) || tool_state.active_tool == Some(BuildMenuAction::Orders);
    let active = picking && order_state.vehicle_id().is_some() && !over_menu;
    let hover_valid = active
        && order_state.vehicle_id().is_some_and(|vehicle_id| {
            hovered
                .pos
                .is_some_and(|pos| order_pick_valid(&sim, vehicle_id, pos))
        });
    let key = (active, hover_valid);
    if *prev == Some(key) {
        return;
    }
    *prev = Some(key);
    let Ok(window) = windows.single() else {
        return;
    };
    if !active {
        commands.entity(window).remove::<CursorIcon>();
        return;
    }
    let icon = if hover_valid {
        SystemCursorIcon::Pointer
    } else {
        SystemCursorIcon::Crosshair
    };
    commands.entity(window).insert(CursorIcon::from(icon));
}
