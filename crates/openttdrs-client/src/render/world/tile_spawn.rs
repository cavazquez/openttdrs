//! Construcción de tiles del mundo y setup inicial.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::prelude::*;

use crate::config::{env_flag, env_string};
use crate::iso::{
    GROUND_SPRITE_CENTER_X_OFFSET, HEIGHT_PX, ISO_QH, ground_draw_z, iso, shore_png_index,
    shore_tileh_for_draw_shore, slope_half_h, slope_sprite_offset,
};
use crate::render::world_draw_trace::WorldDrawTrace;
use crate::render::{
    CompanyColoredSprites, HouseSpawnResources, MapLabelSpatialIndex, MapSpriteBatches, RenderGrid,
    TileAtlas, TileRenderContext, TileViewportBounds, WorldAssets, chunk_tile_bounds,
    flush_map_batches, push_forest_tree, push_water_tile, spawn_bridge_middle_with_road_types,
    spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile, spawn_rail_tile,
    spawn_road_tile, spawn_station_tile_with_world, spawn_transport_object_tile_with_road_types,
    spawn_void_tile,
};
use crate::sprites::CompanyColour;
use crate::state::SimWorld;

/// El pase `DrawGroundSprite` usa una banda de profundidad negativa para
/// mantenerse antes de los parents. La cámara 2D vive cerca de z=1000 y Bevy
/// invierte los argumentos de profundidad de la proyección 2D: el campo
/// `far=1000` termina recortando el lado delantero en `z≈-1000`, antes de que
/// el suelo (z≈-100) llegue al framebuffer.
pub(super) const WORLD_CAMERA_NEAR: f32 = -2000.0;
pub(super) const WORLD_CAMERA_FAR: f32 = 2000.0;

use super::plugin::{LoadedMapTileChunks, MapTileSpawnViewport};
use super::viewport::{
    initial_map_camera_pose, overview_stride_for_viewport, resolve_spawn_viewport_at,
};

use crate::render::vehicles::{NewGrfTrainSpriteCache, TruckHandles, spawn_initial_vehicles};

fn owner_colour_for_tile(
    sim: &SimWorld,
    coord: TileCoord,
    kind: TileKind,
) -> Option<CompanyColour> {
    if kind == TileKind::Grass
        && sim.state.map.object_type_at(coord)
            == Some(u16::from(openttdrs_core::OBJECT_TYPE_STATUE_COMPANY))
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
    house_sprites: &mut crate::render::NewGrfHouseSpriteCache,
    object_sprites: &mut crate::render::NewGrfObjectSpriteCache,
    action5_sprites: &mut crate::render::NewGrfAction5SpriteCache,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let world_draw_trace = WorldDrawTrace::start(mw, mh, spawn_bounds);
    // En modo world-draw la región solicitada es independiente de la cámara:
    // permite inspeccionar una anomalía concreta en un mapa grande sin hacer
    // pan manualmente ni desactivar el culling de viewport.
    let spawn_bounds = world_draw_trace
        .as_ref()
        .map_or(spawn_bounds, WorldDrawTrace::render_bounds);
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

        if let Some(trace) = &world_draw_trace {
            trace.begin_tile(&ctx);
        }

        let slope_half_ground = slope_half_h(tileh);
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
                    &sim.state.runtime.foundation_newgrf_sprites,
                    Some(action5_sprites),
                    &sim.state.runtime.catenary_newgrf_sprites,
                    Some(catenary_sprites),
                );
            }
            TileKind::Rail => {
                spawn_rail_tile(
                    commands,
                    map,
                    (mw, mh),
                    assets,
                    Some(company),
                    owner_colour_for_tile(sim, ctx.coord, TileKind::Rail),
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
                    &sim.state.runtime.rail_type_underlay_newgrf,
                    &sim.state.runtime.rail_type_overlay_newgrf,
                    &sim.state.runtime.rail_type_ground_complete_newgrf,
                    &sim.state.runtime.rail_type_props,
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
                spawn_transport_object_tile_with_road_types(
                    commands,
                    assets,
                    Some(company),
                    owner_colour_for_tile(sim, ctx.coord, kind),
                    &ctx,
                    slope_half_ground,
                    show_pbs_reservations,
                    map,
                    (mw, mh),
                    &sim.state.stations,
                    &sim.state.runtime.catenary_newgrf_sprites,
                    Some(catenary_sprites),
                    &sim.state.runtime.bridge_decks_newgrf_sprites,
                    &sim.state.runtime.foundation_newgrf_sprites,
                    climate,
                    &sim.state.road_type_catalog,
                    Some(road_sprites),
                    &sim.state.newgrf_stack,
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
            TileKind::Void => {
                spawn_void_tile(
                    commands,
                    assets,
                    &ctx,
                    slope_half_ground,
                    sim.state.construction.freeform_edges,
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
        }

        if kind == TileKind::Forest {
            push_forest_tree(commands, assets, &ctx, mw);
        }

        // Tramos de puente que pasan por encima de esta tesela (IsBridgeAbove).
        spawn_bridge_middle_with_road_types(
            commands,
            map,
            (mw, mh),
            assets,
            &ctx,
            show_pbs_reservations,
            climate,
            &sim.state.road_type_catalog,
            Some(road_sprites),
            &sim.state.newgrf_stack,
            &sim.state.runtime.catenary_newgrf_sprites,
            Some(catenary_sprites),
            &sim.state.runtime.bridge_decks_newgrf_sprites,
            Some(action5_sprites),
            Some(images),
        );
        if let Some(trace) = &world_draw_trace {
            trace.end_tile();
        }
    }

    flush_map_batches(commands, batches);
    for (tx, ty) in defer_overlay_tiles {
        let ctx = TileRenderContext::new(map, &render_grid, tx, ty);
        let slope_half_ground = slope_half_h(ctx.info.tileh);
        if let Some(trace) = &world_draw_trace {
            trace.begin_tile(&ctx);
        }
        match ctx.kind {
            TileKind::Station => spawn_station_tile_with_world(
                commands,
                map,
                (mw, mh),
                assets,
                Some(company),
                owner_colour_for_tile(sim, ctx.coord, TileKind::Station),
                &ctx,
                &sim.state.stations,
                slope_half_ground,
                show_pbs_reservations,
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
                Some(openttdrs_core::RoadStopWorldContext {
                    towns: &sim.state.towns,
                    companies: &sim.state.companies,
                    industries: &sim.state.industries,
                    road_type_catalog: &sim.state.road_type_catalog,
                }),
            ),
            TileKind::House => spawn_house_tile(
                commands,
                assets,
                &ctx,
                HouseSpawnResources {
                    map,
                    map_dims: (mw, mh),
                    house_catalog: &sim.state.house_spec_catalog,
                    climate,
                    newgrf_stack: &sim.state.newgrf_stack,
                    foundation_newgrf: &sim.state.runtime.foundation_newgrf_sprites,
                    house_sprites: Some(house_sprites),
                    action5_sprites: Some(action5_sprites),
                    images: Some(images),
                },
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
        if let Some(trace) = &world_draw_trace {
            trace.end_tile();
        }
    }
    if let Some(path) = trace_path {
        if let Err(e) = std::fs::write(&path, trace_rows.join("\n")) {
            error!("No se pudo escribir OPENTTDRS_RENDER_TRACE_OUT={path}: {e}");
        } else {
            info!("Render trace escrito en {path}");
        }
    }
    if let Some(trace) = world_draw_trace {
        trace.finish();
    }
}

/// Resumen estable de una celda de overview.
///
/// OpenTTD todavía compone el terreno de todas las teselas en `Out4x`/`Out8x`;
/// nuestro camino agregado usa una sola entidad por bloque para no disparar el
/// número de sprites. La antigua implementación tomaba la esquina superior
/// izquierda, por lo que una costa o un bosque que ocupase el resto del bloque
/// desaparecía. La reducción por mayoría conserva la cobertura dominante y la
/// altura media sin depender del orden de iteración de ECS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverviewBlockSummary {
    kind: TileKind,
    average_height: u8,
    tile_count: u32,
}

fn overview_block_summary(
    map: &Map,
    tx: u32,
    ty: u32,
    block_w: u32,
    block_h: u32,
) -> OverviewBlockSummary {
    let mut total = 0u32;
    let mut water = 0u32;
    let mut forest = 0u32;
    let mut height_sum = 0u32;
    for y in ty..ty.saturating_add(block_h) {
        for x in tx..tx.saturating_add(block_w) {
            let Some(tile) = map.get(TileCoord::new(x as i32, y as i32)) else {
                continue;
            };
            total = total.saturating_add(1);
            height_sum = height_sum.saturating_add(u32::from(tile.height));
            match tile.kind {
                TileKind::Water | TileKind::ShipDepot => water = water.saturating_add(1),
                TileKind::Forest => forest = forest.saturating_add(1),
                _ => {}
            }
        }
    }
    let tile_count = total.max(1);
    // Water wins ties, matching the way OpenTTD keeps a coast readable when
    // exactly half of a macro block is flooded. Forest wins only when it is a
    // strict majority; otherwise the block is the base grass terrain.
    let kind = if water.saturating_mul(2) >= tile_count {
        TileKind::Water
    } else if forest.saturating_mul(2) > tile_count {
        TileKind::Forest
    } else {
        TileKind::Grass
    };
    OverviewBlockSummary {
        kind,
        average_height: u8::try_from((height_sum + tile_count / 2) / tile_count).unwrap_or(u8::MAX),
        tile_count: total,
    }
}

/// Render agregado para `Out4x`/`Out8x`.
///
/// Un bloque cuadrado de teselas se representa con un único rombo escalado.
/// Las capas de infraestructura y edificios se reservan para el zoom normal;
/// el resumen conserva la lectura macro de tierra/agua y relieve.
fn spawn_overview_tiles_in_bounds(
    commands: &mut Commands,
    assets: &WorldAssets,
    sim: &SimWorld,
    bounds: TileViewportBounds,
    stride: u32,
) {
    let stride = stride.max(2);

    for ty in (bounds.ty0..bounds.ty1).step_by(stride as usize) {
        for tx in (bounds.tx0..bounds.tx1).step_by(stride as usize) {
            let block_w = stride.min(bounds.tx1.saturating_sub(tx));
            let block_h = stride.min(bounds.ty1.saturating_sub(ty));
            if block_w == 0 || block_h == 0 {
                continue;
            }
            let summary = overview_block_summary(&sim.state.map, tx, ty, block_w, block_h);
            // A macro block has no single OpenTTD slope. Use the flat sprite;
            // the average elevation below still keeps neighbouring blocks at
            // the correct vertical level.
            let slope = usize::from(slope_sprite_offset(0)).min(18);
            let image = match summary.kind {
                TileKind::Water | TileKind::ShipDepot => assets.water.clone(),
                TileKind::Forest => assets
                    .rough_slopes
                    .get(slope)
                    .cloned()
                    .unwrap_or_else(|| assets.grass_density[0][slope].clone()),
                _ => assets.grass_density[0][slope].clone(),
            };
            let color = match summary.kind {
                TileKind::Water | TileKind::ShipDepot => Color::WHITE,
                TileKind::Road
                | TileKind::RoadDepot
                | TileKind::RoadBridge
                | TileKind::RoadTunnel => Color::srgba(0.82, 0.72, 0.56, 1.0),
                TileKind::Rail
                | TileKind::RailDepot
                | TileKind::RailBridge
                | TileKind::RailTunnel => Color::srgba(0.78, 0.78, 0.72, 1.0),
                TileKind::House | TileKind::Station | TileKind::Industry | TileKind::Airport => {
                    Color::srgba(0.84, 0.68, 0.36, 1.0)
                }
                TileKind::Forest => Color::srgba(0.72, 0.92, 0.70, 1.0),
                _ => Color::WHITE,
            };
            let footprint = (block_w + block_h) as f32;
            let top = iso(tx as i32, ty as i32);
            // La textura vanilla mide 31 px de alto, pero la cuadrícula
            // isométrica ocupa 32 px por tesela. Al ampliarla N veces, ese
            // píxel de diferencia se convertía en una grieta negra de N px
            // entre bloques. El rombo lógico debe conservar `ISO_QH`.
            let half_h = ISO_QH * footprint * 0.5;
            let elev = f32::from(summary.average_height) * HEIGHT_PX;
            let block_z = ground_draw_z(tx as i32, ty as i32, 0.0);
            // El sprite fuente se amplía `stride` veces, por lo que una
            // columna transparente termina ocupando varios píxeles del mundo.
            // Este solapamiento equivale a dos píxeles de pantalla en Out8x.
            const EDGE_OVERLAP_WORLD: f32 = 16.0;
            let chunk = crate::render::MapTileChunk::from_tile(tx, ty);
            if let Some(overview) = assets.overview.as_ref() {
                let material = match summary.kind {
                    TileKind::Water | TileKind::ShipDepot => &overview.water_material,
                    TileKind::Forest => &overview.forest_material,
                    _ => &overview.grass_material,
                };
                commands.spawn((
                    crate::render::MapVisualLayer,
                    chunk,
                    Mesh2d(overview.diamond.clone()),
                    MeshMaterial2d(material.clone()),
                    Transform::from_translation(Vec3::new(
                        top.x + GROUND_SPRITE_CENTER_X_OFFSET * footprint * 0.5,
                        top.y - half_h + elev,
                        block_z + 0.1,
                    ))
                    .with_scale(Vec3::new(
                        32.0 * footprint + EDGE_OVERLAP_WORLD,
                        16.0 * footprint + EDGE_OVERLAP_WORLD,
                        1.0,
                    )),
                ));
            }
            let mut sprite = image.sprite_colored(color);
            sprite.custom_size = Some(Vec2::new(
                32.0 * footprint + EDGE_OVERLAP_WORLD,
                16.0 * footprint + EDGE_OVERLAP_WORLD,
            ));
            let pos = Vec3::new(
                top.x + GROUND_SPRITE_CENTER_X_OFFSET * footprint * 0.5,
                top.y - half_h + elev,
                block_z + 0.2,
            );
            if matches!(summary.kind, TileKind::Water | TileKind::ShipDepot) {
                commands.spawn((
                    crate::render::MapVisualLayer,
                    chunk,
                    crate::render::WaterTile::STATIC,
                    sprite,
                    Transform::from_translation(pos),
                ));
            } else {
                commands.spawn((
                    crate::render::MapVisualLayer,
                    chunk,
                    sprite,
                    Transform::from_translation(pos),
                ));
            }
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
    label_index: &MapLabelSpatialIndex,
    spawn_bounds: TileViewportBounds,
    include_world_extras: bool,
    show_pbs_reservations: bool,
    show_full_detail: bool,
    show_town_labels: bool,
    show_station_labels: bool,
    show_waypoint_labels: bool,
    show_competitor_labels: bool,
    overview_stride: Option<u32>,
    road_sprites: &mut crate::render::NewGrfRoadSpriteCache,
    station_sprites: &mut crate::render::NewGrfStationSpriteCache,
    shore_sprites: &mut crate::render::NewGrfShoreSpriteCache,
    catenary_sprites: &mut crate::render::NewGrfCatenarySpriteCache,
    signal_sprites: &mut crate::render::NewGrfSignalSpriteCache,
    industry_sprites: &mut crate::render::NewGrfIndustrySpriteCache,
    house_sprites: &mut crate::render::NewGrfHouseSpriteCache,
    object_sprites: &mut crate::render::NewGrfObjectSpriteCache,
    action5_sprites: &mut crate::render::NewGrfAction5SpriteCache,
) {
    if include_world_extras {
        let truck_handles = TruckHandles::load(asset_server);
        let mut newgrf_train_sprites = NewGrfTrainSpriteCache::default();
        if overview_stride.is_none() {
            spawn_initial_vehicles(
                commands,
                sim,
                &truck_handles,
                company,
                &mut newgrf_train_sprites,
                images,
            );
        }
        commands.insert_resource(truck_handles);
        commands.insert_resource(newgrf_train_sprites);
        let label_font = asset_server.load::<Font>(crate::ui::font::UI_FONT_PATH);
        let label_candidates = label_index.candidates(spawn_bounds);
        crate::render::town_labels::spawn_town_labels(
            commands,
            sim,
            &label_font,
            &label_candidates,
            show_town_labels,
        );
        crate::render::station_labels::spawn_station_labels(
            commands,
            sim,
            &label_font,
            &label_candidates,
            show_station_labels,
            show_waypoint_labels,
            show_competitor_labels,
        );
        crate::render::sign_labels::spawn_sign_labels(
            commands,
            sim,
            &label_font,
            &label_candidates,
            show_competitor_labels,
        );
    }
    if let Some(stride) = overview_stride {
        spawn_overview_tiles_in_bounds(commands, assets, sim, spawn_bounds, stride);
    } else {
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
            house_sprites,
            object_sprites,
            action5_sprites,
        );
    }
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
    house_sprites: &mut crate::render::NewGrfHouseSpriteCache,
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
        house_sprites,
        object_sprites,
        action5_sprites,
    );
}

/// Almacenes de assets que el setup del mundo inicializa en conjunto.
///
/// Agruparlos evita que el sistema de arranque acumule parámetros ECS al
/// incorporar la geometría opaca que respalda el overview de mapas grandes.
#[derive(SystemParam)]
pub(crate) struct WorldSetupAssetStores<'w> {
    layout_assets: ResMut<'w, Assets<TextureAtlasLayout>>,
    images: ResMut<'w, Assets<Image>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
}

pub(crate) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut asset_stores: WorldSetupAssetStores,
    sim: Res<SimWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    prefs: Option<Res<crate::settings::ClientPreferences>>,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let (cam_pos, cam_scale) = initial_map_camera_pose(&sim);

    commands.spawn((
        Camera2d,
        crate::render::PrimaryGameCamera,
        // El blitter 8bpp de OpenTTD compone píxeles sin suavizado de bordes.
        // El MSAA ×4 por defecto de Bevy mezcla los bordes de quads incluso
        // cuando los PNG y el sampler son estrictamente nearest, creando
        // colores que no existen en la paleta OpenGFX.
        Msaa::Off,
        Camera {
            // Fuera del rombo no hay agua: el blitter 8bpp de OpenTTD deja el
            // framebuffer negro. Mantenerlo así evita que el borde del mapa
            // parezca prolongarse con una franja azul.
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Transform::from_translation(cam_pos),
        Projection::Orthographic(OrthographicProjection {
            scale: cam_scale,
            near: WORLD_CAMERA_NEAR,
            far: WORLD_CAMERA_FAR,
            ..OrthographicProjection::default_2d()
        }),
    ));

    let spawn_bounds = resolve_spawn_viewport_at(&sim, &windows, cam_pos.truncate(), cam_scale);
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
        last_ortho_scale: cam_scale,
        last_overview_stride: overview_stride_for_viewport(cam_scale, spawn_bounds),
    });
    let atlas = TileAtlas::build(&asset_server, &mut asset_stores.layout_assets);
    let mut assets = WorldAssets::load(&atlas, &mut asset_stores.images);
    assets.overview = Some(crate::render::OverviewRenderAssets::new(
        &mut asset_stores.meshes,
        &mut asset_stores.materials,
    ));
    commands.insert_resource(assets.clone());
    commands.insert_resource(crate::render::water_anim_frames_from_assets(
        &assets,
        &asset_stores.layout_assets,
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
    company_sprites.build_all(&mut asset_stores.images);
    commands.insert_resource(company_sprites.clone());
    let show_town_labels = prefs.as_ref().map(|p| p.show_town_labels).unwrap_or(true);
    let show_station_labels = prefs
        .as_ref()
        .map(|p| p.show_station_labels)
        .unwrap_or(true);
    let show_waypoint_labels = prefs
        .as_ref()
        .map(|p| p.show_waypoint_labels)
        .unwrap_or(true);
    let show_competitor_labels = prefs
        .as_ref()
        .map(|p| p.show_competitor_labels)
        .unwrap_or(true);
    let show_full_detail = prefs.as_ref().map(|p| p.full_detail).unwrap_or(true);
    let label_index = MapLabelSpatialIndex::from_state(&sim.state);
    let mut road_sprites = crate::render::NewGrfRoadSpriteCache::default();
    let mut station_sprites = crate::render::NewGrfStationSpriteCache::default();
    let mut shore_sprites = crate::render::NewGrfShoreSpriteCache::default();
    let mut catenary_sprites = crate::render::NewGrfCatenarySpriteCache::default();
    let mut signal_sprites = crate::render::NewGrfSignalSpriteCache::default();
    let mut industry_sprites = crate::render::NewGrfIndustrySpriteCache::default();
    let mut house_sprites = crate::render::NewGrfHouseSpriteCache::default();
    let mut object_sprites = crate::render::NewGrfObjectSpriteCache::default();
    let mut action5_sprites = crate::render::NewGrfAction5SpriteCache::default();
    spawn_world_layer(
        &mut commands,
        &asset_server,
        &assets,
        &mut company_sprites,
        &mut asset_stores.images,
        &sim,
        &label_index,
        spawn_bounds,
        true,
        true,
        show_full_detail,
        show_town_labels,
        show_station_labels,
        show_waypoint_labels,
        show_competitor_labels,
        overview_stride_for_viewport(cam_scale, spawn_bounds),
        &mut road_sprites,
        &mut station_sprites,
        &mut shore_sprites,
        &mut catenary_sprites,
        &mut signal_sprites,
        &mut industry_sprites,
        &mut house_sprites,
        &mut object_sprites,
        &mut action5_sprites,
    );
    commands.insert_resource(road_sprites);
    commands.insert_resource(station_sprites);
    commands.insert_resource(shore_sprites);
    commands.insert_resource(catenary_sprites);
    commands.insert_resource(signal_sprites);
    commands.insert_resource(industry_sprites);
    commands.insert_resource(house_sprites);
    commands.insert_resource(object_sprites);
    commands.insert_resource(action5_sprites);
    commands.insert_resource(label_index);
    commands.insert_resource(atlas);
    commands.insert_resource(LoadedMapTileChunks::from_spawn_bounds(spawn_bounds, mw, mh));
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
    let mut house_sprites = crate::render::NewGrfHouseSpriteCache::default();
    let mut object_sprites = crate::render::NewGrfObjectSpriteCache::default();
    let mut action5_sprites = crate::render::NewGrfAction5SpriteCache::default();
    let label_index = MapLabelSpatialIndex::from_state(&sim.state);
    spawn_world_layer(
        commands,
        asset_server,
        &assets,
        &mut company_sprites,
        images,
        sim,
        &label_index,
        spawn_bounds,
        false,
        true,
        true,
        true,
        true,
        true,
        true,
        None,
        &mut road_sprites,
        &mut station_sprites,
        &mut shore_sprites,
        &mut catenary_sprites,
        &mut signal_sprites,
        &mut industry_sprites,
        &mut house_sprites,
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
    commands.insert_resource(house_sprites);
    commands.insert_resource(atlas);
    commands.insert_resource(LoadedMapTileChunks::from_spawn_bounds(spawn_bounds, mw, mh));
}

#[cfg(test)]
#[allow(clippy::expect_used)] // El fixture usa coordenadas fijas que deben estar dentro del mapa.
mod tests {
    use super::{OverviewBlockSummary, overview_block_summary};
    use openttdrs_core::prelude::{Map, TileCoord, TileKind};

    #[test]
    fn overview_uses_water_on_a_tie_and_rounds_height() {
        let mut map = Map::new_flat(4, 4, 0);
        for y in 0..2 {
            for x in 0..4 {
                map.set_kind(TileCoord::new(x, y), TileKind::Water)
                    .expect("water in bounds");
                map.set_height(TileCoord::new(x, y), 3)
                    .expect("height in bounds");
            }
        }
        for y in 2..4 {
            for x in 0..4 {
                map.set_height(TileCoord::new(x, y), 2)
                    .expect("height in bounds");
            }
        }

        assert_eq!(
            overview_block_summary(&map, 0, 0, 4, 4),
            OverviewBlockSummary {
                kind: TileKind::Water,
                average_height: 3,
                tile_count: 16,
            }
        );
    }

    #[test]
    fn overview_requires_a_strict_forest_majority() {
        let mut map = Map::new_flat(4, 4, 0);
        for y in 0..3 {
            for x in 0..4 {
                map.set_kind(TileCoord::new(x, y), TileKind::Forest)
                    .expect("forest in bounds");
            }
        }
        assert_eq!(
            overview_block_summary(&map, 0, 0, 4, 4).kind,
            TileKind::Forest
        );
        map.set_kind(TileCoord::new(0, 0), TileKind::Grass)
            .expect("grass in bounds");
        assert_eq!(
            overview_block_summary(&map, 0, 0, 4, 4).kind,
            TileKind::Forest
        );
    }
}
