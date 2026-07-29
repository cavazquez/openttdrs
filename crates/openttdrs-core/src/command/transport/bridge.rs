use crate::GameState;
use crate::bridge_spec::{
    BridgeType, bridge_available_at_tick_in, bridge_build_cost_in, set_bridge_middle_mapt,
    set_bridge_type_m6,
};
use crate::map::{
    Map, TileCoord, TileKind, complement_slope, inclined_slope_direction, resolve_tunnel_end,
    tile_slope_and_z, tunnel_entrance_m5, tunnel_path_tiles, tunnel_preview_path,
};

use super::super::CommandError;

#[allow(unused_imports)]
use crate::command::transport::internal::{
    axis_line, build_error_for_kind, check_in_bounds, transport_tile_is_buildable,
};

/// Dirección diagonal «hacia el sur» del eje (`AxisToDiagDir` en `direction_func.h`).
fn axis_to_diag_dir(axis_y: bool) -> u8 {
    u8::from(!axis_y) + 1 // SE en eje Y, SW en eje X
}

fn reverse_diag_dir(dir: u8) -> u8 {
    2 ^ (dir & 0x03)
}

fn bridge_ramp_m5(is_rail: bool, dir: u8) -> u8 {
    let transport = u8::from(!is_rail);
    0x80 | (transport << 2) | (dir & 0x03)
}

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
    _m5: u8,
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
        if !bridge_available_at_tick_in(&state.bridge_spec_catalog, bridge_type, state.tick, a, b) {
            return Err(CommandError::BridgeTypeNotAvailable);
        }
        bridge_build_cost_in(&state.bridge_spec_catalog, bridge_type, a, b)
    };
    for (i, c) in line.iter().enumerate() {
        let mut tile = state.map.get(*c).ok_or(CommandError::OutOfBounds)?;
        if is_tunnel {
            tile.kind = kind_to_place;
            tile.mapt = mapt;
            tile.m5 = tile_slope_and_z(&state.map, *c)
                .and_then(|(h, _)| tunnel_entrance_m5(h, is_rail))
                .unwrap_or(0);
        } else {
            let is_endpoint = i == 0 || i + 1 == line.len();
            if is_endpoint {
                tile.kind = kind_to_place;
                tile.mapt = mapt;
                let is_start = i == 0;
                let dir = if is_start {
                    axis_to_diag_dir(bridge_axis_y)
                } else {
                    reverse_diag_dir(axis_to_diag_dir(bridge_axis_y))
                };
                tile.m5 = bridge_ramp_m5(is_rail, dir);
                tile.m6 = set_bridge_type_m6(tile.m6, bridge_type);
            } else {
                tile.mapt = set_bridge_middle_mapt(tile.mapt, bridge_axis_y);
                tile.m6 = set_bridge_type_m6(tile.m6, bridge_type);
            }
        }
        // Dueño de la infra (`MAPO` / `m1`), igual que vía y carretera.
        tile.m1 = state.active_company.0;
        state
            .map
            .set_tile(*c, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= cost;
    Ok(())
}
