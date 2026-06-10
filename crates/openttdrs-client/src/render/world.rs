//! Sistemas Bevy que construyen y refrescan la capa visual del mundo.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileKind;

use crate::bevy_app::UpdateSet;
use crate::config::{env_flag, env_string};
use crate::iso::{
    ISO_HW, ISO_QH, SLOPE_HALF_H, TILE_HALF_H, shore_png_index, shore_tileh_for_draw_shore,
};
use crate::render::viewport::initial_camera_span_tiles;
use crate::render::viewport::{VIEWPORT_MARGIN_TILES, VIEWPORT_REBUILD_LEAD_TILES};
use crate::render::{
    MapPreviewCamera, MapSpriteBatches, MapVisualLayer, PrimaryGameCamera, RenderGrid,
    TileRenderContext, TileViewportBounds, WorldAssets, flush_map_batches,
    large_map_viewport_cull_enabled, ortho_visible_tile_bounds, push_forest_tree, push_water_tile,
    spawn_bridge_middle, spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile,
    spawn_rail_tile, spawn_road_tile, spawn_station_tile, spawn_transport_object_tile,
};
use crate::state::{ClientScreen, SimWorld};

use super::vehicles::{TruckHandles, VehicleIndex, spawn_initial_vehicles};

/// Petición de redibujo del mapa. `sync_camera`: solo tras F9 / cambio de tamaño.
#[derive(Resource, Default)]
pub(crate) struct RemapMapVisualsPending {
    pub(crate) pending: bool,
    pub(crate) sync_camera: bool,
}

/// Rectángulo de teselas para las que se generaron sprites (`MapVisualLayer`).
#[derive(Resource)]
pub(crate) struct MapTileSpawnViewport {
    pub(crate) bounds: TileViewportBounds,
}

impl Default for MapTileSpawnViewport {
    fn default() -> Self {
        Self {
            bounds: TileViewportBounds::full(1, 1),
        }
    }
}

pub(crate) struct WorldRenderPlugin;

impl Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemapMapVisualsPending>()
            .init_resource::<MapTileSpawnViewport>()
            .add_systems(OnEnter(ClientScreen::InGame), setup)
            .add_systems(
                Update,
                (sync_map_tile_spawn_viewport, apply_remap_map_visuals)
                    .chain()
                    .in_set(UpdateSet::Camera)
                    .after(crate::camera::move_camera)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

fn resolve_spawn_viewport(
    sim: &SimWorld,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_q: &Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
) -> TileViewportBounds {
    let (mw, mh) = sim.state.map.dimensions();
    if !large_map_viewport_cull_enabled(mw, mh) {
        return TileViewportBounds::full(mw, mh);
    }
    let Ok((cam_tf, proj)) = cam_q.single() else {
        return TileViewportBounds::full(mw, mh);
    };
    let Projection::Orthographic(ortho) = proj else {
        return TileViewportBounds::full(mw, mh);
    };
    let (win_w, win_h) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 720.0));
    let visible = ortho_visible_tile_bounds(
        cam_tf.translation.truncate(),
        ortho.scale,
        win_w,
        win_h,
        mw,
        mh,
        VIEWPORT_MARGIN_TILES,
    );
    visible.expand(VIEWPORT_REBUILD_LEAD_TILES, mw, mh)
}

/// En mapas grandes, regenera sprites si la cámara sale del bloque ya instanciado.
pub(crate) fn sync_map_tile_spawn_viewport(
    sim: Res<SimWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut viewport: ResMut<MapTileSpawnViewport>,
) {
    let (mw, mh) = sim.state.map.dimensions();
    if !large_map_viewport_cull_enabled(mw, mh) {
        viewport.bounds = TileViewportBounds::full(mw, mh);
        return;
    }
    let needed = resolve_spawn_viewport(&sim, &windows, &cam_q);
    if !viewport.bounds.contains(needed) {
        viewport.bounds = needed;
        pending.pending = true;
        pending.sync_camera = false;
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
) {
    let (mw, mh) = sim.state.map.dimensions();

    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;

    let target_tiles_wide = initial_camera_span_tiles(mw, mh, sim.loaded_file);
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);

    commands.spawn((
        Camera2d,
        PrimaryGameCamera,
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

    let spawn_bounds = resolve_spawn_viewport(&sim, &windows, &cam_q);
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
    });
    spawn_world_layer(&mut commands, &asset_server, &sim, spawn_bounds);
}

#[allow(clippy::too_many_lines)]
fn spawn_world_layer(
    commands: &mut Commands,
    asset_server: &AssetServer,
    sim: &SimWorld,
    spawn_bounds: TileViewportBounds,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let debug_coast = env_flag("OPENTTDRS_DEBUG_COAST");
    let trace_path = env_string("OPENTTDRS_RENDER_TRACE_OUT");
    let mut trace_rows: Vec<String> = Vec::new();
    if trace_path.is_some() {
        trace_rows.push(
            "x,y,kind,tileh,base_z,use_shore,shore_tileh,shore_png_index,mapt,m5".to_string(),
        );
    }

    let assets = WorldAssets::load(asset_server);
    commands.insert_resource(super::WaterAnimFrames {
        water: assets.water_frames.clone(),
        shore: assets.shore_frames.clone(),
    });
    commands.insert_resource(super::ChimneySmokeFrames(assets.chimney_smoke.clone()));
    let truck_handles = TruckHandles::load(asset_server);
    let map = &sim.state.map;
    let render_grid = RenderGrid::from_map(map, mw, mh);
    let mut batches = MapSpriteBatches::default();

    let mut rail_layers: Vec<u32> = Vec::with_capacity(8);
    let mut defer_overlay_tiles: Vec<(u32, u32)> = Vec::new();
    for (tx, ty) in spawn_bounds.iter_coords() {
        let ctx = TileRenderContext::new(map, &render_grid, tx, ty);
        let kind = ctx.kind;
        let tileh = ctx.info.tileh;

        if kind == TileKind::Void {
            continue;
        }

        let slope_half_ground = SLOPE_HALF_H[tileh as usize];
        if trace_path.is_some() {
            let (mapt, m5) = ctx.tile.map_or((0u8, 0u8), |t| (t.mapt, t.m5));
            let (shore_tileh, shore_png) = if kind == TileKind::Water && ctx.info.use_shore {
                let th = shore_tileh_for_draw_shore(map, tx, ty, mw, mh);
                (th as i32, shore_png_index(th) as i32)
            } else {
                (-1, -1)
            };
            trace_rows.push(format!(
                "{tx},{ty},{},{},{},{},{},{},{},{}",
                tile_kind_name(kind),
                ctx.info.tileh,
                ctx.info.base_z,
                if ctx.info.use_shore { 1 } else { 0 },
                shore_tileh,
                shore_png,
                mapt,
                m5
            ));
        }

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
            TileKind::House | TileKind::Station => {
                defer_overlay_tiles.push((tx, ty));
            }
            TileKind::RoadDepot
            | TileKind::RailDepot
            | TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge => {
                spawn_transport_object_tile(commands, &assets, &ctx, slope_half_ground);
            }
            TileKind::Industry => {
                defer_overlay_tiles.push((tx, ty));
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
            push_forest_tree(&assets, &ctx, &mut batches, mw);
        }

        // Tramos de puente que pasan por encima de esta tesela (IsBridgeAbove).
        spawn_bridge_middle(commands, map, (mw, mh), &assets, &ctx);
    }

    flush_map_batches(commands, batches);
    for (tx, ty) in defer_overlay_tiles {
        let ctx = TileRenderContext::new(map, &render_grid, tx, ty);
        let slope_half_ground = SLOPE_HALF_H[ctx.info.tileh as usize];
        match ctx.kind {
            TileKind::Station => spawn_station_tile(
                commands,
                &assets,
                &ctx,
                &sim.state.stations,
                slope_half_ground,
            ),
            TileKind::House => spawn_house_tile(commands, &assets, &ctx, slope_half_ground),
            TileKind::Industry => {
                spawn_industry_tile(commands, &assets, &ctx, slope_half_ground);
            }
            _ => {}
        }
    }
    spawn_initial_vehicles(commands, sim, &truck_handles);
    commands.insert_resource(truck_handles);
    let label_font = asset_server.load::<Font>(crate::ui::font::UI_FONT_PATH);
    super::town_labels::spawn_town_labels(commands, sim, &label_font);
    if let Some(path) = trace_path {
        if let Err(e) = std::fs::write(&path, trace_rows.join("\n")) {
            error!("No se pudo escribir OPENTTDRS_RENDER_TRACE_OUT={path}: {e}");
        } else {
            info!("Render trace escrito en {path}");
        }
    }
}

fn tile_kind_name(kind: TileKind) -> &'static str {
    match kind {
        TileKind::Void => "Void",
        TileKind::Grass => "Grass",
        TileKind::Water => "Water",
        TileKind::Road => "Road",
        TileKind::Rail => "Rail",
        TileKind::RoadDepot => "RoadDepot",
        TileKind::RailDepot => "RailDepot",
        TileKind::RoadTunnel => "RoadTunnel",
        TileKind::RailTunnel => "RailTunnel",
        TileKind::RoadBridge => "RoadBridge",
        TileKind::RailBridge => "RailBridge",
        TileKind::House => "House",
        TileKind::Industry => "Industry",
        TileKind::Station => "Station",
        TileKind::Forest => "Forest",
        TileKind::CoalField => "CoalField",
        TileKind::Unknown(_) => "Unknown",
    }
}

fn sync_camera_for_sim(
    q_cam: &mut Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    sim: &SimWorld,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;
    let target_tiles_wide = initial_camera_span_tiles(mw, mh, sim.loaded_file);
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_remap_map_visuals(
    mut commands: Commands,
    mut pending: ResMut<RemapMapVisualsPending>,
    q_vis: Query<Entity, With<MapVisualLayer>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q_cam: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
) {
    if !pending.pending {
        return;
    }
    let do_sync_camera = pending.sync_camera;
    pending.pending = false;
    pending.sync_camera = false;
    let spawn_bounds = resolve_spawn_viewport(&sim, &windows, &q_cam);
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
    });
    let to_remove: Vec<Entity> = q_vis.iter().collect();
    for e in to_remove {
        commands.entity(e).despawn();
    }
    vehicle_index.rebuild(&sim.state.vehicles);
    if large_map_viewport_cull_enabled(sim.state.map.dimensions().0, sim.state.map.dimensions().1) {
        info!(
            "Mapa visual: {} teselas en viewport (de {})",
            spawn_bounds.tile_count(),
            u64::from(sim.state.map.dimensions().0) * u64::from(sim.state.map.dimensions().1)
        );
    }
    spawn_world_layer(&mut commands, &asset_server, &sim, spawn_bounds);
    if do_sync_camera {
        sync_camera_for_sim(&mut q_cam, &sim);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;

    use crate::render::assets::stub_opengfx_tiles_for_tests;

    fn with_assets_app() -> App {
        let dir = tempfile::tempdir().expect("tempdir");
        stub_opengfx_tiles_for_tests(dir.path());
        let root = dir.path().to_str().expect("utf8");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.add_plugins(AssetPlugin {
            file_path: root.into(),
            ..default()
        });
        app.add_plugins(ImagePlugin::default());
        // Las etiquetas de ciudades cargan la fuente Text2d en spawn_world_layer.
        app.init_asset::<Font>();
        app.update();
        app.insert_resource(SimWorld::default());
        app.insert_resource(RemapMapVisualsPending::default());
        app.insert_resource(VehicleIndex::default());
        app
    }

    #[test]
    fn setup_and_apply_remap_execute_main_paths() {
        let mut app = with_assets_app();
        let world = app.world_mut();

        world.run_system_once(setup).unwrap();
        {
            let mut pending = world.resource_mut::<RemapMapVisualsPending>();
            pending.pending = true;
            pending.sync_camera = true;
        }
        world.run_system_once(apply_remap_map_visuals).unwrap();
    }

    #[test]
    fn tile_kind_name_covers_all_variants() {
        for kind in [
            TileKind::Void,
            TileKind::Grass,
            TileKind::Water,
            TileKind::Road,
            TileKind::Rail,
            TileKind::RoadDepot,
            TileKind::RailDepot,
            TileKind::RoadTunnel,
            TileKind::RailTunnel,
            TileKind::RoadBridge,
            TileKind::RailBridge,
            TileKind::House,
            TileKind::Industry,
            TileKind::Station,
            TileKind::Forest,
            TileKind::CoalField,
            TileKind::Unknown(3),
        ] {
            assert!(!tile_kind_name(kind).is_empty());
        }
    }

    #[test]
    fn apply_remap_returns_early_when_pending_false() {
        let mut app = with_assets_app();
        let world = app.world_mut();
        world.run_system_once(setup).unwrap();
        world.run_system_once(apply_remap_map_visuals).unwrap();
    }

    #[test]
    fn large_map_spawn_viewport_covers_fewer_tiles_than_full_map() {
        let bounds = ortho_visible_tile_bounds(
            Vec2::new(0.0, -200.0),
            2.0,
            1280.0,
            720.0,
            256,
            256,
            VIEWPORT_MARGIN_TILES,
        )
        .expand(VIEWPORT_REBUILD_LEAD_TILES, 256, 256);
        assert!(bounds.tile_count() < 256 * 256);
        assert!(bounds.tile_count() > 100);
    }

    #[test]
    fn sync_camera_for_sim_handles_camera_query_variants() {
        let mut world = World::new();
        let sim = SimWorld {
            loaded_file: true,
            ..SimWorld::default()
        };
        world.insert_resource(sim);

        // Sin cámara: no debe panicar.
        world
            .run_system_once(
                |sim: Res<SimWorld>,
                 mut q_cam: Query<
                    (&mut Transform, &mut Projection),
                    (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
                >| {
                    sync_camera_for_sim(&mut q_cam, &sim);
                },
            )
            .unwrap();

        // Cámara ortográfica: debe ajustar escala/transform.
        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world
            .run_system_once(
                |sim: Res<SimWorld>,
                 mut q_cam: Query<
                    (&mut Transform, &mut Projection),
                    (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
                >| {
                    sync_camera_for_sim(&mut q_cam, &sim);
                },
            )
            .unwrap();

        // Cámara no ortográfica: sigue sin panicar (sale por early return).
        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        world
            .run_system_once(
                |sim: Res<SimWorld>,
                 mut q_cam: Query<
                    (&mut Transform, &mut Projection),
                    (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
                >| {
                    sync_camera_for_sim(&mut q_cam, &sim);
                },
            )
            .unwrap();
    }
}
