//! Aplicar intenciones de clic resueltas como efectos ECS (Commands, feedback, drag, paneles).

use bevy::prelude::*;
use openttdrs_core::{
    BridgeType, Command, CommandError, TileCoord, apply_command, command_would_fail,
};

use crate::render::{RemapMapVisualsPending, request_map_visual_remap};
use crate::state::{OrderPickState, SimWorld};
use crate::ui::hud::{
    HudBuildFeedback, SelectedTileInfo, enqueue_build_place_flash, push_build_command_error,
};
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::toolbar::bridge_window::{BridgeBuildState, PendingBridge};
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::order_panel::{
    open_order_edit_for_vehicle, start_order_destination_pick, try_append_order_at_tile,
};
use crate::ui::toolbar::preview::rail_signal_flash_position;
use crate::ui::toolbar::station_panel::StationCargoPanelState;
use crate::ui::toolbar::{BuildMenuAction, DragBuildState, OrderEditState, StationBuildState};
use crate::ui::town_window::TownWindowState;
use crate::ui::vehicle_window::VehicleWindowState;

use super::click_intent::MapClickIntent;
use super::commands::command_for_action;
use super::drag::{
    action_is_tunnel, apply_drag_action, drag_line_tiles, subsample_drag_tiles,
    tunnel_placement_is_valid,
};
use super::orders::order_pick_valid;
use super::placement::cancel_placement;
use super::remap_plan::tiles_for_visual_remap;
use super::selection;

/// Parámetros ECS para aplicar intenciones.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct IntentApplyContext<'w> {
    pub sim: ResMut<'w, SimWorld>,
    pub selected: ResMut<'w, SelectedTileInfo>,
    pub drag_state: ResMut<'w, DragBuildState>,
    pub station_state: ResMut<'w, StationBuildState>,
    pub bridge_state: ResMut<'w, BridgeBuildState>,
    pub pending: ResMut<'w, RemapMapVisualsPending>,
    pub hud_feedback: ResMut<'w, HudBuildFeedback>,
    pub order_state: ResMut<'w, OrderEditState>,
    pub pick_next: ResMut<'w, NextState<OrderPickState>>,
    pub depot_state: ResMut<'w, DepotPanelState>,
    pub station_panel: ResMut<'w, StationCargoPanelState>,
    pub industry_panel: ResMut<'w, IndustryPanelState>,
    pub town_window: ResMut<'w, TownWindowState>,
    pub vehicle_window: ResMut<'w, VehicleWindowState>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_intent(intent: MapClickIntent, ctx: &mut IntentApplyContext, time_secs: f32) {
    match intent {
        MapClickIntent::Ignore => {}
        MapClickIntent::CancelDrag => {
            cancel_placement(&mut ctx.drag_state);
            ctx.station_state.signal_drag_fract = None;
        }
        MapClickIntent::SelectTileForInspection(pos) => {
            ctx.selected.pos = Some(pos);
        }
        MapClickIntent::HandleOrderDestination(pos) => {
            // Replicar lógica de handle_order_destination_click sin verificar mouse
            let Some(vehicle_id) = ctx.order_state.vehicle_id else {
                return;
            };
            // Prioridad 1: añadir la parada válida
            if order_pick_valid(&ctx.sim, vehicle_id, pos) {
                match try_append_order_at_tile(
                    &mut ctx.sim,
                    vehicle_id,
                    pos,
                    &mut ctx.order_state.orders,
                ) {
                    Ok(()) => {
                        ctx.pending.pending = true;
                        ctx.order_state.selected_slot = ctx.order_state.orders.len().checked_sub(1);
                    }
                    Err(e) => {
                        ctx.order_state.orders.pop();
                        push_build_command_error(&mut ctx.hud_feedback, e, time_secs);
                    }
                }
                return;
            }
            // Prioridad 2: clic sobre otro vehículo
            if let Some(vehicle) = ctx.sim.state.vehicles.iter().find(|v| v.pos == pos) {
                open_order_edit_for_vehicle(&mut ctx.order_state, vehicle, &mut ctx.pick_next);
                return;
            }
            // Estación incompatible
            if ctx.sim.state.stations.iter().any(|s| s.pos == pos) {
                let err = ctx
                    .sim
                    .state
                    .vehicles
                    .iter()
                    .find(|v| v.id == vehicle_id)
                    .and_then(|v| {
                        ctx.sim
                            .state
                            .stations
                            .iter()
                            .find(|s| s.pos == pos)
                            .filter(|s| !s.can_service_vehicle(v.kind))
                            .map(|_| CommandError::IncompatibleStopForVehicle)
                    })
                    .unwrap_or(CommandError::StationNotFound);
                push_build_command_error(&mut ctx.hud_feedback, err, time_secs);
            }
        }
        MapClickIntent::StartOrderEditForVehicle(vehicle_id) => {
            if let Some(vehicle) = ctx.sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                open_order_edit_for_vehicle(&mut ctx.order_state, vehicle, &mut ctx.pick_next);
                start_order_destination_pick(&ctx.order_state, &mut ctx.pick_next);
            }
        }
        MapClickIntent::SelectVehicleOnMap(vehicle_id) => {
            if let Some(vehicle) = ctx.sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                selection::select_vehicle_on_map(
                    &mut ctx.order_state,
                    &mut ctx.depot_state,
                    &mut ctx.station_panel,
                    &mut ctx.industry_panel,
                    &mut ctx.town_window,
                    &mut ctx.vehicle_window,
                    vehicle,
                );
            }
        }
        MapClickIntent::OpenTownWindow(town_id) => {
            selection::open_town_window(
                &mut ctx.town_window,
                &mut ctx.depot_state,
                &mut ctx.station_panel,
                &mut ctx.industry_panel,
                &mut ctx.order_state,
                &mut ctx.vehicle_window,
                town_id,
            );
        }
        MapClickIntent::OpenIndustryPanel(pos) => {
            selection::open_industry_panel(
                &mut ctx.industry_panel,
                &mut ctx.depot_state,
                &mut ctx.order_state,
                &mut ctx.station_panel,
                &mut ctx.town_window,
                &mut ctx.vehicle_window,
                pos,
            );
        }
        MapClickIntent::OpenDepotPanel {
            depot_pos,
            vehicle_id,
        } => {
            selection::open_depot_panel(
                &mut ctx.depot_state,
                &mut ctx.order_state,
                &mut ctx.station_panel,
                &mut ctx.industry_panel,
                &mut ctx.town_window,
                &mut ctx.vehicle_window,
                depot_pos,
                vehicle_id,
            );
        }
        MapClickIntent::OpenStationPanel(station_pos) => {
            selection::open_station_panel(
                &mut ctx.station_panel,
                &mut ctx.depot_state,
                &mut ctx.order_state,
                &mut ctx.industry_panel,
                &mut ctx.town_window,
                &mut ctx.vehicle_window,
                station_pos,
            );
        }
        MapClickIntent::StartDrag {
            action,
            start_tile,
            rail_lane_bit,
            signal_drag_fract,
            press_world_pos,
        } => {
            ctx.drag_state.armed = true;
            ctx.drag_state.start_tile = Some(start_tile);
            ctx.drag_state.last_tile = Some(start_tile);
            ctx.drag_state.last_action = Some(action);
            ctx.drag_state.pending_tiles = vec![start_tile];
            ctx.drag_state.rail_lane_bit = rail_lane_bit;
            ctx.drag_state.press_world_pos = Some(press_world_pos);
            if let Some(fract) = signal_drag_fract {
                ctx.station_state.signal_drag_fract = Some(fract);
            }
        }
        MapClickIntent::UpdateDrag {
            end_tile,
            signal_tap: _,
        } => {
            if let Some(action) = ctx.drag_state.last_action
                && let Some(start) = ctx.drag_state.start_tile
            {
                let line = drag_line_tiles(Some(&ctx.sim.state.map), action, start, end_tile);
                ctx.drag_state.pending_tiles = if action == BuildMenuAction::RailSignals {
                    subsample_drag_tiles(&line, ctx.station_state.signal_density)
                } else {
                    line
                };
                ctx.drag_state.last_tile = Some(end_tile);
            }
        }
        MapClickIntent::ConfirmDrag { signal_tap: _ } => {
            if let Some(action) = ctx.drag_state.last_action {
                let build_pos = ctx
                    .drag_state
                    .start_tile
                    .map(|(x, y)| TileCoord::new(x, y))
                    .unwrap_or(TileCoord::new(0, 0));
                confirm_drag_placement(
                    action,
                    &mut ctx.drag_state,
                    &mut ctx.sim,
                    &ctx.station_state,
                    build_pos,
                    &mut ctx.bridge_state,
                    &mut ctx.pending,
                    &mut ctx.hud_feedback,
                    time_secs,
                );
                ctx.station_state.signal_drag_fract = None;
                ctx.drag_state.press_world_pos = None;
            }
        }
        MapClickIntent::JoinStationClick { clicked, keep } => match keep {
            None => {
                ctx.station_state.join_keep = Some(clicked);
            }
            Some(k) if k == clicked => {
                ctx.station_state.join_keep = None;
            }
            Some(k) => {
                match apply_command(
                    &mut ctx.sim.state,
                    &Command::JoinStations {
                        keep: k,
                        merge: clicked,
                    },
                ) {
                    Ok(()) => {
                        ctx.station_state.join_keep = None;
                        let (mw, mh) = ctx.sim.state.map.dimensions();
                        request_map_visual_remap(&mut ctx.pending, mw, mh, &[]);
                    }
                    Err(e) => {
                        push_build_command_error(&mut ctx.hud_feedback, e, time_secs);
                    }
                }
            }
        },
        MapClickIntent::BuildImmediate {
            action,
            pos,
            rail_lane_bit,
            tile_fract,
            ctrl_held,
            cycle_signal,
        } => {
            let mut sig_type = ctx.station_state.signal_type;
            let cycle = cycle_signal;
            if ctrl_held && action == BuildMenuAction::RailSignals && !cycle_signal {
                sig_type = openttdrs_core::next_placeable_signal_type(sig_type);
            }
            if let Some(cmd) = command_for_action(
                action,
                pos,
                &ctx.station_state,
                rail_lane_bit,
                Some(&ctx.sim.state.map),
                Some(tile_fract),
                sig_type,
                cycle,
            ) {
                if let Err(e) = apply_command(&mut ctx.sim.state, &cmd) {
                    push_build_command_error(&mut ctx.hud_feedback, e, time_secs);
                } else {
                    if ctrl_held && action == BuildMenuAction::RailSignals && !cycle {
                        ctx.station_state.signal_type = sig_type;
                    }
                    let (mw, mh) = ctx.sim.state.map.dimensions();
                    let tiles = tiles_for_visual_remap(Some(&ctx.sim.state.map), action, pos, &[]);
                    request_map_visual_remap(&mut ctx.pending, mw, mh, &tiles);
                    if action == BuildMenuAction::RailSignals
                        && let Some(flash_pos) = rail_signal_flash_position(
                            &ctx.sim.state.map,
                            pos,
                            ctx.station_state.orientation,
                            tile_fract.0,
                            tile_fract.1,
                            ctx.sim.state.tick,
                        )
                    {
                        enqueue_build_place_flash(&mut ctx.hud_feedback, flash_pos);
                    }
                }
            }
        }
    }
}

/// Arrastrar: clic para anclar, mover el ratón y soltar para confirmar. Clic derecho cancela.
#[allow(clippy::too_many_arguments)]
fn confirm_drag_placement(
    action: BuildMenuAction,
    drag_state: &mut DragBuildState,
    sim: &mut SimWorld,
    station_state: &StationBuildState,
    build_pos: TileCoord,
    bridge_state: &mut BridgeBuildState,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    time_secs: f32,
) {
    if matches!(
        action,
        BuildMenuAction::RoadBridge | BuildMenuAction::RailBridge
    ) {
        let tiles = std::mem::take(&mut drag_state.pending_tiles);
        cancel_placement(drag_state);
        if tiles.len() < 3 {
            push_build_command_error(hud_feedback, CommandError::InvalidBridgeSpan, time_secs);
            return;
        }
        let start = TileCoord::new(tiles[0].0, tiles[0].1);
        let end = TileCoord::new(tiles[tiles.len() - 1].0, tiles[tiles.len() - 1].1);
        let probe = if action == BuildMenuAction::RoadBridge {
            Command::PlaceRoadBridge(start, end, BridgeType::Wooden)
        } else {
            Command::PlaceRailBridge(start, end, BridgeType::Wooden)
        };
        match command_would_fail(&sim.state, &probe) {
            None
            | Some(CommandError::BridgeTypeNotAvailable)
            | Some(CommandError::InsufficientFunds) => {
                bridge_state.pending = Some(PendingBridge {
                    start,
                    end,
                    road: action == BuildMenuAction::RoadBridge,
                });
            }
            Some(e) => {
                push_build_command_error(hud_feedback, e, time_secs);
            }
        }
        return;
    }

    if action_is_tunnel(action)
        && !tunnel_placement_is_valid(&sim.state, action, &drag_state.pending_tiles)
    {
        cancel_placement(drag_state);
        return;
    }

    let tiles = std::mem::take(&mut drag_state.pending_tiles);
    let remap_tiles = tiles_for_visual_remap(Some(&sim.state.map), action, build_pos, &tiles);
    let lane = drag_state.rail_lane_bit;
    let (changed, err) = apply_drag_action(sim, action, tiles, station_state, lane);
    cancel_placement(drag_state);
    if changed {
        let (mw, mh) = sim.state.map.dimensions();
        request_map_visual_remap(pending, mw, mh, &remap_tiles);
    } else if let Some(e) = err {
        push_build_command_error(hud_feedback, e, time_secs);
    }
}
