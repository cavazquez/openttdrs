use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Command, Map, TileCoord, TileKind, VehicleKind, VehicleOrder, apply_command};

use crate::iso::world_pos_to_tile_coord;
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending};
use crate::state::SimWorld;
use crate::ui::hud::SelectedTileInfo;
use crate::ui::industry_panel::IndustryPanelState;

use super::depot_panel::DepotPanelState;
use super::order_panel::apply_order_edit;
use super::station_panel::StationCargoPanelState;
use super::{
    BuildMenuAction, BuildMenuUi, DragBuildState, OrderEditState, StationBuildState, UiToolState,
};

pub(crate) fn cancel_placement(drag_state: &mut DragBuildState) {
    drag_state.armed = false;
    drag_state.start_tile = None;
    drag_state.last_tile = None;
    drag_state.last_action = None;
    drag_state.pending_tiles.clear();
}

pub(crate) fn order_for_clicked_tile(
    sim: &SimWorld,
    vehicle_id: u32,
    pos: TileCoord,
) -> Option<VehicleOrder> {
    let vehicle = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)?;
    if let Some(station) = sim.state.stations.iter().find(|station| station.pos == pos) {
        return station
            .can_service_vehicle(vehicle.kind)
            .then_some(VehicleOrder::station(pos));
    }
    if sim.state.map.get_kind(pos) == Some(TileKind::RoadDepot)
        && !matches!(vehicle.kind, VehicleKind::Train)
    {
        return Some(VehicleOrder::tile(pos));
    }
    Some(VehicleOrder::tile(pos))
}

pub(crate) fn action_supports_drag(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::Road
            | BuildMenuAction::RoadX
            | BuildMenuAction::RoadY
            | BuildMenuAction::RoadBridge
            | BuildMenuAction::RoadTunnel
            | BuildMenuAction::Rail
            | BuildMenuAction::RailBridge
            | BuildMenuAction::RailTunnel
            | BuildMenuAction::Clear
    )
}

pub(crate) fn action_is_tunnel(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel | BuildMenuAction::RailTunnel
    )
}

pub(crate) fn tunnel_placement_is_valid(
    map: &Map,
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> bool {
    if !action_is_tunnel(action) || tiles.len() < 3 {
        return false;
    }
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    let Some(start) = map.get(TileCoord::new(sx, sy)) else {
        return false;
    };
    let Some(end) = map.get(TileCoord::new(ex, ey)) else {
        return false;
    };
    !matches!(start.kind, TileKind::Water | TileKind::Void)
        && !matches!(end.kind, TileKind::Water | TileKind::Void)
        && start.height == end.height
}

pub(crate) fn command_for_action(
    action: BuildMenuAction,
    pos: TileCoord,
    station_state: &StationBuildState,
) -> Option<Command> {
    match action {
        BuildMenuAction::Road => Some(Command::PlaceRoadBits(pos, 0x0F)),
        BuildMenuAction::RoadX => Some(Command::PlaceRoadBits(pos, 0x0A)),
        BuildMenuAction::RoadY => Some(Command::PlaceRoadBits(pos, 0x05)),
        BuildMenuAction::Rail => Some(Command::PlaceRail(pos)),
        BuildMenuAction::Station => Some(Command::PlaceStationDir(pos, station_state.orientation)),
        BuildMenuAction::BusStop => Some(Command::PlaceBusStop(pos, station_state.orientation)),
        BuildMenuAction::Clear => Some(Command::ClearTile(pos)),
        BuildMenuAction::RoadDepot => {
            Some(Command::PlaceRoadDepotDir(pos, station_state.orientation))
        }
        BuildMenuAction::RailDepot => Some(Command::PlaceRailDepot(pos)),
        BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::Orders => None,
        BuildMenuAction::BuildHouse => Some(Command::PlaceHouse(pos)),
        BuildMenuAction::BuildCoalMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::CoalMine,
        )),
        BuildMenuAction::BuildIronOreMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::IronOreMine,
        )),
        BuildMenuAction::BuildGoldMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::GoldMine,
        )),
        BuildMenuAction::BuildOilWell => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::OilWells,
        )),
        BuildMenuAction::BuildOilRefinery => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::OilRefinery,
        )),
        BuildMenuAction::BuildFactory => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Factory,
        )),
        BuildMenuAction::BuildSawmill => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Sawmill,
        )),
        BuildMenuAction::BuildForest => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Forest,
        )),
        BuildMenuAction::BuildFarm => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Farm,
        )),
    }
}

pub(crate) fn command_for_line_action(
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let &(ex, ey) = tiles.last()?;
    let a = TileCoord::new(sx, sy);
    let b = TileCoord::new(ex, ey);
    match action {
        BuildMenuAction::RoadTunnel => Some(Command::PlaceRoadTunnel(a, b)),
        BuildMenuAction::RailTunnel => Some(Command::PlaceRailTunnel(a, b)),
        BuildMenuAction::RoadBridge => Some(Command::PlaceRoadBridge(a, b)),
        BuildMenuAction::RailBridge => Some(Command::PlaceRailBridge(a, b)),
        _ => None,
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

pub(crate) fn apply_drag_action(
    sim: &mut SimWorld,
    action: BuildMenuAction,
    tiles: Vec<(i32, i32)>,
    station_state: &StationBuildState,
) -> bool {
    if let Some(cmd) = command_for_line_action(action, &tiles) {
        return apply_command(&mut sim.state, &cmd).is_ok();
    }

    if let Some(road_bits) = road_bits_for_drag_action(action, &tiles) {
        let mut changed = false;
        for (x, y) in tiles {
            changed |= apply_command(
                &mut sim.state,
                &Command::SetRoadBits(TileCoord::new(x, y), road_bits),
            )
            .is_ok();
        }
        return changed;
    }

    let mut changed = false;
    for (x, y) in tiles {
        if let Some(cmd) = command_for_action(action, TileCoord::new(x, y), station_state) {
            changed |= apply_command(&mut sim.state, &cmd).is_ok();
        }
    }
    changed
}

pub(crate) fn drag_line_tiles(
    action: BuildMenuAction,
    from: (i32, i32),
    to: (i32, i32),
) -> Vec<(i32, i32)> {
    let use_x_axis = match action {
        BuildMenuAction::RoadX => true,
        BuildMenuAction::RoadY => false,
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

    if let Some(cmd) = command_for_action(action, TileCoord::new(tx, ty), &station_state)
        && apply_command(&mut sim.state, &cmd).is_ok()
    {
        pending.pending = true;
    }
}
