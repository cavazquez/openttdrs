use openttdrs_core::{Command, GameState, TileCoord, command_would_fail, resolve_tunnel_end};

use crate::ui::toolbar::build_input::commands::{command_for_action, command_for_line_action};
use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

pub(crate) fn action_is_tunnel(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel | BuildMenuAction::RailTunnel
    )
}

fn is_line_build_action(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel
            | BuildMenuAction::RailTunnel
            | BuildMenuAction::RoadBridge
            | BuildMenuAction::RailBridge
    )
}

/// Misma validación que `apply_command` para el preview (ghost verde/rojo).
#[must_use]
pub(crate) fn preview_build_command_valid(
    state: &GameState,
    action: BuildMenuAction,
    coord: TileCoord,
    station_state: &StationBuildState,
    preview_tiles: &[(i32, i32)],
    rail_lane_bits: Option<u8>,
) -> bool {
    if action_is_tunnel(action) {
        let Some(&(sx, sy)) = preview_tiles.first() else {
            return false;
        };
        let start = TileCoord::new(sx, sy);
        let Some(end) = resolve_tunnel_end(&state.map, start) else {
            return false;
        };
        let cmd = match action {
            BuildMenuAction::RoadTunnel => Command::PlaceRoadTunnel(start, end),
            BuildMenuAction::RailTunnel => Command::PlaceRailTunnel(start, end),
            _ => return false,
        };
        return command_would_fail(state, &cmd).is_none();
    }
    if is_line_build_action(action) {
        let Some(cmd) = command_for_line_action(action, preview_tiles) else {
            return false;
        };
        return command_would_fail(state, &cmd).is_none();
    }
    let Some(cmd) = command_for_action(action, coord, station_state, rail_lane_bits) else {
        return true;
    };
    command_would_fail(state, &cmd).is_none()
}
