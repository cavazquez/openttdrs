use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{CommandError, TileCoord, TileKind, apply_command};

use crate::iso::world_pos_to_tile_coord;
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending};
use crate::state::SimWorld;
use crate::ui::hud::{HudBuildFeedback, SelectedTileInfo};
use crate::ui::industry_panel::IndustryPanelState;

use super::commands::command_for_action;
use super::drag::{
    action_is_tunnel, action_supports_drag, apply_drag_action, drag_line_tiles,
    tunnel_placement_is_valid,
};
use super::orders::order_for_clicked_tile;
use super::placement::cancel_placement;
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::order_panel::apply_order_edit;
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
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if mouse.just_pressed(MouseButton::Right) && drag_state.armed {
        cancel_placement(&mut drag_state);
        return;
    }

    if menu_pointer.iter().any(|i| *i != Interaction::None) && mouse.just_pressed(MouseButton::Left)
    {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.single() else {
        return;
    };
    let cam_global = GlobalTransform::from(*cam_tf);
    let Ok(world_pos) = camera.viewport_to_world_2d(&cam_global, cursor_pos) else {
        return;
    };

    let Some((tx, ty)) = world_pos_to_tile_coord(world_pos, &sim.state.map) else {
        selected.pos = None;
        return;
    };
    let pos = TileCoord::new(tx, ty);
    selected.pos = Some(pos);

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
                    station_panel.station_pos = None;
                    return;
                }
                Some(TileKind::RoadDepot) => {
                    depot_state.depot_pos = Some(pos);
                    depot_state.selected_vehicle = sim
                        .state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.pos == pos)
                        .map(|vehicle| vehicle.id);
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    station_panel.station_pos = None;
                    industry_panel.open = false;
                    return;
                }
                Some(TileKind::Station) => {
                    station_panel.station_pos = Some(pos);
                    depot_state.depot_pos = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    industry_panel.open = false;
                    return;
                }
                _ => {}
            }
            if let Some(vehicle) = sim.state.vehicles.iter().find(|vehicle| vehicle.pos == pos) {
                order_state.vehicle_id = Some(vehicle.id);
                order_state.orders = vehicle.orders.clone();
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

    if action == BuildMenuAction::Orders {
        if mouse.just_pressed(MouseButton::Right) {
            order_state.vehicle_id = None;
            order_state.orders.clear();
            return;
        }
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.pos == pos) {
            order_state.vehicle_id = Some(vehicle.id);
            order_state.orders = vehicle.orders.clone();
            return;
        }
        let Some(vehicle_id) = order_state.vehicle_id else {
            return;
        };
        let Some(order) = order_for_clicked_tile(&sim, vehicle_id, pos) else {
            return;
        };
        order_state.orders.push(order);
        if apply_order_edit(&mut sim.state, vehicle_id, &order_state.orders).is_ok() {
            pending.pending = true;
        }
        return;
    }

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
                && !tunnel_placement_is_valid(&sim.state.map, action, &drag_state.pending_tiles)
            {
                return;
            }
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let changed = apply_drag_action(&mut sim, action, tiles, &station_state);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            }
        } else if mouse.just_released(MouseButton::Left) && drag_state.pending_tiles.len() == 1 {
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let changed = apply_drag_action(&mut sim, action, tiles, &station_state);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            }
        }
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    if let Some(cmd) = command_for_action(action, TileCoord::new(tx, ty), &station_state) {
        match apply_command(&mut sim.state, &cmd) {
            Ok(()) => {
                pending.pending = true;
            }
            Err(CommandError::StationNotAdjacentToTransport) => {
                hud_feedback.message =
                    Some("La parada necesita carretera o vía adyacente.".to_string());
                hud_feedback.expires_at_secs = time.elapsed_secs() + 5.0;
                hud_feedback.pending_soft_ping = true;
            }
            Err(_) => {}
        }
    }
}
