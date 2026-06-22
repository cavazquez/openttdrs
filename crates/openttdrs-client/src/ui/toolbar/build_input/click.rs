use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{TileCoord, TileKind, apply_command};

use crate::iso::{world_pos_to_tile_coord, world_pos_to_tile_fract};
use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, pick_vehicle_id_at_world,
    town_id_at_label_pos,
};
use crate::state::SimWorld;
use crate::ui::hud::{HudBuildFeedback, SelectedTileInfo, push_build_command_error};
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::town_window::{TownWindowState, town_for_house_tile};
use crate::ui::vehicle_window::VehicleWindowState;

use super::commands::command_for_action;
use super::drag::{
    action_is_tunnel, action_supports_drag, apply_drag_action, drag_line_tiles,
    tunnel_placement_is_valid,
};
use super::placement::cancel_placement;
use super::rail_lane::rail_lane_bits_for_action;
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

/// Estados de paneles/ventanas mutuamente excluyentes, agrupados para no
/// exceder el límite de parámetros de sistema de Bevy.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PanelStates<'w> {
    order: ResMut<'w, OrderEditState>,
    depot: ResMut<'w, DepotPanelState>,
    station: ResMut<'w, StationCargoPanelState>,
    industry: ResMut<'w, IndustryPanelState>,
    town: ResMut<'w, TownWindowState>,
    vehicle: ResMut<'w, VehicleWindowState>,
}

/// Clic en un vehículo del mapa: abre su ventana flotante (las órdenes se
/// abren desde el botón «Órdenes» de esa ventana).
#[allow(clippy::too_many_arguments)]
fn select_vehicle_on_map(
    order_state: &mut OrderEditState,
    depot_state: &mut DepotPanelState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    vehicle: &openttdrs_core::Vehicle,
) {
    vehicle_window.vehicle_id = Some(vehicle.id);
    order_state.vehicle_id = None;
    order_state.orders.clear();
    order_state.picking_destination = false;
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    station_panel.station_pos = None;
    industry_panel.open = false;
    town_window.town_id = None;
}

#[allow(clippy::too_many_arguments)]
fn open_town_window(
    town_window: &mut TownWindowState,
    depot_state: &mut DepotPanelState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    order_state: &mut OrderEditState,
    vehicle_window: &mut VehicleWindowState,
    town_id: u32,
) {
    town_window.town_id = Some(town_id);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    station_panel.station_pos = None;
    industry_panel.open = false;
    order_state.vehicle_id = None;
    order_state.orders.clear();
    order_state.picking_destination = false;
    vehicle_window.vehicle_id = None;
}

/// Dos clicks: el primero ancla el ghost, el segundo confirma. Click derecho cancela.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &Transform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut selected: ResMut<SelectedTileInfo>,
    mut sim: ResMut<SimWorld>,
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
    mut panels: PanelStates,
    mut pending: ResMut<RemapMapVisualsPending>,
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
    let order_state = &mut *panels.order;
    let depot_state = &mut *panels.depot;
    let station_panel = &mut *panels.station;
    let industry_panel = &mut *panels.industry;
    let town_window = &mut *panels.town;
    let vehicle_window = &mut *panels.vehicle;

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
        if mouse.just_pressed(MouseButton::Left)
            && let Some(vehicle_id) = pick_vehicle_id_at_world(world_pos, &sim)
            && let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
        {
            order_state.vehicle_id = Some(vehicle.id);
            order_state.orders = vehicle.orders.clone();
            order_state.picking_destination = false;
            return;
        }
        if order_state.vehicle_id.is_some()
            && handle_order_destination_click(
                &mouse,
                pos,
                order_state,
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
            && let Some(vehicle_id) = pick_vehicle_id_at_world(world_pos, &sim)
            && let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
        {
            order_state.vehicle_id = Some(vehicle.id);
            order_state.orders = vehicle.orders.clone();
            start_order_destination_pick(order_state);
            return;
        }
        return;
    }

    let Some(action) = tool_state.active_tool else {
        cancel_placement(&mut drag_state);
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(vehicle_id) = pick_vehicle_id_at_world(world_pos, &sim)
                && let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
            {
                select_vehicle_on_map(
                    order_state,
                    depot_state,
                    station_panel,
                    industry_panel,
                    town_window,
                    vehicle_window,
                    vehicle,
                );
                return;
            }

            if let Some(town_id) = town_id_at_label_pos(&sim, world_pos) {
                open_town_window(
                    town_window,
                    depot_state,
                    station_panel,
                    industry_panel,
                    order_state,
                    vehicle_window,
                    town_id,
                );
                return;
            }

            let tile_kind = sim.state.map.get_kind(pos);
            match tile_kind {
                Some(TileKind::Industry) => {
                    industry_panel.open = true;
                    industry_panel.focus_tile = Some(pos);
                    depot_state.depot_pos = None;
                    depot_state.selected_vehicle = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    order_state.picking_destination = false;
                    station_panel.station_pos = None;
                    town_window.town_id = None;
                    vehicle_window.vehicle_id = None;
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
                    town_window.town_id = None;
                    vehicle_window.vehicle_id = None;
                    return;
                }
                Some(TileKind::Station) => {
                    station_panel.station_pos = Some(pos);
                    depot_state.depot_pos = None;
                    depot_state.selected_vehicle = None;
                    order_state.vehicle_id = None;
                    order_state.orders.clear();
                    order_state.picking_destination = false;
                    industry_panel.open = false;
                    town_window.town_id = None;
                    vehicle_window.vehicle_id = None;
                    return;
                }
                Some(TileKind::House) => {
                    if let Some(town_id) = town_for_house_tile(&sim.state, pos) {
                        open_town_window(
                            town_window,
                            depot_state,
                            station_panel,
                            industry_panel,
                            order_state,
                            vehicle_window,
                            town_id,
                        );
                        return;
                    }
                }
                _ => {}
            }
            depot_state.depot_pos = None;
            depot_state.selected_vehicle = None;
            station_panel.station_pos = None;
            industry_panel.open = false;
            order_state.vehicle_id = None;
            order_state.orders.clear();
            order_state.picking_destination = false;
            town_window.town_id = None;
            vehicle_window.vehicle_id = None;
        }
        return;
    };

    let current = (tx, ty);
    let tile_fract = world_pos_to_tile_fract(world_pos, &sim.state.map, tx, ty);
    let rail_lane_bit = match action {
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert => {
            rail_lane_bits_for_action(action, Some(tile_fract))
        }
        _ => None,
    };

    if action_supports_drag(action) {
        if !drag_state.armed || drag_state.last_action != Some(action) {
            if mouse.just_pressed(MouseButton::Left) {
                drag_state.armed = true;
                drag_state.start_tile = Some(current);
                drag_state.last_tile = Some(current);
                drag_state.last_action = Some(action);
                drag_state.pending_tiles = vec![current];
                drag_state.rail_lane_bit = rail_lane_bit;
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
            let lane = drag_state.rail_lane_bit;
            let (changed, err) = apply_drag_action(&mut sim, action, tiles, &station_state, lane);
            cancel_placement(&mut drag_state);
            if changed {
                pending.pending = true;
            } else if let Some(e) = err {
                push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
            }
        } else if mouse.just_released(MouseButton::Left) && drag_state.pending_tiles.len() == 1 {
            let tiles = std::mem::take(&mut drag_state.pending_tiles);
            let lane = drag_state.rail_lane_bit;
            let (changed, err) = apply_drag_action(&mut sim, action, tiles, &station_state, lane);
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

    if let Some(cmd) = command_for_action(
        action,
        TileCoord::new(tx, ty),
        &station_state,
        rail_lane_bit,
    ) {
        if let Err(e) = apply_command(&mut sim.state, &cmd) {
            push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
        } else {
            pending.pending = true;
        }
    }
}
