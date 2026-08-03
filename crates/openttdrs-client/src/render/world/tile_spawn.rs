//! Construcción de tiles del mundo y setup inicial.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::prelude::*;

use crate::config::{env_flag, env_string};
use crate::iso::{SLOPE_HALF_H, shore_png_index, shore_tileh_for_draw_shore};
use crate::render::{
    CompanyColoredSprites, MapSpriteBatches, RenderGrid, TileAtlas, TileRenderContext,
    TileViewportBounds, WorldAssets, chunk_tile_bounds, chunks_in_bounds, flush_map_batches,
    push_forest_tree, push_water_tile, spawn_bridge_middle, spawn_generic_land_tile,
    spawn_house_tile, spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
    spawn_transport_object_tile,
};
use crate::sprites::CompanyColour;
use crate::state::SimWorld;

use super::plugin::{LoadedMapTileChunks, MapTileSpawnViewport};
use super::viewport::{initial_map_camera_pose, resolve_spawn_viewport_at};

use crate::render::vehicles::{NewGrfTrainSpriteCache, TruckHandles, spawn_initial_vehicles};

fn owner_colour_for_tile(
    sim: &SimWorld,
    coord: TileCoord,
    kind: TileKind,
) -> Option<CompanyColour> {
    if kind == TileKind::Grass
        && sim.state.map.get(coord).is_some_and(|tile| {
            openttdrs_core::map::object_type_from_tile(&tile)
                == Some(openttdrs_core::OBJECT_TYPE_STATUE_COMPANY)
        })
    {
        let owner = sim.state.map.get(coord).map_or(0, |tile| tile.m1);
        return sim
            .state
            .companies
            .iter()
            .find(|company| company.id.0 == owner)
            .map(|company| CompanyColour::from_u8(company.colour));
    }
    openttdrs_core::tile_owner_colour(
        &sim.state.companies,
        &sim.state.stations,
        &sim.state.map,
        coord,
        kind,
        sim.state.company_colour,
    )
    .map(CompanyColour::from_u8)
}

pub(super) fn tile_kind_name(kind: TileKind) -> &'static str {
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn spawn_map_tiles_in_bounds(
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
    signal_sprites: &mut crate::render::NewGrfSignalSpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
    object_sprites: &mut crate::render::NewGrfObjectSpriteCache,
    action5_sprites: &mut crate::render::NewGrfAction5SpriteCache,
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
                    &sim.state.runtime.oneway_newgrf_sprites,
                    Some(action5_sprites),
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
                    sim.state.construction.signals_on_right(),
                    &sim.state.runtime.catenary_newgrf_sprites,
                    Some(catenary_sprites),
                    &sim.state.runtime.rail_signal_newgrf,
                    Some(signal_sprites),
                    &sim.state.runtime.signal_action5_newgrf_sprites,
                    &sim.state.runtime.foundation_newgrf_sprites,
                    Some(action5_sprites),
                    Some(images),
                    sim.state.calendar.date,
                    &sim.state.newgrf_stack,
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
                    show_pbs_reservations,
                    map,
                    (mw, mh),
                    &sim.state.runtime.catenary_newgrf_sprites,
                    Some(catenary_sprites),
                    &sim.state.runtime.bridge_decks_newgrf_sprites,
                    Some(action5_sprites),
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
                    &sim.state.runtime.shore_newgrf_sprites,
                    Some(shore_sprites),
                    Some(images),
                );
            }
            TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Unknown(_) => {
                spawn_generic_land_tile(
                    commands,
                    assets,
                    Some(company),
                    owner_colour_for_tile(sim, ctx.coord, kind),
                    &ctx,
                    slope_half_ground,
                    climate,
                    world_seed,
                    &sim.state.object_spec_catalog,
                    Some(object_sprites),
                    Some(images),
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
            show_pbs_reservations,
            &sim.state.runtime.catenary_newgrf_sprites,
            Some(catenary_sprites),
            &sim.state.runtime.bridge_decks_newgrf_sprites,
            Some(action5_sprites),
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
                &sim.state.road_stop_spec_catalog,
                Some(station_sprites),
                Some(images),
                &sim.state.runtime.catenary_newgrf_sprites,
                Some(catenary_sprites),
                &sim.state.runtime.foundation_newgrf_sprites,
                Some(action5_sprites),
                &sim.state.runtime.roadstop_action5_newgrf_sprites,
                climate,
                &sim.state.newgrf_stack,
            ),
            TileKind::House => spawn_house_tile(
                commands,
                assets,
                &ctx,
                slope_half_ground,
                &sim.state.house_spec_catalog,
            ),
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
                    &sim.state.runtime.foundation_newgrf_sprites,
                    Some(action5_sprites),
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_world_layer(
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
    signal_sprites: &mut crate::render::NewGrfSignalSpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
    object_sprites: &mut crate::render::NewGrfObjectSpriteCache,
    action5_sprites: &mut crate::render::NewGrfAction5SpriteCache,
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
        crate::render::town_labels::spawn_town_labels(
            commands,
            sim,
            &label_font,
            spawn_bounds,
            show_town_labels,
        );
        crate::render::station_labels::spawn_station_labels(
            commands,
            sim,
            &label_font,
            spawn_bounds,
            show_station_labels,
        );
        crate::render::sign_labels::spawn_sign_labels(commands, sim, &label_font, spawn_bounds);
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
        signal_sprites,
        industry_sprites,
        object_sprites,
        action5_sprites,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_map_chunk(
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
    signal_sprites: &mut crate::render::NewGrfSignalSpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
    object_sprites: &mut crate::render::NewGrfObjectSpriteCache,
    action5_sprites: &mut crate::render::NewGrfAction5SpriteCache,
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
        signal_sprites,
        industry_sprites,
        object_sprites,
        action5_sprites,
    );
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
        crate::render::PrimaryGameCamera,
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
    commands.insert_resource(crate::render::water_anim_frames_from_assets(
        &assets,
        &layout_assets,
    ));
    commands.insert_resource(crate::render::RefineryFireAnimFrames {
        by_sprite: assets.refinery_fire_frames.clone(),
    });
    commands.insert_resource(crate::render::FizzyDrinkAnimFrames {
        by_sprite: assets.fizzy_drink_frames.clone(),
    });
    commands.insert_resource(crate::render::LighthouseAnimFrames {
        by_sprite: assets.lighthouse_anim_frames.clone(),
    });
    commands.insert_resource(crate::render::ChimneySmokeFrames(
        assets.chimney_smoke.clone(),
    ));
    commands.insert_resource(crate::render::CopperMineSmokeFrames(
        assets.copper_mine_smoke.clone(),
    ));
    commands.insert_resource(crate::render::EffectVehicleFrames::from_world_assets(
        &assets,
    ));
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
    let mut signal_sprites = crate::render::NewGrfSignalSpriteCache::default();
    let mut industry_sprites = crate::render::NewGrfIndustrySpriteCache::default();
    let mut object_sprites = crate::render::NewGrfObjectSpriteCache::default();
    let mut action5_sprites = crate::render::NewGrfAction5SpriteCache::default();
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
        &mut signal_sprites,
        &mut industry_sprites,
        &mut object_sprites,
        &mut action5_sprites,
    );
    commands.insert_resource(road_sprites);
    commands.insert_resource(station_sprites);
    commands.insert_resource(shore_sprites);
    commands.insert_resource(catenary_sprites);
    commands.insert_resource(signal_sprites);
    commands.insert_resource(industry_sprites);
    commands.insert_resource(object_sprites);
    commands.insert_resource(action5_sprites);
    commands.insert_resource(atlas);
    commands.insert_resource(LoadedMapTileChunks {
        chunks: chunks_in_bounds(spawn_bounds),
    });
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
    commands.insert_resource(crate::render::water_anim_frames_from_assets(
        &assets,
        layout_assets,
    ));
    commands.insert_resource(crate::render::RefineryFireAnimFrames {
        by_sprite: assets.refinery_fire_frames.clone(),
    });
    commands.insert_resource(crate::render::FizzyDrinkAnimFrames {
        by_sprite: assets.fizzy_drink_frames.clone(),
    });
    commands.insert_resource(crate::render::LighthouseAnimFrames {
        by_sprite: assets.lighthouse_anim_frames.clone(),
    });
    commands.insert_resource(crate::render::ChimneySmokeFrames(
        assets.chimney_smoke.clone(),
    ));
    commands.insert_resource(crate::render::CopperMineSmokeFrames(
        assets.copper_mine_smoke.clone(),
    ));
    commands.insert_resource(crate::render::EffectVehicleFrames::from_world_assets(
        &assets,
    ));
    let company_colour = CompanyColour::from_u8(sim.state.company_colour);
    let mut company_sprites = CompanyColoredSprites::new(company_colour);
    company_sprites.build_all(images);
    commands.insert_resource(company_sprites.clone());
    let mut road_sprites = crate::render::NewGrfRoadSpriteCache::default();
    let mut station_sprites = crate::render::NewGrfStationSpriteCache::default();
    let mut shore_sprites = crate::render::NewGrfShoreSpriteCache::default();
    let mut catenary_sprites = crate::render::NewGrfCatenarySpriteCache::default();
    let mut signal_sprites = crate::render::NewGrfSignalSpriteCache::default();
    let mut industry_sprites = crate::render::NewGrfIndustrySpriteCache::default();
    let mut object_sprites = crate::render::NewGrfObjectSpriteCache::default();
    let mut action5_sprites = crate::render::NewGrfAction5SpriteCache::default();
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
        &mut signal_sprites,
        &mut industry_sprites,
        &mut object_sprites,
        &mut action5_sprites,
    );
    commands.insert_resource(road_sprites);
    commands.insert_resource(station_sprites);
    commands.insert_resource(shore_sprites);
    commands.insert_resource(catenary_sprites);
    commands.insert_resource(action5_sprites);
    commands.insert_resource(signal_sprites);
    commands.insert_resource(industry_sprites);
    commands.insert_resource(object_sprites);
    commands.insert_resource(atlas);
    commands.insert_resource(LoadedMapTileChunks {
        chunks: chunks_in_bounds(spawn_bounds),
    });
}
