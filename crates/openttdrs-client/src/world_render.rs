//! Sistemas Bevy que construyen y refrescan la capa visual del mundo.

use bevy::prelude::*;
use openttdrs_core::TileKind;

use crate::bevy_app::UpdateSet;
use crate::config::env_flag;
use crate::iso::{ISO_HW, ISO_QH, SLOPE_HALF_H, TILE_HALF_H};
use crate::render::{
    MapSpriteBatches, MapVisualLayer, RenderGrid, TileRenderContext, WorldAssets,
    flush_map_batches, push_forest_tree, push_water_tile, spawn_generic_land_tile,
    spawn_house_tile, spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
};
use crate::state::{ClientScreen, SimWorld};
use crate::vehicle_render::{TruckHandles, spawn_initial_vehicles};

/// Petición de redibujo del mapa. `sync_camera`: solo tras F9 / cambio de tamaño.
#[derive(Resource, Default)]
pub(crate) struct RemapMapVisualsPending {
    pub(crate) pending: bool,
    pub(crate) sync_camera: bool,
}

pub(crate) struct WorldRenderPlugin;

impl Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemapMapVisualsPending>()
            .add_systems(OnEnter(ClientScreen::InGame), setup)
            .add_systems(
                Update,
                apply_remap_map_visuals
                    .in_set(UpdateSet::RenderRefresh)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

pub(crate) fn setup(mut commands: Commands, asset_server: Res<AssetServer>, sim: Res<SimWorld>) {
    let (mw, mh) = sim.state.map.dimensions();

    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;

    let target_tiles_wide: f32 = if sim.loaded_file { 64.0 } else { mw as f32 };
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        Transform::from_translation(Vec3::new(cam_x, cam_y, 999.9)),
        Projection::Orthographic(OrthographicProjection {
            scale: cam_scale,
            ..OrthographicProjection::default_2d()
        }),
    ));

    spawn_world_layer(&mut commands, &asset_server, &sim);
}

#[allow(clippy::too_many_lines)]
fn spawn_world_layer(commands: &mut Commands, asset_server: &AssetServer, sim: &SimWorld) {
    let (mw, mh) = sim.state.map.dimensions();
    let debug_coast = env_flag("OPENTTDRS_DEBUG_COAST");

    let assets = WorldAssets::load(asset_server);
    let truck_handles = TruckHandles::load(asset_server);
    let map = &sim.state.map;
    let render_grid = RenderGrid::from_map(map, mw, mh);
    let mut batches = MapSpriteBatches::default();

    let mut rail_layers: Vec<u32> = Vec::with_capacity(8);
    for ty in 0..mh {
        for tx in 0..mw {
            let ctx = TileRenderContext::new(map, &render_grid, tx, ty);
            let kind = ctx.kind;
            let tileh = ctx.info.tileh;

            if kind == TileKind::Void {
                continue;
            }

            let slope_half_ground = SLOPE_HALF_H[tileh as usize];

            match kind {
                TileKind::Road => {
                    spawn_road_tile(commands, map, mw, mh, &assets, &ctx, slope_half_ground);
                }
                TileKind::Rail => {
                    spawn_rail_tile(
                        commands,
                        map,
                        (mw, mh),
                        &assets,
                        &ctx,
                        slope_half_ground,
                        &mut rail_layers,
                    );
                }
                TileKind::House => {
                    spawn_house_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Station => {
                    spawn_station_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Industry => {
                    spawn_industry_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Water => {
                    push_water_tile(
                        commands,
                        map,
                        (mw, mh),
                        &assets,
                        &ctx,
                        debug_coast,
                        &mut batches,
                    );
                }
                TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Unknown(_) => {
                    spawn_generic_land_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Void => unreachable!(),
            }

            if kind == TileKind::Forest {
                push_forest_tree(&assets, &ctx, &mut batches);
            }
        }
    }

    flush_map_batches(commands, batches);
    spawn_initial_vehicles(commands, sim, &truck_handles);
    commands.insert_resource(truck_handles);
}

fn sync_camera_for_sim(
    q_cam: &mut Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    sim: &SimWorld,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;
    let target_tiles_wide: f32 = if sim.loaded_file { 64.0 } else { mw as f32 };
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);
    let Ok((mut tf, mut proj)) = q_cam.single_mut() else {
        return;
    };
    tf.translation = Vec3::new(cam_x, cam_y, 999.9);
    let Projection::Orthographic(ref mut o) = *proj else {
        return;
    };
    o.scale = cam_scale;
}

pub(crate) fn apply_remap_map_visuals(
    mut commands: Commands,
    mut pending: ResMut<RemapMapVisualsPending>,
    q_vis: Query<Entity, With<MapVisualLayer>>,
    mut q_cam: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
) {
    if !pending.pending {
        return;
    }
    let do_sync_camera = pending.sync_camera;
    pending.pending = false;
    pending.sync_camera = false;
    let to_remove: Vec<Entity> = q_vis.iter().collect();
    for e in to_remove {
        commands.entity(e).despawn();
    }
    spawn_world_layer(&mut commands, &asset_server, &sim);
    if do_sync_camera {
        sync_camera_for_sim(&mut q_cam, &sim);
    }
}
