use crate::GameState;
use crate::bridge_spec::{
    BridgeType, bridge_above_axis_from_mapt, bridge_available_at_tick_in, bridge_build_cost_in,
    bridge_line_tiles, rail_bridge_other_end, road_bridge_other_end, set_bridge_middle_mapt,
    set_bridge_type_m6,
};
use crate::company::{OWNER_NONE_M1, OWNER_TOWN_M1};
use crate::economy::{
    bridge_clear_cost, rail_clear_cost, road_clear_cost_factored, tunnel_clear_cost,
};
use crate::map::{
    Map, TileCoord, TileKind, complement_slope, diag_dir_offset, inclined_slope_direction,
    openttd_tile_index_to_coord, resolve_existing_tunnel_end, resolve_tunnel_end, tile_slope_and_z,
    tunnel_entrance_m5, tunnel_path_tiles, tunnel_preview_path,
};
use crate::station::{Station, StopKind, station_at_tile};

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

struct TunnelBridgeClearPlan {
    kind: TileKind,
    end: TileCoord,
    line: Vec<TileCoord>,
    clear_tiles: Vec<TileCoord>,
    is_tunnel: bool,
}

pub(in crate::command) fn tunnel_bridge_kind(kind: TileKind) -> Option<bool> {
    match kind {
        TileKind::RoadTunnel | TileKind::RoadBridge => Some(false),
        TileKind::RailTunnel | TileKind::RailBridge => Some(true),
        _ => None,
    }
}

fn check_tunnel_bridge_owner(state: &GameState, c: TileCoord) -> Result<(), CommandError> {
    if state.cheats.magic_bulldozer_active() {
        return Ok(());
    }
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let owner = tile.m1 & 0x1F;
    let active = state.active_company.0 & 0x1F;
    if owner != active && owner != (OWNER_NONE_M1 & 0x1F) && owner != (OWNER_TOWN_M1 & 0x1F) {
        return Err(CommandError::TileNotOwned);
    }
    Ok(())
}

fn check_tunnel_bridge_transport(tile: crate::map::Tile, is_rail: bool) -> bool {
    let expected = if is_rail { 0 } else { 0x04 };
    tile.m5 & 0x0C == expected
}

fn bridge_other_end_for_kind(map: &Map, ramp: TileCoord, kind: TileKind) -> Option<TileCoord> {
    match kind {
        TileKind::RoadBridge => road_bridge_other_end(map, ramp),
        TileKind::RailBridge => rail_bridge_other_end(map, ramp),
        _ => None,
    }
}

fn bridge_pair_matches(map: &Map, start: TileCoord, end: TileCoord, kind: TileKind) -> bool {
    map.get(start).is_some_and(|tile| {
        tile.kind == kind
            && tile.is_tunnel_bridge_tile()
            && tile.m5 & 0x80 != 0
            && map.get(end).is_some_and(|other| {
                other.kind == kind && other.is_tunnel_bridge_tile() && other.m5 & 0x80 != 0
            })
            && bridge_other_end_for_kind(map, start, kind) == Some(end)
    })
}

/// Valida que un puente nuevo no sobrescriba una estructura parcialmente.
///
/// El comando nativo tiene una rama especial sólo cuando las dos rampas
/// actuales forman exactamente el puente solicitado. En ese caso se permite
/// el reemplazo visual; una boca de otro puente, un túnel o un vano de puente
/// que cruza el trazado debe demolerse primero.
fn check_bridge_replacement(
    state: &GameState,
    start: TileCoord,
    end: TileCoord,
    kind: TileKind,
) -> Result<(), CommandError> {
    let exact_pair = bridge_pair_matches(&state.map, start, end, kind);
    for tile_coord in [start, end] {
        let Some(tile) = state.map.get(tile_coord) else {
            continue;
        };
        match tile.kind {
            TileKind::RoadTunnel | TileKind::RailTunnel => {
                return Err(CommandError::MustDemolishTunnelFirst);
            }
            TileKind::RoadBridge | TileKind::RailBridge => {
                if !exact_pair || tile.kind != kind {
                    return Err(CommandError::MustDemolishBridgeFirst);
                }
                check_tunnel_bridge_owner(state, tile_coord)?;
            }
            _ => {}
        }
    }

    if !exact_pair {
        let line = axis_line(start, end);
        if line.iter().any(|tile| {
            state.map.get(*tile).is_some_and(|raw| {
                crate::bridge_spec::bridge_above_axis_from_mapt(raw.mapt).is_some()
            })
        }) {
            return Err(CommandError::MustDemolishBridgeFirst);
        }
    }
    Ok(())
}

pub(in crate::command) fn check_bridge_placement_with_state(
    state: &GameState,
    start: TileCoord,
    end: TileCoord,
    kind: TileKind,
) -> Result<(), CommandError> {
    check_bridge_with_stations(
        &state.map,
        &state.stations,
        &state.road_stop_spec_catalog,
        start,
        end,
    )?;
    check_bridge_replacement(state, start, end, kind)
}

/// Detecta el reemplazo nativo de un puente ferroviario ya existente.
///
/// `CmdBuildBridge` conserva `HasTunnelBridgeReservation` al cambiar sólo el
/// tipo visual del puente. La reserva no vive en `m2_hi`: es el bit 4 de `m5`
/// de ambas rampas, por lo que debemos capturarla antes de reescribirlas.
fn rail_bridge_replacement_has_reservation(map: &Map, start: TileCoord, end: TileCoord) -> bool {
    let Some(tile) = map.get(start) else {
        return false;
    };
    bridge_pair_matches(map, start, end, TileKind::RailBridge)
        && check_tunnel_bridge_transport(tile, true)
        && crate::tunnel_bridge_rail_reserved(tile)
}

fn tunnel_bridge_clear_plan(
    state: &GameState,
    c: TileCoord,
) -> Result<TunnelBridgeClearPlan, CommandError> {
    check_in_bounds(&state.map, c)?;
    let start = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let Some(is_rail) = tunnel_bridge_kind(start.kind) else {
        return Err(CommandError::InvalidTunnelEndpoints);
    };
    let is_tunnel = matches!(start.kind, TileKind::RoadTunnel | TileKind::RailTunnel);
    if !start.is_tunnel_bridge_tile() || !check_tunnel_bridge_transport(start, is_rail) {
        return Err(CommandError::InvalidTunnelEndpoints);
    }

    let end = if is_tunnel {
        resolve_existing_tunnel_end(&state.map, c)
    } else if is_rail {
        rail_bridge_other_end(&state.map, c)
    } else {
        road_bridge_other_end(&state.map, c)
    }
    .ok_or(CommandError::InvalidTunnelEndpoints)?;
    let end_tile = state.map.get(end).ok_or(CommandError::OutOfBounds)?;
    if end_tile.kind != start.kind
        || !end_tile.is_tunnel_bridge_tile()
        || !check_tunnel_bridge_transport(end_tile, is_rail)
    {
        return Err(CommandError::InvalidTunnelEndpoints);
    }
    check_tunnel_bridge_owner(state, c)?;
    check_tunnel_bridge_owner(state, end)?;
    if state
        .vehicles
        .iter()
        .any(|vehicle| vehicle.pos == c || vehicle.pos == end)
    {
        return Err(CommandError::VehicleInTheWay);
    }

    let line = if is_tunnel {
        if start.m5 & 0x80 != 0 || end_tile.m5 & 0x80 != 0 {
            return Err(CommandError::InvalidTunnelEndpoints);
        }
        let (step_x, step_y) = diag_dir_offset(start.m5 & 0x03);
        let next = TileCoord::new(c.x + step_x, c.y + step_y);
        let line = axis_line(c, end);
        if line.len() < 2
            || line.get(1).copied() != Some(next)
            || end_tile.m5 & 0x03 != reverse_diag_dir(start.m5 & 0x03)
        {
            return Err(CommandError::InvalidTunnelEndpoints);
        }
        line
    } else {
        if start.m5 & 0x80 == 0 || end_tile.m5 & 0x80 == 0 {
            return Err(CommandError::InvalidBridgeSpan);
        }
        let line = bridge_line_tiles(c, end);
        let Some(&next) = line.get(1) else {
            return Err(CommandError::InvalidBridgeSpan);
        };
        let (step_x, step_y) = diag_dir_offset(start.m5 & 0x03);
        let expected_next = TileCoord::new(c.x + step_x, c.y + step_y);
        let axis_y = c.x == end.x;
        if line.len() < 3
            || c.x != end.x && c.y != end.y
            || next != expected_next
            || end_tile.m5 & 0x03 != reverse_diag_dir(start.m5 & 0x03)
        {
            return Err(CommandError::InvalidBridgeSpan);
        }
        for middle in &line[1..line.len() - 1] {
            if state
                .map
                .get(*middle)
                .is_none_or(|tile| bridge_above_axis_from_mapt(tile.mapt) != Some(axis_y))
            {
                return Err(CommandError::InvalidBridgeSpan);
            }
        }
        line
    };

    let clear_tiles = if is_tunnel {
        line.iter()
            .enumerate()
            .filter_map(|(index, tile)| {
                let is_endpoint = index == 0 || index + 1 == line.len();
                let is_synthetic_middle = state
                    .map
                    .get(*tile)
                    .is_some_and(|raw| raw.kind == start.kind && raw.is_tunnel_bridge_tile());
                (is_endpoint || is_synthetic_middle).then_some(*tile)
            })
            .collect()
    } else {
        vec![c, end]
    };

    Ok(TunnelBridgeClearPlan {
        kind: start.kind,
        end,
        line,
        clear_tiles,
        is_tunnel,
    })
}

pub(in crate::command) fn check_clear_tunnel_or_bridge(
    state: &GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    tunnel_bridge_clear_plan(state, c).map(|_| ())
}

fn road_type_cost_multiplier(state: &GameState, road_type: crate::road_type::RoadType) -> u16 {
    state
        .road_type_catalog
        .iter()
        .find(|definition| definition.id == road_type)
        .map_or(0, |definition| definition.cost_multiplier)
}

fn tunnel_bridge_clear_cost(state: &GameState, plan: &TunnelBridgeClearPlan) -> i64 {
    let base = if plan.is_tunnel {
        tunnel_clear_cost(&state.global_economy)
    } else {
        bridge_clear_cost(&state.global_economy)
    };
    let Some(tile) = state.map.get(plan.line[0]) else {
        return 0;
    };
    let transport = if matches!(plan.kind, TileKind::RoadTunnel | TileKind::RoadBridge) {
        let road_type = crate::road_type::road_type_from_tile(&tile);
        let road = road_clear_cost_factored(
            &state.global_economy,
            false,
            road_type_cost_multiplier(state, road_type),
        );
        let tram = crate::road_type::tram_road_type_from_tile(&tile).map_or(0, |tram_type| {
            road_clear_cost_factored(
                &state.global_economy,
                true,
                road_type_cost_multiplier(state, tram_type),
            )
        });
        road.saturating_mul(2)
            .saturating_add(tram.saturating_mul(2))
    } else {
        let rail_type = crate::rail_type::rail_type_from_tile(tile);
        let multiplier = crate::rail_type::rail_build_cost_multiplier_for_type(
            rail_type,
            &state.runtime.rail_type_props,
        );
        rail_clear_cost(&state.global_economy, multiplier)
    };
    base.saturating_add(transport)
        .saturating_mul(i64::try_from(plan.line.len()).unwrap_or(i64::MAX))
}

fn clear_structure_square(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0, 0)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_m2_u16(c, 0)
        .map_err(|_| CommandError::OutOfBounds)?;
    crate::command::sign::remove_signs_at(state, c);
    Ok(())
}

fn remove_jgr_tunnel_record(state: &mut GameState, start: TileCoord, end: TileCoord) {
    let (width, height) = state.map.dimensions();
    state.jgr_tunnels_from_footer.retain(|record| {
        let Some(record_start) = openttd_tile_index_to_coord(record.tile_n, width, height) else {
            return true;
        };
        let Some(record_end) = openttd_tile_index_to_coord(record.tile_s, width, height) else {
            return true;
        };
        !((record_start == start && record_end == end)
            || (record_start == end && record_end == start))
    });
}

pub(in crate::command) fn clear_tunnel_or_bridge(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    let plan = tunnel_bridge_clear_plan(state, c)?;
    let cost = tunnel_bridge_clear_cost(state, &plan);
    if matches!(plan.kind, TileKind::RailTunnel | TileKind::RailBridge) {
        crate::rail_pbs::clear_train_reservations_on_tiles(
            &mut state.map,
            &mut state.vehicles,
            &plan.clear_tiles,
            &mut state.runtime.reservation_tiles_active,
            &mut state.runtime.reservation_tile_dirty,
        );
    }
    if plan.is_tunnel {
        for tile in &plan.clear_tiles {
            clear_structure_square(state, *tile)?;
        }
        remove_jgr_tunnel_record(state, c, plan.end);
    } else {
        clear_structure_square(state, c)?;
        clear_structure_square(state, plan.end)?;
        for middle in &plan.line[1..plan.line.len() - 1] {
            let mut tile = state.map.get(*middle).ok_or(CommandError::OutOfBounds)?;
            tile.mapt &= !0x0C;
            state
                .map
                .set_tile(*middle, tile)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
    }
    if matches!(plan.kind, TileKind::RailTunnel | TileKind::RailBridge) {
        super::rail::refresh_rail_neighbors(state, c)?;
        super::rail::refresh_rail_neighbors(state, plan.end)?;
        crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, c);
        crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, plan.end);
    }
    state.economy.money -= cost;
    Ok(())
}

fn tile_max_z(map: &Map, c: TileCoord) -> Option<u8> {
    tile_slope_and_z(map, c).map(|(slope, z)| {
        z.saturating_add(if slope == 0 {
            0
        } else if slope & crate::SLOPE_STEEP != 0 {
            2
        } else {
            1
        })
    })
}

/// Comprueba el despeje de una parada vial bajo el tablero del puente.
///
/// `OpenTTD` llama a `CheckBuildAbove` para cada tesela intermedia y usa la
/// entrada de `RoadStopSpec::bridgeable_info` correspondiente al layout de
/// `m5`. Las paradas vanilla y las custom sin spec conservan el fallback
/// histórico de este motor (sin restricción `NewGRF`).
fn check_bridgeable_road_stop(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[crate::road_stop_spec::RoadStopSpecDef],
    c: TileCoord,
    bridge_height: u8,
) -> Result<(), CommandError> {
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Station {
        return Ok(());
    }
    let Some(station) = station_at_tile(map, stations, c) else {
        return Ok(());
    };
    if !matches!(station.stop_kind, StopKind::BusStop | StopKind::TruckStop) {
        return Ok(());
    }
    let Some(spec_id) = station.road_stop_spec_at(c) else {
        return Ok(());
    };
    let Some(spec) = crate::road_stop_spec::road_stop_spec_def(road_stop_catalog, spec_id) else {
        return Ok(());
    };
    let layout = usize::from(tile.m5 & 0x0F);
    let info = spec
        .bridgeable_info
        .get(layout)
        .copied()
        .unwrap_or_default();
    if info.min_height == 0
        || tile_max_z(map, c)
            .is_some_and(|max_z| max_z.saturating_add(info.min_height) > bridge_height)
    {
        return Err(CommandError::BridgeTooLowForRoadStop);
    }
    Ok(())
}

pub(crate) fn check_bridge_with_stations(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[crate::road_stop_spec::RoadStopSpecDef],
    a: TileCoord,
    b: TileCoord,
) -> Result<(), CommandError> {
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
            check_bridgeable_road_stop(
                map,
                stations,
                road_stop_catalog,
                *c,
                start_z.saturating_add(1),
            )?;
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

pub(crate) fn check_tunnel_or_bridge_with_stations(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[crate::road_stop_spec::RoadStopSpecDef],
    a: TileCoord,
    b: TileCoord,
    is_tunnel: bool,
) -> Result<(), CommandError> {
    if is_tunnel {
        check_tunnel(map, a)
    } else {
        check_bridge_with_stations(map, stations, road_stop_catalog, a, b)
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
    check_tunnel_or_bridge_with_stations(
        &state.map,
        &state.stations,
        &state.road_stop_spec_catalog,
        a,
        b,
        is_tunnel,
    )?;
    if !is_tunnel {
        check_bridge_replacement(state, a, b, kind_to_place)?;
    }
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
    let preserve_rail_bridge_reservation =
        !is_tunnel && is_rail && rail_bridge_replacement_has_reservation(&state.map, a, b);
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
                tile.m5 = bridge_ramp_m5(is_rail, dir)
                    | if preserve_rail_bridge_reservation {
                        0x10
                    } else {
                        0
                    };
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
