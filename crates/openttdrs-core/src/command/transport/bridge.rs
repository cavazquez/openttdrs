use crate::GameState;
use crate::bridge_spec::{BridgeType, bridge_build_cost, set_bridge_type_m6};
use crate::map::{
    Map, TileCoord, TileKind, complement_slope, inclined_slope_direction, resolve_tunnel_end,
    tile_slope_and_z, tunnel_entrance_m5, tunnel_path_tiles, tunnel_preview_path,
};

use super::super::CommandError;

#[allow(unused_imports)]
use crate::command::transport::internal::{
    axis_line, build_error_for_kind, check_in_bounds, transport_tile_is_buildable,
};

pub const BRIDGE_AXIS_Y_M5: u8 = 0x10;

pub(crate) fn check_bridge(map: &Map, a: TileCoord, b: TileCoord) -> Result<(), CommandError> {
    let line = axis_line(a, b);
    if line.len() < 3 {
        return Err(CommandError::InvalidBridgeSpan);
    }
    let (Some(start_z), Some(end_z)) = (
        tile_slope_and_z(map, a).map(|(_, z)| z),
        tile_slope_and_z(map, b).map(|(_, z)| z),
    ) else {
        return Err(CommandError::OutOfBounds);
    };
    if start_z != end_z {
        return Err(CommandError::InvalidBridgeSpan);
    }
    let mut span_has_gap = false;
    for (i, c) in line.iter().enumerate() {
        check_in_bounds(map, *c)?;
        let kind = map.get_kind(*c).unwrap_or(TileKind::Grass);
        let is_endpoint = i == 0 || i + 1 == line.len();
        if is_endpoint {
            if !transport_tile_is_buildable(kind) {
                return Err(build_error_for_kind(kind));
            }
        } else if kind == TileKind::Water {
            span_has_gap = true;
        } else {
            if !transport_tile_is_buildable(kind) {
                return Err(build_error_for_kind(kind));
            }
            if tile_slope_and_z(map, *c).is_some_and(|(_, z)| z < start_z) {
                span_has_gap = true;
            }
        }
    }
    if span_has_gap {
        Ok(())
    } else {
        Err(CommandError::InvalidBridgeSpan)
    }
}

pub(crate) fn check_tunnel(map: &Map, start: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, start)?;
    let (start_tileh, _) =
        tile_slope_and_z(map, start).ok_or(CommandError::InvalidTunnelEndpoints)?;
    if inclined_slope_direction(start_tileh).is_none() {
        return Err(CommandError::InvalidTunnelEndpoints);
    }
    let Some(path) = tunnel_preview_path(map, start) else {
        return Err(CommandError::InvalidTunnelEndpoints);
    };
    if path.len() < 2 {
        return Err(CommandError::InvalidTunnelEndpoints);
    }
    for c in &path {
        check_in_bounds(map, *c)?;
        let kind = map.get_kind(*c).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(kind) {
            return Err(build_error_for_kind(kind));
        }
    }
    Ok(())
}

pub(crate) fn check_tunnel_or_bridge(
    map: &Map,
    a: TileCoord,
    b: TileCoord,
    is_tunnel: bool,
) -> Result<(), CommandError> {
    if is_tunnel {
        check_tunnel(map, a)
    } else {
        check_bridge(map, a, b)
    }
}

pub(in crate::command) fn place_tunnel_or_bridge(
    state: &mut GameState,
    a: TileCoord,
    b: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    bridge_type: BridgeType,
) -> Result<(), CommandError> {
    let is_tunnel = matches!(kind_to_place, TileKind::RoadTunnel | TileKind::RailTunnel);
    check_tunnel_or_bridge(&state.map, a, b, is_tunnel)?;
    let line = if is_tunnel {
        let end = resolve_tunnel_end(&state.map, a).ok_or(CommandError::InvalidTunnelEndpoints)?;
        let (start_tileh, _) =
            tile_slope_and_z(&state.map, a).ok_or(CommandError::InvalidTunnelEndpoints)?;
        let (end_tileh, _) =
            tile_slope_and_z(&state.map, end).ok_or(CommandError::InvalidTunnelEndpoints)?;
        if complement_slope(start_tileh) != end_tileh {
            return Err(CommandError::InvalidTunnelEndpoints);
        }
        tunnel_path_tiles(&state.map, a, end)
    } else {
        axis_line(a, b)
    };
    let is_rail = matches!(kind_to_place, TileKind::RailTunnel | TileKind::RailBridge);
    let bridge_axis_y = !is_tunnel && (b.x - a.x).abs() < (b.y - a.y).abs();
    let cost = if is_tunnel {
        crate::TUNNEL_BUILD_COST_PER_TILE * i64::try_from(line.len()).unwrap_or(i64::MAX)
    } else {
        bridge_build_cost(bridge_type, a, b)
    };
    for c in line {
        let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
        tile.kind = kind_to_place;
        tile.mapt = mapt;
        tile.m5 = if is_tunnel {
            tile_slope_and_z(&state.map, c)
                .and_then(|(h, _)| tunnel_entrance_m5(h, is_rail))
                .unwrap_or(0)
        } else if bridge_axis_y {
            m5 | BRIDGE_AXIS_Y_M5
        } else {
            m5
        };
        if !is_tunnel {
            tile.m6 = set_bridge_type_m6(tile.m6, bridge_type);
        }
        state
            .map
            .set_tile(c, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= cost;
    Ok(())
}
