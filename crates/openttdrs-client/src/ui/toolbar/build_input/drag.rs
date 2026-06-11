use openttdrs_core::{
    Command, CommandError, GameState, TileCoord, apply_command, resolve_tunnel_end,
};

use crate::state::SimWorld;

use super::commands::{command_for_action, command_for_line_action};
use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

pub(crate) fn action_supports_drag(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::Road
            | BuildMenuAction::RoadX
            | BuildMenuAction::RoadY
            | BuildMenuAction::RoadBridge
            | BuildMenuAction::RoadTunnel
            | BuildMenuAction::Rail
            | BuildMenuAction::RailX
            | BuildMenuAction::RailY
            | BuildMenuAction::RailHorz
            | BuildMenuAction::RailVert
            | BuildMenuAction::RailBridge
            | BuildMenuAction::RailTunnel
            | BuildMenuAction::Clear
    )
}

pub(crate) fn action_is_tunnel(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel | BuildMenuAction::RailTunnel
    )
}

pub(crate) fn tunnel_placement_is_valid(
    state: &GameState,
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> bool {
    if !action_is_tunnel(action) {
        return false;
    }
    let Some(&(sx, sy)) = tiles.first() else {
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
    openttdrs_core::command_would_fail(state, &cmd).is_none()
}

pub(crate) fn rail_bits_for_drag_action(action: BuildMenuAction) -> Option<u8> {
    match action {
        BuildMenuAction::RailX => Some(0x01),
        BuildMenuAction::RailY => Some(0x02),
        BuildMenuAction::RailHorz => Some(0x0C),
        BuildMenuAction::RailVert => Some(0x30),
        _ => None,
    }
}

pub(crate) fn road_bits_for_drag_action(
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> Option<u8> {
    match action {
        BuildMenuAction::RoadX => Some(0x0A),
        BuildMenuAction::RoadY => Some(0x05),
        BuildMenuAction::Road => {
            let &(sx, sy) = tiles.first()?;
            let &(ex, ey) = tiles.last().unwrap_or(&(sx, sy));
            Some(if (ex - sx).abs() >= (ey - sy).abs() {
                0x0A
            } else {
                0x05
            })
        }
        _ => None,
    }
}

pub(crate) fn command_for_tunnel_action(
    state: &GameState,
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let start = TileCoord::new(sx, sy);
    let end = resolve_tunnel_end(&state.map, start)?;
    match action {
        BuildMenuAction::RoadTunnel => Some(Command::PlaceRoadTunnel(start, end)),
        BuildMenuAction::RailTunnel => Some(Command::PlaceRailTunnel(start, end)),
        _ => None,
    }
}

pub(crate) fn apply_drag_action(
    sim: &mut SimWorld,
    action: BuildMenuAction,
    tiles: Vec<(i32, i32)>,
    station_state: &StationBuildState,
) -> (bool, Option<CommandError>) {
    if action_is_tunnel(action) {
        if let Some(cmd) = command_for_tunnel_action(&sim.state, action, &tiles) {
            return match apply_command(&mut sim.state, &cmd) {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e)),
            };
        }
        return (false, Some(CommandError::InvalidTunnelEndpoints));
    }
    if let Some(cmd) = command_for_line_action(action, &tiles) {
        return match apply_command(&mut sim.state, &cmd) {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e)),
        };
    }

    if let Some(road_bits) = road_bits_for_drag_action(action, &tiles) {
        let mut changed = false;
        let mut last_err = None;
        for (x, y) in tiles {
            match apply_command(
                &mut sim.state,
                &Command::SetRoadBits(TileCoord::new(x, y), road_bits),
            ) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
        return (changed, if changed { None } else { last_err });
    }

    if let Some(rail_bits) = rail_bits_for_drag_action(action) {
        let mut changed = false;
        let mut last_err = None;
        for (x, y) in tiles {
            match apply_command(
                &mut sim.state,
                &Command::SetRailBits(TileCoord::new(x, y), rail_bits),
            ) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
        return (changed, if changed { None } else { last_err });
    }

    let mut changed = false;
    let mut last_err = None;
    for (x, y) in tiles {
        if let Some(cmd) = command_for_action(action, TileCoord::new(x, y), station_state) {
            match apply_command(&mut sim.state, &cmd) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
    }
    (changed, if changed { None } else { last_err })
}

pub(crate) fn drag_line_tiles(
    action: BuildMenuAction,
    from: (i32, i32),
    to: (i32, i32),
) -> Vec<(i32, i32)> {
    let use_x_axis = match action {
        BuildMenuAction::RoadX => true,
        BuildMenuAction::RoadY => false,
        _ => (to.0 - from.0).abs() >= (to.1 - from.1).abs(),
    };
    let mut out = Vec::new();

    if use_x_axis {
        let step = if to.0 >= from.0 { 1 } else { -1 };
        let mut x = from.0;
        loop {
            out.push((x, from.1));
            if x == to.0 {
                break;
            }
            x += step;
        }
    } else {
        let step = if to.1 >= from.1 { 1 } else { -1 };
        let mut y = from.1;
        loop {
            out.push((from.0, y));
            if y == to.1 {
                break;
            }
            y += step;
        }
    }

    out
}
