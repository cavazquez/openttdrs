use bevy::prelude::*;

use crate::ui::toolbar::{BuildMenuAction, DragBuildState, StationBuildState, UiToolState};

pub(crate) fn rotate_station_with_right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut tool_state: ResMut<UiToolState>,
    mut station_state: ResMut<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    if drag_state.armed {
        drag_state.armed = false;
        drag_state.start_tile = None;
        drag_state.last_tile = None;
        drag_state.last_action = None;
        drag_state.pending_tiles.clear();
        return;
    }
    match tool_state.active_tool {
        Some(BuildMenuAction::Station)
        | Some(BuildMenuAction::BusStop)
        | Some(BuildMenuAction::RailStation)
        | Some(BuildMenuAction::RoadDepot)
        | Some(BuildMenuAction::RailDepot) => {
            station_state.orientation = (station_state.orientation + 1) % 4;
        }
        Some(BuildMenuAction::RoadX) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadY);
        }
        Some(BuildMenuAction::RoadY) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadX);
        }
        Some(BuildMenuAction::Road) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadX);
        }
        _ => return,
    }
    drag_state.armed = false;
    drag_state.start_tile = None;
    drag_state.last_tile = None;
    drag_state.last_action = None;
    drag_state.pending_tiles.clear();
}
