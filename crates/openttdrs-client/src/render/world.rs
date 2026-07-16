//! Sistemas Bevy que construyen y refrescan la capa visual del mundo.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{TileCoord, TileKind, tile_owner_colour};

use crate::bevy_app::UpdateSet;
use crate::config::{env_flag, env_string};
use crate::iso::{
    ISO_HW, ISO_QH, SLOPE_HALF_H, TILE_HALF_H, shore_png_index, shore_tileh_for_draw_shore,
};
use crate::render::viewport::initial_camera_span_tiles;
use crate::render::viewport::{VIEWPORT_MARGIN_TILES, VIEWPORT_REBUILD_LEAD_TILES};
use crate::render::{
    CompanyColoredSprites, MapPreviewCamera, MapSpriteBatches, MapTileChunk, MapVisualLayer,
    PrimaryGameCamera, RenderGrid, TileAtlas, TileRenderContext, TileViewportBounds, WorldAssets,
    chunk_tile_bounds, chunks_in_bounds, flush_map_batches, large_map_viewport_cull_enabled,
    ortho_visible_tile_bounds, push_forest_tree, push_water_tile, spawn_bridge_middle,
    spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile, spawn_rail_tile,
    spawn_road_tile, spawn_station_tile, spawn_transport_object_tile,
};
use crate::sprites::CompanyColour;
use crate::state::{ClientScreen, SimWorld};

use super::vehicles::{NewGrfTrainSpriteCache, TruckHandles, VehicleIndex, spawn_initial_vehicles};

fn owner_colour_for_tile(
    sim: &SimWorld,
    coord: TileCoord,
    kind: TileKind,
) -> Option<CompanyColour> {
    tile_owner_colour(
        &sim.state.companies,
        &sim.state.stations,
        &sim.state.map,
        coord,
        kind,
        sim.state.company_colour,
    )
    .map(CompanyColour::from_u8)
}

/// Queries de etiquetas del mapa (agrupadas para no superar el límite de params Bevy).
#[derive(SystemParam)]
pub(crate) struct MapLabelEntities<'w, 's> {
    towns: Query<'w, 's, Entity, With<super::town_labels::TownLabel>>,
    stations: Query<'w, 's, Entity, With<super::station_labels::StationLabel>>,
    signs: Query<'w, 's, Entity, With<super::sign_labels::SignLabel>>,
}

/// Agrupa cachés NewGRF in-world para no superar el límite de 16 `SystemParam`.
#[derive(SystemParam)]
pub(crate) struct NewGrfMapSpriteCaches<'w> {
    road: ResMut<'w, crate::render::NewGrfRoadSpriteCache>,
    station: ResMut<'w, crate::render::NewGrfStationSpriteCache>,
    shore: ResMut<'w, crate::render::NewGrfShoreSpriteCache>,
    catenary: ResMut<'w, crate::render::NewGrfCatenarySpriteCache>,
    industry: ResMut<'w, crate::render::NewGrfIndustrySpriteCache>,
}

/// Petición de redibujo del mapa. `sync_camera`: solo tras F9 / cambio de tamaño.
#[derive(Resource)]
pub(crate) struct RemapMapVisualsPending {
    pub(crate) pending: bool,
    pub(crate) sync_camera: bool,
    /// Rebuild completo (construcción, F9). Pan en mapas grandes usa `full = false`.
    pub(crate) full: bool,
    /// Chunks a regenerar in-place (construcción dentro del viewport ya cargado).
    pub(crate) refresh_chunks: HashSet<(u32, u32)>,
}

impl RemapMapVisualsPending {
    pub(crate) fn extend_refresh_chunks(&mut self, tiles: &[(i32, i32)]) {
        for &(tx, ty) in tiles {
            if tx >= 0 && ty >= 0 {
                let ch = MapTileChunk::from_tile(tx as u32, ty as u32);
                self.refresh_chunks.insert((ch.cx, ch.cy));
            }
        }
    }
}

impl Default for RemapMapVisualsPending {
    fn default() -> Self {
        Self {
            pending: false,
            sync_camera: false,
            full: true,
            refresh_chunks: HashSet::new(),
        }
    }
}

/// Marca redibujo tras construcción/sim: en mapas con culling refresca solo los chunks tocados.
pub(crate) fn request_map_visual_remap(
    pending: &mut RemapMapVisualsPending,
    mw: u32,
    mh: u32,
    tiles: &[(i32, i32)],
) {
    pending.pending = true;
    pending.sync_camera = false;
    if large_map_viewport_cull_enabled(mw, mh) {
        pending.full = false;
        pending.extend_refresh_chunks(tiles);
    } else {
        pending.full = true;
    }
}

/// Bloques 16×16 ya instanciados (solo mapas con culling por viewport).
#[derive(Resource, Default)]
pub(crate) struct LoadedMapTileChunks {
    pub chunks: HashSet<(u32, u32)>,
}

/// Rectángulo de teselas para las que se generaron sprites (`MapVisualLayer`).
#[derive(Resource)]
pub(crate) struct MapTileSpawnViewport {
    pub(crate) bounds: TileViewportBounds,
    /// Último `OrthographicProjection::scale` usado para `bounds` (detectar zoom).
    pub(crate) last_ortho_scale: f32,
}

impl Default for MapTileSpawnViewport {
    fn default() -> Self {
        Self {
            bounds: TileViewportBounds::full(1, 1),
            last_ortho_scale: 1.0,
        }
    }
}

pub(crate) struct WorldRenderPlugin;

impl Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemapMapVisualsPending>()
            .init_resource::<MapTileSpawnViewport>()
            .init_resource::<LoadedMapTileChunks>()
            .init_resource::<crate::render::NewGrfRoadSpriteCache>()
            .init_resource::<crate::render::NewGrfStationSpriteCache>()
            .init_resource::<crate::render::NewGrfShoreSpriteCache>()
            .init_resource::<crate::render::NewGrfCatenarySpriteCache>()
            .init_resource::<crate::render::NewGrfIndustrySpriteCache>()
            .add_systems(OnEnter(ClientScreen::InGame), setup)
            .add_systems(
                Update,
                (
                    sync_map_tile_spawn_viewport,
                    sync_company_colored_sprites,
                    apply_remap_map_visuals,
                )
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
    if let Ok((cam_tf, proj)) = cam_q.single() {
        let ortho_scale = if let Projection::Orthographic(o) = proj {
            o.scale
        } else {
            1.0
        };
        return resolve_spawn_viewport_at(sim, windows, cam_tf.translation.truncate(), ortho_scale);
    }
    let (cam_pos, ortho_scale) = initial_map_camera_pose(sim);
    resolve_spawn_viewport_at(sim, windows, cam_pos.truncate(), ortho_scale)
}

fn resolve_spawn_viewport_at(
    sim: &SimWorld,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_translation: Vec2,
    ortho_scale: f32,
) -> TileViewportBounds {
    let (mw, mh) = sim.state.map.dimensions();
    if !large_map_viewport_cull_enabled(mw, mh) {
        return TileViewportBounds::full(mw, mh);
    }
    let (win_w, win_h) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 720.0));
    let visible = ortho_visible_tile_bounds(
        cam_translation,
        ortho_scale,
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
    let ortho_scale = cam_q
        .single()
        .ok()
        .and_then(|(_, proj)| {
            if let Projection::Orthographic(o) = proj {
                Some(o.scale)
            } else {
                None
            }
        })
        .unwrap_or(viewport.last_ortho_scale);
    let scale_changed = (ortho_scale - viewport.last_ortho_scale).abs() > f32::EPSILON;
    if scale_changed || !viewport.bounds.contains(needed) {
        viewport.bounds = needed;
        viewport.last_ortho_scale = ortho_scale;
        pending.pending = true;
        pending.sync_camera = false;
        pending.full = false;
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layout_assets: ResMut<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
    sim: Res<SimWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    prefs: Option<Res<crate::settings::ClientPreferences>>,
) {
    let (cam_pos, cam_scale) = initial_map_camera_pose(&sim);

    commands.spawn((
        Camera2d,
        PrimaryGameCamera,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        Transform::from_translation(cam_pos),
        Projection::Orthographic(OrthographicProjection {
            scale: cam_scale,
            ..OrthographicProjection::default_2d()
        }),
    ));

    let spawn_bounds = resolve_spawn_viewport_at(&sim, &windows, cam_pos.truncate(), cam_scale);
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
        last_ortho_scale: cam_scale,
    });
    let atlas = TileAtlas::build(&asset_server, &mut layout_assets);
    let assets = WorldAssets::load(&atlas, &mut images);
    commands.insert_resource(assets.clone());
    commands.insert_resource(super::WaterAnimFrames {
        water: assets.water_frames.clone(),
        shore: assets.shore_frames.clone(),
    });
    commands.insert_resource(super::RefineryFireAnimFrames {
        by_sprite: assets.refinery_fire_frames.clone(),
    });
    commands.insert_resource(super::FizzyDrinkAnimFrames {
        by_sprite: assets.fizzy_drink_frames.clone(),
    });
    commands.insert_resource(super::LighthouseAnimFrames {
        by_sprite: assets.lighthouse_anim_frames.clone(),
    });
    commands.insert_resource(super::ChimneySmokeFrames(assets.chimney_smoke.clone()));
    commands.insert_resource(super::CopperMineSmokeFrames(
        assets.copper_mine_smoke.clone(),
    ));
    commands.insert_resource(super::EffectVehicleFrames::from_world_assets(&assets));
    let company_colour = CompanyColour::from_u8(sim.state.company_colour);
    let mut company_sprites = CompanyColoredSprites::new(company_colour);
    company_sprites.build_all(&mut images);
    commands.insert_resource(company_sprites.clone());
    let show_town_labels = prefs.as_ref().map(|p| p.show_town_labels).unwrap_or(true);
    let show_station_labels = prefs
        .as_ref()
        .map(|p| p.show_station_labels)
        .unwrap_or(true);
    let show_full_detail = prefs.as_ref().map(|p| p.full_detail).unwrap_or(true);
    let mut road_sprites = crate::render::NewGrfRoadSpriteCache::default();
    let mut station_sprites = crate::render::NewGrfStationSpriteCache::default();
    let mut shore_sprites = crate::render::NewGrfShoreSpriteCache::default();
    let mut catenary_sprites = crate::render::NewGrfCatenarySpriteCache::default();
    let mut industry_sprites = crate::render::NewGrfIndustrySpriteCache::default();
    spawn_world_layer(
        &mut commands,
        &asset_server,
        &assets,
        &mut company_sprites,
        &mut images,
        &sim,
        spawn_bounds,
        true,
        true,
        show_full_detail,
        show_town_labels,
        show_station_labels,
        &mut road_sprites,
        &mut station_sprites,
        &mut shore_sprites,
        &mut catenary_sprites,
        &mut industry_sprites,
    );
    commands.insert_resource(road_sprites);
    commands.insert_resource(station_sprites);
    commands.insert_resource(shore_sprites);
    commands.insert_resource(catenary_sprites);
    commands.insert_resource(industry_sprites);
    commands.insert_resource(atlas);
    commands.insert_resource(LoadedMapTileChunks {
        chunks: chunks_in_bounds(spawn_bounds),
    });
}

/// Posición y escala ortho iniciales para un mapa (menú intro o partida).
#[must_use]
pub(crate) fn initial_map_camera_pose(sim: &SimWorld) -> (Vec3, f32) {
    let (mw, mh) = sim.state.map.dimensions();
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;
    let target_tiles_wide = initial_camera_span_tiles(mw, mh, sim.loaded_file);
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);
    (Vec3::new(cam_x, cam_y, 999.9), cam_scale)
}

/// Capa visual del mapa para el fondo del menú (sin vehículos ni etiquetas).
pub(crate) fn spawn_intro_map_render(
    commands: &mut Commands,
    asset_server: &AssetServer,
    layout_assets: &mut Assets<TextureAtlasLayout>,
    images: &mut Assets<Image>,
    sim: &SimWorld,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let spawn_bounds = TileViewportBounds::full(mw, mh);
    let atlas = TileAtlas::build(asset_server, layout_assets);
    let assets = WorldAssets::load(&atlas, images);
    commands.insert_resource(assets.clone());
    commands.insert_resource(super::WaterAnimFrames {
        water: assets.water_frames.clone(),
        shore: assets.shore_frames.clone(),
    });
    commands.insert_resource(super::RefineryFireAnimFrames {
        by_sprite: assets.refinery_fire_frames.clone(),
    });
    commands.insert_resource(super::FizzyDrinkAnimFrames {
        by_sprite: assets.fizzy_drink_frames.clone(),
    });
    commands.insert_resource(super::LighthouseAnimFrames {
        by_sprite: assets.lighthouse_anim_frames.clone(),
    });
    commands.insert_resource(super::ChimneySmokeFrames(assets.chimney_smoke.clone()));
    commands.insert_resource(super::CopperMineSmokeFrames(
        assets.copper_mine_smoke.clone(),
    ));
    commands.insert_resource(super::EffectVehicleFrames::from_world_assets(&assets));
    let company_colour = CompanyColour::from_u8(sim.state.company_colour);
    let mut company_sprites = CompanyColoredSprites::new(company_colour);
    company_sprites.build_all(images);
    commands.insert_resource(company_sprites.clone());
    let mut road_sprites = crate::render::NewGrfRoadSpriteCache::default();
    let mut station_sprites = crate::render::NewGrfStationSpriteCache::default();
    let mut shore_sprites = crate::render::NewGrfShoreSpriteCache::default();
    let mut catenary_sprites = crate::render::NewGrfCatenarySpriteCache::default();
    let mut industry_sprites = crate::render::NewGrfIndustrySpriteCache::default();
    spawn_world_layer(
        commands,
        asset_server,
        &assets,
        &mut company_sprites,
        images,
        sim,
        spawn_bounds,
        false,
        true,
        true,
        true,
        true,
        &mut road_sprites,
        &mut station_sprites,
        &mut shore_sprites,
        &mut catenary_sprites,
        &mut industry_sprites,
    );
    commands.insert_resource(road_sprites);
    commands.insert_resource(station_sprites);
    commands.insert_resource(shore_sprites);
    commands.insert_resource(catenary_sprites);
    commands.insert_resource(industry_sprites);
    commands.insert_resource(atlas);
    commands.insert_resource(LoadedMapTileChunks {
        chunks: chunks_in_bounds(spawn_bounds),
    });
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn spawn_map_tiles_in_bounds(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: &mut CompanyColoredSprites,
    images: &mut Assets<Image>,
    sim: &SimWorld,
    spawn_bounds: TileViewportBounds,
    show_pbs_reservations: bool,
    show_full_detail: bool,
    road_sprites: &mut crate::render::NewGrfRoadSpriteCache,
    station_sprites: &mut crate::render::NewGrfStationSpriteCache,
    shore_sprites: &mut crate::render::NewGrfShoreSpriteCache,
    catenary_sprites: &mut crate::render::NewGrfCatenarySpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
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

    let map = &sim.state.map;
    let climate = sim.state.climate;
    let world_seed = sim.state.world_seed;
    for c in &sim.state.companies {
        company.ensure_palette(CompanyColour::from_u8(c.colour), images);
    }
    // +1 para vecinos de orilla; no barrer el mapa completo en 1024²+.
    let grid_bounds = spawn_bounds.expand(1, mw, mh);
    let render_grid = RenderGrid::from_bounds(map, mw, mh, grid_bounds);
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
                spawn_road_tile(
                    commands,
                    map,
                    mw,
                    mh,
                    assets,
                    &ctx,
                    slope_half_ground,
                    climate,
                    show_pbs_reservations,
                    show_full_detail,
                    &sim.state.road_type_catalog,
                    Some(road_sprites),
                    Some(images),
                    &sim.state.newgrf_stack,
                );
            }
            TileKind::Rail => {
                spawn_rail_tile(
                    commands,
                    map,
                    (mw, mh),
                    assets,
                    &ctx,
                    slope_half_ground,
                    &mut rail_layers,
                    climate,
                    show_pbs_reservations,
                    show_full_detail,
                    &sim.state.catenary_newgrf_sprites,
                    Some(catenary_sprites),
                    Some(images),
                );
            }
            TileKind::House | TileKind::Station => {
                defer_overlay_tiles.push((tx, ty));
            }
            TileKind::RoadDepot
            | TileKind::RailDepot
            | TileKind::ShipDepot
            | TileKind::Airport
            | TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge => {
                spawn_transport_object_tile(
                    commands,
                    assets,
                    Some(company),
                    owner_colour_for_tile(sim, ctx.coord, kind),
                    &ctx,
                    slope_half_ground,
                    map,
                    (mw, mh),
                    &sim.state.catenary_newgrf_sprites,
                    Some(catenary_sprites),
                    Some(images),
                );
            }
            TileKind::Industry => {
                defer_overlay_tiles.push((tx, ty));
            }
            TileKind::Water => {
                push_water_tile(
                    commands,
                    map,
                    (mw, mh),
                    assets,
                    &ctx,
                    debug_coast,
                    &mut batches,
                    &sim.state.shore_newgrf_sprites,
                    Some(shore_sprites),
                    Some(images),
                );
            }
            TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Unknown(_) => {
                spawn_generic_land_tile(
                    commands,
                    assets,
                    &ctx,
                    slope_half_ground,
                    climate,
                    world_seed,
                );
            }
            TileKind::Void => unreachable!(),
        }

        if kind == TileKind::Forest {
            push_forest_tree(assets, &ctx, &mut batches, mw);
        }

        // Tramos de puente que pasan por encima de esta tesela (IsBridgeAbove).
        spawn_bridge_middle(
            commands,
            map,
            (mw, mh),
            assets,
            &ctx,
            &sim.state.catenary_newgrf_sprites,
            Some(catenary_sprites),
            Some(images),
        );
    }

    flush_map_batches(commands, batches);
    for (tx, ty) in defer_overlay_tiles {
        let ctx = TileRenderContext::new(map, &render_grid, tx, ty);
        let slope_half_ground = SLOPE_HALF_H[ctx.info.tileh as usize];
        match ctx.kind {
            TileKind::Station => spawn_station_tile(
                commands,
                map,
                (mw, mh),
                assets,
                Some(company),
                owner_colour_for_tile(sim, ctx.coord, TileKind::Station),
                &ctx,
                &sim.state.stations,
                slope_half_ground,
                &sim.state.station_spec_catalog,
                Some(station_sprites),
                Some(images),
                &sim.state.catenary_newgrf_sprites,
                Some(catenary_sprites),
                climate,
                &sim.state.newgrf_stack,
            ),
            TileKind::House => spawn_house_tile(commands, assets, &ctx, slope_half_ground),
            TileKind::Industry => {
                spawn_industry_tile(
                    commands,
                    assets,
                    map,
                    &ctx,
                    slope_half_ground,
                    &sim.state.industries,
                    company,
                    images,
                    &sim.state.industry_tile_spec_catalog,
                    &sim.state.industry_tile_overrides,
                    Some(industry_sprites),
                    &sim.state.newgrf_stack,
                );
            }
            _ => {}
        }
    }
    if let Some(path) = trace_path {
        if let Err(e) = std::fs::write(&path, trace_rows.join("\n")) {
            error!("No se pudo escribir OPENTTDRS_RENDER_TRACE_OUT={path}: {e}");
        } else {
            info!("Render trace escrito en {path}");
        }
    }
}

fn sync_company_colored_sprites(
    sim: Res<SimWorld>,
    mut company: ResMut<CompanyColoredSprites>,
    mut images: ResMut<Assets<Image>>,
    mut pending: ResMut<RemapMapVisualsPending>,
) {
    let colour = CompanyColour::from_u8(sim.state.company_colour);
    if company.colour == colour {
        return;
    }
    company.colour = colour;
    company.build_all(&mut images);
    pending.pending = true;
    pending.full = true;
    pending.sync_camera = false;
}

#[allow(clippy::too_many_arguments)]
fn spawn_world_layer(
    commands: &mut Commands,
    asset_server: &AssetServer,
    assets: &WorldAssets,
    company: &mut CompanyColoredSprites,
    images: &mut Assets<Image>,
    sim: &SimWorld,
    spawn_bounds: TileViewportBounds,
    include_world_extras: bool,
    show_pbs_reservations: bool,
    show_full_detail: bool,
    show_town_labels: bool,
    show_station_labels: bool,
    road_sprites: &mut crate::render::NewGrfRoadSpriteCache,
    station_sprites: &mut crate::render::NewGrfStationSpriteCache,
    shore_sprites: &mut crate::render::NewGrfShoreSpriteCache,
    catenary_sprites: &mut crate::render::NewGrfCatenarySpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
) {
    if include_world_extras {
        let truck_handles = TruckHandles::load(asset_server);
        let mut newgrf_train_sprites = NewGrfTrainSpriteCache::default();
        spawn_initial_vehicles(
            commands,
            sim,
            &truck_handles,
            company,
            &mut newgrf_train_sprites,
            images,
        );
        commands.insert_resource(truck_handles);
        commands.insert_resource(newgrf_train_sprites);
        let label_font = asset_server.load::<Font>(crate::ui::font::UI_FONT_PATH);
        super::town_labels::spawn_town_labels(
            commands,
            sim,
            &label_font,
            spawn_bounds,
            show_town_labels,
        );
        super::station_labels::spawn_station_labels(
            commands,
            sim,
            &label_font,
            spawn_bounds,
            show_station_labels,
        );
        super::sign_labels::spawn_sign_labels(commands, sim, &label_font, spawn_bounds);
    }
    spawn_map_tiles_in_bounds(
        commands,
        assets,
        company,
        images,
        sim,
        spawn_bounds,
        show_pbs_reservations,
        show_full_detail,
        road_sprites,
        station_sprites,
        shore_sprites,
        catenary_sprites,
        industry_sprites,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_map_chunk(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: &mut CompanyColoredSprites,
    images: &mut Assets<Image>,
    sim: &SimWorld,
    cx: u32,
    cy: u32,
    show_pbs_reservations: bool,
    show_full_detail: bool,
    road_sprites: &mut crate::render::NewGrfRoadSpriteCache,
    station_sprites: &mut crate::render::NewGrfStationSpriteCache,
    shore_sprites: &mut crate::render::NewGrfShoreSpriteCache,
    catenary_sprites: &mut crate::render::NewGrfCatenarySpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
) {
    let (mw, mh) = sim.state.map.dimensions();
    spawn_map_tiles_in_bounds(
        commands,
        assets,
        company,
        images,
        sim,
        chunk_tile_bounds(cx, cy, mw, mh),
        show_pbs_reservations,
        show_full_detail,
        road_sprites,
        station_sprites,
        shore_sprites,
        catenary_sprites,
        industry_sprites,
    );
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
        TileKind::ShipDepot => "ShipDepot",
        TileKind::Airport => "Airport",
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
    q_chunks: Query<(Entity, &MapTileChunk), With<MapVisualLayer>>,
    label_entities: MapLabelEntities,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q_cam: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    asset_server: Res<AssetServer>,
    assets: Option<Res<WorldAssets>>,
    mut company: Option<ResMut<CompanyColoredSprites>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut newgrf_sprites: NewGrfMapSpriteCaches,
    sim: Res<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut loaded_chunks: ResMut<LoadedMapTileChunks>,
    prefs: Res<crate::settings::ClientPreferences>,
) {
    if !pending.pending {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    let Some(company) = company.as_mut() else {
        return;
    };
    let Some(images) = images.as_mut() else {
        return;
    };
    let do_sync_camera = pending.sync_camera;
    let full_rebuild = pending.full;
    let mut refresh_chunks = std::mem::take(&mut pending.refresh_chunks);
    pending.pending = false;
    pending.sync_camera = false;
    pending.full = true;

    let (mw, mh) = sim.state.map.dimensions();
    let spawn_bounds = resolve_spawn_viewport(&sim, &windows, &q_cam);
    let ortho_scale = q_cam
        .single()
        .ok()
        .and_then(|(_, proj)| {
            if let Projection::Orthographic(o) = proj {
                Some(o.scale)
            } else {
                None
            }
        })
        .unwrap_or(1.0);
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
        last_ortho_scale: ortho_scale,
    });

    let use_incremental = !full_rebuild
        && large_map_viewport_cull_enabled(mw, mh)
        && !loaded_chunks.chunks.is_empty();

    let show_pbs = prefs.show_pbs_reservations;
    let show_full_detail = prefs.full_detail;
    let show_town_labels = prefs.show_town_labels;
    let show_station_labels = prefs.show_station_labels;

    if use_incremental {
        let needed = chunks_in_bounds(spawn_bounds);
        // Construcción: regenerar todo el viewport visible para evitar capas
        // de hierba superpuestas (teselas oscuras en rombo).
        if !refresh_chunks.is_empty() {
            refresh_chunks = needed.clone();
        }
        let to_remove: HashSet<_> = loaded_chunks.chunks.difference(&needed).copied().collect();
        let to_add: HashSet<_> = needed.difference(&loaded_chunks.chunks).copied().collect();

        for (entity, chunk) in &q_chunks {
            if to_remove.contains(&(chunk.cx, chunk.cy)) {
                commands.entity(entity).despawn();
            }
        }
        for &(cx, cy) in &to_add {
            if refresh_chunks.contains(&(cx, cy)) {
                continue;
            }
            spawn_map_chunk(
                &mut commands,
                assets.as_ref(),
                company.as_mut(),
                images.as_mut(),
                &sim,
                cx,
                cy,
                show_pbs,
                show_full_detail,
                newgrf_sprites.road.as_mut(),
                newgrf_sprites.station.as_mut(),
                newgrf_sprites.shore.as_mut(),
                newgrf_sprites.catenary.as_mut(),
                newgrf_sprites.industry.as_mut(),
            );
        }
        let mut refresh_despawn = Vec::new();
        for (entity, chunk) in &q_chunks {
            if refresh_chunks.contains(&(chunk.cx, chunk.cy)) {
                refresh_despawn.push(entity);
            }
        }
        for entity in refresh_despawn {
            commands.entity(entity).despawn();
        }
        for &(cx, cy) in &refresh_chunks {
            if !needed.contains(&(cx, cy)) {
                continue;
            }
            spawn_map_chunk(
                &mut commands,
                assets.as_ref(),
                company.as_mut(),
                images.as_mut(),
                &sim,
                cx,
                cy,
                show_pbs,
                show_full_detail,
                newgrf_sprites.road.as_mut(),
                newgrf_sprites.station.as_mut(),
                newgrf_sprites.shore.as_mut(),
                newgrf_sprites.catenary.as_mut(),
                newgrf_sprites.industry.as_mut(),
            );
        }
        loaded_chunks.chunks = needed;
        // Etiquetas no van en chunks: re-sincronizar al panear el viewport.
        let label_font = asset_server.load::<Font>(crate::ui::font::UI_FONT_PATH);
        let town_entities: Vec<Entity> = label_entities.towns.iter().collect();
        super::town_labels::resync_town_labels(
            &mut commands,
            town_entities,
            &sim,
            &label_font,
            spawn_bounds,
            show_town_labels,
        );
        let station_label_entities: Vec<Entity> = label_entities.stations.iter().collect();
        super::station_labels::resync_station_labels(
            &mut commands,
            station_label_entities,
            &sim,
            &label_font,
            spawn_bounds,
            show_station_labels,
        );
        let sign_entities: Vec<Entity> = label_entities.signs.iter().collect();
        super::sign_labels::resync_sign_labels(
            &mut commands,
            sign_entities,
            &sim,
            &label_font,
            spawn_bounds,
        );
        if !to_add.is_empty() || !to_remove.is_empty() || !refresh_chunks.is_empty() {
            info!(
                "Mapa visual incremental: +{} −{} ↻{} chunks ({} teselas visibles)",
                to_add.len(),
                to_remove.len(),
                refresh_chunks.len(),
                spawn_bounds.tile_count()
            );
        }
    } else {
        let to_remove: Vec<Entity> = q_vis.iter().collect();
        for e in to_remove {
            commands.entity(e).despawn();
        }
        vehicle_index.rebuild(&sim.state.vehicles);
        if large_map_viewport_cull_enabled(mw, mh) {
            info!(
                "Mapa visual: {} teselas en viewport (de {})",
                spawn_bounds.tile_count(),
                u64::from(mw) * u64::from(mh)
            );
        }
        spawn_world_layer(
            &mut commands,
            &asset_server,
            assets.as_ref(),
            company.as_mut(),
            images.as_mut(),
            &sim,
            spawn_bounds,
            true,
            show_pbs,
            show_full_detail,
            show_town_labels,
            show_station_labels,
            newgrf_sprites.road.as_mut(),
            newgrf_sprites.station.as_mut(),
            newgrf_sprites.shore.as_mut(),
            newgrf_sprites.catenary.as_mut(),
            newgrf_sprites.industry.as_mut(),
        );
        loaded_chunks.chunks = chunks_in_bounds(spawn_bounds);
    }

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
        app.init_asset::<TextureAtlasLayout>();
        app.update();
        app.insert_resource(SimWorld::default());
        app.insert_resource(crate::settings::ClientPreferences::default());
        app.insert_resource(RemapMapVisualsPending::default());
        app.insert_resource(VehicleIndex::default());
        app.insert_resource(LoadedMapTileChunks::default());
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
