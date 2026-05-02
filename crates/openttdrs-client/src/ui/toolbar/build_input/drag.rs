use openttdrs_core::{Command, Map, TileCoord, TileKind, apply_command};

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
    map: &Map,
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> bool {
    if !action_is_tunnel(action) || tiles.len() < 3 {
        return false;
    }
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    let Some(start) = map.get(TileCoord::new(sx, sy)) else {
        return false;
    };
    let Some(end) = map.get(TileCoord::new(ex, ey)) else {
        return false;
    };
    !matches!(start.kind, TileKind::Water | TileKind::Void)
        && !matches!(end.kind, TileKind::Water | TileKind::Void)
        && start.height == end.height
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

pub(crate) fn apply_drag_action(
    sim: &mut SimWorld,
    action: BuildMenuAction,
    tiles: Vec<(i32, i32)>,
    station_state: &StationBuildState,
) -> bool {
    if let Some(cmd) = command_for_line_action(action, &tiles) {
        return apply_command(&mut sim.state, &cmd).is_ok();
    }

    if let Some(road_bits) = road_bits_for_drag_action(action, &tiles) {
        let mut changed = false;
        for (x, y) in tiles {
            changed |= apply_command(
                &mut sim.state,
                &Command::SetRoadBits(TileCoord::new(x, y), road_bits),
            )
            .is_ok();
        }
        return changed;
    }

    let mut changed = false;
    for (x, y) in tiles {
        if let Some(cmd) = command_for_action(action, TileCoord::new(x, y), station_state) {
            changed |= apply_command(&mut sim.state, &cmd).is_ok();
        }
    }
    changed
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
