//! Fantasma de construcción: preview de herramientas sobre el mapa.

mod industry;
mod orders;
mod rotate;
mod sprites;
mod station_coverage;
mod validation;

pub(crate) use rotate::rotate_station_with_right_click;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileCoord;

use crate::iso::{tile_pos, world_pos_to_tile_coord};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;

use super::{BuildMenuAction, DragBuildState, OrderEditState, StationBuildState, UiToolState};

use industry::{industry_spec_for_action, spawn_industry_template_preview};
use orders::spawn_order_route_preview;
use sprites::preview_image_for_action;
use station_coverage::{spawn_station_coverage_preview, station_preview_has_coverage};
use validation::{
    action_is_tunnel, preview_station_has_transport_neighbor, preview_target_is_valid,
    tunnel_preview_is_valid,
};

#[derive(Component)]
pub(crate) struct BuildGhostPreview;

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
    if matches!(
        action,
        BuildMenuAction::Station | BuildMenuAction::BusStop | BuildMenuAction::RailStation
    ) {
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
        let station_ok = if matches!(
            action,
            BuildMenuAction::Station | BuildMenuAction::BusStop | BuildMenuAction::RailStation
        ) {
            preview_station_has_transport_neighbor(&sim.state.map, coord, action)
        } else {
            true
        };
        let valid_target = preview_target_is_valid(action, tile.kind)
            && (!action_is_tunnel(action) || tunnel_valid)
            && station_ok;
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ui::toolbar::{BuildMenuAction, DragBuildState, StationBuildState, UiToolState};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::ecs::world::World;
    use bevy::input::ButtonInput;
    use bevy::prelude::{MouseButton, default};
    use openttdrs_core::{
        Industry, IndustryKind, IndustrySpec, Map, Tile, TileCoord, TileKind, industry_template,
    };

    use super::industry::industry_spec_for_action;
    use super::station_coverage::station_preview_has_coverage;
    use super::validation::{action_is_tunnel, preview_target_is_valid, tunnel_preview_is_valid};

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
        run_rotate(&mut world, Some(BuildMenuAction::RoadDepot), false);
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
