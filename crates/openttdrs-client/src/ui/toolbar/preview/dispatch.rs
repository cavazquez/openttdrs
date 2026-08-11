//! Dispatch: decidir qué tipo de preview construir según la herramienta activa.

use openttdrs_core::prelude::*;
use openttdrs_core::{is_tunnel_entrance_slope, tile_slope_and_z};

use crate::iso::tile_slope_and_min_z;
use crate::sprites::road_flat_sprite_index;
use crate::ui::toolbar::BuildMenuAction;

use super::industry::industry_spec_for_action;
use super::plan::{
    PreviewContext, PreviewPlan, TilePreviewKind, TilePreviewPlan, compute_preview_tiles,
};
use super::road_stop::road_stop_preview_dir;
use super::validation::{action_is_tunnel, preview_bridge_span_valid, preview_build_command_valid};

/// Construye el plan de preview según el contexto.
pub(crate) fn build_preview_plan(ctx: &PreviewContext, game_state: &GameState) -> PreviewPlan {
    let action = ctx.action;
    let (tx, ty) = ctx.cursor_tile;

    // Caso especial: estación de tren
    if action == BuildMenuAction::RailStation {
        return PreviewPlan::RailStation {
            origin: TileCoord::new(tx, ty),
            show_coverage: ctx.station_state.rail_show_coverage,
        };
    }

    // Caso especial: aeropuerto
    if action == BuildMenuAction::Airport {
        return PreviewPlan::Airport {
            origin: TileCoord::new(tx, ty),
            show_coverage: ctx.station_state.airport_show_coverage,
        };
    }

    // Caso especial: waypoint ferroviario
    if action == BuildMenuAction::RailWaypoint {
        let coord = TileCoord::new(tx, ty);
        if game_state.map.get(coord).is_none() {
            return PreviewPlan::None;
        }
        let valid = preview_build_command_valid(
            game_state,
            action,
            coord,
            ctx.station_state,
            &[(tx, ty)],
            ctx.rail_lane_bit,
            Some(ctx.tile_fract),
        );
        return PreviewPlan::RailWaypoint { coord, valid };
    }

    // Caso especial: waypoint de carretera
    if action == BuildMenuAction::RoadWaypoint {
        let coord = TileCoord::new(tx, ty);
        if game_state.map.get(coord).is_none() {
            return PreviewPlan::None;
        }
        let valid = preview_build_command_valid(
            game_state,
            action,
            coord,
            ctx.station_state,
            &[(tx, ty)],
            ctx.rail_lane_bit,
            Some(ctx.tile_fract),
        );
        return PreviewPlan::RoadWaypoint { coord, valid };
    }

    // Caso especial: señales ferroviarias (handled por update_rail_signal_ghost_preview)
    if action == BuildMenuAction::RailSignals {
        // Si hay arrastre multi-tile, retornar plan especial
        let tiles = compute_preview_tiles(ctx);
        if tiles.len() > 1 {
            let signal_fract = if ctx.drag_state.armed {
                ctx.station_state
                    .signal_drag_fract
                    .unwrap_or(ctx.tile_fract)
            } else {
                ctx.tile_fract
            };
            return PreviewPlan::RailSignalDrag {
                tiles,
                signal_fract,
            };
        }
        // 1×1: handled por sistema dedicado
        return PreviewPlan::HandledByDedicatedSystem;
    }

    // Puentes: span completo
    if matches!(
        action,
        BuildMenuAction::RoadBridge | BuildMenuAction::RailBridge | BuildMenuAction::Aqueduct
    ) {
        let preview_tiles = compute_preview_tiles(ctx);
        let valid = preview_tiles.len() >= 3
            && preview_tiles
                .iter()
                .all(|&(px, py)| game_state.map.get(TileCoord::new(px, py)).is_some())
            && preview_bridge_span_valid(game_state, action, &preview_tiles);
        return PreviewPlan::BridgeSpan {
            tiles: preview_tiles,
            valid,
        };
    }

    // Preview tile-by-tile para otras acciones
    let preview_tiles = compute_preview_tiles(ctx);
    let mut tile_plans = Vec::new();

    for &(px, py) in &preview_tiles {
        let coord = TileCoord::new(px, py);
        if game_state.map.get(coord).is_none() {
            continue;
        }

        let valid = preview_build_command_valid(
            game_state,
            action,
            coord,
            ctx.station_state,
            &preview_tiles,
            ctx.rail_lane_bit,
            Some(ctx.tile_fract),
        );

        // Dispatch según tipo de acción
        let kind = dispatch_tile_kind(ctx, game_state, coord, &preview_tiles);
        if let Some(k) = kind {
            tile_plans.push(TilePreviewPlan {
                coord,
                valid,
                kind: k,
            });
        }
    }

    PreviewPlan::TileByTile { tiles: tile_plans }
}

/// Determina el tipo de preview para un tile individual.
fn dispatch_tile_kind(
    ctx: &PreviewContext,
    game_state: &GameState,
    coord: TileCoord,
    preview_tiles: &[(i32, i32)],
) -> Option<TilePreviewKind> {
    let action = ctx.action;

    // Industrias
    if let Some(spec) = industry_spec_for_action(action) {
        return Some(TilePreviewKind::Industry { spec });
    }

    // Paradas de carretera (bus/camión)
    if matches!(action, BuildMenuAction::BusStop | BuildMenuAction::Station) {
        let dir = road_stop_preview_dir(ctx.station_state.orientation);
        return Some(TilePreviewKind::RoadStop {
            is_bus: action == BuildMenuAction::BusStop,
            dir,
        });
    }

    // Convertir vía: ghost con trackbits existentes y tipo destino seleccionado.
    if action == BuildMenuAction::RailConvert {
        return rail_convert_preview_kind(game_state, coord);
    }

    // Vía ferroviaria (bit por tesela: en L Manhattan X/Y/curva coinciden con el path)
    if let Some(bits) = rail_preview_bits(action, coord, preview_tiles, ctx.rail_lane_bit) {
        let (tileh, _) = tile_slope_and_min_z(&game_state.map, coord.x as u32, coord.y as u32);
        return Some(TilePreviewKind::Rail {
            bits,
            tileh,
            rail_type: game_state.current_rail_type,
        });
    }

    // Depósito de carretera
    if action == BuildMenuAction::RoadDepot {
        return Some(TilePreviewKind::RoadDepot {
            dir: road_stop_preview_dir(ctx.station_state.orientation),
        });
    }

    // Depósito ferroviario
    if action == BuildMenuAction::RailDepot {
        return Some(TilePreviewKind::RailDepot {
            dir: road_stop_preview_dir(ctx.station_state.orientation),
            rail_type: game_state.current_rail_type,
        });
    }

    // Carretera/tranvía
    if let Some((_bits, path)) = road_preview_at(&game_state.map, action, coord, preview_tiles) {
        return Some(TilePreviewKind::Road { path });
    }

    // Túnel
    if action_is_tunnel(action) {
        let (tileh, _) = tile_slope_and_min_z(&game_state.map, coord.x as u32, coord.y as u32);
        if is_tunnel_entrance_slope(tileh) {
            return Some(TilePreviewKind::Tunnel);
        }
        return None;
    }

    // Sprite genérico para otras acciones
    Some(TilePreviewKind::GenericSprite)
}

/// Ghost de conversión: misma geometría de vía, tint/tipo = `current_rail_type`.
fn rail_convert_preview_kind(game_state: &GameState, coord: TileCoord) -> Option<TilePreviewKind> {
    use openttdrs_core::{OTTD_MP_RAILWAY, TileKind, effective_rail_trackbits};

    let tile = game_state.map.get(coord)?;
    if tile.kind != TileKind::Rail {
        return None;
    }
    let bits = effective_rail_trackbits(tile.mapt, tile.m5, tile.kind, OTTD_MP_RAILWAY)
        .unwrap_or(tile.m5 & 0x3F)
        & 0x3F;
    if bits == 0 {
        return None;
    }
    let (tileh, _) = tile_slope_and_min_z(&game_state.map, coord.x as u32, coord.y as u32);
    Some(TilePreviewKind::Rail {
        bits,
        tileh,
        rail_type: game_state.current_rail_type,
    })
}

/// Trackbits a previsualizar para las herramientas de vía.
fn rail_preview_bits(
    action: BuildMenuAction,
    coord: TileCoord,
    preview_tiles: &[(i32, i32)],
    rail_lane_bit: Option<u8>,
) -> Option<u8> {
    use crate::sprites::{RAIL_TB_X, RAIL_TB_Y};
    use crate::ui::toolbar::build_input::drag::rail_bits_for_drag_tile;

    let index = preview_tiles
        .iter()
        .position(|&(x, y)| x == coord.x && y == coord.y)
        .unwrap_or(0);
    match action {
        BuildMenuAction::RailX => Some(RAIL_TB_X),
        BuildMenuAction::RailY => Some(RAIL_TB_Y),
        BuildMenuAction::Rail => {
            rail_bits_for_drag_tile(action, preview_tiles, index, rail_lane_bit).or(Some(RAIL_TB_X))
        }
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert | BuildMenuAction::RailRemove => {
            rail_lane_bit
        }
        _ => None,
    }
}

/// Bits y PNG de preview de carretera.
fn road_preview_at(
    map: &Map,
    action: BuildMenuAction,
    pos: TileCoord,
    preview_tiles: &[(i32, i32)],
) -> Option<(u8, String)> {
    use openttdrs_core::{
        infer_road_drag_axis, preview_road_bits_at, road_bits_for_autoroute, road_locked_tool_axis,
    };

    let (requested, _force_axis) = match action {
        BuildMenuAction::RoadX | BuildMenuAction::TramX => (0x0A, true),
        BuildMenuAction::RoadY | BuildMenuAction::TramY => (0x05, true),
        BuildMenuAction::Road | BuildMenuAction::Tram => (road_bits_for_autoroute(map, pos), false),
        _ => return None,
    };

    let tool_bits =
        super::super::build_input::drag::road_bits_for_drag_action(action, preview_tiles)
            .unwrap_or(requested);
    let start = preview_tiles
        .first()
        .map(|&(x, y)| TileCoord::new(x, y))
        .unwrap_or(pos);
    let end = preview_tiles
        .last()
        .map(|&(x, y)| TileCoord::new(x, y))
        .unwrap_or(pos);

    let axis = match action {
        BuildMenuAction::RoadX | BuildMenuAction::TramX => {
            road_locked_tool_axis(map, start, end, 0x0A)
        }
        BuildMenuAction::RoadY | BuildMenuAction::TramY => {
            road_locked_tool_axis(map, start, end, 0x05)
        }
        BuildMenuAction::Road | BuildMenuAction::Tram => {
            infer_road_drag_axis(map, start, end, tool_bits)
        }
        _ => tool_bits,
    };

    let bits = preview_road_bits_at(map, pos, axis, true);
    let tileh = tile_slope_and_z(map, pos).map(|(h, _)| h).unwrap_or(0);
    let idx = road_flat_sprite_index(tileh, bits);
    let prefix = if matches!(
        action,
        BuildMenuAction::Tram | BuildMenuAction::TramX | BuildMenuAction::TramY
    ) {
        "tram_flat"
    } else {
        "road_flat"
    };
    Some((bits, format!("assets/opengfx/tiles/{prefix}_{idx:02}.png")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::toolbar::{DragBuildState, StationBuildState};
    use openttdrs_core::Map;

    #[test]
    fn dispatch_rail_station_plan() {
        let map = Map::new_flat(10, 10, 0);
        let ctx = PreviewContext {
            map: &map,
            action: BuildMenuAction::RailStation,
            cursor_tile: (5, 5),
            tile_fract: (0, 0),
            station_state: &StationBuildState::default(),
            drag_state: &DragBuildState::default(),
            rail_lane_bit: None,
        };
        let state = GameState::new(10, 10);
        let plan = build_preview_plan(&ctx, &state);
        match plan {
            PreviewPlan::RailStation { origin, .. } => {
                assert_eq!(origin, TileCoord::new(5, 5));
            }
            _ => panic!("Expected RailStation plan"),
        }
    }

    #[test]
    fn rail_preview_bits_computes_trackbits() {
        use crate::sprites::RAIL_TB_X;
        let bits = rail_preview_bits(
            BuildMenuAction::RailX,
            TileCoord::new(5, 5),
            &[(5, 5)],
            None,
        );
        assert_eq!(bits, Some(RAIL_TB_X));
    }

    #[test]
    fn rail_convert_preview_uses_existing_bits_and_selected_type() {
        use crate::sprites::RAIL_TB_X;
        use openttdrs_core::{Command, RailType, apply_command};

        let mut state = GameState::new(10, 10);
        state.economy.money = 100_000;
        state.current_rail_type = RailType::Electric;
        let coord = TileCoord::new(4, 4);
        assert!(apply_command(&mut state, &Command::PlaceRailBits(coord, RAIL_TB_X)).is_ok());
        let Some(TilePreviewKind::Rail {
            bits, rail_type, ..
        }) = rail_convert_preview_kind(&state, coord)
        else {
            panic!("expected Rail convert ghost");
        };
        assert_eq!(bits, RAIL_TB_X);
        assert_eq!(rail_type, RailType::Electric);
        assert!(rail_convert_preview_kind(&state, TileCoord::new(5, 5)).is_none());
    }
}
