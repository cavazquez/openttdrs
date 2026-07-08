use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{
    BridgeType, Command, CommandError, TileCoord, TileKind, apply_command, command_would_fail,
    industry_template,
};

use crate::iso::{world_pos_to_tile_coord, world_pos_to_tile_fract};
use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, pick_vehicle_id_at_world,
    request_map_visual_remap, town_id_at_label_pos,
};
use crate::state::SimWorld;
use crate::ui::hud::{
    HoveredTileCoord, HudBuildFeedback, SelectedTileInfo, enqueue_build_place_flash,
    push_build_command_error,
};
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::save_window::SaveWindowState;
use crate::ui::toolbar::preview::rail_signal_flash_position;
use crate::ui::town_window::{TownWindowState, town_for_house_tile};
use crate::ui::vehicle_window::VehicleWindowState;

use super::commands::command_for_action;
use super::drag::{
    action_is_tunnel, action_supports_drag, apply_drag_action, drag_line_tiles,
    rail_action_refreshes_neighbors, rail_remap_neighbor_tiles, tunnel_placement_is_valid,
    tunnel_remap_tiles,
};
use super::placement::cancel_placement;
use super::rail_lane::rail_lane_bits_for_action;
use crate::ui::toolbar::bridge_window::{BridgeBuildState, PendingBridge};
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::minimap::minimap_contains_cursor;
use crate::ui::toolbar::minimap::{MinimapCell, MinimapRoot};
use crate::ui::toolbar::order_panel::{
    handle_order_destination_click, start_order_destination_pick,
};
use crate::ui::toolbar::preview::industry_spec_for_action;
use crate::ui::toolbar::station_panel::StationCargoPanelState;
use crate::ui::toolbar::{
    BuildMenuAction, BuildMenuUi, DragBuildState, OrderEditState, StationBuildState, UiToolState,
    open_order_edit_for_vehicle,
};

fn road_action_refreshes_neighbors(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::Road | BuildMenuAction::RoadX | BuildMenuAction::RoadY
    )
}

fn tiles_for_visual_remap(
    map: Option<&openttdrs_core::Map>,
    action: BuildMenuAction,
    origin: TileCoord,
    drag_tiles: &[(i32, i32)],
) -> Vec<(i32, i32)> {
    let base = if action_is_tunnel(action) {
        if let Some(map) = map {
            return tunnel_remap_tiles(map, drag_tiles);
        }
        let start = drag_tiles.first().copied().unwrap_or((origin.x, origin.y));
        vec![start]
    } else if drag_tiles.len() > 1 {
        drag_tiles.to_vec()
    } else if let Some(spec) = industry_spec_for_action(action) {
        industry_template(origin, spec)
            .into_iter()
            .map(|(c, _)| (c.x, c.y))
            .collect()
    } else if let Some(&(tx, ty)) = drag_tiles.first() {
        vec![(tx, ty)]
    } else {
        vec![(origin.x, origin.y)]
    };
    if (rail_action_refreshes_neighbors(action) || road_action_refreshes_neighbors(action))
        && let Some(map) = map
    {
        return rail_remap_neighbor_tiles(map, &base);
    }
    base
}

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
    order_state.clear();
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
    order_state.clear();
    vehicle_window.vehicle_id = None;
}

/// Arrastrar: clic para anclar, mover el ratón y soltar para confirmar. Clic derecho cancela.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
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

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    save_window: Option<Res<SaveWindowState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut selected: ResMut<SelectedTileInfo>,
    mut sim: ResMut<SimWorld>,
    tool_state: Res<UiToolState>,
    mut station_state: ResMut<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
    mut bridge_state: ResMut<BridgeBuildState>,
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
    hovered: Res<HoveredTileCoord>,
    time: Res<Time>,
) {
    let order_state = &mut *panels.order;
    let depot_state = &mut *panels.depot;
    let station_panel = &mut *panels.station;
    let industry_panel = &mut *panels.industry;
    let town_window = &mut *panels.town;
    let vehicle_window = &mut *panels.vehicle;

    if save_window.is_some_and(|w| w.open) {
        return;
    }

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
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_tf, cursor_pos) else {
        return;
    };

    let tile_on_map = world_pos_to_tile_coord(world_pos, &sim.state.map);
    if tile_on_map.is_none() {
        if drag_state.armed
            && mouse.just_released(MouseButton::Left)
            && let Some(action) = tool_state.active_tool
            && action_supports_drag(action)
            && drag_state.last_action == Some(action)
        {
            let build_pos = drag_state
                .start_tile
                .map(|(x, y)| TileCoord::new(x, y))
                .unwrap_or(TileCoord::new(0, 0));
            confirm_drag_placement(
                action,
                &mut drag_state,
                &mut sim,
                &station_state,
                build_pos,
                &mut bridge_state,
                &mut pending,
                &mut hud_feedback,
                time.elapsed_secs(),
            );
        }
        return;
    }
    let Some((tx, ty)) = tile_on_map else {
        return;
    };
    let pos = TileCoord::new(tx, ty);

    if mouse.just_pressed(MouseButton::Left) && tool_state.active_tool.is_none() {
        selected.pos = Some(pos);
    }

    let orders_mode =
        order_state.picking_destination || tool_state.active_tool == Some(BuildMenuAction::Orders);
    if orders_mode {
        // Modo selección de destino: el clic añade la parada clicada. Se procesa
        // primero para que un depósito con el tren dentro se añada como destino
        // en vez de reabrir las órdenes de ese vehículo.
        if order_state.picking_destination {
            if order_state.vehicle_id.is_some() {
                handle_order_destination_click(
                    &mouse,
                    pos,
                    order_state,
                    &mut sim,
                    &mut pending,
                    &mut hud_feedback,
                    time.elapsed_secs(),
                );
            }
            return;
        }
        // Herramienta «Órdenes» (sin selección activa): clic en un vehículo abre
        // sus órdenes y arma la selección de destino.
        if mouse.just_pressed(MouseButton::Left)
            && let Some(vehicle_id) = pick_vehicle_id_at_world(world_pos, &sim)
            && let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
        {
            open_order_edit_for_vehicle(order_state, vehicle);
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
                    order_state.clear();
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
                    order_state.clear();
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
                    order_state.clear();
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
        }
        return;
    };

    let current = (tx, ty);
    let (build_pos, tile_fract) = if action == BuildMenuAction::RailSignals {
        let Some(pos) = hovered.pos else {
            return;
        };
        (pos, (hovered.fract_x, hovered.fract_y))
    } else {
        (
            pos,
            world_pos_to_tile_fract(world_pos, &sim.state.map, tx, ty),
        )
    };
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
        drag_state.pending_tiles = drag_line_tiles(Some(&sim.state.map), action, start, current);
        drag_state.last_tile = Some(current);

        if mouse.just_released(MouseButton::Left) {
            confirm_drag_placement(
                action,
                &mut drag_state,
                &mut sim,
                &station_state,
                build_pos,
                &mut bridge_state,
                &mut pending,
                &mut hud_feedback,
                time.elapsed_secs(),
            );
        }
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let ctrl = station_state.ctrl_held;
    let mut sig_type = station_state.signal_type;
    let mut cycle_existing_signal_type = false;
    if ctrl && action == BuildMenuAction::RailSignals {
        let (fx, fy) = tile_fract;
        if let Some(tile) = sim.state.map.get(build_pos)
            && tile.kind == TileKind::Rail
            && openttdrs_core::rail_signals::rail_tile_is_signals(tile.m5)
        {
            let tb = tile.m5 & 0x3F;
            if let Some(track) = openttdrs_core::rail_signals::resolve_signal_track(tb, fx, fy)
                && openttdrs_core::rail_signals::rail_signal_present_mask(tile.m3)
                    & openttdrs_core::rail_signals::signal_on_track_mask(track)
                    != 0
            {
                cycle_existing_signal_type = true;
            }
        }
        if !cycle_existing_signal_type {
            sig_type = openttdrs_core::next_placeable_signal_type(sig_type);
        }
    }

    if let Some(cmd) = command_for_action(
        action,
        build_pos,
        &station_state,
        rail_lane_bit,
        Some(&sim.state.map),
        Some(tile_fract),
        sig_type,
        cycle_existing_signal_type,
    ) {
        if let Err(e) = apply_command(&mut sim.state, &cmd) {
            push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
        } else {
            if ctrl && action == BuildMenuAction::RailSignals && !cycle_existing_signal_type {
                station_state.signal_type = sig_type;
            }
            let (mw, mh) = sim.state.map.dimensions();
            let tiles = tiles_for_visual_remap(Some(&sim.state.map), action, build_pos, &[]);
            request_map_visual_remap(&mut pending, mw, mh, &tiles);
            if action == BuildMenuAction::RailSignals
                && let Some(flash_pos) = rail_signal_flash_position(
                    &sim.state.map,
                    build_pos,
                    station_state.orientation,
                    tile_fract.0,
                    tile_fract.1,
                    sim.state.tick,
                )
            {
                enqueue_build_place_flash(&mut hud_feedback, flash_pos);
            }
        }
    }
}

pub(crate) fn sync_build_pointer_modifiers(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut station_state: ResMut<crate::ui::toolbar::StationBuildState>,
) {
    station_state.ctrl_held =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
}
