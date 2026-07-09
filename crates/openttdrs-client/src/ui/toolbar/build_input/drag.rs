use openttdrs_core::{
    Command, CommandError, GameState, Map, ROAD_PLACE_FORCE_AXIS, TileCoord, apply_command,
    finalize_road_drag_line, infer_road_drag_axis, resolve_tunnel_end, road_drag_line_tiles,
    road_locked_tool_axis, tunnel_preview_path,
};

use crate::state::SimWorld;

use super::commands::{
    buy_land_command_for_tiles, command_for_action, command_for_line_action,
    terraform_command_for_tiles,
};
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
            | BuildMenuAction::RailRemove
            | BuildMenuAction::RailConvert
            | BuildMenuAction::RailSignals
            | BuildMenuAction::Clear
            | BuildMenuAction::Canal
            | BuildMenuAction::RaiseLand
            | BuildMenuAction::LowerLand
            | BuildMenuAction::LevelLand
            | BuildMenuAction::BuyLand
    )
}

pub(crate) fn action_supports_area_drag(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RaiseLand
            | BuildMenuAction::LowerLand
            | BuildMenuAction::LevelLand
            | BuildMenuAction::BuyLand
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

pub(crate) fn rail_bits_for_drag_action(
    action: BuildMenuAction,
    lane_bit: Option<u8>,
) -> Option<u8> {
    match action {
        BuildMenuAction::RailX | BuildMenuAction::RailY => {
            super::rail_lane::rail_lane_bits_for_action(action, None)
        }
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert => {
            lane_bit.or_else(|| super::rail_lane::rail_lane_bits_for_action(action, None))
        }
        _ => None,
    }
}

fn road_tool_axis(action: BuildMenuAction, from: (i32, i32), to: (i32, i32)) -> u8 {
    match action {
        BuildMenuAction::RoadX => 0x0A,
        BuildMenuAction::RoadY => 0x05,
        BuildMenuAction::Road => {
            if (to.0 - from.0).abs() >= (to.1 - from.1).abs() {
                0x0A
            } else {
                0x05
            }
        }
        _ => 0x0A,
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

fn road_drag_axis(
    map: &Map,
    action: BuildMenuAction,
    start: TileCoord,
    end: TileCoord,
    tool_axis: u8,
) -> u8 {
    match action {
        BuildMenuAction::RoadX => road_locked_tool_axis(map, start, end, 0x0A),
        BuildMenuAction::RoadY => road_locked_tool_axis(map, start, end, 0x05),
        BuildMenuAction::Road => infer_road_drag_axis(map, start, end, tool_axis),
        _ => tool_axis,
    }
}

/// Herramientas cuya sim actualiza piezas de vía en teselas vecinas (`refresh_rail_neighbors`).
#[inline]
pub(crate) fn rail_action_refreshes_neighbors(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::Rail
            | BuildMenuAction::RailRemove
            | BuildMenuAction::RailX
            | BuildMenuAction::RailY
            | BuildMenuAction::RailHorz
            | BuildMenuAction::RailVert
    )
}

/// Teselas a redibujar tras colocar/quitar vía con refresco de vecinos (autorraíl / quitar).
#[must_use]
pub(crate) fn rail_remap_neighbor_tiles(map: &Map, tiles: &[(i32, i32)]) -> Vec<(i32, i32)> {
    use std::collections::BTreeSet;
    let (mw, mh) = map.dimensions();
    let mw_i = mw as i32;
    let mh_i = mh as i32;
    let mut set = BTreeSet::new();
    for &(x, y) in tiles {
        set.insert((x, y));
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let nx = x + dx;
            let ny = y + dy;
            if nx >= 0 && ny >= 0 && nx < mw_i && ny < mh_i {
                set.insert((nx, ny));
            }
        }
    }
    set.into_iter().collect()
}

/// Teselas a redibujar tras colocar un túnel (entrada, interior y salida).
#[must_use]
pub(crate) fn tunnel_remap_tiles(map: &Map, tiles: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let Some(&(sx, sy)) = tiles.first() else {
        return Vec::new();
    };
    let start = TileCoord::new(sx, sy);
    tunnel_preview_path(map, start)
        .map(|path| path.into_iter().map(|c| (c.x, c.y)).collect())
        .unwrap_or_else(|| vec![(sx, sy)])
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
    rail_lane_bit: Option<u8>,
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
    if action == BuildMenuAction::BuyLand {
        if let Some(cmd) = buy_land_command_for_tiles(&tiles) {
            return match apply_command(&mut sim.state, &cmd) {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e)),
            };
        }
        return (false, Some(CommandError::CannotBuyLandHere));
    }
    if let Some(cmd) = terraform_command_for_tiles(action, &tiles) {
        return match apply_command(&mut sim.state, &cmd) {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e)),
        };
    }

    if matches!(
        action,
        BuildMenuAction::RoadBridge | BuildMenuAction::RailBridge
    ) {
        return (false, None);
    }

    if let Some(cmd) = command_for_line_action(action, &tiles, openttdrs_core::BridgeType::Wooden) {
        return match apply_command(&mut sim.state, &cmd) {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e)),
        };
    }

    if let Some(tool_axis) = road_bits_for_drag_action(action, &tiles) {
        let mut changed = false;
        let mut last_err = None;
        let placed: Vec<TileCoord> = tiles.iter().map(|&(x, y)| TileCoord::new(x, y)).collect();
        let start = placed.first().copied().unwrap_or(TileCoord::new(0, 0));
        let end = placed.last().copied().unwrap_or(start);
        let axis = road_drag_axis(&sim.state.map, action, start, end, tool_axis);
        for c in &placed {
            match apply_command(
                &mut sim.state,
                &Command::PlaceRoadBits(*c, axis | ROAD_PLACE_FORCE_AXIS),
            ) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
        if changed {
            let _ = finalize_road_drag_line(&mut sim.state, &placed, axis);
        }
        return (changed, if changed { None } else { last_err });
    }

    if action == BuildMenuAction::RailConvert {
        use openttdrs_core::rail_type_from_tile;
        let mut changed = false;
        let mut last_err = None;
        for (x, y) in tiles {
            let pos = TileCoord::new(x, y);
            let to = sim
                .state
                .map
                .get(pos)
                .map(|t| rail_type_from_tile(t).next().as_u8())
                .unwrap_or(1);
            match apply_command(&mut sim.state, &Command::ConvertRail(pos, to)) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
        return (changed, if changed { None } else { last_err });
    }

    if let Some(rail_bits) = rail_bits_for_drag_action(action, rail_lane_bit) {
        let mut changed = false;
        let mut last_err = None;
        let cmd_fn = if action == BuildMenuAction::RailRemove {
            |pos: TileCoord, bits: u8| Command::RemoveRailBits(pos, bits)
        } else {
            |pos: TileCoord, bits: u8| Command::PlaceRailBits(pos, bits)
        };
        for (x, y) in tiles {
            match apply_command(&mut sim.state, &cmd_fn(TileCoord::new(x, y), rail_bits)) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
        return (changed, if changed { None } else { last_err });
    }

    if action == BuildMenuAction::RailSignals {
        let density = station_state.signal_density.max(1);
        let (fx, fy) = station_state.signal_drag_fract.unwrap_or((128, 128));
        let spaced = subsample_drag_tiles(&tiles, density);
        let mut changed = false;
        let mut last_err = None;
        for (x, y) in spaced {
            let cmd = Command::PlaceRailSignal(
                TileCoord::new(x, y),
                station_state.orientation,
                fx,
                fy,
                station_state.signal_type,
            );
            match apply_command(&mut sim.state, &cmd) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
        return (changed, if changed { None } else { last_err });
    }

    let mut changed = false;
    let mut last_err = None;
    for (x, y) in tiles {
        if let Some(cmd) = command_for_action(
            action,
            TileCoord::new(x, y),
            station_state,
            rail_lane_bit,
            Some(&sim.state.map),
            station_state.signal_drag_fract,
            station_state.signal_type,
            false,
        ) {
            match apply_command(&mut sim.state, &cmd) {
                Ok(()) => changed = true,
                Err(e) => last_err = Some(e),
            }
        }
    }
    (changed, if changed { None } else { last_err })
}

/// Teselas de arrastre espaciadas cada `density` (incluye la primera).
pub(crate) fn subsample_drag_tiles(tiles: &[(i32, i32)], density: u8) -> Vec<(i32, i32)> {
    let step = usize::from(density.max(1));
    tiles
        .iter()
        .copied()
        .enumerate()
        .filter(|(i, _)| i % step == 0)
        .map(|(_, t)| t)
        .collect()
}

pub(crate) fn drag_line_tiles(
    map: Option<&Map>,
    action: BuildMenuAction,
    from: (i32, i32),
    to: (i32, i32),
) -> Vec<(i32, i32)> {
    if action_supports_area_drag(action) {
        return rect_drag_tiles(from, to);
    }

    if action == BuildMenuAction::Road
        && let Some(map) = map
    {
        return road_drag_line_tiles(map, from, to, road_tool_axis(action, from, to)).0;
    }

    let use_x_axis = match action {
        BuildMenuAction::RoadX | BuildMenuAction::RailVert => true,
        BuildMenuAction::RoadY | BuildMenuAction::RailHorz => false,
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

fn rect_drag_tiles(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let min_x = from.0.min(to.0);
    let max_x = from.0.max(to.0);
    let min_y = from.1.min(to.1);
    let max_y = from.1.max(to.1);
    let mut out = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            out.push((x, y));
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::state::SimWorld;
    use openttdrs_core::{GameState, TileCoord, TileKind, apply_command};

    #[test]
    fn rail_remap_neighbor_tiles_includes_orthogonal_neighbors() {
        let sim = SimWorld {
            state: GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let tiles = rail_remap_neighbor_tiles(&sim.state.map, &[(3, 3), (4, 3)]);
        assert!(tiles.contains(&(3, 3)));
        assert!(tiles.contains(&(4, 3)));
        assert!(tiles.contains(&(2, 3)));
        assert!(tiles.contains(&(5, 3)));
        assert!(tiles.contains(&(3, 2)));
        assert!(tiles.contains(&(3, 4)));
        assert!(tiles.contains(&(4, 2)));
        assert!(tiles.contains(&(4, 4)));
        assert!(!tiles.contains(&(2, 2)));
    }

    #[test]
    fn tunnel_remap_tiles_includes_both_portals() {
        let mut sim = SimWorld {
            state: GameState::new(12, 12),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        sim.state.map.set_height(c(5, 5), 2).unwrap();
        sim.state.map.set_height(c(5, 6), 2).unwrap();
        sim.state.map.set_height(c(6, 5), 1).unwrap();
        sim.state.map.set_height(c(6, 6), 1).unwrap();
        sim.state.map.set_height(c(3, 5), 1).unwrap();
        sim.state.map.set_height(c(3, 6), 1).unwrap();
        sim.state.map.set_height(c(4, 5), 2).unwrap();
        sim.state.map.set_height(c(4, 6), 2).unwrap();
        let tiles = tunnel_remap_tiles(&sim.state.map, &[(5, 5)]);
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles.first().copied(), Some((5, 5)));
        assert_eq!(tiles.last().copied(), Some((3, 5)));
    }

    #[test]
    fn drag_road_merge_bits_at_perpendicular_intersection() {
        let mut sim = SimWorld {
            state: GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let junction = (3, 3);
        let (changed, err) = apply_drag_action(
            &mut sim,
            BuildMenuAction::RoadX,
            vec![(1, 3), (2, 3), junction, (4, 3), (5, 3)],
            &StationBuildState::default(),
            None,
        );
        assert!(changed);
        assert!(err.is_none());
        assert_eq!(
            sim.state.map.get(TileCoord::new(3, 3)).unwrap().m5 & 0x0F,
            0x0A
        );
        let (changed, err) = apply_drag_action(
            &mut sim,
            BuildMenuAction::RoadY,
            vec![(3, 1), (3, 2), junction, (3, 4), (3, 5)],
            &StationBuildState::default(),
            None,
        );
        assert!(changed);
        assert!(err.is_none());
        assert_eq!(
            sim.state.map.get(TileCoord::new(3, 3)).unwrap().m5 & 0x0F,
            0x0F,
            "arrastre perpendicular debe formar cruce (OR de bits), no pisar"
        );
        assert_eq!(
            sim.state.map.get_kind(TileCoord::new(3, 3)),
            Some(TileKind::Road)
        );
    }

    #[test]
    fn drag_road_x_force_axis_on_first_isolated_tile() {
        let mut sim = SimWorld {
            state: GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let (changed, err) = apply_drag_action(
            &mut sim,
            BuildMenuAction::RoadX,
            vec![(1, 5), (2, 5), (3, 5)],
            &StationBuildState::default(),
            None,
        );
        assert!(changed);
        assert!(err.is_none());
        assert_eq!(
            sim.state.map.get(TileCoord::new(1, 5)).unwrap().m5 & 0x0F,
            0x0A,
            "primera tesela del arrastre horizontal debe ser eje X"
        );
    }

    #[test]
    fn road_x_drag_keeps_horizontal_axis_near_vertical_road() {
        let mut sim = SimWorld {
            state: GameState::new(12, 12),
            loaded_file: false,
            ottdmap_extras: None,
        };
        for y in 2..=5 {
            apply_command(
                &mut sim.state,
                &Command::PlaceRoadBits(TileCoord::new(8, y), 0x05),
            )
            .unwrap();
        }
        let line = drag_line_tiles(Some(&sim.state.map), BuildMenuAction::RoadX, (3, 4), (6, 4));
        assert_eq!(line, vec![(3, 4), (4, 4), (5, 4), (6, 4)]);
        let (changed, err) = apply_drag_action(
            &mut sim,
            BuildMenuAction::RoadX,
            line,
            &StationBuildState::default(),
            None,
        );
        assert!(changed);
        assert!(err.is_none());
        for x in 3..=6 {
            assert_eq!(
                sim.state.map.get(TileCoord::new(x, 4)).unwrap().m5 & 0x0F,
                0x0A,
                "RoadX no debe girar 90° por vía vertical cercana"
            );
        }
    }

    #[test]
    fn generic_road_drag_extends_horizontal_from_colinear_network() {
        let mut sim = SimWorld {
            state: GameState::new(12, 12),
            loaded_file: false,
            ottdmap_extras: None,
        };
        for x in 3..=6 {
            apply_command(
                &mut sim.state,
                &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A),
            )
            .unwrap();
        }
        let line = drag_line_tiles(Some(&sim.state.map), BuildMenuAction::Road, (8, 6), (11, 8));
        assert_eq!(line, vec![(8, 6), (9, 6), (10, 6), (11, 6)]);
        let (changed, err) = apply_drag_action(
            &mut sim,
            BuildMenuAction::Road,
            line,
            &StationBuildState::default(),
            None,
        );
        assert!(changed);
        assert!(err.is_none());
        for x in 8..=11 {
            assert_eq!(
                sim.state.map.get(TileCoord::new(x, 6)).unwrap().m5 & 0x0F,
                0x0A
            );
        }
    }

    #[test]
    fn subsample_drag_tiles_density_4() {
        let line: Vec<(i32, i32)> = (0..12).map(|x| (x, 0)).collect();
        assert_eq!(subsample_drag_tiles(&line, 4), vec![(0, 0), (4, 0), (8, 0)]);
    }

    #[test]
    fn signal_drag_places_with_density() {
        let mut sim = SimWorld {
            state: GameState::new(16, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        for x in 1..=12 {
            apply_command(
                &mut sim.state,
                &Command::SetRailBits(TileCoord::new(x, 2), 0x01),
            )
            .unwrap();
        }
        let station = StationBuildState {
            signal_density: 4,
            signal_drag_fract: Some((128, 128)),
            ..Default::default()
        };
        let line: Vec<(i32, i32)> = (1..=12).map(|x| (x, 2)).collect();
        let (changed, err) =
            apply_drag_action(&mut sim, BuildMenuAction::RailSignals, line, &station, None);
        assert!(changed, "{err:?}");
        assert!(err.is_none());
        let signaled: Vec<_> = (1..=12)
            .filter(|&x| {
                let t = sim.state.map.get(TileCoord::new(x, 2)).unwrap();
                openttdrs_core::rail_signals::rail_tile_is_signals(t.m5)
            })
            .collect();
        assert_eq!(signaled, vec![1, 5, 9]);
    }
}
