use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{TileCoord, TileKind, apply_command};

use crate::iso::world_pos_to_tile_coord;
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending};
use crate::state::SimWorld;
use crate::ui::hud::{HudBuildFeedback, SelectedTileInfo, push_build_command_error};
use crate::ui::industry_panel::IndustryPanelState;

use super::commands::command_for_action;
use super::drag::{
    action_is_tunnel, action_supports_drag, apply_drag_action, drag_line_tiles,
    tunnel_placement_is_valid,
};
use super::placement::cancel_placement;
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::minimap::minimap_contains_cursor;
use crate::ui::toolbar::minimap::{MinimapCell, MinimapRoot};
use crate::ui::toolbar::order_panel::{
    handle_order_destination_click, start_order_destination_pick,
};
use crate::ui::toolbar::station_panel::StationCargoPanelState;
use crate::ui::toolbar::{
    BuildMenuAction, BuildMenuUi, DragBuildState, OrderEditState, StationBuildState, UiToolState,
};

/// Dos clicks: el primero ancla el ghost, el segundo confirma. Click derecho cancela.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &Transform), (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
    mut selected: ResMut<SelectedTileInfo>,
    mut sim: ResMut<SimWorld>,
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
    mut order_state: ResMut<OrderEditState>,
    mut depot_state: ResMut<DepotPanelState>,
    mut station_panel: ResMut<StationCargoPanelState>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut industry_panel: ResMut<IndustryPanelState>,
    toolbar_pointer: Query<
        &Interaction,
        (
            With<BuildMenuUi>,
            Without<MinimapRoot>,
            Without<MinimapCell>,
        ),
    >,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if mouse.just_pressed(MouseButton::Right) && drag_state.armed {
        cancel_placement(&mut drag_state);
        return;
    }

    if toolbar_pointer.iter().any(|i| *i != Interaction::None)
        && mouse.just_pressed(MouseButton::Left)
    {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    if minimap_contains_cursor(cursor_pos, window) {
        return;
    }
    let Ok((camera, cam_tf)) = cam_q.single() else {
        return;
    };
    let cam_global = GlobalTransform::from(*cam_tf);
    let Ok(world_pos) = camera.viewport_to_world_2d(&cam_global, cursor_pos) else {
        return;
    };

    let Some((tx, ty)) = world_pos_to_tile_coord(world_pos, &sim.state.map) else {
        return;
    };
    let pos = TileCoord::new(tx, ty);

    if mouse.just_pressed(MouseButton::Left) && tool_state.active_tool.is_none() {
        selected.pos = Some(pos);
    }

    let orders_mode =
        order_state.picking_destination || tool_state.active_tool == Some(BuildMenuAction::Orders);
    if orders_mode {
        if order_state.vehicle_id.is_some()
            && handle_order_destination_click(
                &mouse,
                pos,
                &mut order_state,
                &mut sim,
                &mut pending,
                &mut hud_feedback,
                time.elapsed_secs(),
            )
        {
            return;
        }
        if tool_state.active_tool == Some(BuildMenuAction::Orders)
            && mouse.just_pressed(MouseButton::Left)
            && let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.pos == pos)
        {
            order_state.vehicle_id = Some(vehicle.id);
            order_state.orders = vehicle.orders.clone();
            start_order_destination_pick(&mut order_state);
            return;
        }
        return;
    }

    let Some(action) = tool_state.active_tool else {
        cancel_placement(&mut drag_state);
        if mouse.just_pressed(MouseButton::Left) {
            let tile_kind = sim.state.map.get_kind(pos);
            match tile_kind {
                Some(TileKind::Industry) => {
                    industry_panel.open = true;
                    industry_panel.focus_tile = Some(pos);
                    depot_state.depot_pos = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    order_state.picking_destination = false;
                    station_panel.station_pos = None;
                    return;
                }
                Some(TileKind::RoadDepot) | Some(TileKind::RailDepot) => {
                    depot_state.depot_pos = Some(pos);
                    depot_state.selected_vehicle = sim
                        .state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.pos == pos)
                        .map(|vehicle| vehicle.id);
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    order_state.picking_destination = false;
                    station_panel.station_pos = None;
                    industry_panel.open = false;
                    return;
                }
                Some(TileKind::Station) => {
                    station_panel.station_pos = Some(pos);
                    depot_state.depot_pos = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    order_state.picking_destination = false;
                    industry_panel.open = false;
                    return;
                }
                _ => {}
            }
            if let Some(vehicle) = sim.state.vehicles.iter().find(|vehicle| vehicle.pos == pos) {
                order_state.vehicle_id = Some(vehicle.id);
                order_state.orders = vehicle.orders.clone();
                order_state.picking_destination = false;
                depot_state.depot_pos = None;
                station_panel.station_pos = None;
                industry_panel.open = false;
                return;
            }
            depot_state.depot_pos = None;
            station_panel.station_pos = None;
            industry_panel.open = false;
        }
        return;
    };

    let current = (tx, ty);

    if action_supports_drag(action) {
        if !drag_state.armed || drag_state.last_action != Some(action) {
            if mouse.just_pressed(MouseButton::Left) {
                drag_state.armed = true;
                drag_state.start_tile = Some(current);
                drag_state.last_tile = Some(current);
                drag_state.last_action = Some(action);
                drag_state.pending_tiles = vec![current];
            }
            return;
        }

        let start = drag_state.start_tile.unwrap_or(current);
        drag_state.pending_tiles = drag_line_tiles(action, start, current);
        drag_state.last_tile = Some(current);

        if mouse.just_pressed(MouseButton::Left) {
            if action_is_tunnel(action)
                && !tunnel_placement_is_valid(&sim.state, action, &drag_state.pending_tiles)
            {
                return;
            }
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let (changed, err) = apply_drag_action(&mut sim, action, tiles, &station_state);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            } else if let Some(e) = err {
                push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
            }
        } else if mouse.just_released(MouseButton::Left) && drag_state.pending_tiles.len() == 1 {
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let (changed, err) = apply_drag_action(&mut sim, action, tiles, &station_state);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            } else if let Some(e) = err {
                push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
            }
        }
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    if let Some(cmd) = command_for_action(action, TileCoord::new(tx, ty), &station_state) {
        if let Err(e) = apply_command(&mut sim.state, &cmd) {
            push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
        } else {
            pending.pending = true;
        }
    }
}
