use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::hud::HoveredTileCoord;
use crate::ui::toolbar::{BuildMenuAction, DragBuildState, StationBuildState, UiToolState};

pub(crate) fn rotate_station_with_right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut tool_state: ResMut<UiToolState>,
    mut station_state: ResMut<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
    sim: Option<Res<SimWorld>>,
    hovered: Option<Res<HoveredTileCoord>>,
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
        | Some(BuildMenuAction::RoadDepot)
        | Some(BuildMenuAction::RailDepot) => {
            station_state.orientation = (station_state.orientation + 1) % 4;
        }
        Some(BuildMenuAction::RailStation) => {
            station_state.rail_axis_y = !station_state.rail_axis_y;
        }
        Some(BuildMenuAction::RailSignals) => {
            if let (Some(sim), Some(coord)) = (sim.as_ref(), hovered.and_then(|h| h.pos)) {
                if let Some(tile) = sim.state.map.get(coord) {
                    let tb = tile.m5 & 0x3F;
                    station_state.orientation = openttdrs_core::rail_signals::cycle_signal_facing(
                        tb,
                        station_state.orientation,
                    );
                } else {
                    station_state.orientation = (station_state.orientation + 1) % 4;
                }
            } else {
                station_state.orientation = (station_state.orientation + 1) % 4;
            }
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
