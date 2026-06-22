//! Fantasma de construcción: preview de herramientas sobre el mapa.

mod industry;
mod orders;
mod rail_waypoint;
mod road_depot;
mod road_stop;
mod rotate;
mod sprites;
mod station_coverage;
mod validation;

pub(crate) use rotate::rotate_station_with_right_click;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{TileCoord, is_tunnel_entrance_slope, tile_slope_and_z};

use crate::iso::{
    SLOPE_HALF_H, TILE_HALF_H, tile_pos_half, tile_slope_and_min_z, world_pos_to_tile_coord,
    world_pos_to_tile_fract,
};
use crate::render::{MapPreviewCamera, PrimaryGameCamera, TileAtlas};
use crate::sprites::rail_ghost_overlay_offset;
use crate::state::SimWorld;
use crate::ui::hud::HoveredTileCoord;

use super::{BuildMenuAction, DragBuildState, OrderEditState, StationBuildState, UiToolState};

use industry::{industry_spec_for_action, spawn_industry_template_preview};
use orders::{spawn_order_pick_target_preview, spawn_order_route_preview};
use rail_waypoint::spawn_rail_waypoint_preview;
use road_depot::{RoadDepotPreviewSpawn, spawn_road_depot_preview};
use road_stop::{
    RoadStopPreviewSpawn, bus_stop_ground_path, road_stop_preview_dir, spawn_road_stop_preview,
    truck_stop_ground_path,
};
use sprites::preview_image_for_action;
use station_coverage::{spawn_station_coverage_preview, station_preview_has_coverage};
use validation::{action_is_tunnel, preview_build_command_valid};

use crate::ui::toolbar::build_input::rail_lane::rail_lane_bits_for_action;

use crate::sprites::StationTileClass;

#[derive(Component)]
pub(crate) struct BuildGhostPreview;

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn update_build_ghost_preview(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    existing: Query<Entity, With<BuildGhostPreview>>,
    asset_server: Res<AssetServer>,
    atlas: Option<Res<TileAtlas>>,
    sim: Res<SimWorld>,
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    drag_state: Res<DragBuildState>,
    order_state: Res<OrderEditState>,
    hovered: Res<HoveredTileCoord>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let orders_preview =
        order_state.picking_destination || tool_state.active_tool == Some(BuildMenuAction::Orders);
    if orders_preview && order_state.vehicle_id.is_some() {
        spawn_order_route_preview(&mut commands, &asset_server, &sim.state.map, &order_state);
        if order_state.picking_destination
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
    let tile_fract = world_pos_to_tile_fract(world, &sim.state.map, tx, ty);
    let cursor_rail_lane = rail_lane_bits_for_action(action, Some(tile_fract));
    let preview_rail_lane = match action {
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert if drag_state.armed => {
            drag_state.rail_lane_bit
        }
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert => cursor_rail_lane,
        _ => None,
    };

    let preview_tiles: Vec<(i32, i32)> =
        if matches!(action, BuildMenuAction::BusStop | BuildMenuAction::Station) {
            // Parada bus / camión: siempre 1×1 en el cursor (no arrastre ni halo de cobertura).
            vec![(tx, ty)]
        } else if action_is_tunnel(action) {
            let start = TileCoord::new(tx, ty);
            openttdrs_core::tunnel_preview_path(&sim.state.map, start)
                .map(|path| path.into_iter().map(|c| (c.x, c.y)).collect())
                .unwrap_or_else(|| vec![(tx, ty)])
        } else if drag_state.last_action == Some(action) && !drag_state.pending_tiles.is_empty() {
            drag_state.pending_tiles.clone()
        } else {
            vec![(tx, ty)]
        };
    if action == BuildMenuAction::RailStation {
        // Estación de tren: rejilla blanca de la huella completa (andenes ×
        // longitud) + halo de cobertura opcional, como en OpenTTD.
        spawn_rail_station_area_preview(
            &mut commands,
            &asset_server,
            &sim,
            &station_state,
            TileCoord::new(tx, ty),
        );
        return;
    }
    if action == BuildMenuAction::RailWaypoint {
        let coord = TileCoord::new(tx, ty);
        if sim.state.map.get(coord).is_some() {
            let valid = preview_build_command_valid(
                &sim.state,
                action,
                coord,
                &station_state,
                &[(tx, ty)],
                preview_rail_lane,
            );
            spawn_rail_waypoint_preview(
                &mut commands,
                atlas.as_deref(),
                &sim.state.map,
                coord,
                valid,
            );
        }
        return;
    }

    for (px, py) in &preview_tiles {
        let coord = TileCoord::new(*px, *py);
        if sim.state.map.get(coord).is_none() {
            continue;
        }
        let valid_target = preview_build_command_valid(
            &sim.state,
            action,
            coord,
            &station_state,
            &preview_tiles,
            preview_rail_lane,
        );
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

        if action_is_tunnel(action) {
            let Some((tileh, _)) = tile_slope_and_z(&sim.state.map, coord) else {
                continue;
            };
            if !is_tunnel_entrance_slope(tileh) {
                continue;
            }
        }

        let (tileh, base_z) = tile_slope_and_min_z(&sim.state.map, *px as u32, *py as u32);
        let half_h = if tileh == 0 {
            TILE_HALF_H
        } else {
            SLOPE_HALF_H[tileh as usize]
        };

        if matches!(action, BuildMenuAction::BusStop | BuildMenuAction::Station) {
            let dir = road_stop_preview_dir(station_state.orientation);
            let (class, ground) = if action == BuildMenuAction::BusStop {
                (StationTileClass::Bus, bus_stop_ground_path(dir))
            } else {
                (StationTileClass::Truck, truck_stop_ground_path(dir))
            };
            spawn_road_stop_preview(
                &mut commands,
                RoadStopPreviewSpawn {
                    px: *px,
                    py: *py,
                    base_z,
                    half_h,
                    class,
                    dir,
                    ground_path: ground,
                    tint,
                    asset_server: &asset_server,
                },
            );
            continue;
        }

        // Vía: realce blanco de la pieza que se colocaría (autorraíl de OpenTTD),
        // en vez de un sprite de vía sólido flotando sobre la tesela.
        if let Some(bits) = rail_preview_bits(
            action,
            &sim.state.map,
            coord,
            &preview_tiles,
            preview_rail_lane,
        ) && let Some(atlas) = atlas.as_ref()
        {
            spawn_rail_ghost_preview(
                &mut commands,
                atlas,
                RailGhostSpawn {
                    px: *px,
                    py: *py,
                    base_z,
                    half_h,
                    tileh,
                    bits,
                    valid: valid_target,
                },
            );
            continue;
        }

        if action == BuildMenuAction::RoadDepot {
            spawn_road_depot_preview(
                &mut commands,
                RoadDepotPreviewSpawn {
                    px: *px,
                    py: *py,
                    base_z,
                    half_h,
                    dir: road_stop_preview_dir(station_state.orientation),
                    tint,
                    asset_server: &asset_server,
                },
            );
            continue;
        }

        let Some(image) =
            preview_image_for_action(action, &asset_server, &station_state, &preview_tiles)
        else {
            continue;
        };

        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image,
                color: tint,
                ..default()
            },
            Transform::from_translation(tile_pos_half(*px, *py, base_z, 3.0, half_h))
                .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));
    }
}

/// Huella de la estación de tren bajo el cursor: rejilla blanca por tesela
/// (roja si la colocación es inválida) y halo de cobertura si está activado.
fn spawn_rail_station_area_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    sim: &SimWorld,
    station_state: &StationBuildState,
    origin: TileCoord,
) {
    use openttdrs_core::{Command, command_would_fail, rail_station_footprint};

    let (w, h) = rail_station_footprint(
        station_state.rail_axis_y,
        station_state.rail_platforms,
        station_state.rail_length,
    );
    let cmd = Command::PlaceRailStationArea {
        origin,
        axis_y: station_state.rail_axis_y,
        platforms: station_state.rail_platforms,
        length: station_state.rail_length,
    };
    let valid = command_would_fail(&sim.state, &cmd).is_none();

    if station_state.rail_show_coverage {
        let anchor = (origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
        spawn_station_coverage_preview(
            commands,
            asset_server,
            &sim.state.map,
            &[anchor],
            station_preview_has_coverage(&sim.state.map, &sim.state.industries, anchor.0, anchor.1),
        );
    }

    let image = asset_server.load::<Image>("assets/opengfx/tiles/tile_select.png");
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.95)
    } else {
        Color::srgba(1.0, 0.3, 0.25, 0.95)
    };
    for dy in 0..h {
        for dx in 0..w {
            let (x, y) = (origin.x + dx, origin.y + dy);
            let Some(tile) = sim.state.map.get(TileCoord::new(x, y)) else {
                continue;
            };
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: image.clone(),
                    color: tint,
                    ..default()
                },
                Transform::from_translation(crate::iso::tile_pos(x, y, tile.height, 3.0))
                    .with_scale(Vec3::new(1.002, 1.002, 1.0)),
            ));
        }
    }
}

/// Trackbits a previsualizar para las herramientas de vía, o `None` si la
/// acción no es una pieza de vía.
fn rail_preview_bits(
    action: BuildMenuAction,
    map: &openttdrs_core::Map,
    coord: TileCoord,
    preview_tiles: &[(i32, i32)],
    rail_lane_bit: Option<u8>,
) -> Option<u8> {
    use crate::sprites::{RAIL_TB_X, RAIL_TB_Y};
    match action {
        BuildMenuAction::RailX => Some(RAIL_TB_X),
        BuildMenuAction::RailY => Some(RAIL_TB_Y),
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert => rail_lane_bit,
        BuildMenuAction::Rail => {
            if preview_tiles.len() >= 2 {
                // Arrastre: recta a lo largo del eje dominante del tramo.
                let (sx, sy) = preview_tiles[0];
                let (ex, ey) = preview_tiles[preview_tiles.len() - 1];
                Some(if (ex - sx).abs() >= (ey - sy).abs() {
                    RAIL_TB_X
                } else {
                    RAIL_TB_Y
                })
            } else {
                // Una tesela: la pieza que colocaría `PlaceRail` (con curvas).
                Some(openttdrs_core::rail_trackbits_from_neighbors(map, coord))
            }
        }
        _ => None,
    }
}

struct RailGhostSpawn {
    px: i32,
    py: i32,
    base_z: u8,
    half_h: f32,
    tileh: u8,
    bits: u8,
    valid: bool,
}

/// Realce del riel a colocar: overlays solo riel en plano; tinte suave en pendiente.
fn spawn_rail_ghost_preview(commands: &mut Commands, atlas: &TileAtlas, spawn: RailGhostSpawn) {
    let mut ids = Vec::new();
    crate::sprites::collect_rail_ghost_sprites(spawn.bits, spawn.tileh, &mut ids);
    let center = tile_pos_half(spawn.px, spawn.py, spawn.base_z, 3.0, spawn.half_h);
    for (i, sid) in ids.iter().copied().enumerate() {
        let is_overlay = (1005..=1010).contains(&sid);
        let tint = if spawn.valid {
            if is_overlay {
                Color::srgba(2.2, 2.4, 2.8, 0.9)
            } else {
                Color::srgba(1.0, 1.02, 1.05, 0.38)
            }
        } else if is_overlay {
            Color::srgba(3.0, 0.5, 0.45, 0.9)
        } else {
            Color::srgba(1.0, 0.4, 0.35, 0.45)
        };
        let offset = rail_ghost_overlay_offset(sid);
        let img = atlas.get(&format!("rail_{sid}.png"));
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(center + Vec3::new(offset.x, offset.y, i as f32 * 0.001)),
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
    use super::validation::{action_is_tunnel, preview_build_command_valid};
    use crate::state::SimWorld;
    use openttdrs_core::{Command, GameState, command_would_fail};

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
        ));
        assert!(!preview_build_command_valid(
            &sim.state,
            BuildMenuAction::Road,
            c(4, 4),
            &station,
            &[(4, 4)],
            None,
        ));
        assert!(preview_build_command_valid(
            &sim.state,
            BuildMenuAction::Clear,
            c(4, 4),
            &station,
            &[(4, 4)],
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
        ));
        assert!(!preview_build_command_valid(
            &sim.state,
            BuildMenuAction::RoadTunnel,
            c(0, 0),
            &station,
            &[(0, 0)],
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
