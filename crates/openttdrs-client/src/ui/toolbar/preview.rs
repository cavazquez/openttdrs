use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Map, STATION_COVERAGE_RADIUS, TileCoord, TileKind, station_coverage_at};

use crate::iso::{tile_pos, world_pos_to_tile_coord};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;

use super::{BuildMenuAction, DragBuildState, OrderEditState, StationBuildState, UiToolState};

#[derive(Component)]
pub(crate) struct BuildGhostPreview;

pub(crate) fn rotate_station_with_right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut tool_state: ResMut<UiToolState>,
    mut station_state: ResMut<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    if drag_state.armed {
        drag_state.armed = false;
        drag_state.start_tile = None;
        drag_state.last_tile = None;
        drag_state.last_action = None;
        drag_state.pending_tiles.clear();
        return;
    }
    match tool_state.active_tool {
        Some(BuildMenuAction::Station) => {
            station_state.orientation = (station_state.orientation + 1) % 4;
        }
        Some(BuildMenuAction::RoadX) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadY);
        }
        Some(BuildMenuAction::RoadY) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadX);
        }
        Some(BuildMenuAction::Road) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadX);
        }
        _ => return,
    }
    drag_state.armed = false;
    drag_state.start_tile = None;
    drag_state.last_tile = None;
    drag_state.last_action = None;
    drag_state.pending_tiles.clear();
}

pub(crate) fn update_build_ghost_preview(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&Camera, &GlobalTransform),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    existing: Query<Entity, With<BuildGhostPreview>>,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    drag_state: Res<DragBuildState>,
    order_state: Res<OrderEditState>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let Some(action) = tool_state.active_tool else {
        return;
    };

    if action == BuildMenuAction::Orders {
        spawn_order_route_preview(&mut commands, &asset_server, &sim.state.map, &order_state);
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = cam_q.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };
    let Some((tx, ty)) = world_pos_to_tile_coord(world, &sim.state.map) else {
        return;
    };
    if tx < 0 || ty < 0 {
        return;
    }

    let preview_tiles: Vec<(i32, i32)> =
        if drag_state.last_action == Some(action) && !drag_state.pending_tiles.is_empty() {
            drag_state.pending_tiles.clone()
        } else {
            vec![(tx, ty)]
        };
    let tunnel_valid = tunnel_preview_is_valid(&sim.state.map, action, &preview_tiles);
    if action == BuildMenuAction::Station {
        spawn_station_coverage_preview(
            &mut commands,
            &asset_server,
            &sim.state.map,
            &preview_tiles,
            station_preview_has_coverage(&sim.state.map, &sim.state.industries, tx, ty),
        );
    }

    for (px, py) in preview_tiles {
        let coord = TileCoord::new(px, py);
        let Some(tile) = sim.state.map.get(coord) else {
            continue;
        };
        let Some(image) = preview_image_for_action(action, &asset_server, &station_state) else {
            continue;
        };
        let valid_target = preview_target_is_valid(action, tile.kind)
            && (!action_is_tunnel(action) || tunnel_valid);
        let tint = if valid_target {
            Color::srgba(1.0, 1.0, 1.0, 0.55)
        } else {
            Color::srgba(1.0, 0.25, 0.2, 0.55)
        };

        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image,
                color: tint,
                ..default()
            },
            Transform::from_translation(tile_pos(px, py, tile.height, 3.0))
                .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));
    }
}

fn spawn_order_route_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    order_state: &OrderEditState,
) {
    if order_state.vehicle_id.is_none() {
        return;
    }
    let image = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
    for (i, order) in order_state.orders.iter().enumerate() {
        let Some(tile) = map.get(*order) else {
            continue;
        };
        let color = if i == 0 {
            Color::srgba(1.0, 0.95, 0.2, 0.62)
        } else {
            Color::srgba(0.2, 0.85, 1.0, 0.5)
        };
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: image.clone(),
                color,
                ..default()
            },
            Transform::from_translation(tile_pos(order.x, order.y, tile.height, 4.0))
                .with_scale(Vec3::new(1.01, 1.01, 1.0)),
        ));
    }
}

fn spawn_station_coverage_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    preview_tiles: &[(i32, i32)],
    has_coverage: bool,
) {
    let Some(&(tx, ty)) = preview_tiles.first() else {
        return;
    };
    let image = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
    let tint = if has_coverage {
        Color::srgba(1.0, 0.95, 0.25, 0.22)
    } else {
        Color::srgba(1.0, 0.25, 0.15, 0.2)
    };
    for y in ty - STATION_COVERAGE_RADIUS..=ty + STATION_COVERAGE_RADIUS {
        for x in tx - STATION_COVERAGE_RADIUS..=tx + STATION_COVERAGE_RADIUS {
            let Some(tile) = map.get(TileCoord::new(x, y)) else {
                continue;
            };
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: image.clone(),
                    color: tint,
                    ..default()
                },
                Transform::from_translation(tile_pos(x, y, tile.height, 2.5))
                    .with_scale(Vec3::new(1.002, 1.002, 1.0)),
            ));
        }
    }
}

fn station_preview_has_coverage(
    map: &Map,
    industries: &[openttdrs_core::Industry],
    tx: i32,
    ty: i32,
) -> bool {
    let coverage = station_coverage_at(
        map,
        industries,
        TileCoord::new(tx, ty),
        STATION_COVERAGE_RADIUS,
    );
    coverage.accepts_anything() || coverage.supplies_anything()
}

fn preview_image_for_action(
    action: BuildMenuAction,
    asset_server: &AssetServer,
    station_state: &StationBuildState,
) -> Option<Handle<Image>> {
    match action {
        BuildMenuAction::Station => Some(asset_server.load::<Image>(format!(
            "opengfx/tiles/truck_stop_ground_{}.png",
            station_state.orientation
        ))),
        BuildMenuAction::Road => Some(asset_server.load::<Image>("opengfx/tiles/road_flat_02.png")),
        BuildMenuAction::RoadX => {
            Some(asset_server.load::<Image>("opengfx/tiles/road_flat_01.png"))
        }
        BuildMenuAction::RoadY => {
            Some(asset_server.load::<Image>("opengfx/tiles/road_flat_00.png"))
        }
        BuildMenuAction::RoadDepot => {
            Some(asset_server.load::<Image>("opengfx/tiles/road_depot_0.png"))
        }
        BuildMenuAction::RoadBridge => {
            Some(asset_server.load::<Image>("opengfx/tiles/bridge_wood_road_x.png"))
        }
        BuildMenuAction::RoadTunnel => {
            Some(asset_server.load::<Image>("opengfx/tiles/tunnel_road_rear.png"))
        }
        BuildMenuAction::Rail => Some(asset_server.load::<Image>("opengfx/tiles/rail_1005.png")),
        BuildMenuAction::RailDepot => {
            Some(asset_server.load::<Image>("opengfx/tiles/rail_depot_ne.png"))
        }
        BuildMenuAction::RailBridge => {
            Some(asset_server.load::<Image>("opengfx/tiles/bridge_wood_rail_x.png"))
        }
        BuildMenuAction::RailTunnel => {
            Some(asset_server.load::<Image>("opengfx/tiles/tunnel_rail_rear.png"))
        }
        BuildMenuAction::Clear => Some(asset_server.load::<Image>("opengfx/tiles/grass_rough.png")),
        BuildMenuAction::Orders => None,
        BuildMenuAction::BuildHouse => {
            Some(asset_server.load::<Image>("opengfx/tiles/house_church_build.png"))
        }
        BuildMenuAction::BuildCoalMine => {
            Some(asset_server.load::<Image>("opengfx/tiles/industry_2013.png"))
        }
        BuildMenuAction::BuildOilWell => {
            Some(asset_server.load::<Image>("opengfx/tiles/industry_2028.png"))
        }
        BuildMenuAction::BuildFactory => {
            Some(asset_server.load::<Image>("opengfx/tiles/industry_2169.png"))
        }
        BuildMenuAction::BuildForest => {
            Some(asset_server.load::<Image>("opengfx/tiles/tree_01.png"))
        }
    }
}

fn preview_target_is_valid(action: BuildMenuAction, kind: TileKind) -> bool {
    match action {
        BuildMenuAction::Road
        | BuildMenuAction::RoadX
        | BuildMenuAction::RoadY
        | BuildMenuAction::RoadDepot
        | BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::Rail
        | BuildMenuAction::RailDepot
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::Station
        | BuildMenuAction::BuildHouse
        | BuildMenuAction::BuildCoalMine
        | BuildMenuAction::BuildOilWell
        | BuildMenuAction::BuildFactory
        | BuildMenuAction::BuildForest => !matches!(kind, TileKind::Water | TileKind::Void),
        BuildMenuAction::Clear | BuildMenuAction::Orders => !matches!(kind, TileKind::Void),
    }
}

fn action_is_tunnel(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel | BuildMenuAction::RailTunnel
    )
}

fn tunnel_preview_is_valid(map: &Map, action: BuildMenuAction, tiles: &[(i32, i32)]) -> bool {
    if !action_is_tunnel(action) {
        return true;
    }
    if tiles.len() < 3 {
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
