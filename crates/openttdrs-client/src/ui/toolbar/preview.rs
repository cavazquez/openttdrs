use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{
    IndustrySpec, Map, STATION_COVERAGE_RADIUS, TileCoord, TileKind, industry_template,
    station_coverage_at,
};

use crate::iso::{iso, overlay_pos, tile_pos, tile_slope_and_min_z, world_pos_to_tile_coord};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::sprites::industry_gfx_entry;
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

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
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
        let valid_target = preview_target_is_valid(action, tile.kind)
            && (!action_is_tunnel(action) || tunnel_valid);
        let tint = if valid_target {
            Color::srgba(1.0, 1.0, 1.0, 0.55)
        } else {
            Color::srgba(1.0, 0.25, 0.2, 0.55)
        };

        if let Some(spec) = industry_spec_for_action(action) {
            spawn_industry_template_preview(
                &mut commands,
                &asset_server,
                &sim.state.map,
                coord,
                spec,
                tint,
            );
            continue;
        }

        let Some(image) = preview_image_for_action(action, &asset_server, &station_state) else {
            continue;
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

fn spawn_industry_template_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    origin: TileCoord,
    spec: IndustrySpec,
    tint: Color,
) {
    for (coord, m5) in industry_template(origin, spec) {
        if map.get(coord).is_none() {
            continue;
        }
        let (tileh, base_z) =
            tile_slope_and_min_z(map, coord.x.max(0) as u32, coord.y.max(0) as u32);
        let ground = if tileh == 0 {
            asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png")
        } else {
            asset_server.load::<Image>(format!(
                "assets/opengfx/tiles/terrain_rough_slope_{tileh:02}.png"
            ))
        };
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: ground,
                color: tint.with_alpha(tint.alpha() * 0.75),
                ..default()
            },
            Transform::from_translation(tile_pos(coord.x, coord.y, base_z, 2.9))
                .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));

        let Some(entry) = industry_gfx_entry(u16::from(m5)) else {
            continue;
        };
        let ref_pos = iso(coord.x, coord.y);
        if entry.ground_sprite_id != 0 {
            let img = asset_server.load::<Image>(format!(
                "assets/opengfx/tiles/industry_{}.png",
                entry.ground_sprite_id
            ));
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: img,
                    color: tint,
                    ..default()
                },
                Transform::from_translation(overlay_pos(
                    ref_pos, entry.xrel, entry.yrel, entry.w, entry.h, base_z, 3.2, coord.x,
                    coord.y,
                )),
            ));
        }
        if entry.sprite_id != 0 {
            let img = asset_server.load::<Image>(format!(
                "assets/opengfx/tiles/industry_{}.png",
                entry.sprite_id
            ));
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: img,
                    color: tint,
                    ..default()
                },
                Transform::from_translation(overlay_pos(
                    ref_pos, entry.xrel, entry.yrel, entry.w, entry.h, base_z, 3.3, coord.x,
                    coord.y,
                )),
            ));
        }
    }
}

fn industry_spec_for_action(action: BuildMenuAction) -> Option<IndustrySpec> {
    match action {
        BuildMenuAction::BuildCoalMine => Some(IndustrySpec::CoalMine),
        BuildMenuAction::BuildIronOreMine => Some(IndustrySpec::IronOreMine),
        BuildMenuAction::BuildGoldMine => Some(IndustrySpec::GoldMine),
        BuildMenuAction::BuildOilWell => Some(IndustrySpec::OilWells),
        BuildMenuAction::BuildOilRefinery => Some(IndustrySpec::OilRefinery),
        BuildMenuAction::BuildFactory => Some(IndustrySpec::Factory),
        BuildMenuAction::BuildSawmill => Some(IndustrySpec::Sawmill),
        BuildMenuAction::BuildForest => Some(IndustrySpec::Forest),
        BuildMenuAction::BuildFarm => Some(IndustrySpec::Farm),
        _ => None,
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
    let image = asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png");
    for (i, order) in order_state.orders.iter().enumerate() {
        let pos = order.destination();
        let Some(tile) = map.get(pos) else {
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
            Transform::from_translation(tile_pos(pos.x, pos.y, tile.height, 4.0))
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
    let image = asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png");
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
            "assets/opengfx/tiles/truck_stop_ground_{}.png",
            station_state.orientation
        ))),
        BuildMenuAction::Road => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_flat_02.png"))
        }
        BuildMenuAction::RoadX => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_flat_01.png"))
        }
        BuildMenuAction::RoadY => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_flat_00.png"))
        }
        BuildMenuAction::RoadDepot => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_depot_0.png"))
        }
        BuildMenuAction::RoadBridge => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_road_x.png"))
        }
        BuildMenuAction::RoadTunnel => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tunnel_road_rear.png"))
        }
        BuildMenuAction::Rail => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1005.png"))
        }
        BuildMenuAction::RailDepot => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_depot_ne.png"))
        }
        BuildMenuAction::RailBridge => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_rail_x.png"))
        }
        BuildMenuAction::RailTunnel => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tunnel_rail_rear.png"))
        }
        BuildMenuAction::Clear => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png"))
        }
        BuildMenuAction::Orders => None,
        BuildMenuAction::BuildHouse => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/house_church_build.png"))
        }
        BuildMenuAction::BuildCoalMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2013.png"))
        }
        BuildMenuAction::BuildIronOreMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2092.png"))
        }
        BuildMenuAction::BuildGoldMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2247.png"))
        }
        BuildMenuAction::BuildOilWell => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2028.png"))
        }
        BuildMenuAction::BuildOilRefinery => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2047.png"))
        }
        BuildMenuAction::BuildFactory => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2169.png"))
        }
        BuildMenuAction::BuildSawmill => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2063.png"))
        }
        BuildMenuAction::BuildForest => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tree_01.png"))
        }
        BuildMenuAction::BuildFarm => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2190.png"))
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
        | BuildMenuAction::BuildIronOreMine
        | BuildMenuAction::BuildGoldMine
        | BuildMenuAction::BuildOilWell
        | BuildMenuAction::BuildOilRefinery
        | BuildMenuAction::BuildFactory
        | BuildMenuAction::BuildSawmill
        | BuildMenuAction::BuildForest
        | BuildMenuAction::BuildFarm => !matches!(kind, TileKind::Water | TileKind::Void),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;
    use openttdrs_core::{Industry, IndustryKind, Tile};

    fn run_rotate(world: &mut World, tool: Option<BuildMenuAction>, drag_armed: bool) {
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Right);
        world.insert_resource(mouse);
        world.insert_resource(UiToolState { active_tool: tool });
        world.insert_resource(StationBuildState::default());
        world.insert_resource(DragBuildState {
            armed: drag_armed,
            ..default()
        });
        world
            .run_system_once(rotate_station_with_right_click)
            .unwrap();
    }

    #[test]
    fn rotate_station_right_click_covers_branches() {
        let mut world = World::new();
        run_rotate(&mut world, Some(BuildMenuAction::Station), false);
        run_rotate(&mut world, Some(BuildMenuAction::RoadX), false);
        run_rotate(&mut world, Some(BuildMenuAction::RoadY), false);
        run_rotate(&mut world, Some(BuildMenuAction::Road), false);
        run_rotate(&mut world, Some(BuildMenuAction::Rail), false);
        run_rotate(&mut world, None, false);
        run_rotate(&mut world, Some(BuildMenuAction::Station), true);
    }

    #[test]
    fn preview_validators_cover_key_paths() {
        let mut map = Map::new_flat(6, 6, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        map.set_tile(
            c(1, 1),
            Tile {
                kind: TileKind::Road,
                height: 2,
                ..tile_template()
            },
        )
        .unwrap();
        map.set_tile(
            c(3, 1),
            Tile {
                kind: TileKind::Road,
                height: 2,
                ..tile_template()
            },
        )
        .unwrap();
        map.set_kind(c(4, 4), TileKind::Water).unwrap();

        assert!(preview_target_is_valid(
            BuildMenuAction::Road,
            TileKind::Grass
        ));
        assert!(!preview_target_is_valid(
            BuildMenuAction::Road,
            TileKind::Water
        ));
        assert!(preview_target_is_valid(
            BuildMenuAction::Clear,
            TileKind::Water
        ));
        assert!(!preview_target_is_valid(
            BuildMenuAction::Orders,
            TileKind::Void
        ));

        assert!(action_is_tunnel(BuildMenuAction::RoadTunnel));
        assert!(action_is_tunnel(BuildMenuAction::RailTunnel));
        assert!(!action_is_tunnel(BuildMenuAction::Road));

        assert!(!tunnel_preview_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1)]
        ));
        assert!(tunnel_preview_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (3, 1)]
        ));
        assert!(!tunnel_preview_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (4, 4)]
        ));
        assert!(tunnel_preview_is_valid(
            &map,
            BuildMenuAction::Road,
            &[(1, 1), (2, 1)]
        ));

        map.set_height(c(3, 1), 1).unwrap();
        assert!(!tunnel_preview_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (3, 1)]
        ));

        assert!(!tunnel_preview_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(-1, -1), (0, 0), (1, 1)]
        ));

        assert!(preview_target_is_valid(
            BuildMenuAction::BuildFactory,
            TileKind::Grass
        ));
        assert!(!preview_target_is_valid(
            BuildMenuAction::BuildFactory,
            TileKind::Void
        ));
        assert!(preview_target_is_valid(
            BuildMenuAction::Orders,
            TileKind::Industry
        ));

        assert_eq!(
            industry_spec_for_action(BuildMenuAction::BuildFactory),
            Some(IndustrySpec::Factory)
        );
        assert_eq!(industry_spec_for_action(BuildMenuAction::Road), None);
        assert_eq!(
            industry_template(TileCoord::new(2, 2), IndustrySpec::CoalMine).len(),
            6
        );
    }

    #[test]
    fn station_coverage_preview_checks_industries() {
        let map = Map::new_flat(8, 8, 0);
        let industries = vec![Industry {
            pos: TileCoord::new(3, 3),
            tiles: vec![TileCoord::new(3, 3)],
            spec: None,
            kind: IndustryKind::Forest,
            stock: 30,
            capacity: 100,
        }];
        assert!(station_preview_has_coverage(&map, &industries, 3, 3));
        assert!(!station_preview_has_coverage(&map, &[], 0, 0));
    }

    fn tile_template() -> Tile {
        Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: 0,
            m5: 0,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }
}
