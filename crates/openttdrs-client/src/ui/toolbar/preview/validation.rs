use openttdrs_core::{
    BridgeType, Command, CommandError, GameState, TileCoord, command_would_fail, resolve_tunnel_end,
};

use crate::ui::toolbar::build_input::commands::{
    buy_land_command_for_tiles, command_for_action, command_for_line_action,
    terraform_command_for_tiles,
};
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
            | BuildMenuAction::Aqueduct
    )
}

fn is_bridge_action(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadBridge | BuildMenuAction::RailBridge
    )
}

/// Validez del tramo (geometría/agua); tipo y fondos se eligen en la ventana.
#[must_use]
pub(crate) fn preview_bridge_span_valid(
    state: &GameState,
    action: BuildMenuAction,
    preview_tiles: &[(i32, i32)],
) -> bool {
    let Some(cmd) = command_for_line_action(action, preview_tiles, BridgeType::Wooden) else {
        return false;
    };
    match command_would_fail(state, &cmd) {
        None => true,
        Some(CommandError::BridgeTypeNotAvailable) | Some(CommandError::InsufficientFunds) => true,
        Some(_) => false,
    }
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
    tile_fract: Option<(u8, u8)>,
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
        if is_bridge_action(action) {
            return preview_bridge_span_valid(state, action, preview_tiles);
        }
        let Some(cmd) = command_for_line_action(action, preview_tiles, BridgeType::Wooden) else {
            return false;
        };
        return command_would_fail(state, &cmd).is_none();
    }
    if matches!(action, BuildMenuAction::BuyLand) {
        let Some(cmd) = buy_land_command_for_tiles(preview_tiles) else {
            return false;
        };
        return command_would_fail(state, &cmd).is_none();
    }
    if matches!(
        action,
        BuildMenuAction::RaiseLand | BuildMenuAction::LowerLand | BuildMenuAction::LevelLand
    ) {
        let Some(cmd) = terraform_command_for_tiles(action, preview_tiles) else {
            return false;
        };
        return command_would_fail(state, &cmd).is_none();
    }
    let Some(cmd) = command_for_action(
        action,
        coord,
        station_state,
        rail_lane_bits,
        Some(&state.map),
        tile_fract,
        station_state.signal_type,
        false,
    ) else {
        return true;
    };
    command_would_fail(state, &cmd).is_none()
}
