//! Fantasma de construcción: preview de herramientas sobre el mapa.

mod bridge;
mod dispatch;
mod ghost_lerp;
pub(super) mod industry;
mod orders;
mod plan;
mod rail_depot;
mod rail_signal;
mod rail_station;
mod rail_waypoint;
mod road_depot;
mod road_stop;
mod road_waypoint;
mod rotate;
mod spawn;
mod sprites;
pub(super) mod station_coverage;
mod tunnel;
pub(super) mod validation;

pub(crate) use ghost_lerp::{GhostLerp, lerp_ghost_previews};
pub(crate) use industry::{economy_industry_tool_visible, industry_spec_for_action};
pub(crate) use rotate::rotate_station_with_right_click;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileCoord;

use crate::iso::{world_pos_to_rail_signal_pick, world_pos_to_tile_coord, world_pos_to_tile_fract};
use crate::render::{CompanyColoredSprites, MapPreviewCamera, PrimaryGameCamera, TileAtlas};
use crate::state::{OrderPickState, SimWorld, order_pick_active};
use crate::ui::hud::HoveredTileCoord;

use super::build_input::rail_lane::rail_lane_bits_for_action;
use super::{
    BuildMenuAction, DragBuildState, OrderEditState, StationBuildState, ToolbarState, UiToolState,
};

use dispatch::build_preview_plan;
use orders::{spawn_order_pick_target_preview, spawn_order_route_preview};
use plan::PreviewContext;
pub(crate) use rail_signal::{
    RailSignalGhost, RailSignalGhostState, rail_signal_flash_position,
    update_rail_signal_ghost_preview,
};
use spawn::spawn_preview_plan;
use validation::preview_build_command_valid;

#[derive(Component)]
pub(crate) struct BuildGhostPreview;

#[derive(SystemParam)]
pub(crate) struct RailSignalGhostPreviewParams<'w, 's> {
    pub ghosts: Query<'w, 's, Entity, With<RailSignalGhost>>,
    pub state: ResMut<'w, RailSignalGhostState>,
    pub sprites:
        Query<'w, 's, (Entity, &'static mut GhostLerp, &'static mut Sprite), With<RailSignalGhost>>,
    /// Frame de cursor animado (demolición); vive aquí para no superar el límite de params Bevy.
    pub toolbar: Res<'w, ToolbarState>,
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn update_build_ghost_preview(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    existing: Query<Entity, (With<BuildGhostPreview>, Without<RailSignalGhost>)>,
    mut rail_ghost: RailSignalGhostPreviewParams,
    asset_server: Res<AssetServer>,
    atlas: Option<Res<TileAtlas>>,
    company: Option<Res<CompanyColoredSprites>>,
    sim: Res<SimWorld>,
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    drag_state: Res<DragBuildState>,
    order_state: Res<OrderEditState>,
    pick_state: Res<State<OrderPickState>>,
    hovered: Res<HoveredTileCoord>,
    time: Res<Time>,
) {
    let anim_cursor_frame = rail_ghost.toolbar.anim_cursor_frame;

    // Limpieza de ghosts existentes
    if tool_state.active_tool != Some(BuildMenuAction::RailSignals) {
        for entity in &rail_ghost.ghosts {
            commands.entity(entity).despawn();
        }
        rail_ghost.state.key = None;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    // Preview de órdenes (caso especial, manejado aparte)
    let orders_preview =
        order_pick_active(&pick_state) || tool_state.active_tool == Some(BuildMenuAction::Orders);
    if orders_preview && order_state.vehicle_id.is_some() {
        spawn_order_route_preview(&mut commands, &asset_server, &sim.state.map, &order_state);
        if order_pick_active(&pick_state)
            && let Some(hover) = hovered.pos
        {
            spawn_order_pick_target_preview(
                &mut commands,
                &asset_server,
                &sim,
                &order_state,
                hover,
            );
        }
    }

    let Some(action) = tool_state.active_tool else {
        return;
    };

    if action == BuildMenuAction::Orders {
        return;
    }

    // Obtener posición del cursor en el mundo
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

    let (tx, ty, tile_fract) = if action == BuildMenuAction::RailSignals {
        if let Some(pos) = hovered.pos {
            (pos.x, pos.y, (hovered.fract_x, hovered.fract_y))
        } else if let Some((px, py, fx, fy)) = world_pos_to_rail_signal_pick(world, &sim.state.map)
        {
            (px, py, (fx, fy))
        } else {
            return;
        }
    } else {
        let Some((px, py)) = world_pos_to_tile_coord(world, &sim.state.map) else {
            return;
        };
        (
            px,
            py,
            world_pos_to_tile_fract(world, &sim.state.map, px, py),
        )
    };

    if tx < 0 || ty < 0 {
        return;
    }

    let cursor_rail_lane = rail_lane_bits_for_action(action, Some(tile_fract));
    let preview_rail_lane = match action {
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert if drag_state.armed => {
            drag_state.rail_lane_bit
        }
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert => cursor_rail_lane,
        _ => None,
    };

    // Caso especial: señales 1×1 manejadas por sistema dedicado
    if action == BuildMenuAction::RailSignals {
        let (fx, fy) = if drag_state.armed {
            station_state.signal_drag_fract.unwrap_or(tile_fract)
        } else {
            tile_fract
        };
        let tiles: Vec<(i32, i32)> = if drag_state.armed && !drag_state.pending_tiles.is_empty() {
            drag_state.pending_tiles.clone()
        } else {
            vec![(tx, ty)]
        };

        if tiles.len() == 1 {
            let coord = TileCoord::new(tiles[0].0, tiles[0].1);
            if sim.state.map.get(coord).is_some() {
                let valid = preview_build_command_valid(
                    &sim.state,
                    action,
                    coord,
                    &station_state,
                    &tiles,
                    preview_rail_lane,
                    Some((fx, fy)),
                );
                update_rail_signal_ghost_preview(
                    commands,
                    time,
                    asset_server,
                    atlas,
                    rail_ghost.state,
                    &sim.state.map,
                    coord,
                    station_state.orientation,
                    fx,
                    fy,
                    station_state.signal_type,
                    valid,
                    sim.state.tick,
                    rail_ghost.sprites,
                );
            }
            return;
        }
        // Multi-tile: manejado por dispatch/spawn
        for entity in &rail_ghost.ghosts {
            commands.entity(entity).despawn();
        }
        rail_ghost.state.key = None;
    }

    // Construir contexto y plan de preview
    let ctx = PreviewContext {
        map: &sim.state.map,
        action,
        cursor_tile: (tx, ty),
        tile_fract,
        station_state: &station_state,
        drag_state: &drag_state,
        rail_lane_bit: preview_rail_lane,
    };

    let plan = build_preview_plan(&ctx, &sim.state);

    // Spawn entidades según el plan
    spawn_preview_plan(
        &mut commands,
        &plan,
        &asset_server,
        atlas.as_deref(),
        company.as_deref(),
        &sim,
        &station_state,
        action,
        anim_cursor_frame,
    );
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
    use super::validation::{action_is_tunnel, preview_build_command_valid};
    use crate::state::SimWorld;
    use openttdrs_core::{Command, GameState, command_would_fail};

    fn run_rotate(world: &mut World, tool: Option<BuildMenuAction>, drag_armed: bool) {
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Right);
        world.insert_resource(mouse);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(UiToolState {
            active_tool: tool,
            ..Default::default()
        });
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
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        let mut state = GameState::new(6, 6);
        for (x, y) in [(1, 1), (3, 1)] {
            state
                .map
                .set_tile(
                    c(x, y),
                    Tile {
                        kind: TileKind::Road,
                        height: 2,
                        ..tile_template()
                    },
                )
                .unwrap();
        }
        state.map.set_kind(c(4, 4), TileKind::Water).unwrap();
        let mut sim = SimWorld {
            state,
            ..SimWorld::default()
        };
        let station = StationBuildState::default();

        assert!(preview_build_command_valid(
            &sim.state,
            BuildMenuAction::Road,
            c(0, 0),
            &station,
            &[(0, 0)],
            None,
            None,
        ));
        assert!(!preview_build_command_valid(
            &sim.state,
            BuildMenuAction::Road,
            c(4, 4),
            &station,
            &[(4, 4)],
            None,
            None,
        ));
        assert!(preview_build_command_valid(
            &sim.state,
            BuildMenuAction::Clear,
            c(4, 4),
            &station,
            &[(4, 4)],
            None,
            None,
        ));

        assert!(action_is_tunnel(BuildMenuAction::RoadTunnel));
        assert!(!action_is_tunnel(BuildMenuAction::Road));

        sim.state.map.set_height(c(2, 2), 2).unwrap();
        sim.state.map.set_height(c(2, 3), 2).unwrap();
        sim.state.map.set_height(c(3, 2), 1).unwrap();
        sim.state.map.set_height(c(3, 3), 1).unwrap();
        sim.state.map.set_height(c(0, 2), 1).unwrap();
        sim.state.map.set_height(c(0, 3), 1).unwrap();
        sim.state.map.set_height(c(1, 2), 2).unwrap();
        sim.state.map.set_height(c(1, 3), 2).unwrap();
        assert!(preview_build_command_valid(
            &sim.state,
            BuildMenuAction::RoadTunnel,
            c(2, 2),
            &station,
            &[(2, 2)],
            None,
            None,
        ));
        assert!(!preview_build_command_valid(
            &sim.state,
            BuildMenuAction::RoadTunnel,
            c(0, 0),
            &station,
            &[(0, 0)],
            None,
            None,
        ));
        assert_eq!(
            command_would_fail(&sim.state, &Command::PlaceRoadTunnel(c(0, 0), c(2, 0))),
            Some(openttdrs_core::CommandError::InvalidTunnelEndpoints)
        );

        assert_eq!(
            industry_spec_for_action(BuildMenuAction::BuildFactory),
            Some(IndustrySpec::Factory)
        );
        assert_eq!(industry_spec_for_action(BuildMenuAction::Road), None);
        use super::industry::economy_industry_tool_visible;
        use openttdrs_core::Climate;
        assert!(economy_industry_tool_visible(
            BuildMenuAction::BuildCoalMine,
            Climate::Temperate
        ));
        assert!(!economy_industry_tool_visible(
            BuildMenuAction::BuildCoalMine,
            Climate::Toyland
        ));
        assert!(economy_industry_tool_visible(
            BuildMenuAction::BuildFizzyDrinkFactory,
            Climate::Toyland
        ));
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
            random_colour: 0,
            ..Default::default()
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
