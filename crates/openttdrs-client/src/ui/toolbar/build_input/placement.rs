use crate::ui::toolbar::DragBuildState;

pub(crate) fn cancel_placement(drag_state: &mut DragBuildState) {
    drag_state.armed = false;
    drag_state.start_tile = None;
    drag_state.last_tile = None;
    drag_state.last_action = None;
    drag_state.pending_tiles.clear();
    drag_state.rail_lane_bit = None;
    drag_state.press_world_pos = None;
}
