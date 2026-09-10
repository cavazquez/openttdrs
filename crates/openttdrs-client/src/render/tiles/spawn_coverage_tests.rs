//! Tests de integración: rutas principales de spawn de tiles (carretera, vía, agua, etc.).

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::image::ImagePlugin;
use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    Action2VarAdjust, Action2VarEntry, Action2VarTerm, AirportClassId, AirportSpecId,
    AirportTileGfxId, AirportTileSpecDef, BridgeType, Climate, DecodedSprite,
    FOUNDATION_ORIGINAL_SPRITE_BASE, HouseSpecDef, IndustryTileGfxId, IndustryTileSpecDef,
    NewgrfAirportSpecDef, ObjectSpecDef, RailType, RoadStopSpecDef, RoadTramType, RoadType,
    RoadTypeDef, StationClassId, StationSpecDef, StationSpecId, TrainSpriteAssign,
    TrainSpriteGraphics, WaterClass, make_water_tile, set_water_class_m1,
    vanilla_road_type_catalog,
};

const TEST_CLIMATE: Climate = Climate::Temperate;
const TEST_WORLD_SEED: u64 = 0;

use crate::iso::{ground_draw_z, overlay_pos};
use crate::render::assets::{WorldAssets, stub_opengfx_tiles_for_tests};
use crate::render::tiles::{
    FLAT_WATER_LAYER_FRAC, HouseSpawnResources, TramwayDepotAction5, flush_map_batches,
    push_forest_tree, push_water_tile, push_water_tile_with_action5, spawn_bridge_middle,
    spawn_bridge_middle_with_road_types, spawn_generic_land_tile, spawn_house_tile,
    spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
    spawn_station_tile_with_world_and_road_types, spawn_transport_object_tile,
    spawn_transport_object_tile_with_road_types,
    spawn_transport_object_tile_with_road_types_and_tramway_action5,
};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    AirportStationAnim, CompanyColoredSprites, MapSpriteBatches, MapVisualLayer, RenderGrid,
    TileRenderContext, ViewportSortableChild, ViewportSortableChildDepthWindows,
    ViewportSortableParent, WaterTile, sort_viewport_sortable_parents,
    sync_viewport_sortable_children, viewport_insertion_key,
};
use crate::sprites::{
    RAIL_TB_X, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, WATER_CANAL_DIKE_SPRITE_META,
    WATER_RIVER_SLOPE_SPRITE_META, industry_building_needs_client_anim,
    industry_gfx_entry_for_tile,
};

#[derive(Resource)]
struct TsMap(Map);

#[derive(Resource)]
struct TsGrid(RenderGrid);

#[derive(Resource)]
struct TsAssets(WorldAssets);

fn boot_assets_app() -> WorldAssets {
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
    app.init_asset::<TextureAtlasLayout>();
    app.update();
    let atlas = {
        let world = app.world_mut();
        world.resource_scope(|world, mut layouts: Mut<Assets<TextureAtlasLayout>>| {
            crate::render::TileAtlas::build(world.resource::<AssetServer>(), &mut layouts)
        })
    };
    let mut images = app.world_mut().resource_mut::<Assets<Image>>();
    WorldAssets::load(&atlas, &mut images)
}

fn fresh_map8() -> Map {
    Map::new_flat(8, 8, 0)
}

#[test]
fn water_surface_markers_cover_flat_locks_and_industry_water() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(5, 5, 0);
    for x in 0..5 {
        for y in 0..5 {
            map.set_kind(TileCoord::new(x, y), TileKind::Water)
                .expect("water");
        }
    }
    let flat = TileCoord::new(1, 1);
    let lock = TileCoord::new(2, 2);
    map.set_mapt_m5(lock, 0x60, 0x20).expect("lock");

    let oil_rig = TileCoord::new(3, 2);
    let mut oil_tile = tile_template();
    oil_tile.kind = TileKind::Industry;
    oil_tile.mapt = 0x80;
    oil_tile.m5 = 24; // GFX_OILRIG_1
    oil_tile.m1 = 0x80;
    map.set_tile(oil_rig, oil_tile).expect("oil rig");

    let grid = RenderGrid::from_map(&map, 5, 5);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut company: Local<CompanyColoredSprites>,
                  mut images: Local<Assets<Image>>| {
                let mut batches = MapSpriteBatches::default();
                for coord in [flat, lock] {
                    push_water_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).unwrap(),
                            u32::try_from(coord.y).unwrap(),
                        ),
                        false,
                        &mut batches,
                        &[],
                        None,
                        None,
                    );
                }
                assert!(batches.water[0].1.is_palette_animated());
                assert!(!batches.water[1].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("water coverage");

    let mut water = world.query::<&crate::render::WaterTile>();
    let markers: Vec<_> = water.iter(&world).copied().collect();
    assert_eq!(markers.len(), 3);
    assert_eq!(
        markers
            .iter()
            .filter(|marker| marker.is_palette_animated())
            .count(),
        2
    );
    let expected_industry_water_x =
        crate::iso::iso(oil_rig.x, oil_rig.y).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET;
    assert!(
        world
            .query::<(&crate::render::WaterTile, &Transform)>()
            .iter(&world)
            .any(|(marker, transform)| {
                marker.is_palette_animated() && transform.translation.x == expected_industry_water_x
            }),
        "la industria sobre agua debe conservar el xrel=-31 de SPR_FLAT_WATER_TILE"
    );
}

#[test]
fn river_water_slope_uses_static_action5_sprite_and_nfo_anchor() {
    let assets = boot_assets_app();
    let river_asset = assets.river_slopes[1].clone(); // SPR_WATER_SLOPE_X_DOWN.
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    make_water_tile(&mut map, coord, WaterClass::River).expect("river tile");
    // `tile_slope_and_z` reads N, W, E, S at (tx,ty), (tx+1,ty),
    // (tx,ty+1), (tx+1,ty+1). N+E gives SLOPE_NE (12).
    map.set_height(TileCoord::new(1, 1), 1)
        .expect("north height");
    map.set_height(TileCoord::new(1, 2), 1)
        .expect("east height");

    let grid = RenderGrid::from_map(&map, 4, 4);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    assert_eq!(ctx.info.tileh, openttdrs_core::SLOPE_NE);
    assert!(!ctx.info.use_shore, "un río claro no es una costa");

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut batches = MapSpriteBatches::default();
                push_water_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    false,
                    &mut batches,
                    &[],
                    None,
                    None,
                );
                assert_eq!(batches.water.len(), 1);
                assert!(!batches.water[0].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("river slope spawn");

    let (marker, sprite, transform) = world
        .query::<(&WaterTile, &Sprite, &Transform)>()
        .iter(&world)
        .next()
        .expect("river slope water sprite");
    assert!(!marker.is_palette_animated());
    assert!(river_asset.matches(sprite), "se usa el slot X_DOWN activo");

    let (width, height, xrel, yrel) = WATER_RIVER_SLOPE_SPRITE_META[1];
    let mut expected = overlay_pos(
        crate::iso::iso(1, 1),
        f32::from(xrel),
        f32::from(yrel),
        f32::from(width),
        f32::from(height),
        0,
        0.0,
        1,
        1,
    );
    expected.z = ground_draw_z(1, 1, 0.0);
    assert_eq!(*transform, Transform::from_translation(expected));
}

#[test]
fn river_water_slope_consumes_canal_action5_sprite_and_nfo_anchor() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    make_water_tile(&mut map, coord, WaterClass::River).expect("river tile");
    map.set_height(TileCoord::new(1, 1), 1)
        .expect("north height");
    map.set_height(TileCoord::new(1, 2), 1)
        .expect("east height");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let custom = DecodedSprite {
        width: 11,
        height: 9,
        x_offs: -6,
        y_offs: -8,
        rgba: vec![0xFF; 11 * 9 * 4],
        mask: Vec::new(),
    };
    let mut canal_action5 = vec![None; 65];
    canal_action5[1] = Some(custom);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                let mut batches = MapSpriteBatches::default();
                push_water_tile_with_action5(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    false,
                    &mut batches,
                    &[],
                    None,
                    Some(&mut images),
                    &[],
                    &canal_action5,
                    Some(&mut action5_sprites),
                );
                assert_eq!(batches.water.len(), 1);
                assert!(!batches.water[0].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("river Action5 slope spawn");

    let (sprite, transform) = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find(|(sprite, _)| sprite.texture_atlas.is_none())
        .expect("pendiente de río Action5 materializada");
    assert!(
        world
            .resource::<Assets<Image>>()
            .get(&sprite.image)
            .is_some()
    );
    let mut expected = overlay_pos(crate::iso::iso(1, 1), -6.0, -8.0, 11.0, 9.0, 0, 0.0, 1, 1);
    expected.z = ground_draw_z(1, 1, 0.0);
    assert_eq!(*transform, Transform::from_translation(expected));
}

#[test]
fn river_water_feature_slope_consumes_matching_river_edge_block() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    make_water_tile(&mut map, coord, WaterClass::River).expect("river tile");
    map.set_height(TileCoord::new(1, 1), 1)
        .expect("north height");
    map.set_height(TileCoord::new(1, 2), 1)
        .expect("east height");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let slope = DecodedSprite {
        width: 11,
        height: 9,
        x_offs: -6,
        y_offs: -8,
        rgba: vec![0x11; 11 * 9 * 4],
        mask: Vec::new(),
    };
    let edge = DecodedSprite {
        width: 7,
        height: 4,
        x_offs: 3,
        y_offs: -2,
        rgba: vec![0x22; 7 * 4 * 4],
        mask: Vec::new(),
    };
    let mut features = openttdrs_core::vanilla_canal_feature_catalog();
    features[usize::from(openttdrs_core::CF_RIVER_SLOPE)].newgrf_views = vec![slope.clone(); 4];
    // SLOPE_NE selects the second 12-sprite river-edge block (offset 24).
    features[usize::from(openttdrs_core::CF_RIVER_EDGE)].newgrf_views = vec![edge.clone(); 36];
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                let mut batches = MapSpriteBatches::default();
                push_water_tile_with_action5(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    false,
                    &mut batches,
                    &[],
                    None,
                    Some(&mut images),
                    &features,
                    &[],
                    Some(&mut action5_sprites),
                );
                assert_eq!(batches.water.len(), 1);
                assert!(!batches.water[0].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("river feature slope and edges spawn");

    let rendered: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .map(|(sprite, transform)| (sprite.clone(), *transform))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let (slope_sprite, slope_transform) = rendered
        .iter()
        .find(|(sprite, _)| {
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                == Some(slope.rgba.as_slice())
        })
        .expect("pendiente CF_RIVER_SLOPE materializada");
    let mut expected_slope =
        overlay_pos(crate::iso::iso(1, 1), -6.0, -8.0, 11.0, 9.0, 0, 0.0, 1, 1);
    expected_slope.z = ground_draw_z(1, 1, 0.0);
    assert_eq!(
        *slope_transform,
        Transform::from_translation(expected_slope)
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|(sprite, _)| {
                images
                    .get(&sprite.image)
                    .and_then(|image| image.data.as_deref())
                    == Some(edge.rgba.as_slice())
            })
            .count(),
        8,
        "un río aislado emite ocho bordes del bloque inclinado"
    );
    assert!(slope_sprite.texture_atlas.is_none());
    assert_eq!(images.len(), 9, "pendiente más ocho slots de borde");
}

#[test]
fn canal_water_feature_flat_sprite_keeps_nfo_anchor_before_dikes() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    make_water_tile(&mut map, coord, WaterClass::Canal).expect("canal tile");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let surface = DecodedSprite {
        width: 10,
        height: 6,
        x_offs: -3,
        y_offs: -5,
        rgba: vec![0x33; 10 * 6 * 4],
        mask: Vec::new(),
    };
    let mut features = openttdrs_core::vanilla_canal_feature_catalog();
    features[usize::from(openttdrs_core::CF_WATERSLOPE)].flags =
        openttdrs_core::CFF_HAS_FLAT_SPRITE;
    features[usize::from(openttdrs_core::CF_WATERSLOPE)].newgrf_views = vec![surface.clone()];
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                let mut batches = MapSpriteBatches::default();
                push_water_tile_with_action5(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    false,
                    &mut batches,
                    &[],
                    None,
                    Some(&mut images),
                    &features,
                    &[],
                    Some(&mut action5_sprites),
                );
                assert_eq!(batches.water.len(), 1);
                assert!(!batches.water[0].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("canal feature flat spawn");

    let rendered: Vec<_> = world
        .query::<(&WaterTile, &Sprite, &Transform)>()
        .iter(&world)
        .map(|(marker, sprite, transform)| (*marker, sprite.clone(), *transform))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let (marker, _, transform) = rendered
        .iter()
        .find(|(_, sprite, _)| {
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                == Some(surface.rgba.as_slice())
        })
        .expect("ground CF_WATERSLOPE materializado");
    assert!(!marker.is_palette_animated());
    let expected = overlay_pos(
        crate::iso::iso(1, 1),
        -3.0,
        -5.0,
        10.0,
        6.0,
        0,
        FLAT_WATER_LAYER_FRAC,
        1,
        1,
    );
    assert_eq!(*transform, Transform::from_translation(expected));
    assert_eq!(
        world.query::<&MapVisualLayer>().iter(&world).count(),
        9,
        "el ground custom no elimina los ocho diques"
    );
}

#[test]
fn lock_water_feature_middle_uses_shifted_water_slope_slot() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    make_water_tile(&mut map, coord, WaterClass::Canal).expect("lock water tile");
    map.set_mapt_m5(coord, 0x60, 0x20).expect("middle NE lock");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let views: Vec<_> = (0..5)
        .map(|slot| DecodedSprite {
            width: 10,
            height: 6,
            x_offs: -3,
            y_offs: -5,
            rgba: vec![slot as u8; 10 * 6 * 4],
            mask: Vec::new(),
        })
        .collect();
    let selected = views[2].clone();
    let mut features = openttdrs_core::vanilla_canal_feature_catalog();
    features[usize::from(openttdrs_core::CF_WATERSLOPE)].flags =
        openttdrs_core::CFF_HAS_FLAT_SPRITE;
    features[usize::from(openttdrs_core::CF_WATERSLOPE)].newgrf_views = views;
    let structure = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: vec![0x99; 2 * 2 * 4],
        mask: Vec::new(),
    };
    features[usize::from(openttdrs_core::CF_LOCKS)].newgrf_views = vec![structure.clone(); 24];

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                let mut batches = MapSpriteBatches::default();
                push_water_tile_with_action5(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    false,
                    &mut batches,
                    &[],
                    None,
                    Some(&mut images),
                    &features,
                    &[],
                    Some(&mut action5_sprites),
                );
                assert_eq!(batches.water.len(), 1);
                assert!(!batches.water[0].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("lock CF_WATERSLOPE spawn");

    let rendered: Vec<_> = world
        .query::<(&WaterTile, &Sprite, &Transform)>()
        .iter(&world)
        .map(|(marker, sprite, transform)| (*marker, sprite.clone(), *transform))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let (_, _, transform) = rendered
        .iter()
        .find(|(_, sprite, _)| {
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                == Some(selected.rgba.as_slice())
        })
        .expect("middle lock selects the NE CF_WATERSLOPE view after flat");
    let mut expected = overlay_pos(crate::iso::iso(1, 1), -3.0, -5.0, 10.0, 6.0, 0, 0.02, 1, 1);
    expected.z = ground_draw_z(1, 1, 0.02);
    assert_eq!(*transform, Transform::from_translation(expected));
    assert_eq!(
        images
            .iter()
            .filter(|(_, image)| image.data.as_deref() == Some(structure.rgba.as_slice()))
            .count(),
        2,
        "CF_LOCKS custom no se mezcla con la textura del ground"
    );
    assert_eq!(images.len(), 3, "ground más las dos capas CF_LOCKS");

    let mut parents: Vec<_> = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .filter(|parent| [5333, 5337].contains(&parent.sprite_id))
        .copied()
        .collect();
    parents.sort_by_key(|parent| parent.insertion_key);
    assert_eq!(parents.len(), 2);
    assert_eq!(
        parents
            .iter()
            .map(|parent| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                5333,
                ParentSpriteBounds::new(16, 16, 0, 31, 16, 5),
                viewport_insertion_key(1, 1, 1),
            ),
            (
                5337,
                ParentSpriteBounds::new(16, 31, 0, 31, 31, 9),
                viewport_insertion_key(1, 1, 2),
            ),
        ],
        "las dos capas CF_LOCKS conservan bounds e inserción TILE_SEQ"
    );
}

#[test]
fn canal_water_tile_draws_dikes_after_generic_water_ground() {
    let assets = boot_assets_app();
    let dike_assets = assets.canal_dikes.clone();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    make_water_tile(&mut map, coord, WaterClass::Canal).expect("canal tile");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut batches = MapSpriteBatches::default();
                push_water_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    false,
                    &mut batches,
                    &[],
                    None,
                    None,
                );
                assert_eq!(batches.water.len(), 1);
                assert!(batches.water[0].1.is_palette_animated());
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("generic canal spawn");

    assert_eq!(world.query::<&MapVisualLayer>().iter(&world).count(), 9);
    assert_eq!(world.query::<&WaterTile>().iter(&world).count(), 1);
    let rendered: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .collect();
    for slot in 0..8 {
        let (width, height, xrel, yrel) = WATER_CANAL_DIKE_SPRITE_META[slot];
        let layer = 0.010 + slot as f32 * 0.0001;
        let mut expected = overlay_pos(
            crate::iso::iso(1, 1),
            f32::from(xrel),
            f32::from(yrel),
            f32::from(width),
            f32::from(height),
            0,
            layer,
            1,
            1,
        );
        expected.z = ground_draw_z(1, 1, layer);
        let actual = rendered
            .iter()
            .find_map(|(sprite, transform)| {
                dike_assets[slot]
                    .matches(sprite)
                    .then_some(transform.translation)
            })
            .unwrap_or_else(|| panic!("falta dique genérico slot {slot}"));
        assert_eq!(actual, expected, "dique genérico slot {slot}");
    }
}

/// `DrawGroundSprite` y `DrawShoreTile` usan `xrel=-31` para un PNG de 64 px
/// de ancho. El centro de Bevy debe quedar en `+1`, no en el centro geométrico
/// que desplazaría ambos fondos un píxel hacia la izquierda.
#[test]
fn water_and_shore_keep_openttd_ground_xrel_center() {
    let assets = boot_assets_app();
    let flat = TileCoord::new(1, 1);
    let coast = TileCoord::new(3, 2);
    let mut map = Map::new_flat(5, 5, 0);
    for x in 0..5 {
        for y in 0..5 {
            map.set_kind(TileCoord::new(x, y), TileKind::Water)
                .expect("water");
        }
    }
    // Una única tesela de tierra convierte `(3,2)` en costa, sin afectar el
    // agua interior de `(1,1)`.
    map.set_kind(TileCoord::new(4, 2), TileKind::Grass)
        .expect("coast neighbour");
    let grid = RenderGrid::from_map(&map, 5, 5);
    let flat_ctx = TileRenderContext::new(&map, &grid, flat.x as u32, flat.y as u32);
    let coast_ctx = TileRenderContext::new(&map, &grid, coast.x as u32, coast.y as u32);
    assert!(!flat_ctx.info.use_shore, "agua interior no debe usar shore");
    assert!(coast_ctx.info.use_shore, "agua lindera debe usar shore");
    let coast_tileh = crate::iso::shore_tileh_for_draw_shore(&map, 3, 2, 5, 5);
    let coast_sprite = assets.shore[crate::iso::shore_png_index(coast_tileh)].clone();
    let water_sprite = assets.water.clone();

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut batches = MapSpriteBatches::default();
                for coord in [flat, coast] {
                    push_water_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        &TileRenderContext::new(&m.0, &g.0, coord.x as u32, coord.y as u32),
                        false,
                        &mut batches,
                        &[],
                        None,
                        None,
                    );
                }
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("water and shore spawn");

    let rendered: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .collect();
    let water_x = rendered
        .iter()
        .find_map(|(sprite, transform)| {
            water_sprite
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("flat water sprite");
    let shore_x = rendered
        .iter()
        .find_map(|(sprite, transform)| {
            coast_sprite
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("shore sprite");
    let offset = crate::iso::GROUND_SPRITE_CENTER_X_OFFSET;
    assert_eq!(water_x, crate::iso::iso(flat.x, flat.y).x + offset);
    assert_eq!(shore_x, crate::iso::iso(coast.x, coast.y).x + offset);
}

#[test]
fn oilrig_station_uses_water_even_when_its_station_has_airport_service() {
    let assets = boot_assets_app();
    let airport_apron = assets.airport_apron.clone();
    let oilrig = TileCoord::new(3, 3);
    let mut map = fresh_map8();
    map.set_tile(
        oilrig,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m6: openttdrs_core::STATION_TYPE_OILRIG << 3,
            ..tile_template()
        },
    )
    .expect("oilrig station tile");
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut station = Station::new_with_kind(oilrig, StopKind::OilRig);
    station.airport_tiles.push(oilrig);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    std::slice::from_ref(&station),
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("oilrig spawn");

    let water: Vec<_> = world
        .query::<&crate::render::WaterTile>()
        .iter(&world)
        .copied()
        .collect();
    assert_eq!(water.len(), 1, "oilrig debe conservar el suelo de agua");
    assert!(water[0].is_palette_animated());
    let oilrig_water_x = world
        .query::<(&crate::render::WaterTile, &Transform)>()
        .iter(&world)
        .find_map(|(marker, transform)| {
            marker
                .is_palette_animated()
                .then_some(transform.translation.x)
        })
        .expect("agua de oilrig");
    assert_eq!(
        oilrig_water_x,
        crate::iso::iso(oilrig.x, oilrig.y).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET,
        "el agua de Oilrig usa el mismo xrel=-31 que DrawGroundSprite"
    );
    assert!(
        world
            .query::<&Sprite>()
            .iter(&world)
            .all(|sprite| !airport_apron.matches(sprite)),
        "un Oilrig no puede degradarse al apron de aeropuerto"
    );
}

#[test]
fn buoy_station_water_keeps_openttd_ground_xrel_center() {
    let assets = boot_assets_app();
    let buoy = TileCoord::new(3, 3);
    let mut map = fresh_map8();
    map.set_tile(
        buoy,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m6: openttdrs_core::station::STATION_TYPE_BUOY << 3,
            ..tile_template()
        },
    )
    .expect("buoy station tile");
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("buoy spawn");

    let buoy_water_x = world
        .query::<(&crate::render::WaterTile, &Transform)>()
        .iter(&world)
        .find_map(|(marker, transform)| {
            marker
                .is_palette_animated()
                .then_some(transform.translation.x)
        })
        .expect("agua de boya");
    assert_eq!(
        buoy_water_x,
        crate::iso::iso(buoy.x, buoy.y).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET,
        "la boya debe conservar el xrel=-31 de su DrawWaterClassGround"
    );
}

/// Un muelle vanilla son dos teselas distintas: la de tierra conserva una
/// pendiente y la de agua es plana. En Kale, (137,2)/(138,2) son precisamente
/// la pareja `m5=2/4`; intercambiar sus layouts deja el muelle aparentemente
/// cortado y omitir el suelo hace desaparecer la costa.
#[test]
fn dock_station_keeps_vanilla_slope_and_water_halves() {
    let assets = boot_assets_app();
    let slope_dock = assets.dock_slope[2].clone(); // SPR_DOCK_SLOPE_SW = 2729.
    let water_dock = assets.dock_flat[0].clone(); // SPR_DOCK_FLAT_X = 2731.
    let flat_water = assets.water.clone();
    let shore = assets.shore[crate::iso::shore_png_index(12)].clone();
    let land = TileCoord::new(2, 2);
    let water = TileCoord::new(3, 2);
    let mut map = Map::new_flat(5, 5, 0);

    map.set_tile(
        land,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5: 2,
            m6: openttdrs_core::STATION_TYPE_DOCK << 3,
            ..tile_template()
        },
    )
    .expect("dock land tile");
    map.set_tile(
        water,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m1: set_water_class_m1(0, WaterClass::Sea),
            m5: 4,
            m6: openttdrs_core::STATION_TYPE_DOCK << 3,
            ..tile_template()
        },
    )
    .expect("dock water tile");
    // SLOPE_NE: N y E elevadas. `set_tile` reemplaza también la altura, por
    // eso la pendiente debe escribirse una vez fijadas ambas mitades. El
    // `DiagDirection` de m5=2 es SW: su agua queda a +X en `(3,2)`.
    map.set_height(TileCoord::new(2, 2), 1)
        .expect("north height");
    map.set_height(TileCoord::new(2, 3), 1)
        .expect("east height");

    let grid = RenderGrid::from_map(&map, 5, 5);
    let land_ctx = TileRenderContext::new(&map, &grid, 2, 2);
    let water_ctx = TileRenderContext::new(&map, &grid, 3, 2);
    assert_eq!(land_ctx.info.tileh, 12, "la mitad terrestre debe ser NE");
    assert_eq!(water_ctx.info.tileh, 0, "la mitad de agua debe ser plana");

    let expected_layer_pos = |ctx: &TileRenderContext, m5: u8| {
        let layer = crate::sprites::dock_tile_layer(m5);
        let local = crate::iso::remap_tile_offset(layer.dx, layer.dy, layer.dz) * 0.5;
        let mut pos = crate::iso::overlay_pos(
            ctx.iso_pos + local,
            layer.x_offs,
            layer.y_offs,
            layer.w,
            layer.h,
            ctx.info.base_z,
            0.04,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        pos.z = crate::render::viewport_source_depth(pos.z, ctx.tx, 5);
        pos
    };
    let expected_slope_pos = expected_layer_pos(&land_ctx, 2);
    let expected_water_pos = expected_layer_pos(&water_ctx, 4);

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for coord in [land, water] {
                    spawn_station_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("positive x"),
                            u32::try_from(coord.y).expect("positive y"),
                        ),
                        &[],
                        4.0,
                        true,
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        None,
                        &[],
                        None,
                        &[],
                        TEST_CLIMATE,
                        &[],
                    );
                }
            },
        )
        .expect("dock spawn");

    assert_eq!(
        world.query::<&MapVisualLayer>().iter(&world).count(),
        4,
        "cada mitad aporta exactamente suelo y una capa TILE_SEQ; no césped genérico extra"
    );
    assert_eq!(
        world
            .query::<&crate::render::WaterTile>()
            .iter(&world)
            .count(),
        1,
        "la mitad plana conserva su agua animada"
    );
    let rendered: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .collect();
    assert!(
        rendered.iter().any(|(sprite, _)| shore.matches(sprite)),
        "la mitad inclinada frente al mar debe usar la costa OpenTTD"
    );

    let slope_pos = rendered
        .iter()
        .find_map(|(sprite, transform)| slope_dock.matches(sprite).then_some(transform.translation))
        .expect("pieza de muelle SW");
    let water_pos = rendered
        .iter()
        .find_map(|(sprite, transform)| water_dock.matches(sprite).then_some(transform.translation))
        .expect("pieza de muelle plana X");
    assert_eq!(slope_pos, expected_slope_pos);
    assert_eq!(water_pos, expected_water_pos);
    let dock_water_x = rendered
        .iter()
        .find_map(|(sprite, transform)| {
            flat_water
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("agua de la mitad plana del muelle");
    let dock_shore_x = rendered
        .iter()
        .find_map(|(sprite, transform)| shore.matches(sprite).then_some(transform.translation.x))
        .expect("costa de la mitad terrestre del muelle");
    let offset = crate::iso::GROUND_SPRITE_CENTER_X_OFFSET;
    assert_eq!(dock_water_x, crate::iso::iso(water.x, water.y).x + offset);
    assert_eq!(dock_shore_x, crate::iso::iso(land.x, land.y).x + offset);

    let mut parents: Vec<_> = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .map(|parent| {
            (
                parent.sprite_id,
                parent.bounds.xmin,
                parent.bounds.ymin,
                parent.bounds.zmin,
                parent.bounds.xmax,
                parent.bounds.ymax,
                parent.bounds.zmax,
            )
        })
        .collect();
    parents.sort_unstable();
    assert_eq!(
        parents,
        vec![(2729, 32, 36, 0, 47, 43, 7), (2731, 48, 36, 0, 63, 43, 7),],
        "las dos mitades del muelle entran al sorter con sus cajas StationGfx"
    );
}

/// Una bahía vial normal ya contiene todo el suelo en su layout de estación.
/// `m3` conserva los road bits importados, pero no habilita una segunda
/// carretera genérica: OpenTTD sólo la superpone para roadtypes con overlay,
/// que este renderer todavía no modela como una variante distinta.
#[test]
fn road_stop_bay_uses_only_its_vanilla_ground_and_build_layers() {
    let assets = boot_assets_app();
    let bus_ground = assets.bus_stop_grounds[0].clone();
    let mut map = fresh_map8();
    let stop = TileCoord::new(3, 3);
    map.set_tile(
        stop,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            // StationType::Bus, StationGfx::NE. Los bits de carretera no
            // deben crear un segundo suelo bajo la bahía.
            m3: 0x0A,
            m5: 0,
            m6: 3 << 3,
            ..tile_template()
        },
    )
    .expect("bus stop tile");
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("bus stop spawn");

    let sprites: Vec<_> = world.query::<&Sprite>().iter(&world).collect();
    assert_eq!(
        sprites.len(),
        4,
        "bahía vanilla = ground + BUILD_A/B/C; ni césped ni carretera heurística"
    );
    assert!(
        sprites.iter().any(|sprite| bus_ground.matches(sprite)),
        "debe conservar el suelo de la bahía NE"
    );
    let bay_ground_x = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            bus_ground
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("suelo de bahía bus");
    assert_eq!(
        bay_ground_x,
        crate::iso::iso(stop.x, stop.y).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET,
        "la bahía usa el xrel=-31 del ground OpenGFX"
    );
}

#[test]
fn road_stop_vanilla_layers_join_global_sort() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let coord = TileCoord::new(1, 1);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
            m6: 3 << 3, // StationType::Bus.
            ..tile_template()
        },
    )
    .expect("drive-through bus stop");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("drive-through bus stop spawn");

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [5980, 5981]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                5980,
                ParentSpriteBounds::new(16, 16, 0, 31, 18, 15),
                viewport_insertion_key(1, 1, 12),
            ),
            (
                5981,
                ParentSpriteBounds::new(16, 29, 0, 31, 31, 15),
                viewport_insertion_key(1, 1, 13),
            ),
        ],
        "cada capa TILE_SEQ_LINE de la parada debe ser un parent global"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "los parents mantienen su profundidad fuente antes del sort global"
    );
}

#[test]
fn road_waypoint_vanilla_catenary_and_layers_join_global_sort() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let coord = TileCoord::new(1, 1);
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
        m6: openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT << 3,
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
    map.set_tile(coord, tile).expect("road waypoint X");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("road waypoint X spawn");

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [6071, 6043, 6143, 6144]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                6071,
                ParentSpriteBounds::new(31, 16, 0, 31, 16, 1),
                viewport_insertion_key(1, 1, 4),
            ),
            (
                6071,
                ParentSpriteBounds::new(16, 16, 0, 16, 16, 1),
                viewport_insertion_key(1, 1, 5),
            ),
            (
                6071,
                ParentSpriteBounds::new(16, 31, 0, 16, 31, 1),
                viewport_insertion_key(1, 1, 6),
            ),
            (
                6043,
                ParentSpriteBounds::new(16, 16, 2, 31, 31, 2),
                viewport_insertion_key(1, 1, 7),
            ),
            (
                6143,
                ParentSpriteBounds::new(16, 16, 0, 31, 18, 15),
                viewport_insertion_key(1, 1, 12),
            ),
            (
                6144,
                ParentSpriteBounds::new(16, 29, 0, 31, 31, 15),
                viewport_insertion_key(1, 1, 13),
            ),
        ],
        "catenaria y postes BUILD del waypoint vanilla comparten el compositor global"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "los parents preservan la profundidad fuente antes del sort global"
    );
}

#[test]
fn runtime_only_road_waypoint_surfaces_keep_both_newgrf_views() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
        m6: openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT << 3,
        ..tile_template()
    };
    tile = openttdrs_core::set_road_type_on_tile(tile, RoadType::from_u8(2));
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::from_u8(3)));
    map.set_tile(coord, tile)
        .expect("road waypoint with custom road and tram types");

    let road_rgba = [255, 40, 40, 255].repeat(8 * 8);
    let tram_rgba = [40, 40, 255, 255].repeat(8 * 8);
    let runtime_def =
        |id: u8, class: RoadTramType, label: &'static str, rgba: Vec<u8>| RoadTypeDef {
            id: RoadType::from_u8(id),
            class,
            label: label.into(),
            short_label: label.into(),
            intro_year: 0,
            max_speed: 0,
            cost_multiplier: 0,
            maintenance_multiplier: 0,
            flags: 0,
            powered_mask: 0,
            badges: Vec::new(),
            from_tramtypes_feature: matches!(class, RoadTramType::Tram),
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(TrainSpriteGraphics {
                sets: vec![vec![DecodedSprite {
                    width: 8,
                    height: 8,
                    x_offs: 0,
                    y_offs: 0,
                    rgba,
                    mask: Vec::new(),
                }]],
                assigns: vec![TrainSpriteAssign {
                    local_id: 0,
                    set_id: 0,
                }],
                ..Default::default()
            })),
            newgrf_grfid: 0x5257_0000 | u32::from(id),
            newgrf_type_tables: None,
        };
    let road_catalog = vec![
        runtime_def(
            2,
            RoadTramType::Road,
            "Runtime waypoint road",
            road_rgba.clone(),
        ),
        runtime_def(
            3,
            RoadTramType::Tram,
            "Runtime waypoint tram",
            tram_rgba.clone(),
        ),
    ];
    let station = Station::new_with_kind(coord, StopKind::RoadWaypoint);
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut road_sprites: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_station_tile_with_world_and_road_types(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    std::slice::from_ref(&station),
                    4.0,
                    true,
                    &[],
                    &[],
                    &road_catalog,
                    Some(&mut road_sprites),
                    None,
                    Some(&mut images),
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                    None,
                );
            },
        )
        .expect("runtime-only road waypoint surfaces");

    let rendered_handles: Vec<_> = world
        .query::<&Sprite>()
        .iter(&world)
        .map(|sprite| sprite.image.clone())
        .collect();
    let images = world.resource::<Assets<Image>>();
    let rendered_road = rendered_handles.iter().any(|handle| {
        images.get(handle).and_then(|image| image.data.as_deref()) == Some(road_rgba.as_slice())
    });
    let rendered_tram = rendered_handles.iter().any(|handle| {
        images.get(handle).and_then(|image| image.data.as_deref()) == Some(tram_rgba.as_slice())
    });
    assert!(
        rendered_road,
        "el suelo del waypoint debe resolver el roadtype runtime-only"
    );
    assert!(
        rendered_tram,
        "el overlay de tranvía del waypoint debe resolver el tramtype runtime-only"
    );
}

#[test]
fn road_depot_vanilla_layers_join_global_sort() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let coord = TileCoord::new(1, 1);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RoadDepot,
            mapt: 0x20,
            m5: 1, // Dirección SE: dos capas TILE_SEQ_LINE (1408/1409).
            ..tile_template()
        },
    )
    .expect("road depot SE");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("road depot SE spawn");

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [1408, 1409]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                1408,
                ParentSpriteBounds::new(16, 16, 0, 16, 31, 19),
                viewport_insertion_key(1, 1, 1),
            ),
            (
                1409,
                ParentSpriteBounds::new(31, 16, 0, 31, 31, 19),
                viewport_insertion_key(1, 1, 2),
            ),
        ],
        "cada fachada TILE_SEQ_LINE del depósito debe ser un parent global"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "los parents preservan la profundidad fuente antes del sort global"
    );
}

#[test]
fn pure_vanilla_tram_depot_relocates_the_full_build_sequence() {
    let assets = boot_assets_app();
    let expected_track = assets
        .rail
        .get(&6035)
        .expect("SPR_TRAMWAY_DEPOT_WITH_TRACK + 0")
        .clone();
    let expected_building = assets
        .rail
        .get(&6036)
        .expect("SPR_TRAMWAY_DEPOT_WITH_TRACK + 1")
        .clone();
    let mut map = Map::new_flat(4, 4, 0);
    let coord = TileCoord::new(1, 1);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RoadDepot,
            mapt: 0x20,
            // Dirección SE: `_road_depot_SE` contiene 1408/1409. Un depósito
            // creado como tram conserva `INVALID_ROADTYPE` en m4/m3hi y el
            // tipo tram vanilla en m8[6..12].
            m5: 1,
            m3hi: 0x3F,
            m8: 0x0040,
            ..tile_template()
        },
    )
    .expect("tram depot SE");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("tram depot SE spawn");

    let mut layers: Vec<_> = world
        .query::<(&ViewportSortableParent, &Sprite, &Transform)>()
        .iter(&world)
        .filter(|(parent, _, _)| [6035, 6036].contains(&parent.sprite_id))
        .collect();
    layers.sort_by_key(|(parent, _, _)| parent.insertion_key);
    assert_eq!(
        layers
            .iter()
            .map(|(parent, _, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                6035,
                ParentSpriteBounds::new(16, 16, 0, 16, 31, 19),
                viewport_insertion_key(1, 1, 1),
            ),
            (
                6036,
                ParentSpriteBounds::new(31, 16, 0, 31, 31, 19),
                viewport_insertion_key(1, 1, 2),
            ),
        ],
        "la relocalización mantiene las cajas TILE_SEQ de la tabla vial"
    );
    assert!(expected_track.matches(layers[0].1));
    assert!(expected_building.matches(layers[1].1));
    assert_eq!(layers[0].2.translation.xy(), Vec2::new(1.0, -47.5));
    assert_eq!(layers[1].2.translation.xy(), Vec2::new(3.0, -30.0));
    assert!(
        layers
            .iter()
            .all(|(parent, _, transform)| parent.source_depth == transform.translation.z),
        "los sprites relocalizados conservan la profundidad fuente global"
    );
}

#[test]
fn newgrf_road_depot_group_replaces_relocated_building_layers() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(2, 2);
    let mut map = fresh_map8();
    let mut depot = Tile {
        kind: TileKind::RoadDepot,
        mapt: 0x20,
        // SE consume las dos primeras vistas del bloque relocatable
        // `SPR_ROAD_DEPOT`: SE_1/SE_2.
        m5: 1,
        ..tile_template()
    };
    depot = openttdrs_core::set_road_type_on_tile(depot, RoadType::from_u8(2));
    map.set_tile(coord, depot).expect("road depot NewGRF");

    let mouth_rgba = [230, 70, 210, 255].repeat(6 * 7);
    let building_rgba = [20, 180, 90, 255].repeat(8 * 9);
    let mouth = DecodedSprite {
        width: 6,
        height: 7,
        x_offs: -3,
        y_offs: -5,
        rgba: mouth_rgba.clone(),
        mask: vec![198; 6 * 7],
    };
    let building = DecodedSprite {
        width: 8,
        height: 9,
        x_offs: 4,
        y_offs: -11,
        rgba: building_rgba.clone(),
        mask: vec![199; 8 * 9],
    };
    let suppressed_overlay = DecodedSprite {
        width: 5,
        height: 6,
        x_offs: -13,
        y_offs: -17,
        rgba: [60, 230, 140, 255].repeat(5 * 6),
        mask: vec![202; 5 * 6],
    };
    let mut graphics = TrainSpriteGraphics {
        sets: vec![
            vec![mouth.clone(), building.clone()],
            vec![DecodedSprite {
                width: 1,
                height: 1,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![1, 2, 3, 255],
                mask: Vec::new(),
            }],
            vec![suppressed_overlay.clone()],
        ],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 0,
        }],
        ..TrainSpriteGraphics::default()
    };
    // `RoadTypeSpriteGroup::ROTSG_DEPOT` en `road.h`.
    graphics.specific_assigns.insert((0, 8), 0);
    // Aunque el tipo use GROUND/OVERLAY, un `ROTSG_DEPOT` resuelto cambia
    // `default_gfx` a falso y OpenTTD no dibuja esta capa de vía aparte.
    graphics.specific_assigns.insert((0, 2), 1);
    graphics.specific_assigns.insert((0, 1), 2);
    let road_catalog = vec![RoadTypeDef {
        id: RoadType::from_u8(2),
        class: RoadTramType::Road,
        label: "Depot NewGRF".into(),
        short_label: "NGDP".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 0,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: false,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(graphics)),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    }];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut road_sprites: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    Some(crate::sprites::CompanyColour::Red),
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &road_catalog,
                    Some(&mut road_sprites),
                    &[],
                    None,
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("road depot NewGRF spawn");

    let mut layers: Vec<_> = world
        .query::<(&ViewportSortableParent, &Sprite, &Transform)>()
        .iter(&world)
        .filter(|(parent, _, _)| [1408, 1409].contains(&parent.sprite_id))
        .map(|(parent, sprite, transform)| (*parent, sprite.clone(), transform.translation))
        .collect();
    layers.sort_by_key(|(parent, _, _)| parent.insertion_key);
    assert_eq!(
        layers
            .iter()
            .map(|(parent, _, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                1408,
                ParentSpriteBounds::new(32, 32, 0, 32, 47, 19),
                viewport_insertion_key(2, 2, 1),
            ),
            (
                1409,
                ParentSpriteBounds::new(47, 32, 0, 47, 47, 19),
                viewport_insertion_key(2, 2, 2),
            ),
        ],
        "ROTSG_DEPOT conserva los parents y prismas TILE_SEQ viales"
    );
    assert!(
        layers
            .iter()
            .all(|(parent, _, translation)| parent.source_depth == translation.z),
        "las fachadas NewGRF conservan profundidad fuente antes del sort global"
    );

    let expected_centers = {
        let map = &world.resource::<TsMap>().0;
        let grid = &world.resource::<TsGrid>().0;
        let ctx = TileRenderContext::new(map, grid, 2, 2);
        crate::sprites::road_depot_build_layers(1)
            .iter()
            .zip([&mouth, &building])
            .map(|(layer, view)| {
                let layer = crate::sprites::RoadDepotLayerGfx {
                    w: f32::from(view.width),
                    h: f32::from(view.height),
                    x_offs: f32::from(view.x_offs),
                    y_offs: f32::from(view.y_offs),
                    ..*layer
                };
                crate::iso::road_depot_build_sprite_center(
                    ctx.iso_pos,
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    ctx.info.base_z,
                    layer.z,
                    crate::sprites::road_depot_seq_gfx(&layer),
                    layer.w,
                    layer.h,
                )
                .xy()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(layers[0].2.xy(), expected_centers[0]);
    assert_eq!(layers[1].2.xy(), expected_centers[1]);

    let company_colour = crate::sprites::CompanyColour::Red;
    let expected_mouth =
        openttdrs_core::bake_sprite_company_palette(&mouth, company_colour.as_u8());
    let expected_building =
        openttdrs_core::bake_sprite_company_palette(&building, company_colour.as_u8());
    {
        let images = world.resource::<Assets<Image>>();
        assert_eq!(
            images
                .get(&layers[0].1.image)
                .and_then(|image| image.data.as_deref()),
            Some(expected_mouth.as_slice()),
            "SE_1 debe usar el primer sprite y la paleta del propietario"
        );
        assert_eq!(
            images
                .get(&layers[1].1.image)
                .and_then(|image| image.data.as_deref()),
            Some(expected_building.as_slice()),
            "SE_2 debe usar el segundo sprite y la paleta del propietario"
        );
    }
    let expected_overlay_position = {
        let map = &world.resource::<TsMap>().0;
        let grid = &world.resource::<TsGrid>().0;
        let ctx = TileRenderContext::new(map, grid, 2, 2);
        overlay_pos(
            ctx.iso_pos,
            f32::from(suppressed_overlay.x_offs),
            f32::from(suppressed_overlay.y_offs),
            f32::from(suppressed_overlay.width),
            f32::from(suppressed_overlay.height),
            ctx.info.base_z,
            crate::render::tiles::TRAM_OVERLAY_LAYER_FRAC,
            ctx.tx_i32(),
            ctx.ty_i32(),
        )
    };
    let sprites: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .map(|(sprite, transform)| (sprite.clone(), transform.translation))
        .collect();
    let images = world.resource::<Assets<Image>>();
    assert!(
        !sprites.iter().any(|(sprite, position)| {
            *position == expected_overlay_position
                && images
                    .get(&sprite.image)
                    .and_then(|image| image.data.as_deref())
                    == Some(suppressed_overlay.rgba.as_slice())
        }),
        "ROTSG_DEPOT gana sobre ROTSG_OVERLAY en el depósito"
    );
}

#[test]
fn newgrf_road_depot_overlay_uses_ground_contract_and_nfo_anchor() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(2, 2);
    let mut map = fresh_map8();
    let mut depot = Tile {
        kind: TileKind::RoadDepot,
        mapt: 0x20,
        // SE: `DiagDirToRoadBits` = 0x04 y por tanto el índice de overlay
        // debe ser el mismo que `GetRoadSpriteOffset(SLOPE_FLAT, 0x04)`.
        m5: 1,
        ..tile_template()
    };
    depot = openttdrs_core::set_road_type_on_tile(depot, RoadType::from_u8(2));
    map.set_tile(coord, depot)
        .expect("road depot overlay NewGRF");

    let ground = DecodedSprite {
        width: 1,
        height: 1,
        x_offs: 0,
        y_offs: 0,
        rgba: vec![1, 2, 3, 255],
        mask: Vec::new(),
    };
    let overlay = DecodedSprite {
        width: 7,
        height: 9,
        x_offs: -5,
        y_offs: -11,
        rgba: [17, 190, 230, 255].repeat(7 * 9),
        mask: vec![201; 7 * 9],
    };
    let mut graphics = TrainSpriteGraphics {
        sets: vec![vec![ground], vec![overlay.clone()]],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 0,
        }],
        ..TrainSpriteGraphics::default()
    };
    // `UsesOverlay()` depende de GROUND (2), no de que exista el overlay
    // opcional (1). El set de overlay tiene una sola vista y debe reutilizarse
    // para el offset SE solicitado por el draw-proc.
    graphics.specific_assigns.insert((0, 2), 0);
    graphics.specific_assigns.insert((0, 1), 1);
    let road_catalog = vec![RoadTypeDef {
        id: RoadType::from_u8(2),
        class: RoadTramType::Road,
        label: "Overlay depot NewGRF".into(),
        short_label: "NGOV".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 0,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: false,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(graphics)),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    }];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut road_sprites: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &road_catalog,
                    Some(&mut road_sprites),
                    &[],
                    None,
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("road depot NewGRF overlay spawn");

    let sprite_entities: Vec<_> = world
        .query::<(Entity, &Sprite, &Transform)>()
        .iter(&world)
        .map(|(entity, sprite, transform)| (entity, sprite.clone(), transform.translation))
        .collect();
    let expected_position = {
        let map = &world.resource::<TsMap>().0;
        let grid = &world.resource::<TsGrid>().0;
        let ctx = TileRenderContext::new(map, grid, 2, 2);
        overlay_pos(
            ctx.iso_pos,
            f32::from(overlay.x_offs),
            f32::from(overlay.y_offs),
            f32::from(overlay.width),
            f32::from(overlay.height),
            ctx.info.base_z,
            crate::render::tiles::TRAM_OVERLAY_LAYER_FRAC,
            ctx.tx_i32(),
            ctx.ty_i32(),
        )
    };
    let expected_rgba = overlay.rgba.clone();
    let overlays: Vec<_> = {
        let images = world.resource::<Assets<Image>>();
        sprite_entities
            .into_iter()
            .filter(|(_, sprite, position)| {
                *position == expected_position
                    && images
                        .get(&sprite.image)
                        .and_then(|image| image.data.as_deref())
                        == Some(expected_rgba.as_slice())
            })
            .collect()
    };
    assert_eq!(
        overlays.len(),
        1,
        "el depósito debe materializar ROTSG_OVERLAY"
    );
    let (entity, _, position) = overlays[0];
    assert_eq!(
        position, expected_position,
        "la ancla NFO debe llegar sin recorte"
    );
    assert!(
        world
            .entity(entity)
            .get::<ViewportSortableParent>()
            .is_none(),
        "DrawGroundSprite de ROTSG_OVERLAY no crea otra fachada sortable"
    );
    assert!(
        world
            .entity(entity)
            .get::<ViewportSortableChild>()
            .is_none(),
        "en plano no existe foundation a la que adjuntar el overlay"
    );
}

#[test]
fn action5_no_track_tram_depot_relocates_buildings_and_draws_the_overlay() {
    let assets = boot_assets_app();
    let overlay_index = crate::sprites::road_flat_sprite_index(0, 0x04); // SE
    let expected_overlay = assets.tram_flat[overlay_index].clone();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RoadDepot,
            mapt: 0x20,
            // SE; el depósito tram puro guarda el tipo road inválido y el
            // tram vanilla en m8[6..12].
            m5: 1,
            m3hi: 0x3F,
            m8: 0x0040,
            ..tile_template()
        },
    )
    .expect("tram depot SE");
    let mouth = DecodedSprite {
        width: 6,
        height: 7,
        x_offs: -3,
        y_offs: -5,
        rgba: [215, 70, 190, 255].repeat(6 * 7),
        mask: vec![198; 6 * 7],
    };
    let building = DecodedSprite {
        width: 8,
        height: 9,
        x_offs: 4,
        y_offs: -11,
        rgba: [20, 170, 90, 255].repeat(8 * 9),
        mask: vec![199; 8 * 9],
    };
    let mut tramway = vec![None; openttdrs_core::TRAMWAY_ACTION5_SLOT_COUNT];
    tramway[openttdrs_core::TRAMWAY_DEPOT_NO_TRACK_ACTION5_SLOT] = Some(mouth.clone());
    tramway[openttdrs_core::TRAMWAY_DEPOT_NO_TRACK_ACTION5_SLOT + 1] = Some(building.clone());

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types_and_tramway_action5(
                    &mut commands,
                    &a.0,
                    None,
                    Some(crate::sprites::CompanyColour::Red),
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    &[],
                    &[],
                    Some(&mut action5_sprites),
                    Some(&mut images),
                    &[],
                    &[],
                    TramwayDepotAction5 {
                        sprites: &tramway,
                        replacement: openttdrs_core::TramwayDepotReplacement::NoTrack,
                    },
                );
            },
        )
        .expect("tram depot Action5 no-track spawn");

    let mut layers: Vec<_> = world
        .query::<(&ViewportSortableParent, &Sprite, &Transform)>()
        .iter(&world)
        .filter(|(parent, _, _)| [6099, 6100].contains(&parent.sprite_id))
        .map(|(parent, sprite, transform)| (*parent, sprite.clone(), transform.translation))
        .collect();
    layers.sort_by_key(|(parent, _, _)| parent.insertion_key);
    assert_eq!(
        layers
            .iter()
            .map(|(parent, _, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                6099,
                ParentSpriteBounds::new(16, 16, 0, 16, 31, 19),
                viewport_insertion_key(1, 1, 1),
            ),
            (
                6100,
                ParentSpriteBounds::new(31, 16, 0, 31, 31, 19),
                viewport_insertion_key(1, 1, 2),
            ),
        ],
        "DEPOT_NO_TRACK conserva los prismas de la secuencia ROAD_DEPOT"
    );
    let colour = crate::sprites::CompanyColour::Red;
    let expected_mouth = openttdrs_core::bake_sprite_company_palette(&mouth, colour.as_u8());
    let expected_building = openttdrs_core::bake_sprite_company_palette(&building, colour.as_u8());
    {
        let images = world.resource::<Assets<Image>>();
        assert_eq!(
            images
                .get(&layers[0].1.image)
                .and_then(|image| image.data.as_deref()),
            Some(expected_mouth.as_slice())
        );
        assert_eq!(
            images
                .get(&layers[1].1.image)
                .and_then(|image| image.data.as_deref()),
            Some(expected_building.as_slice())
        );
    }
    assert_eq!(
        world
            .query::<&Sprite>()
            .iter(&world)
            .filter(|sprite| expected_overlay.matches(sprite))
            .count(),
        1,
        "DEPOT_NO_TRACK agrega el SPR_TRAMWAY_OVERLAY que no viene dentro de la fachada"
    );
}

#[test]
fn action5_catenary_depot_fallback_covers_custom_tram_and_road_types() {
    let assets = boot_assets_app();
    let tram_coord = TileCoord::new(1, 1);
    let road_coord = TileCoord::new(3, 1);
    let mut map = Map::new_flat(6, 4, 0);
    let mut custom_tram = Tile {
        kind: TileKind::RoadDepot,
        mapt: 0x20,
        m5: 1, // SE
        // Un depósito tram puro no tiene roadtype válido.
        m3hi: 0x3F,
        ..tile_template()
    };
    custom_tram =
        openttdrs_core::set_tram_road_type_on_tile(custom_tram, Some(RoadType::from_u8(2)));
    map.set_tile(tram_coord, custom_tram)
        .expect("custom tram depot");

    let road = openttdrs_core::set_road_type_on_tile(
        Tile {
            kind: TileKind::RoadDepot,
            mapt: 0x20,
            m5: 1, // SE
            ..tile_template()
        },
        RoadType::from_u8(3),
    );
    map.set_tile(road_coord, road).expect("custom road depot");

    let catenary_type = |id: u8, class: RoadTramType| RoadTypeDef {
        id: RoadType::from_u8(id),
        class,
        label: format!("Catenary {id}"),
        short_label: format!("C{id}"),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        // RoadTypeFlag::Catenary.
        flags: 1,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: matches!(class, RoadTramType::Tram),
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        // No publican vistas ni grupos Action3: la decisión debe venir de
        // Action0 y usar el fallback Action5, no de la caché de sprites.
        newgrf_runtime: None,
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    };
    let road_catalog = vec![
        catenary_type(2, RoadTramType::Tram),
        catenary_type(3, RoadTramType::Road),
    ];
    let mut tramway = vec![None; openttdrs_core::TRAMWAY_ACTION5_SLOT_COUNT];
    for (slot, rgba) in [
        (
            openttdrs_core::TRAMWAY_DEPOT_WITH_TRACK_ACTION5_SLOT,
            [210, 60, 180, 255],
        ),
        (
            openttdrs_core::TRAMWAY_DEPOT_WITH_TRACK_ACTION5_SLOT + 1,
            [40, 160, 90, 255],
        ),
        (
            openttdrs_core::TRAMWAY_DEPOT_NO_TRACK_ACTION5_SLOT,
            [230, 130, 30, 255],
        ),
        (
            openttdrs_core::TRAMWAY_DEPOT_NO_TRACK_ACTION5_SLOT + 1,
            [40, 120, 220, 255],
        ),
    ] {
        tramway[slot] = Some(DecodedSprite {
            width: 4,
            height: 5,
            x_offs: -2,
            y_offs: -4,
            rgba: rgba.repeat(4 * 5),
            mask: Vec::new(),
        });
    }

    let grid = RenderGrid::from_map(&map, 6, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                for coord in [tram_coord, road_coord] {
                    spawn_transport_object_tile_with_road_types_and_tramway_action5(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("x positiva"),
                            u32::try_from(coord.y).expect("y positiva"),
                        ),
                        4.0,
                        false,
                        &m.0,
                        m.0.dimensions(),
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        &[],
                        TEST_CLIMATE,
                        0,
                        &road_catalog,
                        None,
                        &[],
                        &[],
                        &[],
                        Some(&mut action5_sprites),
                        Some(&mut images),
                        &[],
                        &[],
                        TramwayDepotAction5 {
                            sprites: &tramway,
                            replacement: openttdrs_core::TramwayDepotReplacement::WithTrack,
                        },
                    );
                }
            },
        )
        .expect("custom catenary depot spawn");

    let parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .map(|(parent, transform)| (*parent, transform.translation))
        .collect();
    for sprite_id in [6035, 6036] {
        assert!(
            parents
                .iter()
                .any(|(parent, _)| parent.sprite_id == sprite_id),
            "el tramtype eléctrico puro usa DEPOT_WITH_TRACK ({sprite_id})"
        );
    }
    for sprite_id in [6099, 6100] {
        assert!(
            parents
                .iter()
                .any(|(parent, _)| parent.sprite_id == sprite_id),
            "un roadtype eléctrico válido fuerza DEPOT_NO_TRACK ({sprite_id})"
        );
    }
    assert!(
        parents
            .iter()
            .all(|(parent, transform)| parent.source_depth == transform.z),
        "ambas relocalizaciones conservan la profundidad fuente global"
    );
}

#[test]
fn drive_through_tram_stop_draws_vanilla_catenary() {
    let assets = boot_assets_app();
    let expected_back = assets
        .rail
        .get(&6071)
        .expect("catenaria trasera plana ROAD_X")
        .clone();
    let expected_front = assets
        .rail
        .get(&6043)
        .expect("catenaria delantera plana ROAD_X")
        .clone();
    let stop = TileCoord::new(3, 3);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
        m6: 3 << 3, // StationType::Bus.
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
    map.set_tile(stop, tile)
        .expect("parada drive-through con tranvía");
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("parada drive-through con catenaria");

    let catenary: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .filter(|(sprite, _)| expected_back.matches(sprite) || expected_front.matches(sprite))
        .collect();
    assert_eq!(
        catenary.len(),
        4,
        "una parada drive-through X emite tres recortes traseros y un frente"
    );
    assert_eq!(
        catenary
            .iter()
            .filter(|(sprite, _)| expected_back.matches(sprite))
            .count(),
        3
    );
    assert_eq!(
        catenary
            .iter()
            .filter(|(sprite, _)| expected_front.matches(sprite))
            .count(),
        1
    );

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [6071, 6043, 5980, 5981]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                6071,
                ParentSpriteBounds::new(63, 48, 0, 63, 48, 1),
                viewport_insertion_key(3, 3, 4),
            ),
            (
                6071,
                ParentSpriteBounds::new(48, 48, 0, 48, 48, 1),
                viewport_insertion_key(3, 3, 5),
            ),
            (
                6071,
                ParentSpriteBounds::new(48, 63, 0, 48, 63, 1),
                viewport_insertion_key(3, 3, 6),
            ),
            (
                6043,
                ParentSpriteBounds::new(48, 48, 2, 63, 63, 2),
                viewport_insertion_key(3, 3, 7),
            ),
            (
                5980,
                ParentSpriteBounds::new(48, 48, 0, 63, 50, 15),
                viewport_insertion_key(3, 3, 12),
            ),
            (
                5981,
                ParentSpriteBounds::new(48, 61, 0, 63, 63, 15),
                viewport_insertion_key(3, 3, 13),
            ),
        ],
        "la catenaria de la parada vanilla precede a sus capas BUILD en el stream global"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "catenaria y BUILD conservan profundidad fuente antes del sort global"
    );
}

fn assert_static_newgrf_road_stop_layout_joins_global_catenary_sort(
    stop_kind: StopKind,
    station_type: u8,
    draw_mode: u8,
    direct_base_ground: bool,
    incomplete_layout: bool,
) {
    assert_static_newgrf_road_stop_layout_with_options(
        stop_kind,
        station_type,
        draw_mode,
        direct_base_ground,
        incomplete_layout,
        false,
    );
}

fn assert_static_newgrf_road_stop_layout_with_options(
    stop_kind: StopKind,
    station_type: u8,
    draw_mode: u8,
    direct_base_ground: bool,
    incomplete_layout: bool,
    empty_sequence: bool,
) {
    assert_static_newgrf_road_stop_layout_with_orientation(
        stop_kind,
        station_type,
        draw_mode,
        direct_base_ground,
        incomplete_layout,
        openttdrs_core::RSV_DRIVE_THROUGH_X,
        empty_sequence,
    );
}

fn assert_static_newgrf_road_stop_layout_with_orientation(
    stop_kind: StopKind,
    station_type: u8,
    draw_mode: u8,
    direct_base_ground: bool,
    incomplete_layout: bool,
    orientation: u8,
    empty_sequence: bool,
) {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let coord = TileCoord::new(3, 3);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: orientation,
        m6: station_type << 3,
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
    map.set_tile(coord, tile)
        .expect("parada NewGRF drive-through con tranvía");
    let (catenary_back_id, catenary_front_id) = match orientation {
        openttdrs_core::RSV_DRIVE_THROUGH_Y => (6070, 6042),
        _ => (6071, 6043),
    };

    let ground_rgba = [240, 10, 10, 255].repeat(4);
    let parent_rgba = [10, 240, 10, 255].repeat(4);
    let child_rgba = [10, 10, 240, 255].repeat(4);
    let ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: ground_rgba.clone(),
        mask: Vec::new(),
    };
    let parent = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -3,
        y_offs: -4,
        rgba: parent_rgba.clone(),
        mask: Vec::new(),
    };
    let child = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -5,
        y_offs: -6,
        rgba: child_rgba.clone(),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![ground], vec![parent], vec![child]],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 9,
        }],
        ..Default::default()
    };
    runtime.tile_layouts.insert(
        9,
        TileLayout {
            ground: if direct_base_ground {
                TileLayoutSpriteRef {
                    direct_sprite: 3981,
                    ..Default::default()
                }
            } else {
                TileLayoutSpriteRef {
                    action1_set: Some(0),
                    ..Default::default()
                }
            },
            sequence: if empty_sequence {
                Vec::new()
            } else {
                vec![
                    TileLayoutSpriteRef {
                        action1_set: Some(1),
                        origin: [1, 2, 3],
                        extent: [4, 5, 6],
                        ..Default::default()
                    },
                    TileLayoutSpriteRef {
                        action1_set: Some(2),
                        origin: [7, -4, i8::MIN],
                        flags: if incomplete_layout { 0x04 } else { 0 },
                        ..Default::default()
                    },
                ]
            },
        },
    );
    let spec = RoadStopSpecDef {
        id: 7,
        class: 0,
        label: "TileLayout estático".into(),
        short_label: "TLS".into(),
        stop_type: openttdrs_core::ROADSTOP_TYPE_ALL,
        from_newgrf: true,
        grfid: 0x5449_4C45,
        newgrf_local_id: 0,
        newgrf_grf_version: 8,
        draw_mode,
        random_cargo_triggers: 0,
        flags: 0,
        build_cost_multiplier: 16,
        clear_cost_multiplier: 16,
        bridgeable_info: [openttdrs_core::road_stop_spec::RoadStopBridgeableInfo::default();
            openttdrs_core::road_stop_spec::ROADSTOP_LAYOUT_COUNT],
        callback_mask: 0,
        animation_status: 0xFF,
        animation_frames: 0,
        animation_speed: 2,
        animation_triggers: 0,
        newgrf_views: Vec::new(),
        newgrf_runtime: Some(Box::new(runtime)),
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
    };
    let mut station = Station::new_with_kind(coord, stop_kind);
    station.road_stop_spec = Some(spec.id);

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfAction5SpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    std::slice::from_ref(&station),
                    4.0,
                    true,
                    &[],
                    std::slice::from_ref(&spec),
                    None,
                    Some(&mut images),
                    &[],
                    None,
                    &[],
                    Some(&mut cache),
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("TileLayout estático de parada vial");

    let mut parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Sprite, &Transform)>()
        .iter(&world)
        .filter_map(|(entity, parent, sprite, transform)| {
            [catenary_back_id, catenary_front_id, u32::MAX]
                .contains(&parent.sprite_id)
                .then_some((
                    entity,
                    *parent,
                    sprite.image.clone(),
                    transform.translation.z,
                ))
        })
        .collect();
    parents.sort_by_key(|(_, parent, _, _)| parent.insertion_key);
    let expected_parent_rows = if incomplete_layout {
        Vec::new()
    } else if empty_sequence {
        vec![
            (
                catenary_back_id,
                ParentSpriteBounds::new(63, 48, 0, 63, 48, 1),
                viewport_insertion_key(3, 3, 4),
            ),
            (
                catenary_back_id,
                ParentSpriteBounds::new(48, 48, 0, 48, 48, 1),
                viewport_insertion_key(3, 3, 5),
            ),
            (
                catenary_back_id,
                ParentSpriteBounds::new(48, 63, 0, 48, 63, 1),
                viewport_insertion_key(3, 3, 6),
            ),
            (
                catenary_front_id,
                ParentSpriteBounds::new(48, 48, 2, 63, 63, 2),
                viewport_insertion_key(3, 3, 7),
            ),
        ]
    } else {
        vec![
            (
                catenary_back_id,
                ParentSpriteBounds::new(63, 48, 0, 63, 48, 1),
                viewport_insertion_key(3, 3, 4),
            ),
            (
                catenary_back_id,
                ParentSpriteBounds::new(48, 48, 0, 48, 48, 1),
                viewport_insertion_key(3, 3, 5),
            ),
            (
                catenary_back_id,
                ParentSpriteBounds::new(48, 63, 0, 48, 63, 1),
                viewport_insertion_key(3, 3, 6),
            ),
            (
                catenary_front_id,
                ParentSpriteBounds::new(48, 48, 2, 63, 63, 2),
                viewport_insertion_key(3, 3, 7),
            ),
            (
                u32::MAX,
                ParentSpriteBounds::new(49, 50, 3, 52, 54, 8),
                viewport_insertion_key(3, 3, 12),
            ),
        ]
    };
    assert_eq!(
        parents
            .iter()
            .map(|(_, parent, _, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        expected_parent_rows,
        "la catenaria debe preceder el TileSeq o su fallback en el stream global"
    );
    assert!(
        parents
            .iter()
            .all(|(_, parent, _, depth)| parent.source_depth == *depth),
        "catenaria y TileSeq conservan la profundidad fuente antes del sort global"
    );

    let children: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .map(|(child, sprite)| (*child, sprite.image.clone()))
        .collect();
    let sprite_handles: Vec<_> = world
        .query::<&Sprite>()
        .iter(&world)
        .map(|sprite| sprite.image.clone())
        .collect();

    if incomplete_layout {
        let vanilla_building_ids: Vec<_> = world
            .query::<&ViewportSortableParent>()
            .iter(&world)
            .filter_map(|parent| {
                [5980, 5981]
                    .contains(&parent.sprite_id)
                    .then_some(parent.sprite_id)
            })
            .collect();
        assert_eq!(
            vanilla_building_ids,
            vec![5980, 5981],
            "un TileLayout incompleto debe usar las capas BUILD vanilla completas"
        );
        let custom_image_count = {
            let images = world.resource::<Assets<Image>>();
            sprite_handles
                .iter()
                .filter(|handle| {
                    images
                        .get(*handle)
                        .and_then(|image| image.data.as_deref())
                        .is_some_and(|rgba| {
                            [
                                ground_rgba.as_slice(),
                                parent_rgba.as_slice(),
                                child_rgba.as_slice(),
                            ]
                            .contains(&rgba)
                        })
                })
                .count()
        };
        assert_eq!(
            custom_image_count, 0,
            "un layout incompleto no debe mezclar ninguna textura NewGRF"
        );
        let vanilla_ground_count = {
            let vanilla_ground = {
                let assets = &world.resource::<TsAssets>().0;
                assets.road_paved[crate::sprites::road_flat_sprite_index(0, 0x0A)].clone()
            };
            world
                .query::<&Sprite>()
                .iter(&world)
                .filter(|sprite| vanilla_ground.matches(sprite))
                .count()
        };
        assert_eq!(
            vanilla_ground_count, 1,
            "el fallback atómico debe conservar el suelo vanilla"
        );
        assert!(
            parents
                .iter()
                .all(|(_, parent, _, _)| parent.sprite_id != u32::MAX),
            "un layout incompleto no debe publicar parents custom parciales"
        );
        return;
    }

    if empty_sequence {
        let vanilla_building_ids: Vec<_> = world
            .query::<&ViewportSortableParent>()
            .iter(&world)
            .filter_map(|parent| {
                [5980, 5981]
                    .contains(&parent.sprite_id)
                    .then_some(parent.sprite_id)
            })
            .collect();
        assert!(
            vanilla_building_ids.is_empty(),
            "un TileLayout vacío no debe reintroducir BUILD vanilla"
        );
        assert!(
            parents
                .iter()
                .all(|(_, parent, _, _)| parent.sprite_id != u32::MAX),
            "un TileLayout vacío no debe publicar un parent custom ficticio"
        );
        assert!(
            children.is_empty(),
            "un TileLayout vacío no debe crear children"
        );
        return;
    }

    let (parent_entity, _, parent_handle, _) = parents
        .iter()
        .find(|(_, parent, _, _)| parent.sprite_id == u32::MAX)
        .expect("parent TileSeq estático");
    let (parent_matches, child_matches) = {
        let images = world.resource::<Assets<Image>>();
        let has_rgba = |handle: &Handle<Image>, rgba: &[u8]| {
            images.get(handle).and_then(|image| image.data.as_deref()) == Some(rgba)
        };
        let parent_matches = has_rgba(parent_handle, parent_rgba.as_slice());
        let child_matches = children.iter().any(|(child_component, handle)| {
            child_component.parent == *parent_entity && has_rgba(handle, child_rgba.as_slice())
        });
        (parent_matches, child_matches)
    };
    let (ground_x_offs, ground_y_offs, ground_width, ground_height) = if direct_base_ground {
        (-31.0, 0.0, 64.0, 31.0)
    } else {
        (-1.0, -2.0, 2.0, 2.0)
    };
    let expected_ground_position = overlay_pos(
        crate::iso::iso(coord.x, coord.y),
        ground_x_offs,
        ground_y_offs,
        ground_width,
        ground_height,
        0,
        0.025,
        coord.x,
        coord.y,
    );
    let ground_depths: Vec<_> = if direct_base_ground {
        let expected_ground = world.resource::<TsAssets>().0.grass.clone();
        let matches: Vec<_> = world
            .query::<(&Sprite, &Transform)>()
            .iter(&world)
            .filter_map(|(sprite, transform)| {
                expected_ground
                    .matches(sprite)
                    .then_some(transform.translation.z)
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "el ground base directo debe materializarse una sola vez"
        );
        matches
    } else {
        let ground_handles = {
            let images = world.resource::<Assets<Image>>();
            let has_rgba = |handle: &Handle<Image>, rgba: &[u8]| {
                images.get(handle).and_then(|image| image.data.as_deref()) == Some(rgba)
            };
            let mut handles = Vec::new();
            for handle in &sprite_handles {
                if has_rgba(handle, ground_rgba.as_slice()) && !handles.contains(handle) {
                    handles.push(handle.clone());
                }
            }
            handles
        };
        assert_eq!(
            ground_handles.len(),
            1,
            "el ground debe materializarse una vez"
        );
        world
            .query::<(&Sprite, &Transform)>()
            .iter(&world)
            .filter_map(|(sprite, transform)| {
                (ground_handles.contains(&sprite.image)
                    && transform.translation.truncate() == expected_ground_position.truncate())
                .then_some(transform.translation.z)
            })
            .collect()
    };
    assert_eq!(
        ground_depths,
        vec![ground_draw_z(coord.x, coord.y, 0.025)],
        "el ground TileLayout de parada/waypoint usa DrawGroundSprite, no el pase sortable"
    );
    assert!(
        parent_matches,
        "el parent debe usar el sprite NewGRF del TileLayout"
    );
    assert!(
        child_matches,
        "el child TileSeq debe seguir unido al parent NewGRF"
    );
}

#[test]
fn static_newgrf_road_stop_layout_joins_global_catenary_sort() {
    assert_static_newgrf_road_stop_layout_joins_global_catenary_sort(
        StopKind::BusStop,
        3,
        openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        false,
        false,
    );
}

#[test]
fn static_newgrf_road_stop_layout_y_axis_joins_global_catenary_sort() {
    assert_static_newgrf_road_stop_layout_with_orientation(
        StopKind::BusStop,
        3,
        openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        false,
        false,
        openttdrs_core::RSV_DRIVE_THROUGH_Y,
        false,
    );
}

#[test]
fn static_newgrf_truck_stop_layout_joins_global_catenary_sort() {
    assert_static_newgrf_road_stop_layout_joins_global_catenary_sort(
        StopKind::TruckStop,
        2,
        openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        false,
        false,
    );
}

#[test]
fn static_newgrf_road_waypoint_layout_joins_global_catenary_sort() {
    assert_static_newgrf_road_stop_layout_joins_global_catenary_sort(
        StopKind::RoadWaypoint,
        openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT,
        openttdrs_core::ROADSTOP_DRAW_MODE_WAYP_GROUND,
        false,
        false,
    );
}

#[test]
fn incomplete_newgrf_road_stop_layout_falls_back_atomically() {
    assert_static_newgrf_road_stop_layout_joins_global_catenary_sort(
        StopKind::BusStop,
        3,
        openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        false,
        true,
    );
}

#[test]
fn empty_newgrf_road_stop_layout_does_not_fallback_to_vanilla_buildings() {
    assert_static_newgrf_road_stop_layout_with_options(
        StopKind::BusStop,
        3,
        openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        false,
        false,
        true,
    );
}

#[test]
fn direct_base_ground_newgrf_road_layouts_join_global_catenary_sort() {
    for (stop_kind, station_type, draw_mode) in [
        (
            StopKind::BusStop,
            3,
            openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        ),
        (
            StopKind::TruckStop,
            2,
            openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        ),
        (
            StopKind::RoadWaypoint,
            openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT,
            openttdrs_core::ROADSTOP_DRAW_MODE_WAYP_GROUND,
        ),
    ] {
        assert_static_newgrf_road_stop_layout_joins_global_catenary_sort(
            stop_kind,
            station_type,
            draw_mode,
            true,
            false,
        );
    }
}

#[test]
fn normal_road_catenary_layers_join_global_sort() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        m5: 0x0A, // ROAD_X
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_track_bits_on_tile(tile, 0x0A);
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
    map.set_tile(coord, tile)
        .expect("calle X con carretera y tranvía electrificados");

    // El catálogo vanilla sólo marca el tranvía; habilitar también el road
    // conserva la secuencia nativa completa carretera → tranvía y prueba que
    // los ocho `AddSortableSpriteToDraw` no comparten un ordinal.
    let mut road_catalog = vanilla_road_type_catalog();
    road_catalog
        .iter_mut()
        .find(|def| def.id == RoadType::ROAD)
        .expect("road type vanilla")
        .flags = 1;

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    8,
                    8,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    false,
                    false,
                    &road_catalog,
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("catenaria vial global");

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [6071, 6043]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    let west = ParentSpriteBounds::new(31, 16, 0, 31, 16, 1);
    let north = ParentSpriteBounds::new(16, 16, 0, 16, 16, 1);
    let east = ParentSpriteBounds::new(16, 31, 0, 16, 31, 1);
    let front = ParentSpriteBounds::new(16, 16, 2, 31, 31, 2);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (6071, west, viewport_insertion_key(1, 1, 4)),
            (6071, north, viewport_insertion_key(1, 1, 5)),
            (6071, east, viewport_insertion_key(1, 1, 6)),
            (6043, front, viewport_insertion_key(1, 1, 7)),
            (6071, west, viewport_insertion_key(1, 1, 8)),
            (6071, north, viewport_insertion_key(1, 1, 9)),
            (6071, east, viewport_insertion_key(1, 1, 10)),
            (6043, front, viewport_insertion_key(1, 1, 11)),
        ],
        "cada recorte de road/tram debe entrar al compositor global en orden C++"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "cada parent conserva su profundidad fuente antes de reordenarse"
    );
}

#[test]
fn normal_road_newgrf_catenary_layers_join_global_sort() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        m5: 0x0A, // ROAD_X
        ..tile_template()
    };
    tile = openttdrs_core::set_road_type_on_tile(tile, RoadType::from_u8(2));
    map.set_tile(coord, tile)
        .expect("calle NewGRF electrificada");

    // Un ancho de 64 y `x_offs=-32` deja píxeles en los tres recortes de
    // `DrawRoadTypeCatenary`, a diferencia de un icono pequeño que podría
    // desaparecer completamente de uno de los sub-sprites.
    let back = DecodedSprite {
        width: 64,
        height: 16,
        x_offs: -32,
        y_offs: -8,
        rgba: [0, 255, 0, 255].repeat(64 * 16),
        mask: Vec::new(),
    };
    let front = DecodedSprite {
        width: 64,
        height: 16,
        x_offs: -32,
        y_offs: -8,
        rgba: [255, 0, 0, 255].repeat(64 * 16),
        mask: Vec::new(),
    };
    let mut graphics = TrainSpriteGraphics {
        sets: vec![vec![back], vec![front]],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 0,
        }],
        ..TrainSpriteGraphics::default()
    };
    graphics.specific_assigns.insert((0, 5), 0); // ROTSG_CATENARY_BACK
    graphics.specific_assigns.insert((0, 4), 1); // ROTSG_CATENARY_FRONT
    let road_catalog = vec![RoadTypeDef {
        id: RoadType::from_u8(2),
        class: RoadTramType::Road,
        label: "Catenaria NewGRF".into(),
        short_label: "NCAT".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 1,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: false,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(graphics)),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    }];

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    8,
                    8,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    false,
                    false,
                    &road_catalog,
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("catenaria NewGRF global");

    let catenary: Vec<_> = world
        .query::<(&ViewportSortableParent, &Sprite, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, sprite, transform)| {
            [6071, 6043].contains(&parent.sprite_id).then_some((
                *parent,
                sprite.image.clone(),
                transform.translation.z,
            ))
        })
        .collect();
    assert_eq!(
        catenary.len(),
        4,
        "tres recortes traseros y un frente custom"
    );
    assert!(
        catenary
            .iter()
            .all(|(parent, _, depth)| parent.source_depth == *depth),
        "los grupos custom conservan la profundidad fuente del parent"
    );
    let colours: Vec<_> = catenary
        .iter()
        .filter_map(|(parent, image, _)| {
            let rgba = world
                .resource::<Assets<Image>>()
                .get(image)?
                .data
                .as_deref()?;
            Some((parent.sprite_id, rgba.get(0..4)?.to_vec()))
        })
        .collect();
    assert_eq!(
        colours
            .iter()
            .filter(|(_, rgba)| rgba.as_slice() == [0, 255, 0, 255])
            .count(),
        3,
        "los tres parents traseros deben conservar el grupo NewGRF"
    );
    assert_eq!(
        colours
            .iter()
            .filter(|(_, rgba)| rgba.as_slice() == [255, 0, 0, 255])
            .count(),
        1,
        "el parent frontal debe conservar su grupo NewGRF distinto"
    );
}

#[test]
fn normal_road_overlay_groups_replace_default_surface() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        m5: 0x0A, // ROAD_X
        ..tile_template()
    };
    tile = openttdrs_core::set_road_type_on_tile(tile, RoadType::from_u8(2));
    map.set_tile(coord, tile)
        .expect("calle con grupos GROUND/OVERLAY");

    let ground = DecodedSprite {
        width: 8,
        height: 8,
        x_offs: 0,
        y_offs: 0,
        rgba: [255, 0, 0, 255].repeat(8 * 8),
        mask: Vec::new(),
    };
    let overlay = DecodedSprite {
        rgba: [0, 0, 255, 255].repeat(8 * 8),
        ..ground.clone()
    };
    let road_catalog = vec![RoadTypeDef {
        id: RoadType::from_u8(2),
        class: RoadTramType::Road,
        label: "Overlay road".into(),
        short_label: "OVLY".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 0,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: false,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(TrainSpriteGraphics {
            sets: vec![vec![ground.clone()], vec![overlay.clone()]],
            specific_assigns: [((0, 2), 0), ((0, 1), 1)].into_iter().collect(),
            ..Default::default()
        })),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    }];

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    8,
                    8,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    false,
                    false,
                    &road_catalog,
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("road overlay groups spawn");

    let custom_colours: Vec<Vec<u8>> = world
        .query::<&Sprite>()
        .iter(&world)
        .filter_map(|sprite| {
            if sprite.image.path().is_some() {
                return None;
            }
            world
                .resource::<Assets<Image>>()
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                .and_then(|rgba| rgba.get(0..4))
                .filter(|rgba| **rgba == [255, 0, 0, 255] || **rgba == [0, 0, 255, 255])
                .map(<[u8]>::to_vec)
        })
        .collect();
    assert_eq!(
        custom_colours,
        vec![vec![255, 0, 0, 255], vec![0, 0, 255, 255]],
        "GROUND y OVERLAY deben llegar a la calle en el orden de DrawRoadOverlays"
    );
}

#[test]
fn pure_tram_overlay_groups_replace_default_surface() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        m5: 0,    // tranvía puro: no hay roadbits
        m3: 0x0A, // ROAD_X como trazado de tranvía
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::from_u8(2)));
    map.set_tile(coord, tile)
        .expect("tesela de tranvía puro con grupos GROUND/OVERLAY");

    let ground = DecodedSprite {
        width: 8,
        height: 8,
        x_offs: 0,
        y_offs: 0,
        rgba: [255, 0, 0, 255].repeat(8 * 8),
        mask: Vec::new(),
    };
    let overlay = DecodedSprite {
        rgba: [0, 0, 255, 255].repeat(8 * 8),
        ..ground.clone()
    };
    let road_catalog = vec![RoadTypeDef {
        id: RoadType::from_u8(2),
        class: RoadTramType::Tram,
        label: "Overlay tram".into(),
        short_label: "OVTR".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 0,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: true,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(TrainSpriteGraphics {
            sets: vec![vec![ground.clone()], vec![overlay.clone()]],
            specific_assigns: [((0, 2), 0), ((0, 1), 1)].into_iter().collect(),
            ..Default::default()
        })),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    }];

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    8,
                    8,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    false,
                    false,
                    &road_catalog,
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("pure tram overlay groups spawn");

    let custom_colours: Vec<Vec<u8>> = world
        .query::<&Sprite>()
        .iter(&world)
        .filter_map(|sprite| {
            if sprite.image.path().is_some() {
                return None;
            }
            world
                .resource::<Assets<Image>>()
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                .and_then(|rgba| rgba.get(0..4))
                .filter(|rgba| **rgba == [255, 0, 0, 255] || **rgba == [0, 0, 255, 255])
                .map(<[u8]>::to_vec)
        })
        .collect();
    assert_eq!(
        custom_colours,
        vec![vec![255, 0, 0, 255], vec![0, 0, 255, 255]],
        "el tranvía puro debe dibujar GROUND bajo OVERLAY y no la carretera vanilla"
    );
}

fn assert_drive_through_stop_overlay_groups(pure_tram: bool) {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
        m6: 3 << 3, // parada de bus pasante X
        ..tile_template()
    };
    if pure_tram {
        // `INVALID_ROADTYPE`: sólo el tramtype tiene precedencia en el
        // underlay de una parada pasante.
        tile.m3hi = 0x3F;
    } else {
        tile = openttdrs_core::set_road_type_on_tile(tile, RoadType::from_u8(2));
    }
    tile =
        openttdrs_core::set_tram_road_type_on_tile(tile, pure_tram.then_some(RoadType::from_u8(2)));
    map.set_tile(coord, tile)
        .expect("parada pasante con grupo de overlay custom");

    let ground = DecodedSprite {
        width: 8,
        height: 8,
        x_offs: 0,
        y_offs: 0,
        rgba: [255, 0, 0, 255].repeat(8 * 8),
        mask: Vec::new(),
    };
    let overlay = DecodedSprite {
        rgba: [0, 0, 255, 255].repeat(8 * 8),
        ..ground.clone()
    };
    let road_catalog = vec![RoadTypeDef {
        id: RoadType::from_u8(2),
        class: if pure_tram {
            RoadTramType::Tram
        } else {
            RoadTramType::Road
        },
        label: "Stop overlay".into(),
        short_label: "STOV".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 0,
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: pure_tram,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(TrainSpriteGraphics {
            sets: vec![vec![ground.clone()], vec![overlay.clone()]],
            specific_assigns: [((0, 2), 0), ((0, 1), 1)].into_iter().collect(),
            ..Default::default()
        })),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    }];
    let station = Station::new_with_kind(coord, StopKind::BusStop);
    let stations = vec![station];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_station_tile_with_world_and_road_types(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &stations,
                    4.0,
                    true,
                    &[],
                    &[],
                    &road_catalog,
                    Some(&mut cache),
                    None,
                    Some(&mut images),
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                    None,
                );
            },
        )
        .expect("road stop overlay groups spawn");

    let custom_colours: Vec<Vec<u8>> = world
        .query::<&Sprite>()
        .iter(&world)
        .filter_map(|sprite| {
            if sprite.image.path().is_some() {
                return None;
            }
            world
                .resource::<Assets<Image>>()
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                .and_then(|rgba| rgba.get(0..4))
                .filter(|rgba| **rgba == [255, 0, 0, 255] || **rgba == [0, 0, 255, 255])
                .map(<[u8]>::to_vec)
        })
        .collect();
    assert_eq!(
        custom_colours,
        vec![vec![255, 0, 0, 255], vec![0, 0, 255, 255]],
        "una parada pasante debe conservar GROUND y OVERLAY del tipo activo"
    );
}

#[test]
fn drive_through_roadtype_overlay_groups_replace_station_surface() {
    assert_drive_through_stop_overlay_groups(false);
}

#[test]
fn drive_through_pure_tram_overlay_groups_replace_station_surface() {
    assert_drive_through_stop_overlay_groups(true);
}

#[test]
fn road_stop_no_catenary_flag_suppresses_road_and_tram_wires() {
    let assets = boot_assets_app();
    let expected_back = assets.rail.get(&6071).expect("catenaria trasera").clone();
    let expected_front = assets.rail.get(&6043).expect("catenaria delantera").clone();
    let stop = TileCoord::new(3, 3);
    let mut map = fresh_map8();
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
        m6: 3 << 3,
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
    map.set_tile(stop, tile).expect("parada con NoCatenary");
    let mut station = Station::new_with_kind(stop, StopKind::BusStop);
    station.road_stop_spec = Some(7);
    let stations = vec![station];
    let spec = RoadStopSpecDef {
        id: 7,
        class: 0,
        label: "Sin catenaria".into(),
        short_label: "NC".into(),
        stop_type: openttdrs_core::ROADSTOP_TYPE_BUS,
        from_newgrf: true,
        grfid: 1,
        newgrf_local_id: 0,
        newgrf_grf_version: 0,
        draw_mode: openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
        random_cargo_triggers: 0,
        flags: openttdrs_core::ROADSTOP_FLAG_NO_CATENARY,
        build_cost_multiplier: 16,
        clear_cost_multiplier: 16,
        bridgeable_info: [openttdrs_core::road_stop_spec::RoadStopBridgeableInfo::default();
            openttdrs_core::road_stop_spec::ROADSTOP_LAYOUT_COUNT],
        callback_mask: 0,
        animation_status: 0xFF,
        animation_frames: 0,
        animation_speed: 2,
        animation_triggers: 0,
        newgrf_views: Vec::new(),
        newgrf_runtime: None,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
    };
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    &stations,
                    4.0,
                    true,
                    &[],
                    std::slice::from_ref(&spec),
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("parada sin catenaria");
    assert_eq!(
        world
            .query::<&Sprite>()
            .iter(&world)
            .filter(|sprite| expected_back.matches(sprite) || expected_front.matches(sprite))
            .count(),
        0,
        "NoCatenary debe bloquear ambos tipos de cable"
    );
}

#[test]
fn drive_through_waypoint_and_road_depot_grounds_keep_opengfx_xrel_center() {
    let assets = boot_assets_app();
    let drive_through_ground =
        assets.road_paved[crate::sprites::road_flat_sprite_index(0, 0x0A)].clone();
    // `m5` conserva `GetDriveThroughStopAxis` y los bits 2..3 de m3 son
    // `Roadside::Paved`, igual que un waypoint vial recién construido.
    let waypoint_ground =
        assets.road_paved[crate::sprites::road_flat_sprite_index(0, 0x0A)].clone();
    let waypoint_x_w = assets.road_waypoint[2].clone();
    let waypoint_x_e = assets.road_waypoint[3].clone();
    let depot_ground = assets.road_depot_ground.clone();
    let drive_through = TileCoord::new(2, 2);
    let waypoint = TileCoord::new(4, 2);
    let depot = TileCoord::new(6, 2);
    let mut map = fresh_map8();
    map.set_tile(
        drive_through,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
            m6: 3 << 3, // StationType::Bus.
            ..tile_template()
        },
    )
    .expect("drive-through bus stop");
    map.set_tile(
        waypoint,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m3: 0x08,
            m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
            m6: openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT << 3,
            ..tile_template()
        },
    )
    .expect("road waypoint");
    map.set_tile(
        depot,
        Tile {
            kind: TileKind::RoadDepot,
            mapt: 0x20,
            ..tile_template()
        },
    )
    .expect("road depot");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for coord in [drive_through, waypoint] {
                    spawn_station_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("positive x"),
                            u32::try_from(coord.y).expect("positive y"),
                        ),
                        &[],
                        4.0,
                        true,
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        None,
                        &[],
                        None,
                        &[],
                        TEST_CLIMATE,
                        &[],
                    );
                }
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 6, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("road ground variants spawn");

    let drive_through_x = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            drive_through_ground
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("ground drive-through");
    let offset = crate::iso::GROUND_SPRITE_CENTER_X_OFFSET;
    let waypoint_x = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            waypoint_ground
                .matches(sprite)
                .then_some(transform.translation.x)
                .filter(|x| {
                    (*x - (crate::iso::iso(waypoint.x, waypoint.y).x + offset)).abs() < 0.01
                })
        })
        .expect("ground waypoint");
    let depot_x = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            depot_ground
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("ground depot");
    assert_eq!(
        drive_through_x,
        crate::iso::iso(drive_through.x, drive_through.y).x + offset
    );
    assert_eq!(
        waypoint_x,
        crate::iso::iso(waypoint.x, waypoint.y).x + offset
    );
    assert_eq!(depot_x, crate::iso::iso(depot.x, depot.y).x + offset);
    assert_eq!(
        world
            .query::<&Sprite>()
            .iter(&world)
            .filter(|sprite| {
                sprite
                    .texture_atlas
                    .as_ref()
                    .is_some_and(|atlas| atlas.index == waypoint_x_w.atlas.index)
            })
            .count(),
        1,
        "el waypoint X debe dibujar su poste oeste vanilla"
    );
    assert_eq!(
        world
            .query::<&Sprite>()
            .iter(&world)
            .filter(|sprite| {
                sprite
                    .texture_atlas
                    .as_ref()
                    .is_some_and(|atlas| atlas.index == waypoint_x_e.atlas.index)
            })
            .count(),
        1,
        "el waypoint X debe dibujar su poste este vanilla"
    );
}

#[test]
fn sloped_road_waypoint_levels_ground_and_attaches_surface_to_foundation() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let coord = TileCoord::new(1, 1);
    // NE slope: DrawTile_Station must replace it with a flat surface before
    // drawing the road waypoint, while m5 still carries the drive-through X
    // axis and m3 bits 2..3 carry the roadside decoration.
    map.set_height(coord, 5).expect("waypoint height");
    for neighbour in [
        TileCoord::new(0, 1),
        TileCoord::new(1, 0),
        TileCoord::new(2, 1),
        TileCoord::new(1, 2),
    ] {
        map.set_height(neighbour, 4).expect("neighbour height");
    }
    let mut tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m3: 0x04, // Roadside::Grass.
        m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
        m6: openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT << 3,
        ..tile_template()
    };
    tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
    map.set_tile(coord, tile).expect("sloped road waypoint");

    let grid = RenderGrid::from_map(&map, 3, 3);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    assert_ne!(ctx.info.tileh, 0, "the fixture must remain sloped");
    let expected_surface_base_z = ctx.info.base_z.saturating_add(1);
    let expected_ground = assets
        .road_flat
        .get(crate::sprites::road_flat_sprite_index(0, 0x0A))
        .expect("waypoint road ground")
        .clone();
    let expected_waypoint_posts = [
        assets.road_waypoint[2].atlas.index,
        assets.road_waypoint[3].atlas.index,
    ];

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("sloped road waypoint");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(
        !foundation_parents.is_empty(),
        "una pendiente de waypoint debe materializar DrawFoundation"
    );
    let catenary_z = i32::from(expected_surface_base_z) * 8;
    let mut catenary: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [6071, 6043]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    catenary.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        catenary
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                6071,
                ParentSpriteBounds::new(31, 16, catenary_z, 31, 16, catenary_z + 1),
                viewport_insertion_key(1, 1, 4),
            ),
            (
                6071,
                ParentSpriteBounds::new(16, 16, catenary_z, 16, 16, catenary_z + 1),
                viewport_insertion_key(1, 1, 5),
            ),
            (
                6071,
                ParentSpriteBounds::new(16, 31, catenary_z, 16, 31, catenary_z + 1),
                viewport_insertion_key(1, 1, 6),
            ),
            (
                6043,
                ParentSpriteBounds::new(16, 16, catenary_z + 2, 31, 31, catenary_z + 2),
                viewport_insertion_key(1, 1, 7),
            ),
        ],
        "la catenaria del waypoint nivelado conserva la superficie efectiva como parent global"
    );
    assert!(
        catenary
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "la catenaria inclinada conserva su profundidad fuente antes del sort global"
    );
    let attached_ground = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .any(|(child, sprite)| {
            foundation_parents.contains(&child.parent) && expected_ground.matches(sprite)
        });
    assert!(
        attached_ground,
        "el suelo plano del waypoint debe ser child de la fundación"
    );
    for (post_index, label) in expected_waypoint_posts.into_iter().zip(["oeste", "este"]) {
        assert!(
            world
                .query::<(&ViewportSortableChild, &Sprite)>()
                .iter(&world)
                .any(|(child, sprite)| {
                    foundation_parents.contains(&child.parent)
                        && sprite
                            .texture_atlas
                            .as_ref()
                            .is_some_and(|atlas| atlas.index == post_index)
                }),
            "el poste {label} del waypoint inclinado debe ser child de la fundación"
        );
    }
    assert!(
        world.query::<&Sprite>().iter(&world).all(|sprite| {
            !world
                .resource::<TsAssets>()
                .0
                .grass_slopes
                .iter()
                .any(|grass| grass.matches(sprite))
        }),
        "un waypoint vial inclinado no debe conservar césped inclinado"
    );
}

#[test]
fn spawn_road_rail_station_and_transport_cover_main_paths() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // Carretera plana + tranvía (m3 con track bits).
    let mut road_tram = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        m5: 0x0F,
        m3: 0x05,
        ..tile_template()
    };
    map.set_tile(c(2, 2), road_tram).expect("tile");

    // Cruce a nivel: subtipo Crossing en bits 6–7 de m5.
    road_tram.m5 = 0x4F;
    road_tram.m3 = 0;
    map.set_tile(c(2, 3), road_tram).expect("tile");

    // Vía con señales (bits 6–7 = señales) + track bits bajos.
    let mut rail_sig = Tile {
        kind: TileKind::Rail,
        mapt: 0x10,
        m5: ((RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS) << 6) | 0x05,
        m2: 0x10,
        m3: 0x20,
        m3hi: 0x40,
        ..tile_template()
    };
    map.set_tile(c(3, 2), rail_sig).expect("tile");

    // Vía simple (sin señales).
    rail_sig.m5 = 0x05;
    map.set_tile(c(3, 3), rail_sig).expect("tile");

    // Estación.
    let mut st = tile_template();
    st.kind = TileKind::Station;
    st.m5 = 0x02;
    map.set_tile(c(4, 2), st).expect("tile");

    // Depósitos / túneles / puentes.
    for (coord, kind) in [
        (c(5, 2), TileKind::RoadDepot),
        (c(5, 3), TileKind::RailDepot),
        (c(5, 4), TileKind::RoadTunnel),
        (c(5, 5), TileKind::RailTunnel),
        (c(5, 6), TileKind::RoadBridge),
        (c(5, 7), TileKind::RailBridge),
    ] {
        let mut t = tile_template();
        t.kind = kind;
        t.m5 = 0x01;
        map.set_tile(coord, t).expect("tile");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                let mut rails = Vec::new();
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    TEST_CLIMATE,
                    true,
                    true,
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 3),
                    4.0,
                    TEST_CLIMATE,
                    true,
                    true,
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    4.0,
                    &mut rails,
                    TEST_CLIMATE,
                    true,
                    true,
                    false,
                    &[],
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &openttdrs_core::RailTypeRuntimeProps::defaults(),
                    None,
                    &[],
                    &[],
                    None,
                    None,
                    0,
                    &[],
                );
                rails.clear();
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    4.0,
                    &mut rails,
                    TEST_CLIMATE,
                    true,
                    true,
                    false,
                    &[],
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &openttdrs_core::RailTypeRuntimeProps::defaults(),
                    None,
                    &[],
                    &[],
                    None,
                    None,
                    0,
                    &[],
                );
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 4, 2),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
                for (x, y) in [(5, 2), (5, 3), (5, 4), (5, 5), (5, 6), (5, 7)] {
                    spawn_transport_object_tile(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(&m.0, &g.0, x as u32, y as u32),
                        4.0,
                        false,
                        &m.0,
                        (m.0.dimensions().0, m.0.dimensions().1),
                        &[],
                        &[],
                        None,
                        &[],
                        &[],
                        None,
                        None,
                    );
                }
            },
        )
        .expect("spawn batch");
}

#[test]
fn rail_fence_and_signal_join_the_global_viewport_sorter() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let m5 = ((RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS) << 6) | RAIL_TB_X;
    // Señal X hacia SW (bit 3) y FenceNW en el nibble bajo de m3hi. Los
    // estados de señal viven en el nibble alto, por lo que ambos contratos
    // pueden coexistir en una única tesela plana.
    let m3 = 1 << 7;
    let m3hi = 2;
    let signal_sprite_id = crate::sprites::collect_signal_sprite_draws(0, m3, m3hi, m5)
        .into_iter()
        .next()
        .expect("signal X")
        .sprite_id;
    map.set_tile(
        TileCoord::new(3, 2),
        Tile {
            kind: TileKind::Rail,
            mapt: 0x10,
            m2: 0,
            m3,
            m3hi,
            m5,
            ..tile_template()
        },
    )
    .expect("rail detail tile");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut rail_layers = Vec::new();
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    4.0,
                    &mut rail_layers,
                    TEST_CLIMATE,
                    false,
                    true,
                    false,
                    &[],
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &openttdrs_core::RailTypeRuntimeProps::defaults(),
                    None,
                    &[],
                    &[],
                    None,
                    None,
                    0,
                    &[],
                );
            },
        )
        .expect("rail detail spawn");

    let parents: Vec<_> = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .copied()
        .collect();
    let fence = parents
        .iter()
        .find(|parent| parent.sprite_id == 1301)
        .expect("fence parent");
    let signal = parents
        .iter()
        .find(|parent| parent.sprite_id == signal_sprite_id)
        .expect("signal parent");
    assert_eq!(
        fence.bounds,
        ParentSpriteBounds::new(48, 33, 0, 63, 33, 3),
        "FenceNW conserva la caja _fence_offsets de DrawTrackDetails"
    );
    assert_eq!(
        signal.bounds,
        ParentSpriteBounds::new(59, 35, 0, 59, 35, 5),
        "la señal X usa SignalPositions[LEFT][8] y BB_HEIGHT_UNDER_BRIDGE"
    );
    assert!(
        fence.insertion_key < signal.insertion_key,
        "DrawTrackDetails debe conservarse antes de DrawSignals en la misma tesela"
    );
}

#[test]
fn ship_depot_uses_water_and_all_vanilla_two_tile_parts() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let depot = |m5| Tile {
        kind: TileKind::ShipDepot,
        mapt: 0x60,
        m5,
        ..tile_template()
    };
    // WaterTileType::Depot = 3 (nibble alto); bits bajos: part + axis.
    for (x, y, m5) in [(1, 1, 0x30), (2, 1, 0x31), (1, 2, 0x32), (2, 2, 0x33)] {
        map.set_tile(TileCoord::new(x, y), depot(m5))
            .expect("ship depot tile");
    }
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
                    spawn_transport_object_tile(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(&m.0, &g.0, x as u32, y as u32),
                        4.0,
                        false,
                        &m.0,
                        m.0.dimensions(),
                        &[],
                        &[],
                        None,
                        &[],
                        &[],
                        None,
                        None,
                    );
                }
            },
        )
        .expect("ship depot spawn");

    let mut water = world.query::<&crate::render::WaterTile>();
    assert_eq!(water.iter(&world).count(), 4, "cada parte conserva agua");
    let mut water_x: Vec<_> = world
        .query::<(&crate::render::WaterTile, &Transform)>()
        .iter(&world)
        .map(|(_, transform)| transform.translation.x)
        .collect();
    let mut expected_water_x: Vec<_> = [(1, 1), (2, 1), (1, 2), (2, 2)]
        .into_iter()
        .map(|(x, y)| crate::iso::iso(x, y).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET)
        .collect();
    water_x.sort_by(f32::total_cmp);
    expected_water_x.sort_by(f32::total_cmp);
    assert_eq!(
        water_x, expected_water_x,
        "cada parte del depósito naval conserva el xrel=-31 de agua plana"
    );
    let mut visuals = world.query::<&crate::render::MapVisualLayer>();
    // 4 fondos de agua + 1/2/1/2 capas de edificio para las cuatro variantes.
    assert_eq!(visuals.iter(&world).count(), 10);

    // Las seis capas BUILD no quedan relegadas al orden local de la tesela:
    // sus prismas TILE_SEQ entran al mismo sorter global que casas y puentes.
    // Los máximos son inclusivos, como `AddSortableSpriteToDraw` de OpenTTD.
    let mut parents: Vec<_> = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .map(|parent| {
            (
                parent.sprite_id,
                parent.bounds.xmin,
                parent.bounds.ymin,
                parent.bounds.zmin,
                parent.bounds.xmax,
                parent.bounds.ymax,
                parent.bounds.zmax,
            )
        })
        .collect();
    parents.sort_unstable();
    assert_eq!(
        parents,
        vec![
            (4070, 32, 31, 0, 47, 31, 19),
            (4071, 47, 32, 0, 47, 47, 19),
            (4072, 16, 31, 0, 31, 31, 19),
            (4073, 31, 32, 0, 31, 47, 19),
            (4074, 32, 16, 0, 47, 16, 19),
            (4075, 32, 32, 0, 32, 47, 19),
        ],
        "las cuatro variantes conservan los bounds de OpenTTD"
    );

    // Los bounds de TILE_SEQ no describen el rectángulo del PNG. El centro
    // visual debe conservar los metadatos NFO de cada sprite, especialmente
    // en las piezas de 32x53 y en las dos fachadas de 64x64.
    let expected_nfo = [
        (
            4070, 2_i32, 1_i32, 0.0_f32, 15.0_f32, -61.0_f32, -48.0_f32, 64.0_f32, 64.0_f32,
        ),
        (4071, 2, 2, 15.0, 0.0, -1.0, -47.0, 64.0, 64.0),
        (4072, 1, 1, 0.0, 15.0, -29.0, -37.0, 32.0, 53.0),
        (4073, 1, 2, 15.0, 0.0, -1.0, -36.0, 32.0, 53.0),
        (4074, 2, 1, 0.0, 0.0, -31.0, 2.0, 14.0, 13.0),
        (4075, 2, 2, 0.0, 0.0, 19.0, 3.0, 14.0, 13.0),
    ];
    let mut parent_transforms = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .map(|(parent, transform)| (parent.sprite_id, transform.translation))
        .collect::<Vec<_>>();
    parent_transforms.sort_unstable_by_key(|(sprite_id, _)| *sprite_id);
    for (sprite_id, tx, ty, dx, dy, xrel, yrel, width, height) in expected_nfo {
        let local = crate::iso::remap_tile_offset(dx, dy, 0.0) * 0.5;
        let mut expected = crate::iso::overlay_pos(
            crate::iso::iso(tx, ty) + local,
            xrel,
            yrel,
            width,
            height,
            0,
            0.04,
            tx,
            ty,
        );
        expected.z = crate::render::viewport_source_depth(expected.z, tx as u32, 4);
        let actual = parent_transforms
            .iter()
            .find_map(|(id, transform)| (*id == sprite_id).then_some(*transform))
            .expect("sprite de depósito naval");
        assert_eq!(
            (actual.x, actual.y),
            (expected.x, expected.y),
            "sprite {sprite_id} debe conservar su ancla y tamaño NFO"
        );
    }
}

#[test]
fn canal_ship_depot_draws_dikes_with_active_nfo_anchors() {
    let assets = boot_assets_app();
    let dike_assets = assets.canal_dikes.clone();
    let depot = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    let mut tile = tile_template();
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60;
    tile.m5 = 0x30;
    tile.m1 = set_water_class_m1(tile.m1, WaterClass::Canal);
    map.set_tile(depot, tile).expect("canal ship depot");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("canal ship depot spawn");

    assert_eq!(
        world.query::<&MapVisualLayer>().iter(&world).count(),
        10,
        "agua + ocho diques + la capa norte del depósito"
    );
    let rendered: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .collect();
    for slot in 0..8 {
        let (width, height, xrel, yrel) = WATER_CANAL_DIKE_SPRITE_META[slot];
        let layer = 0.010 + slot as f32 * 0.0001;
        let mut expected = crate::iso::overlay_pos(
            crate::iso::iso(1, 1),
            f32::from(xrel),
            f32::from(yrel),
            f32::from(width),
            f32::from(height),
            0,
            layer,
            1,
            1,
        );
        expected.z = ground_draw_z(1, 1, layer);
        let actual = rendered
            .iter()
            .find_map(|(sprite, transform)| {
                dike_assets[slot]
                    .matches(sprite)
                    .then_some(transform.translation)
            })
            .unwrap_or_else(|| panic!("falta dique slot {slot}"));
        assert_eq!(
            actual, expected,
            "dique slot {slot} debe conservar la ancla NFO y el pase ground"
        );
    }
}

#[test]
fn canal_ship_depot_parts_suppress_the_shared_dike_edge() {
    let assets = boot_assets_app();
    let dike_assets = assets.canal_dikes.clone();
    let mut map = Map::new_flat(4, 4, 0);
    for (coord, m5) in [
        (TileCoord::new(1, 1), 0x30), // eje X, parte norte
        (TileCoord::new(2, 1), 0x31), // eje X, parte sur
    ] {
        let mut tile = tile_template();
        tile.kind = TileKind::ShipDepot;
        tile.mapt = 0x60;
        tile.m5 = m5;
        tile.m1 = set_water_class_m1(tile.m1, WaterClass::Canal);
        map.set_tile(coord, tile).expect("canal ship depot part");
    }
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for (x, y) in [(1, 1), (2, 1)] {
                    spawn_transport_object_tile(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(&m.0, &g.0, x, y),
                        4.0,
                        false,
                        &m.0,
                        m.0.dimensions(),
                        &[],
                        &[],
                        None,
                        &[],
                        &[],
                        None,
                        None,
                    );
                }
            },
        )
        .expect("two-part canal ship depot spawn");

    let sprites: Vec<_> = world.query::<&Sprite>().iter(&world).collect();
    let dike_counts: Vec<_> = dike_assets
        .iter()
        .map(|asset| {
            sprites
                .iter()
                .filter(|sprite| asset.matches(sprite))
                .count()
        })
        .collect();

    // Parte norte: slots 0,1,3,4,7. Parte sur: slots 1,2,3,5,6.
    // Los slots 2/0 son los dos lados del contacto y no se dibujan.
    assert_eq!(dike_counts, vec![1, 2, 1, 2, 1, 1, 1, 1, 0, 0, 0, 0]);
    assert_eq!(
        world.query::<&MapVisualLayer>().iter(&world).count(),
        15,
        "dos aguas, diez diques exteriores y tres capas del depósito"
    );
}

#[test]
fn canal_ship_depot_consumes_action5_dike_sprite_and_nfo_anchor() {
    let assets = boot_assets_app();
    let depot = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    let mut tile = tile_template();
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60;
    tile.m5 = 0x30;
    tile.m1 = set_water_class_m1(tile.m1, WaterClass::Canal);
    map.set_tile(depot, tile).expect("canal ship depot");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let custom = DecodedSprite {
        width: 13,
        height: 5,
        x_offs: 8,
        y_offs: -3,
        rgba: vec![0xFF; 13 * 5 * 4],
        mask: Vec::new(),
    };
    let mut canal_action5 = vec![None; 65];
    canal_action5[52] = Some(custom);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types_and_tramway_action5(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    &[],
                    &canal_action5,
                    Some(&mut action5_sprites),
                    Some(&mut images),
                    &[],
                    &[],
                    TramwayDepotAction5::default(),
                );
            },
        )
        .expect("canal ship depot Action5 spawn");

    let sprites: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .map(|(sprite, transform)| (sprite.clone(), transform.translation))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let (_, actual) = sprites
        .iter()
        .find(|(sprite, _)| sprite.texture_atlas.is_none() && images.get(&sprite.image).is_some())
        .expect("dique Action5 materializado");
    let mut expected =
        crate::iso::overlay_pos(crate::iso::iso(1, 1), 8.0, -3.0, 13.0, 5.0, 0, 0.010, 1, 1);
    expected.z = ground_draw_z(1, 1, 0.010);
    assert_eq!(*actual, expected, "el slot 52 conserva el ancla NFO");
}

#[test]
fn canal_ship_depot_consumes_feature_ground_and_dike_views() {
    let assets = boot_assets_app();
    let depot = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    let mut tile = tile_template();
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60;
    tile.m5 = 0x30;
    tile.m1 = set_water_class_m1(tile.m1, WaterClass::Canal);
    map.set_tile(depot, tile).expect("canal ship depot");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let surface = DecodedSprite {
        width: 10,
        height: 6,
        x_offs: -3,
        y_offs: -5,
        rgba: vec![0x44; 10 * 6 * 4],
        mask: Vec::new(),
    };
    let dike = DecodedSprite {
        width: 13,
        height: 5,
        x_offs: 8,
        y_offs: -3,
        rgba: vec![0x55; 13 * 5 * 4],
        mask: Vec::new(),
    };
    let mut features = openttdrs_core::vanilla_canal_feature_catalog();
    features[usize::from(openttdrs_core::CF_WATERSLOPE)].flags =
        openttdrs_core::CFF_HAS_FLAT_SPRITE;
    features[usize::from(openttdrs_core::CF_WATERSLOPE)].newgrf_views = vec![surface.clone()];
    features[usize::from(openttdrs_core::CF_DIKES)].newgrf_views = vec![dike.clone(); 12];
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types_and_tramway_action5(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    &features,
                    &[],
                    Some(&mut action5_sprites),
                    Some(&mut images),
                    &[],
                    &[],
                    TramwayDepotAction5::default(),
                );
            },
        )
        .expect("canal ship depot feature spawn");

    let water: Vec<_> = world
        .query::<(&WaterTile, &Sprite, &Transform)>()
        .iter(&world)
        .map(|(marker, sprite, transform)| (*marker, sprite.clone(), *transform))
        .collect();
    let sprites: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .map(|(sprite, transform)| (sprite.clone(), *transform))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let (marker, _, surface_transform) = water
        .iter()
        .find(|(_, sprite, _)| {
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                == Some(surface.rgba.as_slice())
        })
        .expect("ground CF_WATERSLOPE del depósito");
    assert!(!marker.is_palette_animated());
    let mut expected_surface = overlay_pos(
        crate::iso::iso(1, 1),
        -3.0,
        -5.0,
        10.0,
        6.0,
        0,
        FLAT_WATER_LAYER_FRAC,
        1,
        1,
    );
    expected_surface.z = ground_draw_z(1, 1, 0.0);
    assert_eq!(
        *surface_transform,
        Transform::from_translation(expected_surface)
    );
    let dike_sprites: Vec<_> = sprites
        .iter()
        .filter(|(sprite, _)| {
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                == Some(dike.rgba.as_slice())
        })
        .collect();
    assert_eq!(
        dike_sprites.len(),
        8,
        "el depósito aislado emite ocho diques"
    );
    let mut expected_dike =
        overlay_pos(crate::iso::iso(1, 1), 8.0, -3.0, 13.0, 5.0, 0, 0.010, 1, 1);
    expected_dike.z = ground_draw_z(1, 1, 0.010);
    assert!(
        dike_sprites
            .iter()
            .any(|(_, transform)| { *transform == Transform::from_translation(expected_dike) })
    );
    assert_eq!(images.len(), 9, "ground y ocho slots de dike custom");
}

#[test]
fn river_ship_depot_uses_static_slope_ground_before_depot_layers() {
    let assets = boot_assets_app();
    let river_asset = assets.river_slopes[1].clone(); // SPR_WATER_SLOPE_X_DOWN.
    let depot = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    let mut tile = tile_template();
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60;
    tile.m5 = 0x30;
    tile.m1 = set_water_class_m1(tile.m1, WaterClass::River);
    map.set_tile(depot, tile).expect("river ship depot");
    map.set_height(TileCoord::new(1, 1), 1)
        .expect("north height");
    map.set_height(TileCoord::new(1, 2), 1)
        .expect("east height");

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("river ship depot spawn");

    let mut waters = world.query::<(&WaterTile, &Sprite)>();
    let (marker, sprite) = waters.iter(&world).next().expect("river depot ground");
    assert!(!marker.is_palette_animated());
    assert!(
        river_asset.matches(sprite),
        "el depósito conserva X_DOWN de río"
    );
}

#[test]
fn river_ship_depot_consumes_flat_feature_ground_in_ground_pass() {
    let assets = boot_assets_app();
    let depot = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    let mut tile = tile_template();
    tile.kind = TileKind::ShipDepot;
    tile.mapt = 0x60;
    tile.m5 = 0x30;
    tile.m1 = set_water_class_m1(tile.m1, WaterClass::River);
    map.set_tile(depot, tile).expect("river ship depot");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let surface = DecodedSprite {
        width: 10,
        height: 6,
        x_offs: -3,
        y_offs: -5,
        rgba: vec![0x66; 10 * 6 * 4],
        mask: Vec::new(),
    };
    let mut features = openttdrs_core::vanilla_canal_feature_catalog();
    features[usize::from(openttdrs_core::CF_RIVER_SLOPE)].flags =
        openttdrs_core::CFF_HAS_FLAT_SPRITE;
    features[usize::from(openttdrs_core::CF_RIVER_SLOPE)].newgrf_views = vec![surface.clone()];
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());

    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut action5_sprites: Local<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types_and_tramway_action5(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    &features,
                    &[],
                    Some(&mut action5_sprites),
                    Some(&mut images),
                    &[],
                    &[],
                    TramwayDepotAction5::default(),
                );
            },
        )
        .expect("river ship depot feature spawn");

    let water: Vec<_> = world
        .query::<(&WaterTile, &Sprite, &Transform)>()
        .iter(&world)
        .map(|(marker, sprite, transform)| (*marker, sprite.clone(), *transform))
        .collect();
    assert_eq!(water.len(), 1, "River plano no agrega bordes ni diques");
    let images = world.resource::<Assets<Image>>();
    let (_, _, surface_transform) = water
        .iter()
        .find(|(_, sprite, _)| {
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref())
                == Some(surface.rgba.as_slice())
        })
        .expect("ground CF_RIVER_SLOPE plano del depósito");
    let mut expected_surface = overlay_pos(
        crate::iso::iso(1, 1),
        -3.0,
        -5.0,
        10.0,
        6.0,
        0,
        FLAT_WATER_LAYER_FRAC,
        1,
        1,
    );
    expected_surface.z = ground_draw_z(1, 1, 0.0);
    assert_eq!(
        *surface_transform,
        Transform::from_translation(expected_surface),
        "el ground River custom conserva el pase y el ancla NFO"
    );
    assert_eq!(images.len(), 1, "sólo se materializa el ground custom");
}

#[test]
fn forest_combined_layers_attach_to_the_global_sort_parent() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let mut tree = tile_template();
    tree.kind = TileKind::Forest;
    tree.mapt = 0x40;
    // Tres capas: la primera es parent y las otras dos son CombinedSprite.
    tree.m5 = 0x80;
    map.set_tile(TileCoord::new(1, 1), tree)
        .expect("forest tile");
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                push_forest_tree(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    m.0.dimensions().0,
                );
            },
        )
        .expect("forest spawn");

    let mut parent_query = world.query::<(Entity, &ViewportSortableParent)>();
    let parents: Vec<_> = parent_query.iter(&world).collect();
    assert_eq!(parents.len(), 1, "el árbol comienza un parent sortable");
    let (parent_entity, parent) = parents[0];
    assert_eq!(
        (
            parent.bounds.xmin,
            parent.bounds.ymin,
            parent.bounds.zmin,
            parent.bounds.xmax,
            parent.bounds.ymax,
            parent.bounds.zmax,
        ),
        (16, 16, 0, 31, 31, 47)
    );

    let mut child_query = world.query::<&ViewportSortableChild>();
    let children: Vec<_> = child_query.iter(&world).collect();
    assert_eq!(children.len(), 2, "las capas combinadas siguen al parent");
    assert!(
        children.iter().all(|child| child.parent == parent_entity),
        "ninguna copa combinada puede quedar con profundidad independiente"
    );
}

#[test]
fn sloped_house_ground_attaches_to_the_last_foundation_parent() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let mut house = tile_template();
    house.kind = TileKind::House;
    // Primera entrada vanilla: tiene `s1` y `s2`, sin ascensor adicional.
    house.m8 = 0;
    map.set_tile(TileCoord::new(1, 1), house)
        .expect("house tile");
    // Esquina oeste elevada: `tileh = SLOPE_W`, por lo que DrawTile_Town
    // fuerza `DrawFoundation(Leveled)` antes del suelo de la casa.
    map.set_height(TileCoord::new(2, 1), 1)
        .expect("west corner height");
    let grid = RenderGrid::from_map(&map, 3, 3);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_house_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    HouseSpawnResources {
                        map: &m.0,
                        map_dims: m.0.dimensions(),
                        house_catalog: &[],
                        house_counts: None,
                        towns: &[],
                        climate: openttdrs_core::Climate::Temperate,
                        newgrf_stack: &[],
                        foundation_newgrf: &[],
                        house_sprites: None,
                        action5_sprites: None,
                        images: None,
                    },
                );
            },
        )
        .expect("sloped house spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(
        !foundation_parents.is_empty(),
        "la pendiente debe materializar el parent de DrawFoundation"
    );

    let mut children = world.query::<(Entity, &ViewportSortableChild, &Transform)>();
    let attached: Vec<_> = children
        .iter(&world)
        .filter(|(_, child, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        1,
        "el ground de la casa debe seguir al último parent de la fundación"
    );
    let (_, child, transform) = attached[0];
    assert_eq!(child.source_depth, transform.translation.z);
}

#[test]
fn flat_house_ground_stays_in_the_dedicated_ground_pass() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let mut house = tile_template();
    house.kind = TileKind::House;
    // La entrada vanilla inicial tiene el par s1=1422/s2=1423.
    house.m8 = 0;
    map.set_tile(TileCoord::new(1, 1), house)
        .expect("house tile");
    let grid = RenderGrid::from_map(&map, 3, 3);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_house_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    HouseSpawnResources {
                        map: &m.0,
                        map_dims: m.0.dimensions(),
                        house_catalog: &[],
                        house_counts: None,
                        towns: &[],
                        climate: openttdrs_core::Climate::Temperate,
                        newgrf_stack: &[],
                        foundation_newgrf: &[],
                        house_sprites: None,
                        action5_sprites: None,
                        images: None,
                    },
                );
            },
        )
        .expect("flat house spawn");

    let building_depth = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .next()
        .map(|(_, transform)| transform.translation.z)
        .expect("building parent");
    let ground_depth = world
        .query_filtered::<&Transform, (With<MapVisualLayer>, Without<ViewportSortableParent>)>()
        .iter(&world)
        .next()
        .map(|transform| transform.translation.z)
        .expect("house s1 ground");
    assert_eq!(ground_depth, ground_draw_z(1, 1, 0.4));
    assert!(
        ground_depth < building_depth,
        "DrawGroundSprite s1 debe quedar detrás de todos los parents: ground={ground_depth}, building={building_depth}"
    );
}

#[test]
fn newgrf_house_building_uses_runtime_action2_view() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let mut house = tile_template();
    house.kind = TileKind::House;
    house.m8 = 110;
    house.m3 = 0x80;
    house.m5 = 2; // age: Action2 var 0x41 chooses the default (blue) view
    map.set_tile(TileCoord::new(1, 1), house)
        .expect("newgrf house tile");

    let solid = |r: u8, g: u8, b: u8| DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -4,
        y_offs: -8,
        rgba: vec![r, g, b, 255, r, g, b, 255, r, g, b, 255, r, g, b, 255],
        mask: Vec::new(),
    };
    let red = solid(255, 0, 0);
    let blue = solid(0, 0, 255);
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![red.clone()], vec![blue.clone()]],
        assigns: vec![TrainSpriteAssign {
            local_id: 3,
            set_id: 4,
        }],
        ..Default::default()
    };
    runtime.action2_var.insert(
        4,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x41,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: 0xFF,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(7, 1, 1)],
            default: 8,
        },
    );
    runtime.action2_to_action1.insert(7, 0);
    runtime.action2_to_action1.insert(8, 1);
    let house_def = HouseSpecDef {
        id: 110,
        local_id: 3,
        subst_id: 0,
        building_flags: openttdrs_core::house_spec::BUILDING_FLAG_SIZE_1X1,
        min_year: 0,
        max_year: 5000,
        population: 1,
        mail_generation: 1,
        availability: openttdrs_core::DEFAULT_HOUSE_AVAILABILITY,
        probability: openttdrs_core::DEFAULT_HOUSE_PROBABILITY,
        processing_time: 0,
        extra_flags: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        override_id: None,
        callback_mask: 0,
        name: "runtime house".into(),
        from_newgrf: true,
        grfid: 0,
        newgrf_views: vec![red, blue],
        newgrf_local_id: 3,
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let grid = RenderGrid::from_map(&map, 3, 3);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfHouseSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfHouseSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_house_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    HouseSpawnResources {
                        map: &m.0,
                        map_dims: m.0.dimensions(),
                        house_catalog: std::slice::from_ref(&house_def),
                        house_counts: None,
                        towns: &[],
                        climate: TEST_CLIMATE,
                        newgrf_stack: &[],
                        foundation_newgrf: &[],
                        house_sprites: Some(&mut cache),
                        action5_sprites: None,
                        images: Some(&mut images),
                    },
                );
            },
        )
        .expect("newgrf house spawn");

    let (sprite, parent) = world
        .query::<(&Sprite, &ViewportSortableParent)>()
        .iter(&world)
        .find(|(_, parent)| parent.sprite_id == 110)
        .expect("runtime NewGRF house parent");
    let image = world
        .resource::<Assets<Image>>()
        .get(&sprite.image)
        .expect("runtime house image");
    assert_eq!(
        image.data.as_deref().and_then(|rgba| rgba.get(2)),
        Some(&255)
    );
    assert_eq!(parent.bounds.xmin, 12);
    assert_eq!(parent.bounds.ymin, 8);
}

#[test]
fn complete_newgrf_house_tile_layout_keeps_ground_in_ground_pass_and_build_in_global_sort() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let first = TileCoord::new(1, 1);
    let second = TileCoord::new(2, 1);
    let mut map = Map::new_flat(4, 4, 0);
    for coord in [first, second] {
        map.set_tile(
            coord,
            Tile {
                kind: TileKind::House,
                m8: 110,
                m3: 0x80,
                m5: 2,
                ..tile_template()
            },
        )
        .expect("newgrf house TileLayout tile");
    }

    let ground_rgba = [240, 10, 10, 255].repeat(4);
    let parent_rgba = [10, 240, 10, 255].repeat(4);
    let child_rgba = [10, 10, 240, 255].repeat(4);
    let ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: ground_rgba.clone(),
        mask: Vec::new(),
    };
    let parent = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -3,
        y_offs: -4,
        rgba: parent_rgba.clone(),
        mask: Vec::new(),
    };
    let child = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -5,
        y_offs: -6,
        rgba: child_rgba.clone(),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![ground], vec![parent], vec![child]],
        assigns: vec![TrainSpriteAssign {
            local_id: 3,
            set_id: 9,
        }],
        ..Default::default()
    };
    runtime.tile_layouts.insert(
        9,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(0),
                ..Default::default()
            },
            sequence: vec![
                TileLayoutSpriteRef {
                    action1_set: Some(1),
                    origin: [1, 2, 3],
                    extent: [4, 5, 6],
                    ..Default::default()
                },
                TileLayoutSpriteRef {
                    action1_set: Some(2),
                    origin: [7, -4, i8::MIN],
                    ..Default::default()
                },
            ],
        },
    );
    let house_def = HouseSpecDef {
        id: 110,
        local_id: 3,
        subst_id: 0,
        building_flags: openttdrs_core::house_spec::BUILDING_FLAG_SIZE_1X1,
        min_year: 0,
        max_year: 5000,
        population: 1,
        mail_generation: 1,
        availability: openttdrs_core::DEFAULT_HOUSE_AVAILABILITY,
        probability: openttdrs_core::DEFAULT_HOUSE_PROBABILITY,
        processing_time: 0,
        extra_flags: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        override_id: None,
        callback_mask: 0,
        name: "TileLayout house".into(),
        from_newgrf: true,
        grfid: 0x484F_5553,
        newgrf_views: Vec::new(),
        newgrf_local_id: 3,
        newgrf_runtime: Some(Box::new(runtime)),
    };

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfHouseSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world.init_resource::<ViewportSortableChildDepthWindows>();
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfHouseSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                for coord in [first, second] {
                    spawn_house_tile(
                        &mut commands,
                        &a.0,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("house x"),
                            u32::try_from(coord.y).expect("house y"),
                        ),
                        HouseSpawnResources {
                            map: &m.0,
                            map_dims: m.0.dimensions(),
                            house_catalog: std::slice::from_ref(&house_def),
                            house_counts: None,
                            towns: &[],
                            climate: TEST_CLIMATE,
                            newgrf_stack: &[],
                            foundation_newgrf: &[],
                            house_sprites: Some(&mut cache),
                            action5_sprites: None,
                            images: Some(&mut images),
                        },
                    );
                }
            },
        )
        .expect("newgrf house TileLayout spawn");

    let sprite_handles: Vec<_> = world
        .query::<&Sprite>()
        .iter(&world)
        .map(|sprite| sprite.image.clone())
        .collect();
    let (ground_handles, child_handles) = {
        let images = world.resource::<Assets<Image>>();
        let handles_for = |rgba: &[u8]| {
            sprite_handles
                .iter()
                .filter(|handle| {
                    images.get(*handle).and_then(|image| image.data.as_deref()) == Some(rgba)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        (handles_for(&ground_rgba), handles_for(&child_rgba))
    };
    assert_eq!(ground_handles.len(), 2, "un ground por tesela TileLayout");
    assert_eq!(child_handles.len(), 2, "un child por tesela TileLayout");
    let mut ground_depths: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .filter_map(|(sprite, transform)| {
            ground_handles
                .contains(&sprite.image)
                .then_some(transform.translation.z)
        })
        .collect();
    ground_depths.sort_by(f32::total_cmp);
    let mut expected_ground_depths = vec![
        ground_draw_z(first.x, first.y, 0.4),
        ground_draw_z(second.x, second.y, 0.4),
    ];
    expected_ground_depths.sort_by(f32::total_cmp);
    assert_eq!(
        ground_depths, expected_ground_depths,
        "TileLayout ground usa DrawGroundSprite y no puede entrar al pase sortable"
    );

    let mut parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter(|(_, parent)| parent.sprite_id == u32::MAX)
        .map(|(entity, parent)| (entity, *parent))
        .collect();
    parents.sort_by_key(|(_, parent)| parent.insertion_key);
    assert_eq!(parents.len(), 2, "cada BUILD debe emitir su parent");
    assert_eq!(
        parents[0].1.bounds,
        ParentSpriteBounds::new(17, 18, 3, 20, 22, 8),
        "el primer BUILD conserva el prisma TILE_SEQ_LINE inclusivo"
    );
    assert_eq!(
        parents[0].1.insertion_key,
        viewport_insertion_key(1, 1, 2),
        "el BUILD entra con su ordinal estable en el stream global"
    );
    assert!(
        world
            .query::<(&ViewportSortableChild, &Sprite)>()
            .iter(&world)
            .any(|(child_component, sprite)| {
                parents
                    .iter()
                    .any(|(entity, _)| child_component.parent == *entity)
                    && child_handles.contains(&sprite.image)
            }),
        "el child TileSeq debe continuar unido a su BUILD anterior"
    );

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            sort_viewport_sortable_parents,
            sync_viewport_sortable_children,
        )
            .chain(),
    );
    schedule.run(&mut world);

    let mut sorted_parents: Vec<_> = world
        .query::<(Entity, &Transform)>()
        .iter(&world)
        .filter(|(entity, _)| parents.iter().any(|(parent, _)| parent == entity))
        .map(|(entity, transform)| (entity, transform.translation.z))
        .collect();
    sorted_parents.sort_by(|left, right| left.1.total_cmp(&right.1));
    let (first_parent, first_parent_depth) = sorted_parents[0];
    let (_, next_parent_depth) = sorted_parents[1];
    let child_depth = world
        .query::<(&ViewportSortableChild, &Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(child_component, sprite, transform)| {
            (child_component.parent == first_parent && child_handles.contains(&sprite.image))
                .then_some(transform.translation.z)
        })
        .expect("child del primer parent global");
    assert!(
        first_parent_depth < child_depth && child_depth < next_parent_depth,
        "el child TileSeq debe quedar dentro de la ventana de su parent: parent={first_parent_depth}, child={child_depth}, next={next_parent_depth}"
    );
}

#[test]
fn newgrf_house_draw_foundations_callback_can_suppress_default() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let coord = TileCoord::new(1, 1);
    let mut house = tile_template();
    house.kind = TileKind::House;
    house.m8 = 110;
    house.m3 = 0x80;
    house.m5 = 2;
    map.set_tile(coord, house).expect("house tile");
    map.set_height(TileCoord::new(2, 1), 1)
        .expect("west corner height");
    let view = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [220, 40, 40, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut runtime = callback_literal_runtime(3, 0);
    runtime.sets = vec![vec![view.clone()]];
    let house_def = HouseSpecDef {
        id: 110,
        local_id: 3,
        subst_id: 0,
        building_flags: openttdrs_core::house_spec::BUILDING_FLAG_SIZE_1X1,
        min_year: 0,
        max_year: 5000,
        population: 1,
        mail_generation: 1,
        availability: openttdrs_core::DEFAULT_HOUSE_AVAILABILITY,
        probability: openttdrs_core::DEFAULT_HOUSE_PROBABILITY,
        processing_time: 0,
        extra_flags: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        override_id: None,
        callback_mask: openttdrs_core::HOUSE_CALLBACK_DRAW_FOUNDATIONS_MASK,
        name: "no foundation house".into(),
        from_newgrf: true,
        grfid: 0,
        newgrf_views: vec![view],
        newgrf_local_id: 3,
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let grid = RenderGrid::from_map(&map, 3, 3);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfHouseSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfHouseSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_house_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    HouseSpawnResources {
                        map: &m.0,
                        map_dims: m.0.dimensions(),
                        house_catalog: std::slice::from_ref(&house_def),
                        house_counts: None,
                        towns: &[],
                        climate: TEST_CLIMATE,
                        newgrf_stack: &[],
                        foundation_newgrf: &[],
                        house_sprites: Some(&mut cache),
                        action5_sprites: None,
                        images: Some(&mut images),
                    },
                );
            },
        )
        .expect("house draw-foundations callback");

    let foundation_count = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .filter(|parent| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
        })
        .count();
    assert_eq!(
        foundation_count, 0,
        "CB 0x30 = 0 debe suprimir la fundación"
    );
}

#[test]
fn sloped_bridge_ramp_ground_attaches_to_the_last_foundation_parent() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // Puente vial sobre el eje X. Elevar W/S/E de la rampa izquierda produce
    // una pendiente de tres esquinas altas: GetBridgeFoundation la nivela y
    // deja un sprite vanilla materializable antes de dibujar su ground.
    let mut ramp = tile_template();
    ramp.kind = TileKind::RoadBridge;
    ramp.mapt = 0x90;
    ramp.m5 = 0x86;
    map.set_tile(c(1, 1), ramp).expect("rampa oeste");
    ramp.m5 = 0x84;
    map.set_tile(c(4, 1), ramp).expect("rampa este");
    for x in 2..=3 {
        let mut water = tile_template();
        water.kind = TileKind::Water;
        water.mapt = 0x64;
        map.set_tile(c(x, 1), water).expect("vano de agua");
    }
    for corner in [c(2, 1), c(1, 2), c(2, 2)] {
        map.set_height(corner, 1)
            .expect("esquina elevada de la rampa");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let dims = m.0.dimensions();
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    false,
                    &m.0,
                    dims,
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("sloped bridge ramp spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(
        !foundation_parents.is_empty(),
        "la rampa inclinada debe materializar el parent de DrawFoundation"
    );

    let mut children = world.query::<(&ViewportSortableChild, &Transform)>();
    let attached: Vec<_> = children
        .iter(&world)
        .filter(|(child, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        1,
        "el ground de la rampa debe seguir al último parent de la fundación"
    );
    let (child, transform) = attached[0];
    assert_eq!(child.source_depth, transform.translation.z);
}

#[test]
fn sloped_rail_track_attaches_to_its_foundation_parent() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    let mut rail = tile_template();
    rail.kind = TileKind::Rail;
    rail.mapt = 0x10;
    rail.m5 = openttdrs_core::RAIL_TB_X;
    map.set_tile(c(1, 1), rail).expect("vía inclinada");
    // W/S/E elevadas fuerzan una fundación nivelada antes de DrawTrackBits.
    for corner in [c(2, 1), c(1, 2), c(2, 2)] {
        map.set_height(corner, 1)
            .expect("esquina elevada de la vía");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut rail_layers = Vec::new();
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &mut rail_layers,
                    TEST_CLIMATE,
                    false,
                    false,
                    false,
                    &[],
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &openttdrs_core::RailTypeRuntimeProps::defaults(),
                    None,
                    &[],
                    &[],
                    None,
                    None,
                    0,
                    &[],
                );
            },
        )
        .expect("sloped rail spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(
        !foundation_parents.is_empty(),
        "la vía inclinada debe materializar DrawFoundation"
    );

    let mut children = world.query::<(&ViewportSortableChild, &Transform)>();
    let attached: Vec<_> = children
        .iter(&world)
        .filter(|(child, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        1,
        "la capa de vía debe seguir al parent de DrawFoundation"
    );
    let (child, transform) = attached[0];
    assert_eq!(child.source_depth, transform.translation.z);
}

#[test]
fn sloped_road_ground_attaches_to_its_foundation_parent() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    let mut road = tile_template();
    road.kind = TileKind::Road;
    road.mapt = 0x20;
    road.m5 = 0x0F;
    map.set_tile(c(1, 1), road).expect("carretera inclinada");
    // Tres esquinas altas fuerzan una fundación vial continua antes de
    // `DrawRoadGroundSprites`.
    for corner in [c(2, 1), c(1, 2), c(2, 2)] {
        map.set_height(corner, 1)
            .expect("esquina elevada de la carretera");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    8,
                    8,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    false,
                    false,
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("sloped road spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(
        !foundation_parents.is_empty(),
        "la carretera inclinada debe materializar DrawFoundation"
    );
    let attached: Vec<_> = world
        .query::<(&ViewportSortableChild, &Transform)>()
        .iter(&world)
        .filter(|(child, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        1,
        "el asfalto posterior debe seguir al último parent de la fundación"
    );
    assert_eq!(attached[0].0.source_depth, attached[0].1.translation.z);
}

#[test]
fn sloped_newgrf_tram_overlay_attaches_to_its_foundation_parent() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    let mut road = tile_template();
    road.kind = TileKind::Road;
    road.mapt = 0x20;
    road.m5 = 0x0F;
    road.m3 = 0x05;
    road = openttdrs_core::set_tram_road_type_on_tile(road, Some(RoadType::from_u8(2)));
    map.set_tile(c(1, 1), road)
        .expect("tranvía NewGRF inclinado");
    for corner in [c(2, 1), c(1, 2), c(2, 2)] {
        map.set_height(corner, 1)
            .expect("esquina elevada de la carretera");
    }

    let mut tram = vanilla_road_type_catalog()
        .into_iter()
        .find(|def| def.id == RoadType::TRAM)
        .expect("tipo tranvía vanilla");
    tram.id = RoadType::from_u8(2);
    tram.class = RoadTramType::Tram;
    tram.flags |= 1; // RoadTypeFlag::Catenary.
    tram.from_newgrf = true;
    tram.from_tramtypes_feature = true;
    let surface = openttdrs_core::DecodedSprite {
        width: 8,
        height: 8,
        x_offs: 0,
        y_offs: 0,
        rgba: [255, 255, 0, 255].repeat(8 * 8),
        mask: Vec::new(),
    };
    let catenary_back = openttdrs_core::DecodedSprite {
        width: 64,
        height: 16,
        x_offs: -32,
        y_offs: -8,
        rgba: [0, 255, 255, 255].repeat(64 * 16),
        mask: Vec::new(),
    };
    let catenary_front = openttdrs_core::DecodedSprite {
        width: 64,
        height: 16,
        x_offs: -32,
        y_offs: -8,
        rgba: [255, 0, 255, 255].repeat(64 * 16),
        mask: Vec::new(),
    };
    // El tipo publica sólo Action2 runtime; no hay preview estático del que
    // el renderer pueda depender para seleccionar la superficie inclinada.
    tram.newgrf_preview = None;
    tram.newgrf_views = Vec::new();
    tram.newgrf_runtime = Some(Box::new(TrainSpriteGraphics {
        sets: vec![
            vec![surface.clone()],
            vec![catenary_back.clone()],
            vec![catenary_front.clone()],
        ],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 0,
        }],
        specific_assigns: [((0, 5), 1), ((0, 4), 2)].into_iter().collect(),
        ..Default::default()
    }));
    let road_catalog = vec![tram];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    8,
                    8,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    false,
                    false,
                    &road_catalog,
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("tranvía NewGRF inclinado");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(!foundation_parents.is_empty());
    let attached: Vec<_> = world
        .query::<&ViewportSortableChild>()
        .iter(&world)
        .filter(|child| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        2,
        "asfalto y overlay NewGRF de tranvía deben seguir al cimiento"
    );

    let attached_handles: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .filter(|(child, _)| foundation_parents.contains(&child.parent))
        .map(|(_, sprite)| sprite.image.clone())
        .collect();
    {
        let images = world.resource::<Assets<Image>>();
        assert!(
            attached_handles.iter().any(|handle| {
                images.get(handle).and_then(|image| image.data.as_deref())
                    == Some(surface.rgba.as_slice())
            }),
            "la superficie custom del tramtype debe conservar su texture sobre foundation"
        );
    }

    let catenary_handles: Vec<_> = world
        .query::<(&ViewportSortableParent, &Sprite)>()
        .iter(&world)
        .filter(|(parent, _)| [6070, 6042].contains(&parent.sprite_id))
        .map(|(_, sprite)| sprite.image.clone())
        .collect();
    assert_eq!(
        catenary_handles.len(),
        4,
        "el tramtype custom debe conservar tres recortes traseros y un frente"
    );
    let images = world.resource::<Assets<Image>>();
    assert_eq!(
        catenary_handles
            .iter()
            .filter(|handle| {
                images.get(*handle).and_then(|image| image.data.as_deref())
                    == Some(catenary_back.rgba.as_slice())
            })
            .count(),
        3,
        "los recortes traseros deben usar el grupo custom del tramtype"
    );
    assert_eq!(
        catenary_handles
            .iter()
            .filter(|handle| {
                images.get(*handle).and_then(|image| image.data.as_deref())
                    == Some(catenary_front.rgba.as_slice())
            })
            .count(),
        1,
        "el frente debe usar su grupo custom separado"
    );
}

#[test]
fn sloped_road_stop_grounds_attach_to_their_foundation_parent() {
    let assets = boot_assets_app();
    let bay_ground = assets.bus_stop_grounds[0].clone();
    let drive_through_ground =
        assets.road_paved[crate::sprites::road_flat_sprite_index(0, 0x0A)].clone();
    let drive_through_tram_overlay = assets
        .rail
        .get(&(crate::sprites::TRAMWAY_SPRITE_BASE + 5))
        .expect("overlay vanilla del tranvía en eje X")
        .clone();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    let bay = c(1, 1);
    let drive_through = c(5, 1);
    for (coord, m5) in [
        (bay, 0),
        (drive_through, openttdrs_core::RSV_DRIVE_THROUGH_X),
    ] {
        let mut tile = Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5,
            m6: 3 << 3, // StationType::Bus.
            ..tile_template()
        };
        if coord == drive_through {
            tile = openttdrs_core::set_tram_road_type_on_tile(tile, Some(RoadType::TRAM));
        }
        map.set_tile(coord, tile).expect("parada vial inclinada");
        for corner in [
            c(coord.x + 1, coord.y),
            c(coord.x, coord.y + 1),
            c(coord.x + 1, coord.y + 1),
        ] {
            map.set_height(corner, 1)
                .expect("esquina elevada de la parada");
        }
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let drive_through_ctx = TileRenderContext::new(&map, &grid, 5, 1);
    assert_ne!(
        drive_through_ctx.info.tileh, 0,
        "la parada debe seguir inclinada"
    );
    let catenary_z = i32::from(drive_through_ctx.info.base_z.saturating_add(1)) * 8;
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for coord in [bay, drive_through] {
                    spawn_station_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("x positiva"),
                            u32::try_from(coord.y).expect("y positiva"),
                        ),
                        &[],
                        4.0,
                        true,
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        None,
                        &[],
                        None,
                        &[],
                        TEST_CLIMATE,
                        &[],
                    );
                }
            },
        )
        .expect("sloped road stops spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert_eq!(
        foundation_parents.len(),
        2,
        "cada parada inclinada tiene fundación"
    );

    let mut catenary: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(parent, transform)| {
            [6071, 6043]
                .contains(&parent.sprite_id)
                .then_some((*parent, transform.translation.z))
        })
        .collect();
    catenary.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        catenary
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                6071,
                ParentSpriteBounds::new(95, 16, catenary_z, 95, 16, catenary_z + 1),
                viewport_insertion_key(5, 1, 4),
            ),
            (
                6071,
                ParentSpriteBounds::new(80, 16, catenary_z, 80, 16, catenary_z + 1),
                viewport_insertion_key(5, 1, 5),
            ),
            (
                6071,
                ParentSpriteBounds::new(80, 31, catenary_z, 80, 31, catenary_z + 1),
                viewport_insertion_key(5, 1, 6),
            ),
            (
                6043,
                ParentSpriteBounds::new(80, 16, catenary_z + 2, 95, 31, catenary_z + 2),
                viewport_insertion_key(5, 1, 7),
            ),
        ],
        "la catenaria de una parada nivelada usa la altura efectiva como parent global"
    );
    assert!(
        catenary
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "la catenaria inclinada preserva profundidad fuente antes del sort global"
    );

    let attached: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite, &Transform)>()
        .iter(&world)
        .filter(|(child, _, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        3,
        "cada suelo vial y el overlay vanilla del tranvía deben ser child del cimiento"
    );
    assert!(
        attached
            .iter()
            .any(|(_, sprite, _)| bay_ground.matches(sprite))
    );
    assert!(
        attached
            .iter()
            .any(|(_, sprite, _)| drive_through_ground.matches(sprite))
    );
    assert!(
        attached
            .iter()
            .any(|(_, sprite, _)| drive_through_tram_overlay.matches(sprite))
    );
    assert!(
        attached
            .iter()
            .all(|(child, _, transform)| child.source_depth == transform.translation.z)
    );
}

#[test]
fn sloped_depot_grounds_and_reservation_attach_to_their_foundation_parent() {
    let assets = boot_assets_app();
    let road_ground = assets.road_depot_ground.clone();
    let rail_ground = assets.rail.get(&1011).expect("vía de depósito SE").clone();
    let reservation = assets
        .pbs_rail_sprite(1006)
        .expect("reserva PBS vertical")
        .clone();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    let road_depot = c(1, 1);
    let rail_depot = c(5, 1);
    map.set_tile(
        road_depot,
        Tile {
            kind: TileKind::RoadDepot,
            mapt: 0x20,
            ..tile_template()
        },
    )
    .expect("depósito vial inclinado");
    map.set_tile(
        rail_depot,
        Tile {
            kind: TileKind::RailDepot,
            mapt: 0x10,
            // Dirección SE + HasDepotReservation: ambos overlays deben
            // colgar de la fundación nivelada.
            m5: 0x11,
            ..tile_template()
        },
    )
    .expect("depósito ferroviario inclinado");
    for coord in [road_depot, rail_depot] {
        for corner in [
            c(coord.x + 1, coord.y),
            c(coord.x, coord.y + 1),
            c(coord.x + 1, coord.y + 1),
        ] {
            map.set_height(corner, 1)
                .expect("esquina elevada del depósito");
        }
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let dims = m.0.dimensions();
                for coord in [road_depot, rail_depot] {
                    spawn_transport_object_tile(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("x positiva"),
                            u32::try_from(coord.y).expect("y positiva"),
                        ),
                        4.0,
                        true,
                        &m.0,
                        dims,
                        &[],
                        &[],
                        None,
                        &[],
                        &[],
                        None,
                        None,
                    );
                }
            },
        )
        .expect("sloped depots spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert_eq!(
        foundation_parents.len(),
        2,
        "cada depósito inclinado tiene fundación"
    );

    let attached: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite, &Transform)>()
        .iter(&world)
        .filter(|(child, _, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        3,
        "suelo vial, vía y reserva PBS deben ser children"
    );
    assert!(
        attached
            .iter()
            .any(|(_, sprite, _)| road_ground.matches(sprite))
    );
    assert!(
        attached
            .iter()
            .any(|(_, sprite, _)| rail_ground.matches(sprite))
    );
    assert!(
        attached
            .iter()
            .any(|(_, sprite, _)| reservation.matches(sprite))
    );
    assert!(
        attached
            .iter()
            .all(|(child, _, transform)| child.source_depth == transform.translation.z)
    );

    let mut rail_building_parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter(|(parent, _)| [1063, 1064].contains(&parent.sprite_id))
        .collect();
    rail_building_parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(rail_building_parents.len(), 2);
    assert!(
        rail_building_parents
            .iter()
            .all(|(parent, transform)| parent.bounds.zmin == 8
                && parent.source_depth == transform.translation.z),
        "las fachadas BUILD usan la superficie nivelada como parents globales"
    );
}

#[test]
fn electric_rail_depot_catenary_and_buildings_join_global_sort() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = fresh_map8();
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RailDepot,
            mapt: 0x10,
            // SE: el oráculo inserta cable 5659 antes de las puertas
            // 1063/1064 y el sorter final lo deja entre ambas.
            m5: 1,
            m8: RailType::Electric as u16,
            ..tile_template()
        },
    )
    .expect("depósito eléctrico");
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.init_resource::<ViewportSortableChildDepthWindows>();
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    true,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("electric rail depot spawn");

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter(|(parent, _)| [5659, 1063, 1064].contains(&parent.sprite_id))
        .map(|(parent, transform)| (*parent, transform.translation.z))
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                5659,
                ParentSpriteBounds::new(23, 16, 10, 23, 30, 10),
                viewport_insertion_key(1, 1, 1),
            ),
            (
                1063,
                ParentSpriteBounds::new(18, 18, 0, 18, 30, 22),
                viewport_insertion_key(1, 1, 2),
            ),
            (
                1064,
                ParentSpriteBounds::new(29, 18, 0, 29, 30, 22),
                viewport_insertion_key(1, 1, 3),
            ),
        ],
        "el cable y las puertas deben publicar sus prismas nativos antes del sort"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "cada parent conserva su profundidad fuente antes del sort global"
    );

    let mut schedule = Schedule::default();
    schedule.add_systems(sort_viewport_sortable_parents);
    schedule.run(&mut world);
    let mut sorted: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter(|(parent, _)| [5659, 1063, 1064].contains(&parent.sprite_id))
        .map(|(parent, transform)| (parent.sprite_id, transform.translation.z))
        .collect();
    sorted.sort_by(|left, right| left.1.total_cmp(&right.1));
    assert_eq!(
        sorted
            .iter()
            .map(|(sprite_id, _)| *sprite_id)
            .collect::<Vec<_>>(),
        vec![1063, 5659, 1064],
        "el orden global debe conservar el resultado de ViewportSortParentSprites"
    );
}

#[test]
fn newgrf_rail_depot_group_replaces_relocated_building_layers() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(2, 2);
    let mut map = fresh_map8();
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RailDepot,
            mapt: 0x10,
            // SE: OpenTTD's RTSG_DEPOT block uses the first two relocated
            // sprites (SE_1/SE_2) for this orientation.
            m5: 1,
            ..tile_template()
        },
    )
    .expect("rail depot");
    for corner in [
        TileCoord::new(coord.x + 1, coord.y),
        TileCoord::new(coord.x, coord.y + 1),
        TileCoord::new(coord.x + 1, coord.y + 1),
    ] {
        map.set_height(corner, 1)
            .expect("esquina elevada del depósito NewGRF");
    }
    let custom_rgba = [230, 70, 210, 255].repeat(16);
    let view = DecodedSprite {
        width: 4,
        height: 4,
        x_offs: -2,
        y_offs: -4,
        rgba: custom_rgba.clone(),
        mask: Vec::new(),
    };
    let graphics = TrainSpriteGraphics {
        sets: vec![vec![view.clone(); 6]],
        specific_assigns: std::collections::HashMap::from([(
            (0, openttdrs_core::RAIL_SPRITE_TYPE_DEPOT),
            0,
        )]),
        ..Default::default()
    };
    let depot_spec = openttdrs_core::RailSignalSpriteSpec {
        rail_type: RailType::Rail,
        local_id: 0,
        sprite_type: openttdrs_core::RAIL_SPRITE_TYPE_DEPOT,
        grfid: 0x4445_504F,
        type_tables: None,
        graphics,
    };
    let depot_specs = vec![Some(depot_spec)];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfSignalSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut signal_sprites: ResMut<crate::render::NewGrfSignalSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &depot_specs,
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    Some(&mut signal_sprites),
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    None,
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("custom rail depot spawn");

    {
        let images = world.resource::<Assets<Image>>();
        let custom_layers = images
            .iter()
            .filter(|(_, image)| image.data.as_deref() == Some(custom_rgba.as_slice()))
            .count();
        assert_eq!(custom_layers, 2, "SE debe consumir SE_1 y SE_2 custom");
    }

    let mut parents: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter(|(parent, _)| [1063, 1064].contains(&parent.sprite_id))
        .map(|(parent, transform)| (*parent, transform.translation.z))
        .collect();
    parents.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (1063, viewport_insertion_key(2, 2, 1)),
            (1064, viewport_insertion_key(2, 2, 2)),
        ],
        "RTSG_DEPOT conserva los parents TileSeq relocalizados"
    );
    assert!(
        parents
            .iter()
            .all(|(parent, depth)| parent.bounds.zmin == 8 && parent.source_depth == *depth),
        "el sprite NewGRF entra al sorter con su ancla NFO y la superficie nivelada"
    );
}

#[test]
fn newgrf_rail_tunnel_group_draws_custom_surface_when_portal_is_defined() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(2, 2);
    let mut map = fresh_map8();
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RailTunnel,
            mapt: 0x10,
            // OpenTTD obtiene la orientación de `GetTunnelBridgeDirection`
            // (`m5 & 3`), no de la pendiente efectiva. Deliberadamente
            // usamos SW (2) con una pendiente NE para cubrir saves
            // importados cuya geometría y bytes no coinciden.
            m5: 2,
            m8: RailType::Electric as u16,
            ..tile_template()
        },
    )
    .expect("rail tunnel");
    // Las dos esquinas del norte/este elevadas producen SLOPE_NE (12), la
    // boca que `DrawTile_TunnelBridge` acepta. La dirección real sigue siendo
    // SW (2), la persistida en m5.
    map.set_height(TileCoord::new(2, 2), 1)
        .expect("north height");
    map.set_height(TileCoord::new(2, 3), 1)
        .expect("east height");
    let custom_rgba = [230u8, 70, 210, 255].repeat(16);
    let view = DecodedSprite {
        width: 4,
        height: 4,
        x_offs: -2,
        y_offs: -4,
        rgba: custom_rgba.clone(),
        mask: Vec::new(),
    };
    let graphics = |selector| TrainSpriteGraphics {
        sets: vec![vec![view.clone(); 4]],
        specific_assigns: std::collections::HashMap::from([((0, selector), 0)]),
        ..Default::default()
    };
    let make_spec = |sprite_type, graphics| openttdrs_core::RailSignalSpriteSpec {
        rail_type: RailType::Electric,
        local_id: 0,
        sprite_type,
        grfid: 0x5455_4E4C,
        type_tables: None,
        graphics,
    };
    // Los catálogos indexan por `RailType`: Electric ocupa el slot 1.
    let underlay_specs = vec![
        None,
        Some(make_spec(
            openttdrs_core::RAIL_SPRITE_TYPE_UNDERLAY,
            graphics(openttdrs_core::RAIL_SPRITE_TYPE_UNDERLAY),
        )),
    ];
    let tunnel_specs = vec![
        None,
        Some(make_spec(
            openttdrs_core::RAIL_SPRITE_TYPE_TUNNEL,
            graphics(openttdrs_core::RAIL_SPRITE_TYPE_TUNNEL),
        )),
    ];
    let portal_specs = vec![
        None,
        Some(make_spec(
            openttdrs_core::RAIL_SPRITE_TYPE_TUNNEL_PORTAL,
            graphics(openttdrs_core::RAIL_SPRITE_TYPE_TUNNEL_PORTAL),
        )),
    ];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfSignalSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut signal_sprites: ResMut<crate::render::NewGrfSignalSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &underlay_specs,
                    &tunnel_specs,
                    &portal_specs,
                    &[],
                    None,
                    Some(&mut signal_sprites),
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    None,
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("custom rail tunnel spawn");

    // Los atlas de los tests comparten handles débiles con el cache Action2;
    // identificar la entidad por su capa de sorter evita confundir el PNG de
    // la boca vanilla con las imágenes materializadas por los grupos.
    let custom_layer_z = crate::iso::sortable_draw_z(2, 2, 0, 0.012);
    let custom_layers = world
        .query::<&Transform>()
        .iter(&world)
        .filter(|transform| (transform.translation.z - custom_layer_z).abs() < f32::EPSILON)
        .count();
    assert_eq!(
        custom_layers, 1,
        "RTSG_TUNNEL debe dibujar la superficie custom"
    );
    let wire_id = crate::sprites::catenary_reference_sprite_id(
        crate::sprites::catenary_tunnel_wire_sprite(2),
    );
    let wire_entity = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .find_map(|(entity, parent)| (parent.sprite_id == wire_id).then_some(entity))
        .expect("el cable eléctrico debe ser el parent combinado");
    assert_eq!(
        world.query::<&ViewportSortableChild>().iter(&world).count(),
        2,
        "la base Action5 y RTSG_TUNNEL_PORTAL deben quedar combinados bajo el cable"
    );
    assert!(
        world
            .query::<&ViewportSortableChild>()
            .iter(&world)
            .all(|child| child.parent == wire_entity),
        "las dos capas frontales deben acompañar el cable al reordenar"
    );
    assert!(
        !world
            .query::<&ViewportSortableParent>()
            .iter(&world)
            .any(|parent| parent.sprite_id == 6128),
        "la fachada base Action5 debe ser child, no un parent independiente"
    );
    let front_offset = crate::iso::remap_tile_offset(15.0, 15.0, 0.0) * 0.5;
    let front_iso = crate::iso::iso(2, 2);
    assert!(
        world.query::<&Transform>().iter(&world).any(|transform| {
            (transform.translation.x - (front_iso.x + front_offset.x)).abs() < f32::EPSILON
                && (transform.translation.y - (front_iso.y + 2.0 + front_offset.y)).abs()
                    < f32::EPSILON
        }),
        "RTSG_TUNNEL_PORTAL debe anclar la fachada custom al borde sortable"
    );
}

#[test]
fn electric_rail_tunnel_combines_front_with_catenary_parent() {
    let assets = boot_assets_app();
    let coord = TileCoord::new(2, 2);
    let mut map = fresh_map8();
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::RailTunnel,
            mapt: 0x10,
            // SW: usa la geometría de cable larga en X y la fachada 6128.
            m5: 2,
            m8: RailType::Electric as u16,
            ..tile_template()
        },
    )
    .expect("rail tunnel eléctrico");
    map.set_height(TileCoord::new(2, 2), 1)
        .expect("north height");
    map.set_height(TileCoord::new(2, 3), 1)
        .expect("east height");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfSignalSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut signal_sprites: ResMut<crate::render::NewGrfSignalSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    Some(&mut signal_sprites),
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    None,
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("electric rail tunnel spawn");

    let wire_id = crate::sprites::catenary_reference_sprite_id(
        crate::sprites::catenary_tunnel_wire_sprite(2),
    );
    let (wire_entity, wire_parent) = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .find(|(_, parent)| parent.sprite_id == wire_id)
        .expect("el cable de boca debe participar como parent sortable");
    assert_eq!(
        wire_parent.bounds,
        ParentSpriteBounds::new(32, 32, 7, 47, 46, 7),
        "la caja del cable debe conservar `SpriteBounds` del túnel"
    );
    let children: Vec<_> = world
        .query::<&ViewportSortableChild>()
        .iter(&world)
        .collect();
    assert_eq!(
        children.len(),
        1,
        "la fachada del túnel debe ser el único child del cable sin portal custom"
    );
    assert_eq!(
        children[0].parent, wire_entity,
        "el techo frontal debe moverse como bloque atómico con la catenaria"
    );
    assert!(
        !world
            .query::<&ViewportSortableParent>()
            .iter(&world)
            .any(|parent| parent.sprite_id == 6128),
        "la fachada ya no debe competir como parent independiente"
    );
}

#[test]
fn airport_pier_tile_seq_layers_attach_to_global_sorter_for_both_import_paths() {
    let assets = boot_assets_app();
    let expected_apron = assets.airport_apron.clone();
    let expected_jetway = assets
        .airport_station_sprite(2661)
        .expect("sprite jetway airport")
        .clone();
    let expected_tunnel = assets
        .airport_station_sprite(2662)
        .expect("sprite túnel aeropuerto")
        .clone();
    let mut map = fresh_map8();
    let station_coord = TileCoord::new(2, 2);
    let imported_coord = TileCoord::new(4, 2);

    // MP_STATION crudo: StationType::Airport (bits 3..6 = 1) y
    // StationGfx 27 = APT_PIER_NW_NE.
    map.set_tile(
        station_coord,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5: 27,
            m6: 1 << 3,
            ..tile_template()
        },
    )
    .expect("station airport pier");

    // El importador también conserva algunos aeropuertos como TileKind::Airport.
    // La asociación STATION (MAP2 + airport_tiles) debe habilitar el mismo
    // StationGfx 28 = APT_PIER, no reducirlo al AirportPiece interno.
    map.set_tile(
        imported_coord,
        Tile {
            kind: TileKind::Airport,
            mapt: 0x50,
            m5: 28,
            m2: 17,
            ..tile_template()
        },
    )
    .expect("imported airport pier");
    let mut imported_station = Station::new_with_kind(imported_coord, StopKind::Airport);
    imported_station.ottd_station_id = Some(17);
    imported_station.airport_tiles.push(imported_coord);
    let imported_stations = vec![imported_station];

    let grid = RenderGrid::from_map(&map, 8, 8);
    let expected_pos = |coord: TileCoord, gfx: u8| {
        let ctx = TileRenderContext::new(
            &map,
            &grid,
            u32::try_from(coord.x).expect("positive x"),
            u32::try_from(coord.y).expect("positive y"),
        );
        let layer = crate::sprites::airport_station_layers_for_gfx(gfx)[0];
        let sprite = crate::sprites::airport_station_sprite_for_id(layer.sprite_id)
            .expect("sprite de capa airport");
        let (xrel, yrel) = crate::sprites::airport_station_overlay_rel_for_sprite(&layer, sprite);
        crate::iso::overlay_pos(
            ctx.iso_pos,
            xrel,
            yrel,
            layer.w,
            layer.h,
            ctx.info.base_z,
            layer.z,
            ctx.tx_i32(),
            ctx.ty_i32(),
        )
    };
    let expected_jetway_pos = expected_pos(station_coord, 27);
    let expected_tunnel_pos = expected_pos(imported_coord, 28);

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let dims = m.0.dimensions();
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    dims,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 4, 2),
                    4.0,
                    false,
                    &m.0,
                    dims,
                    &imported_stations,
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("airport pier spawn");

    let airport_parents: Vec<_> = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .filter_map(|parent| {
            [2661, 2662].contains(&parent.sprite_id).then_some((
                parent.sprite_id,
                (
                    parent.bounds.xmin,
                    parent.bounds.ymin,
                    parent.bounds.zmin,
                    parent.bounds.xmax,
                    parent.bounds.ymax,
                    parent.bounds.zmax,
                ),
                parent.insertion_key,
            ))
        })
        .collect();
    assert!(
        airport_parents.contains(&(
            2661,
            (35, 34, 0, 37, 36, 13),
            crate::render::viewport_insertion_key(2, 2, 0),
        )),
        "el jetway StationGfx del MP_STATION debe conservar prisma y ordinal TILE_SEQ"
    );
    assert!(
        airport_parents.contains(&(
            2662,
            (64, 40, 0, 77, 42, 13),
            crate::render::viewport_insertion_key(4, 2, 0),
        )),
        "el túnel StationGfx del Airport importado debe participar con ordinal cero"
    );

    let mut aprons = 0;
    let mut jetways = Vec::new();
    let mut tunnels = Vec::new();
    for (sprite, transform) in world.query::<(&Sprite, &Transform)>().iter(&world) {
        if expected_apron.matches(sprite) {
            aprons += 1;
        }
        if expected_jetway.matches(sprite) {
            jetways.push(transform.translation);
        }
        if expected_tunnel.matches(sprite) {
            tunnels.push(transform.translation);
        }
    }
    assert_eq!(aprons, 2, "cada pier comienza con SPR_AIRPORT_APRON");
    assert_eq!(
        jetways
            .iter()
            .map(|position| position.truncate())
            .collect::<Vec<_>>(),
        vec![expected_jetway_pos.truncate()],
        "el sorter puede reasignar Z, pero no el ancla NFO del jetway"
    );
    assert_eq!(
        tunnels
            .iter()
            .map(|position| position.truncate())
            .collect::<Vec<_>>(),
        vec![expected_tunnel_pos.truncate()],
        "el sorter puede reasignar Z, pero no el ancla NFO del túnel"
    );
}

#[test]
fn imported_airport_uses_full_station_gfx_not_airport_piece_fallbacks() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coords = [
        (TileCoord::new(1, 1), 19_u8, 2650_u32), // terminal A
        (TileCoord::new(2, 1), 24, 2655),        // hangar front
        (TileCoord::new(3, 1), 44, 2633),        // heliport
        (TileCoord::new(4, 1), 47, 2651),        // tower static
        (TileCoord::new(5, 1), 71, 5968),        // Action5 half-apron
    ];
    let mut station = Station::new_with_kind(coords[0].0, StopKind::RailStation);
    station.ottd_station_id = Some(23);
    station.airport_tiles = coords.iter().map(|(coord, _, _)| *coord).collect();
    for (coord, gfx, _) in coords {
        map.set_tile(
            coord,
            Tile {
                kind: TileKind::Airport,
                mapt: 0x50,
                m2: 23,
                m5: gfx,
                ..tile_template()
            },
        )
        .expect("airport tile");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets.clone()));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for (coord, _, _) in coords {
                    spawn_transport_object_tile(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("positive x"),
                            u32::try_from(coord.y).expect("positive y"),
                        ),
                        4.0,
                        false,
                        &m.0,
                        m.0.dimensions(),
                        &[station.clone()],
                        &[],
                        None,
                        &[],
                        &[],
                        None,
                        None,
                    );
                }
            },
        )
        .expect("airport spawn");

    for (_, _, sprite_id) in coords {
        let expected = assets
            .airport_station_sprite(sprite_id)
            .unwrap_or_else(|| panic!("airport sprite {sprite_id}"));
        assert!(
            world
                .query::<&Sprite>()
                .iter(&world)
                .any(|sprite| expected.matches(sprite)),
            "falta capa StationGfx con sprite {sprite_id}"
        );
    }
}

#[test]
fn imported_airport_radar_keeps_its_rotating_parent_and_frame_anchor() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(3, 3);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Airport,
            mapt: 0x50,
            m2: 23,
            // APT_RADAR_GRASS_FENCE_SW: el frame tres tiene un PNG y ancla
            // distintos del frame cero, pero conserva su prisma TILE_SEQ.
            m5: 31,
            m7: 3,
            ..tile_template()
        },
    )
    .expect("radar airport tile");
    let mut station = Station::new_with_kind(coord, StopKind::Airport);
    station.ottd_station_id = Some(23);
    station.airport_tiles.push(coord);

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets.clone()));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[station.clone()],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("radar airport spawn");

    let mut radars = world.query::<(
        &AirportStationAnim,
        &ViewportSortableParent,
        &Transform,
        &Sprite,
    )>();
    let (anim, parent, transform, sprite) = radars.single(&world).expect("radar animado");
    let expected = anim.frame_for_m7(3, 31).expect("frame radar tres");
    assert_eq!(parent.sprite_id, 2_683);
    assert_eq!(parent.bounds, ParentSpriteBounds::new(55, 55, 0, 56, 56, 7));
    assert_eq!(*parent, expected.parent);
    assert_eq!(transform.translation, expected.translation);
    assert!(
        assets
            .airport_station_sprite(2_683)
            .expect("sprite radar tres")
            .matches(sprite),
        "el spawn debe comenzar en el frame m7 vivo, no en el frame cero"
    );
}

#[test]
fn imported_airport_wind_keeps_its_rotating_parent_and_frame_anchor() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(3, 3);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Airport,
            mapt: 0x50,
            m2: 23,
            // APT_AIRFIELD_WIND_1: el cuarto frame cambia ancho y ancla NFO,
            // pero conserva la caja TILE_SEQ de la manga de viento.
            m5: 39,
            m7: 3,
            ..tile_template()
        },
    )
    .expect("wind airport tile");
    let mut station = Station::new_with_kind(coord, StopKind::Airport);
    station.ottd_station_id = Some(23);
    station.airport_tiles.push(coord);

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets.clone()));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_transport_object_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &[station.clone()],
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("wind airport spawn");

    let mut winds = world.query::<(
        &AirportStationAnim,
        &ViewportSortableParent,
        &Transform,
        &Sprite,
    )>();
    let (anim, parent, transform, sprite) = winds.single(&world).expect("manga animada");
    let expected = anim.frame_for_m7(3, 39).expect("frame manga tres");
    assert_eq!(parent.sprite_id, 2_679);
    assert_eq!(
        parent.bounds,
        ParentSpriteBounds::new(52, 59, 0, 52, 59, 19)
    );
    assert_eq!(parent.insertion_key, viewport_insertion_key(3, 3, 1));
    assert_eq!(*parent, expected.parent);
    assert_eq!(transform.translation, expected.translation);
    assert!(
        assets
            .airport_station_sprite(2_679)
            .expect("sprite manga tres")
            .matches(sprite),
        "el spawn debe comenzar en el frame m7 vivo, no en el frame cero"
    );
}

#[test]
fn built_newgrf_airport_uses_parent_badge_action2_sprite() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(2, 2);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Airport,
            mapt: 0x50,
            // The airport layout stores this vanilla substitute in m5; the
            // custom gfx is carried separately by Station::airport_tile_gfx.
            m5: 24,
            m7: 1,
            ..tile_template()
        },
    )
    .expect("newgrf airport tile");

    let rgba = [255, 0, 0, 255].repeat(4);
    let blue_rgba = [0, 0, 255, 255].repeat(4);
    let view = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: rgba.clone(),
        mask: Vec::new(),
    };
    let blue_view = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: blue_rgba.clone(),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![view.clone()], vec![blue_view.clone()]],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 7,
        }],
        action2_to_action1: [(0, 0), (1, 1), (9, 1)].into_iter().collect(),
        ..Default::default()
    };
    runtime.action2_var.insert(
        7,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0xF0,
                param: None,
                adjust: Action2VarAdjust {
                    // La primera rama exige que el renderer construya el
                    // AirportScope padre con las facilities de la estación.
                    shift: 0x80,
                    and_mask: u32::MAX,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(8, 1 << 3, 1 << 3)],
            default: 9,
        },
    );
    runtime.action2_var.insert(
        8,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x7A,
                param: Some(0),
                adjust: Action2VarAdjust {
                    // Tras F0, el renderer debe conservar el marker de los
                    // grupos Action2 parent y entregar AirportScope 7A.
                    shift: 0x80,
                    and_mask: u32::MAX,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(0, 1, 1)],
            default: 1,
        },
    );
    let gfx = 74;
    let airport_tile = AirportTileSpecDef {
        gfx: AirportTileGfxId(gfx),
        subst_id: 24,
        from_newgrf: true,
        callback_mask: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        animation_triggers: 0,
        animation_special_flags: 0,
        newgrf_local_id: 0,
        newgrf_grfid: 0x4150_544C,
        newgrf_grf_version: 0,
        newgrf_type_tables: None,
        associated_badges: vec![31],
        newgrf_badge_translation: vec![42],
        newgrf_preview: Some(view.clone()),
        newgrf_views: vec![view, blue_view],
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let mut station = Station::new_with_kind(coord, StopKind::Airport);
    station.airport_newgrf_spec_id = Some(10);
    station.airport_tiles.push(coord);
    station.airport_tile_gfx.push((coord, gfx));
    let stations = vec![station];
    let catalog = vec![airport_tile];
    let airport_catalog = vec![NewgrfAirportSpecDef {
        id: 10,
        class: AirportClassId::Small,
        label: "Badge airport".into(),
        short_label: "Badge".into(),
        size_x: 1,
        size_y: 1,
        catchment: 4,
        noise_level: 1,
        subst_id: AirportSpecId::Small,
        ttd_airport_type: 0,
        layouts: Vec::new(),
        enabled: true,
        min_year: 0,
        max_year: u16::MAX,
        maintenance_cost: 0,
        associated_badges: vec![42],
        newgrf_local_id: 0,
        newgrf_grfid: 0x4150_544C,
        newgrf_views: Vec::new(),
        newgrf_purchase_views: Vec::new(),
    }];

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfAction5SpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &stations,
                    &[],
                    &catalog,
                    &airport_catalog,
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("newgrf airport spawn");

    let sprite_handles: Vec<_> = world
        .query::<&Sprite>()
        .iter(&world)
        .map(|sprite| sprite.image.clone())
        .collect();
    let images = world.resource::<Assets<Image>>();
    assert!(
        sprite_handles.iter().any(|handle| {
            images.get(handle).and_then(|image| image.data.as_deref()) == Some(rgba.as_slice())
        }),
        "el aeropuerto construido debe reevaluar Action2 con F0 y el badge del AirportScope padre"
    );
}

#[test]
fn newgrf_airport_tile_layout_emits_ground_sortable_parent_and_child() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(2, 2);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Airport,
            mapt: 0x50,
            m5: 24,
            ..tile_template()
        },
    )
    .expect("newgrf airport TileLayout tile");

    let ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [240, 10, 10, 255].repeat(4),
        mask: Vec::new(),
    };
    let parent = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -3,
        y_offs: -4,
        rgba: [10, 240, 10, 255].repeat(4),
        mask: Vec::new(),
    };
    let child = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -5,
        y_offs: -6,
        rgba: [10, 10, 240, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![
            vec![ground.clone()],
            vec![parent.clone()],
            vec![child.clone()],
        ],
        assigns: vec![TrainSpriteAssign {
            local_id: 2,
            set_id: 9,
        }],
        ..Default::default()
    };
    runtime.tile_layouts.insert(
        9,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(0),
                ..Default::default()
            },
            sequence: vec![
                TileLayoutSpriteRef {
                    action1_set: Some(1),
                    origin: [1, 2, 3],
                    extent: [4, 5, 6],
                    ..Default::default()
                },
                TileLayoutSpriteRef {
                    action1_set: Some(2),
                    origin: [7, -4, i8::MIN],
                    ..Default::default()
                },
            ],
        },
    );
    let gfx = 74;
    let airport_tile = AirportTileSpecDef {
        gfx: AirportTileGfxId(gfx),
        subst_id: 24,
        from_newgrf: true,
        callback_mask: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        animation_triggers: 0,
        animation_special_flags: 0,
        newgrf_local_id: 2,
        newgrf_grfid: 0x4150_544C,
        newgrf_grf_version: 0,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let mut station = Station::new_with_kind(coord, StopKind::Airport);
    station.airport_newgrf_spec_id = Some(10);
    station.airport_tiles.push(coord);
    station.airport_tile_gfx.push((coord, gfx));
    let stations = vec![station];
    let catalog = vec![airport_tile];

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfAction5SpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &stations,
                    &[],
                    &catalog,
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("AirportTile TileLayout spawn");

    let sprites: Vec<_> = world
        .query::<&Sprite>()
        .iter(&world)
        .map(|sprite| sprite.image.clone())
        .collect();
    let parent_sprites: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Sprite)>()
        .iter(&world)
        .map(|(entity, parent, sprite)| (entity, *parent, sprite.image.clone()))
        .collect();
    let child_sprites: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .map(|(child, sprite)| (*child, sprite.image.clone()))
        .collect();
    let (ground_handles, parent_component, child_matches) = {
        let images = world.resource::<Assets<Image>>();
        let has_rgba = |handle: &Handle<Image>, rgba: &[u8]| {
            images.get(handle).and_then(|image| image.data.as_deref()) == Some(rgba)
        };
        let mut ground_handles = Vec::new();
        for handle in &sprites {
            if has_rgba(handle, ground.rgba.as_slice()) && !ground_handles.contains(handle) {
                ground_handles.push(handle.clone());
            }
        }
        let (parent_entity, parent_component) = parent_sprites
            .iter()
            .find_map(|(entity, parent_component, handle)| {
                (images.get(handle).and_then(|image| image.data.as_deref())
                    == Some(parent.rgba.as_slice()))
                .then_some((*entity, *parent_component))
            })
            .expect("parent TileSeq custom");
        let child_matches = child_sprites.iter().any(|(child_component, handle)| {
            child_component.parent == parent_entity
                && images.get(handle).and_then(|image| image.data.as_deref())
                    == Some(child.rgba.as_slice())
        });
        (ground_handles, parent_component, child_matches)
    };
    assert_eq!(
        ground_handles.len(),
        1,
        "el ground debe reemplazar el fallback"
    );
    let expected_ground_position = overlay_pos(
        crate::iso::iso(coord.x, coord.y),
        f32::from(ground.x_offs),
        f32::from(ground.y_offs),
        f32::from(ground.width),
        f32::from(ground.height),
        0,
        0.025,
        coord.x,
        coord.y,
    );
    let ground_depths: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .filter_map(|(sprite, transform)| {
            (ground_handles.contains(&sprite.image)
                && transform.translation.truncate() == expected_ground_position.truncate())
            .then_some(transform.translation.z)
        })
        .collect();
    assert_eq!(
        ground_depths,
        vec![ground_draw_z(coord.x, coord.y, 0.025)],
        "AirportDrawTileLayout debe dejar el ground en DrawGroundSprite"
    );
    assert_eq!(
        parent_component.bounds,
        crate::render::viewport_sort::ParentSpriteBounds::new(33, 34, 3, 36, 38, 8),
        "el parent conserva el prisma TILE_SEQ_LINE inclusivo"
    );
    assert_eq!(
        parent_component.insertion_key,
        crate::render::viewport_insertion_key(2, 2, 2),
        "el parent entra en el mismo ordinal BUILD del compositor"
    );
    assert!(
        child_matches,
        "el child TileSeq debe permanecer unido al parent anterior"
    );
}

#[test]
fn rotated_newgrf_airport_layout_selects_relative_runtime_and_action5_foundation() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let mut map = Map::new_flat(6, 6, 0);
    for x in 0..6 {
        for y in 0..6 {
            let tile = TileCoord::new(x, y);
            map.set_height(tile, 4).expect("airport foundation height");
            // Los vecinos también son construcciones niveladas: así los dos
            // bordes del cimiento usan el bloque Action5 3, sin paredes.
            map.set_kind(tile, TileKind::Airport)
                .expect("airport neighbour");
        }
    }
    let coord = TileCoord::new(3, 2);
    map.set_height(coord, 5).expect("rotated airport slope");
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Airport,
            mapt: 0x50,
            m5: 24,
            ..tile_template()
        },
    )
    .expect("rotated newgrf airport tile");

    let selected_ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [20, 220, 80, 255].repeat(4),
        mask: Vec::new(),
    };
    let fallback_ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [220, 20, 80, 255].repeat(4),
        mask: Vec::new(),
    };
    let build = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -3,
        y_offs: -4,
        rgba: [20, 80, 220, 255].repeat(4),
        mask: Vec::new(),
    };
    let foundation = DecodedSprite {
        width: 4,
        height: 4,
        x_offs: -2,
        y_offs: -3,
        rgba: [220, 180, 20, 255].repeat(16),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![
            vec![selected_ground.clone()],
            vec![fallback_ground.clone()],
            vec![build.clone()],
        ],
        assigns: vec![TrainSpriteAssign {
            local_id: 2,
            set_id: 9,
        }],
        ..Default::default()
    };
    // Primero se consulta AirportScope 0x40 (el layout persistido) y luego
    // AirportTileScope 0x43. Un renderer que transponga la orientación E/O
    // no llega al layout seleccionado para dx=1, dy=0.
    runtime.action2_var.insert(
        9,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x40,
                param: None,
                adjust: Action2VarAdjust {
                    shift: 0x80,
                    and_mask: 0xFF,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(10, 1, 1)],
            default: 11,
        },
    );
    runtime.action2_var.insert(
        10,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x43,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: u32::MAX,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: vec![(12, 0x0001_0001, 0x0001_0001)],
            default: 13,
        },
    );
    runtime.tile_layouts.insert(
        12,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(0),
                ..Default::default()
            },
            sequence: vec![TileLayoutSpriteRef {
                action1_set: Some(2),
                origin: [1, 2, 3],
                extent: [4, 5, 6],
                ..Default::default()
            }],
        },
    );
    runtime.tile_layouts.insert(
        13,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(1),
                ..Default::default()
            },
            sequence: Vec::new(),
        },
    );

    let gfx = 74;
    let airport_tile = AirportTileSpecDef {
        gfx: AirportTileGfxId(gfx),
        subst_id: 24,
        from_newgrf: true,
        callback_mask: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        animation_triggers: 0,
        animation_special_flags: 0,
        newgrf_local_id: 2,
        newgrf_grfid: 0x4150_544C,
        newgrf_grf_version: 0,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        newgrf_preview: Some(selected_ground.clone()),
        newgrf_views: vec![selected_ground.clone(), fallback_ground.clone()],
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let mut station = Station::new_with_kind(TileCoord::new(2, 2), StopKind::Airport);
    station.airport_newgrf_spec_id = Some(10);
    station.airport_layout = 1;
    station.airport_rotation = 2;
    station.airport_tiles.push(coord);
    station.airport_tile_gfx.push((coord, gfx));
    let stations = vec![station];
    let catalog = vec![airport_tile];
    let mut foundation_newgrf = vec![None; openttdrs_core::FOUNDATION_ACTION5_SLOT_COUNT];
    // La altura compartida deja tileh=7 en esta fixture. Leveled + bloque
    // sin paredes selecciona SPR_SLOPES_VIRTUAL_BASE + 3*22 + 7 = 5471,
    // que corresponde al slot 58 de Action5 0x06.
    foundation_newgrf[58] = Some(foundation.clone());

    let grid = RenderGrid::from_map(&map, 6, 6);
    let ctx = TileRenderContext::new(&map, &grid, 3, 2);
    assert_eq!(ctx.info.tileh, 7);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfAction5SpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfAction5SpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_transport_object_tile_with_road_types(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    4.0,
                    false,
                    &m.0,
                    m.0.dimensions(),
                    &stations,
                    &[],
                    &catalog,
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    &foundation_newgrf,
                    TEST_CLIMATE,
                    0,
                    &[],
                    None,
                    &[],
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    &[],
                );
            },
        )
        .expect("rotated airport layout spawn");

    let parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Sprite)>()
        .iter(&world)
        .map(|(entity, parent, sprite)| (entity, *parent, sprite.image.clone()))
        .collect();
    let children: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .map(|(child, sprite)| (*child, sprite.image.clone()))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let foundation_entity = parents
        .iter()
        .find(|(_, _, handle)| {
            images.get(handle).and_then(|image| image.data.as_deref())
                == Some(foundation.rgba.as_slice())
        })
        .map(|(entity, _, _)| *entity)
        .unwrap_or_else(|| {
            panic!(
                "foundation Action5 custom del aeropuerto; parent_ids={:?}",
                parents
                    .iter()
                    .map(|(_, parent, _)| parent.sprite_id)
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        parents.iter().any(|(_, parent, handle)| {
            parent.sprite_id == u32::MAX
                && images.get(handle).and_then(|image| image.data.as_deref())
                    == Some(build.rgba.as_slice())
        }),
        "el BUILD del layout rotado debe conservar su parent independiente"
    );
    assert!(
        images
            .iter()
            .any(|(_, image)| { image.data.as_deref() == Some(selected_ground.rgba.as_slice()) }),
        "0x43 debe seleccionar el ground de la posición directa E/O"
    );
    assert!(
        !images
            .iter()
            .any(|(_, image)| { image.data.as_deref() == Some(fallback_ground.rgba.as_slice()) }),
        "la rama transpuesta/default no debe materializarse"
    );
    let ground_is_child = children.iter().any(|(child, handle)| {
        child.parent == foundation_entity
            && images.get(handle).and_then(|image| image.data.as_deref())
                == Some(selected_ground.rgba.as_slice())
    });
    assert!(
        ground_is_child,
        "el ground del AirportTile debe colgar del cimiento Action5"
    );
}

#[test]
fn newgrf_airport_draw_foundations_callback_controls_slope_foundation() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    for (callback_value, should_draw_foundation) in [(0_u8, false), (1_u8, true)] {
        let assets = boot_assets_app();
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        // Misma pendiente no empinada que usa DrawFoundation(Leveled) en los
        // otros objetos: la esquina de la tesela queda a 7 y las cuatro
        // esquinas vecinas a 4, por lo que el callback se ejecuta realmente.
        map.set_height(coord, 7).expect("airport tile height");
        for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
            map.set_height(TileCoord::new(x, y), 4)
                .expect("airport corner height");
        }
        let mut tile = tile_template();
        tile.kind = TileKind::Airport;
        tile.mapt = 0x50;
        tile.m5 = 24;
        tile.m7 = 1;
        map.set_tile(coord, tile).expect("airport tile");

        let view = DecodedSprite {
            width: 2,
            height: 2,
            x_offs: -1,
            y_offs: -2,
            rgba: [40, 120, 240, 255].repeat(4),
            mask: Vec::new(),
        };
        let mut runtime = callback_literal_runtime(3, callback_value);
        runtime.sets = vec![vec![view.clone()]];
        // El callback CB150 y el layout residen en el mismo grupo Action3:
        // la fundación sólo combina el ground; el BUILD debe continuar como
        // parent independiente del compositor.
        runtime.tile_layouts.insert(
            0,
            TileLayout {
                ground: TileLayoutSpriteRef {
                    action1_set: Some(0),
                    ..Default::default()
                },
                sequence: vec![TileLayoutSpriteRef {
                    action1_set: Some(0),
                    origin: [1, 2, 3],
                    extent: [4, 5, 6],
                    ..Default::default()
                }],
            },
        );
        let gfx = 175;
        let airport_tile = AirportTileSpecDef {
            gfx: AirportTileGfxId(gfx),
            subst_id: 24,
            from_newgrf: true,
            callback_mask: 1 << 5,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            animation_special_flags: 0,
            newgrf_local_id: 3,
            newgrf_grfid: 0x4150_544C,
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_preview: Some(view.clone()),
            newgrf_views: vec![view],
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut station = Station::new_with_kind(coord, StopKind::Airport);
        station.airport_newgrf_spec_id = Some(10);
        station.airport_tiles.push(coord);
        station.airport_tile_gfx.push((coord, gfx));
        let stations = vec![station];
        let catalog = vec![airport_tile];
        let grid = RenderGrid::from_map(&map, 4, 4);
        let mut world = World::new();
        world.insert_resource(TsMap(map));
        world.insert_resource(TsGrid(grid));
        world.insert_resource(TsAssets(assets));
        world.insert_resource(crate::render::NewGrfAction5SpriteCache::default());
        world.insert_resource(Assets::<Image>::default());
        world
            .run_system_once(
                move |mut commands: Commands,
                      m: Res<TsMap>,
                      g: Res<TsGrid>,
                      a: Res<TsAssets>,
                      mut cache: ResMut<crate::render::NewGrfAction5SpriteCache>,
                      mut images: ResMut<Assets<Image>>| {
                    spawn_transport_object_tile_with_road_types(
                        &mut commands,
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(&m.0, &g.0, 1, 1),
                        4.0,
                        false,
                        &m.0,
                        m.0.dimensions(),
                        &stations,
                        &[],
                        &catalog,
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        &[],
                        TEST_CLIMATE,
                        0,
                        &[],
                        None,
                        &[],
                        Some(&mut cache),
                        Some(&mut images),
                        &[],
                        &[],
                    );
                },
            )
            .expect("airport CB150 spawn");

        let foundation_count = world
            .query::<&ViewportSortableParent>()
            .iter(&world)
            .filter(|parent| {
                (FOUNDATION_ORIGINAL_SPRITE_BASE
                    ..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                    .contains(&parent.sprite_id)
            })
            .count();
        assert_eq!(
            foundation_count,
            usize::from(should_draw_foundation),
            "CB150={} debe {} la fundación vanilla",
            callback_value,
            if should_draw_foundation {
                "conservar"
            } else {
                "suprimir"
            }
        );
        assert_eq!(
            world
                .query::<&ViewportSortableParent>()
                .iter(&world)
                .filter(|parent| parent.sprite_id == u32::MAX)
                .count(),
            1,
            "el BUILD de AirportTile conserva un parent TILE_SEQ separado de la fundación"
        );

        let custom_sprite_found = world
            .query::<&Sprite>()
            .iter(&world)
            .find(|sprite| {
                world
                    .resource::<Assets<Image>>()
                    .get(&sprite.image)
                    .and_then(|image| image.data.as_deref())
                    == Some([40, 120, 240, 255].repeat(4).as_slice())
            })
            .is_some();
        assert!(custom_sprite_found, "sprite AirportTile custom");
        let child = world
            .query::<&ViewportSortableChild>()
            .iter(&world)
            .find(|child| {
                world
                    .get_entity(child.parent)
                    .is_ok_and(|entity| entity.contains::<ViewportSortableParent>())
            });
        assert_eq!(
            child.is_some(),
            should_draw_foundation,
            "el sprite AirportTile debe ser child sólo si CB150 conserva la fundación"
        );
        if let Some(child) = child {
            assert!(world.get_entity(child.parent).is_ok_and(|entity| {
                entity
                    .get::<ViewportSortableParent>()
                    .is_some_and(|parent| {
                        (FOUNDATION_ORIGINAL_SPRITE_BASE
                            ..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                            .contains(&parent.sprite_id)
                    })
            }));
        }
    }
}

#[test]
fn spawn_land_house_industry_generics_and_batches() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // Terreno: grass / rough / bosque / carbón / MP_OBJECT (faro).
    let mut t = tile_template();
    t.kind = TileKind::Grass;
    t.m5 = 12;
    map.set_tile(c(0, 0), t).expect("tile");

    t = tile_template();
    t.kind = TileKind::Forest;
    map.set_tile(c(1, 0), t).expect("tile");

    t = tile_template();
    t.kind = TileKind::CoalField;
    map.set_tile(c(2, 0), t).expect("tile");

    t = tile_template();
    t.kind = TileKind::Unknown(7);
    map.set_tile(c(3, 0), t).expect("tile");

    t = tile_template();
    t.kind = TileKind::Grass;
    t.mapt = 0xA0;
    t.m5 = 1;
    map.set_tile(c(4, 0), t).expect("tile");

    // Casa (house id en m8).
    t = tile_template();
    t.kind = TileKind::House;
    t.m8 = 42;
    map.set_tile(c(0, 1), t).expect("tile");

    // Industria gfx índice 0 (tabla INDUSTRY_GFX_DATA).
    t = tile_template();
    t.kind = TileKind::Industry;
    t.m5 = 0;
    t.m6 = 0;
    map.set_tile(c(1, 1), t).expect("tile");

    // Agua en borde (costa) + bloque interior solo agua (plano).
    map.set_kind(c(0, 6), TileKind::Water).expect("w");
    map.set_kind(c(1, 6), TileKind::Grass).expect("g");
    for x in 5..8 {
        for y in 5..8 {
            map.set_kind(c(x, y), TileKind::Water).expect("w");
        }
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                let (mw, mh) = m.0.dimensions();
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 0, 0),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 0),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 0),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 0),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 4, 0),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
                spawn_house_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 0, 1),
                    HouseSpawnResources {
                        map: &m.0,
                        map_dims: (mw, mh),
                        house_catalog: &[],
                        house_counts: None,
                        towns: &[],
                        climate: openttdrs_core::Climate::Temperate,
                        newgrf_stack: &[],
                        foundation_newgrf: &[],
                        house_sprites: None,
                        action5_sprites: None,
                        images: None,
                    },
                );
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );

                let mut batches = MapSpriteBatches::default();
                push_water_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 0, 6),
                    true,
                    &mut batches,
                    &[],
                    None,
                    None,
                );
                push_water_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 6, 6),
                    false,
                    &mut batches,
                    &[],
                    None,
                    None,
                );
                push_forest_tree(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 4, 4),
                    mw,
                );
                flush_map_batches(&mut commands, batches);
            },
        )
        .expect("spawn land batch");
}

#[test]
fn spawn_sloped_road_and_station_hit_slope_ground_branch() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_height(c(1, 1), 7).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(c(x, y), 4).expect("h");
    }
    map.set_kind(c(1, 1), TileKind::Road).expect("k");
    map.set_mapt_m5(c(1, 1), 0x20, 0x0F).expect("m");

    let grid = RenderGrid::from_map(&map, 3, 3);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    TEST_CLIMATE,
                    true,
                    true,
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("sloped");
}

#[test]
fn sloped_rail_station_levels_platform_without_sloped_grass() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    // Pendiente simple: la estación debe convertirla en una superficie plana
    // igual que `DrawTile_Station` en OpenTTD, no conservar el suelo inclinado
    // bajo la plataforma.
    map.set_height(c(1, 1), 5).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(c(x, y), 4).expect("h");
    }
    map.set_tile(
        c(1, 1),
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5: 0, // plataforma X: 1070 + 1072
            m6: 0, // StationType::Rail
            ..tile_template()
        },
    )
    .expect("station");

    let grid = RenderGrid::from_map(&map, 3, 3);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    assert_ne!(ctx.info.tileh, 0, "el caso debe permanecer inclinado");
    let expected_track = assets.rail.get(&1012).expect("track X").clone();
    let expected_platform = [
        assets.rail.get(&1070).expect("platform A").clone(),
        assets.rail.get(&1072).expect("platform B").clone(),
    ];
    let plan =
        openttdrs_core::foundation_draw_plan(ctx.info.tileh, openttdrs_core::FOUNDATION_LEVELED, 0);
    let surface_z = ctx.info.base_z.saturating_add(plan.surface_z_delta);
    let mut expected_track_pos =
        crate::iso::full_tile_sprite_pos_half(1, 1, surface_z, 0.02, crate::iso::TILE_HALF_H);
    expected_track_pos.z = crate::render::viewport_source_depth(expected_track_pos.z, 1, 3);

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("sloped rail station");

    let rendered: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .collect();
    assert!(
        rendered.iter().all(|(sprite, _)| {
            !world
                .resource::<TsAssets>()
                .0
                .grass_slopes
                .iter()
                .any(|grass| grass.matches(sprite))
        }),
        "una estación ferroviaria inclinada no puede conservar césped inclinado debajo"
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|(sprite, _)| expected_track.matches(sprite))
            .count(),
        1,
        "la vía plana debe seguir a la fundación"
    );
    for platform in expected_platform {
        assert_eq!(
            rendered
                .iter()
                .filter(|(sprite, _)| platform.matches(sprite))
                .count(),
            1,
            "las dos capas de plataforma deben seguir presentes en pendiente"
        );
    }
    let track_pos = rendered
        .iter()
        .find_map(|(sprite, transform)| {
            expected_track
                .matches(sprite)
                .then_some(transform.translation)
        })
        .expect("vía de estación");
    assert_eq!(
        track_pos, expected_track_pos,
        "la vía debe usar la superficie nivelada, no la proyección de pendiente"
    );

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    let attached: Vec<_> = world
        .query::<(&ViewportSortableChild, &Transform)>()
        .iter(&world)
        .filter(|(child, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        1,
        "la vía de una estación inclinada debe seguir al parent de DrawFoundation"
    );
    assert_eq!(attached[0].1.translation.z, expected_track_pos.z);
}

#[test]
fn rail_station_platform_parents_and_roof_glass_join_global_sort() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let coord = TileCoord::new(1, 1);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            // Gfx 4: plataforma, borde frontal, techo y vidrio child.
            m5: 4,
            m6: 0,
            ..tile_template()
        },
    )
    .expect("station roof glass");
    let expected_glass = assets.rail.get(&1083).expect("roof glass").clone();
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                    true,
                    &[],
                    &[],
                    None,
                    None,
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("station roof glass spawn");

    let mut parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(entity, parent, transform)| {
            [1076, 1072, 1079].contains(&parent.sprite_id).then_some((
                entity,
                *parent,
                transform.translation.z,
            ))
        })
        .collect();
    parents.sort_by_key(|(_, parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(_, parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                1076,
                ParentSpriteBounds::new(16, 16, 0, 31, 20, 6),
                viewport_insertion_key(1, 1, 16),
            ),
            (
                1072,
                ParentSpriteBounds::new(16, 27, 0, 31, 31, 1),
                viewport_insertion_key(1, 1, 17),
            ),
            (
                1079,
                ParentSpriteBounds::new(16, 16, 16, 31, 31, 25),
                viewport_insertion_key(1, 1, 18),
            ),
        ],
        "cada TILE_SEQ_LINE debe entrar al compositor como parent independiente"
    );
    assert!(
        parents
            .iter()
            .all(|(_, parent, depth)| parent.source_depth == *depth),
        "los parents conservan su slot fuente antes del sort global"
    );
    let roof_parent = parents
        .iter()
        .find_map(|(entity, parent, _)| (parent.sprite_id == 1079).then_some(*entity))
        .expect("roof parent");
    let glass = world
        .query::<(&ViewportSortableChild, &Sprite, &Transform)>()
        .iter(&world)
        .find(|(_, sprite, _)| expected_glass.matches(sprite))
        .expect("roof glass child");
    assert_eq!(glass.0.parent, roof_parent);
    assert_eq!(glass.0.source_depth, glass.2.translation.z);
}

#[test]
fn rail_waypoint_parents_and_awning_children_join_global_sort() {
    let assets = boot_assets_app();
    let x_axis = TileCoord::new(1, 1);
    let y_axis = TileCoord::new(2, 1);
    let mut map = Map::new_flat(4, 4, 0);
    for (coord, m5) in [(x_axis, 0), (y_axis, 1)] {
        map.set_tile(
            coord,
            Tile {
                kind: TileKind::Station,
                mapt: 0x50,
                m5,
                m6: 7 << 3, // StationType::RailWaypoint
                ..tile_template()
            },
        )
        .expect("rail waypoint");
    }
    let awning_parent_pairs = [
        (assets.rail.get(&4978).expect("X west awning").clone(), 4974),
        (assets.rail.get(&4979).expect("X east awning").clone(), 4975),
        (assets.rail.get(&4980).expect("Y west awning").clone(), 4976),
        (assets.rail.get(&4981).expect("Y east awning").clone(), 4977),
    ];
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.init_resource::<ViewportSortableChildDepthWindows>();
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for coord in [x_axis, y_axis] {
                    spawn_station_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("waypoint x"),
                            u32::try_from(coord.y).expect("waypoint y"),
                        ),
                        &[],
                        4.0,
                        true,
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        None,
                        &[],
                        None,
                        &[],
                        TEST_CLIMATE,
                        &[],
                    );
                }
            },
        )
        .expect("rail waypoint spawn");

    let mut parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(entity, parent, transform)| {
            [4974, 4975, 4976, 4977]
                .contains(&parent.sprite_id)
                .then_some((entity, *parent, transform.translation.z))
        })
        .collect();
    parents.sort_by_key(|(_, parent, _)| parent.insertion_key);
    assert_eq!(
        parents
            .iter()
            .map(|(_, parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                4974,
                ParentSpriteBounds::new(16, 16, 0, 31, 18, 15),
                viewport_insertion_key(1, 1, 16),
            ),
            (
                4975,
                ParentSpriteBounds::new(16, 29, 0, 31, 31, 15),
                viewport_insertion_key(1, 1, 17),
            ),
            (
                4976,
                ParentSpriteBounds::new(32, 16, 0, 34, 31, 15),
                viewport_insertion_key(2, 1, 16),
            ),
            (
                4977,
                ParentSpriteBounds::new(45, 16, 0, 47, 31, 15),
                viewport_insertion_key(2, 1, 17),
            ),
        ],
        "los cuerpos OpenGFX2 deben publicar exactamente los dos prismas Action1 por eje"
    );
    assert!(
        parents
            .iter()
            .all(|(_, parent, depth)| parent.source_depth == *depth),
        "cada parent conserva su profundidad fuente antes del sort global"
    );

    let mut awnings = Vec::new();
    {
        let mut children = world.query::<(Entity, &ViewportSortableChild, &Sprite, &Transform)>();
        for (expected_awning, expected_parent_sprite) in awning_parent_pairs {
            let expected_parent = parents
                .iter()
                .find_map(|(entity, parent, _)| {
                    (parent.sprite_id == expected_parent_sprite).then_some(*entity)
                })
                .expect("waypoint body parent");
            let (entity, child, _, transform) = children
                .iter(&world)
                .find(|(_, _, sprite, _)| expected_awning.matches(sprite))
                .expect("waypoint awning child");
            assert_eq!(child.parent, expected_parent);
            assert_eq!(
                child.source_depth, transform.translation.z,
                "el child conserva su slot fuente hasta que corra el compositor"
            );
            awnings.push((entity, expected_parent));
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            sort_viewport_sortable_parents,
            sync_viewport_sortable_children,
        )
            .chain(),
    );
    schedule.run(&mut world);

    let mut sorted_parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter_map(|(entity, parent, transform)| {
            [4974, 4975, 4976, 4977]
                .contains(&parent.sprite_id)
                .then_some((entity, parent.sprite_id, transform.translation.z))
        })
        .collect();
    sorted_parents.sort_by(|left, right| left.2.total_cmp(&right.2));
    let mut checked_between_global_parents = 0;
    for (awning, parent) in awnings {
        let parent_index = sorted_parents
            .iter()
            .position(|(entity, _, _)| *entity == parent)
            .expect("sorted waypoint parent");
        let Some((_, next_sprite, next_depth)) = sorted_parents.get(parent_index + 1).copied()
        else {
            continue;
        };
        let parent_depth = sorted_parents[parent_index].2;
        let awning_depth = world
            .entity(awning)
            .get::<Transform>()
            .expect("waypoint awning transform")
            .translation
            .z;
        assert!(
            parent_depth < awning_depth && awning_depth < next_depth,
            "el toldo debe quedar entre su parent y el siguiente parent global {next_sprite}; got {parent_depth}, {awning_depth}, {next_depth}"
        );
        checked_between_global_parents += 1;
    }
    assert_eq!(
        checked_between_global_parents, 3,
        "sólo el último parent del stream puede carecer de límite superior"
    );
}

#[test]
fn rail_station_roof_glass_stays_between_its_roof_and_next_global_parent() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let roof_tile = TileCoord::new(1, 1);
    let following_tile = TileCoord::new(2, 1);
    for coord in [roof_tile, following_tile] {
        map.set_tile(
            coord,
            Tile {
                kind: TileKind::Station,
                mapt: 0x50,
                // Gfx 4: plataforma, borde frontal, techo y vidrio child.
                m5: 4,
                m6: 0,
                ..tile_template()
            },
        )
        .expect("station roof glass");
    }
    let expected_glass = assets.rail.get(&1083).expect("roof glass").clone();
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.init_resource::<ViewportSortableChildDepthWindows>();
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                for coord in [roof_tile, following_tile] {
                    spawn_station_tile(
                        &mut commands,
                        &m.0,
                        m.0.dimensions(),
                        &a.0,
                        None,
                        None,
                        &TileRenderContext::new(
                            &m.0,
                            &g.0,
                            u32::try_from(coord.x).expect("station x"),
                            u32::try_from(coord.y).expect("station y"),
                        ),
                        &[],
                        4.0,
                        true,
                        &[],
                        &[],
                        None,
                        None,
                        &[],
                        None,
                        &[],
                        None,
                        &[],
                        TEST_CLIMATE,
                        &[],
                    );
                }
            },
        )
        .expect("station roof glass spawn");

    let roof_parent = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .find_map(|(entity, parent)| {
            (parent.sprite_id == 1079 && parent.insertion_key == viewport_insertion_key(1, 1, 18))
                .then_some(entity)
        })
        .expect("first station roof parent");
    let glass = world
        .query::<(Entity, &ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .find_map(|(entity, child, sprite)| {
            (child.parent == roof_parent && expected_glass.matches(sprite)).then_some(entity)
        })
        .expect("first station roof glass child");

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            sort_viewport_sortable_parents,
            sync_viewport_sortable_children,
        )
            .chain(),
    );
    schedule.run(&mut world);

    let mut sorted_parents: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Transform)>()
        .iter(&world)
        .map(|(entity, parent, transform)| (entity, parent.sprite_id, transform.translation.z))
        .collect();
    sorted_parents.sort_by(|left, right| left.2.total_cmp(&right.2));
    let roof_index = sorted_parents
        .iter()
        .position(|(entity, _, _)| *entity == roof_parent)
        .expect("sorted roof parent");
    let (_, next_sprite, next_depth) = sorted_parents
        .get(roof_index + 1)
        .copied()
        .expect("a following global parent after the roof");
    let roof_depth = sorted_parents[roof_index].2;
    let glass_depth = world
        .entity(glass)
        .get::<Transform>()
        .expect("roof glass transform")
        .translation
        .z;
    assert!(
        roof_depth < glass_depth && glass_depth < next_depth,
        "el vidrio 1083 debe quedar entre su techo 1079 y el siguiente parent global {next_sprite}; got {roof_depth}, {glass_depth}, {next_depth}"
    );
    assert!(
        expected_glass.matches(
            world
                .entity(glass)
                .get::<Sprite>()
                .expect("roof glass sprite"),
        ),
        "el sorter sólo puede mover profundidad; no debe sustituir el sprite de vidrio"
    );
}

#[test]
fn sloped_newgrf_station_overlay_follows_foundation_parent() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(3, 3, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_height(c(1, 1), 5).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(c(x, y), 4).expect("h");
    }
    let tile = Tile {
        kind: TileKind::Station,
        mapt: 0x50,
        m5: 0,
        m6: 0,
        ..tile_template()
    };
    map.set_tile(c(1, 1), tile).expect("station");

    let sprite = openttdrs_core::DecodedSprite {
        width: 4,
        height: 4,
        x_offs: -2,
        y_offs: -8,
        rgba: [32, 192, 64, 255].repeat(16),
        mask: Vec::new(),
    };
    let station_spec = StationSpecDef {
        id: StationSpecId::from_u16(1),
        class: StationClassId::DEFAULT,
        label: "Pendiente NewGRF".into(),
        short_label: "NGRF".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        callback_mask: 0,
        flags: 0,
        animation_status: 0,
        animation_frames: 0,
        animation_speed: 2,
        animation_triggers: 0,
        from_newgrf: true,
        newgrf_preview: Some(sprite.clone()),
        newgrf_views: vec![sprite],
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0x5354_4E47,
        newgrf_grf_version: 8,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        custom_layouts: std::collections::HashMap::new(),
    };
    let mut station = Station::new_with_kind(c(1, 1), StopKind::RailStation);
    station.station_spec = StationSpecId::from_u16(1);
    let stations = vec![station];

    let grid = RenderGrid::from_map(&map, 3, 3);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    assert_ne!(ctx.info.tileh, 0, "el caso debe permanecer inclinado");

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut station_sprites = crate::render::NewGrfStationSpriteCache::default();
                let mut images = Assets::<Image>::default();
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &stations,
                    4.0,
                    true,
                    std::slice::from_ref(&station_spec),
                    &[],
                    Some(&mut station_sprites),
                    Some(&mut images),
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("sloped NewGRF station");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    let attached: Vec<_> = world
        .query::<(&ViewportSortableChild, &Transform)>()
        .iter(&world)
        .filter(|(child, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        2,
        "la vía y el overlay NewGRF inclinado deben compartir el parent de DrawFoundation"
    );
}

#[test]
fn flat_newgrf_station_tile_layout_keeps_ground_in_ground_pass() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m5: 0,
            m6: 0,
            ..tile_template()
        },
    )
    .expect("station TileLayout tile");

    let ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [240, 10, 10, 255].repeat(4),
        mask: Vec::new(),
    };
    let parent = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -3,
        y_offs: -4,
        rgba: [10, 240, 10, 255].repeat(4),
        mask: Vec::new(),
    };
    let child = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -5,
        y_offs: -6,
        rgba: [10, 10, 240, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![
            vec![ground.clone()],
            vec![parent.clone()],
            vec![child.clone()],
        ],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 9,
        }],
        ..Default::default()
    };
    runtime.tile_layouts.insert(
        9,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(0),
                ..Default::default()
            },
            sequence: vec![
                TileLayoutSpriteRef {
                    action1_set: Some(1),
                    origin: [1, 2, 3],
                    extent: [4, 5, 6],
                    ..Default::default()
                },
                TileLayoutSpriteRef {
                    action1_set: Some(2),
                    origin: [7, -4, i8::MIN],
                    ..Default::default()
                },
            ],
        },
    );
    let station_spec = StationSpecDef {
        id: StationSpecId::from_u16(1),
        class: StationClassId::DEFAULT,
        label: "TileLayout rail".into(),
        short_label: "TLR".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        callback_mask: 0,
        flags: 0,
        animation_status: 0,
        animation_frames: 0,
        animation_speed: 2,
        animation_triggers: 0,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(runtime)),
        newgrf_grfid: 0x5354_4E47,
        newgrf_grf_version: 8,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        custom_layouts: std::collections::HashMap::new(),
    };
    let mut station = Station::new_with_kind(coord, StopKind::RailStation);
    station.station_spec = StationSpecId::from_u16(1);

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfStationSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfStationSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_station_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    std::slice::from_ref(&station),
                    4.0,
                    true,
                    std::slice::from_ref(&station_spec),
                    &[],
                    Some(&mut cache),
                    Some(&mut images),
                    &[],
                    None,
                    &[],
                    None,
                    &[],
                    TEST_CLIMATE,
                    &[],
                );
            },
        )
        .expect("station TileLayout spawn");

    let sprite_handles: Vec<_> = world
        .query::<&Sprite>()
        .iter(&world)
        .map(|sprite| sprite.image.clone())
        .collect();
    let parent_sprites: Vec<_> = world
        .query::<(Entity, &ViewportSortableParent, &Sprite)>()
        .iter(&world)
        .map(|(entity, parent_component, sprite)| (entity, *parent_component, sprite.image.clone()))
        .collect();
    let child_sprites: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .map(|(child_component, sprite)| (*child_component, sprite.image.clone()))
        .collect();
    let (ground_handles, parent_component, child_matches) = {
        let images = world.resource::<Assets<Image>>();
        let has_rgba = |handle: &Handle<Image>, rgba: &[u8]| {
            images.get(handle).and_then(|image| image.data.as_deref()) == Some(rgba)
        };
        let mut ground_handles = Vec::new();
        for handle in &sprite_handles {
            if has_rgba(handle, ground.rgba.as_slice()) && !ground_handles.contains(handle) {
                ground_handles.push(handle.clone());
            }
        }
        let (parent_entity, parent_component) = parent_sprites
            .iter()
            .find_map(|(entity, parent_component, handle)| {
                has_rgba(handle, parent.rgba.as_slice()).then_some((*entity, *parent_component))
            })
            .expect("parent TileSeq de estación");
        let child_matches = child_sprites.iter().any(|(child_component, handle)| {
            child_component.parent == parent_entity && has_rgba(handle, child.rgba.as_slice())
        });
        (ground_handles, parent_component, child_matches)
    };
    assert_eq!(
        ground_handles.len(),
        1,
        "el ground custom debe reemplazar la vía vanilla"
    );
    let expected_ground_position = overlay_pos(
        crate::iso::iso(coord.x, coord.y),
        f32::from(ground.x_offs),
        f32::from(ground.y_offs),
        f32::from(ground.width),
        f32::from(ground.height),
        0,
        0.025,
        coord.x,
        coord.y,
    );
    let ground_depths: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .filter_map(|(sprite, transform)| {
            (ground_handles.contains(&sprite.image)
                && transform.translation.truncate() == expected_ground_position.truncate())
            .then_some(transform.translation.z)
        })
        .collect();
    assert_eq!(
        ground_depths,
        vec![ground_draw_z(coord.x, coord.y, 0.025)],
        "DrawStationTile debe dejar el ground TileLayout en DrawGroundSprite"
    );
    assert_eq!(
        parent_component.bounds,
        ParentSpriteBounds::new(17, 18, 3, 20, 22, 8),
        "el parent conserva el prisma TILE_SEQ_LINE inclusivo"
    );
    assert_eq!(
        parent_component.insertion_key,
        viewport_insertion_key(1, 1, 2),
        "el BUILD entra con su ordinal estable en el compositor"
    );
    assert!(
        child_matches,
        "el child TileSeq debe permanecer unido al parent anterior"
    );
}

#[test]
fn spawn_industry_on_slope_spawns_foundation_layer() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_height(c(1, 1), 7).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(c(x, y), 4).expect("h");
    }
    let mut tile = tile_template();
    tile.kind = TileKind::Industry;
    tile.mapt = 0x80;
    tile.m5 = 11;
    tile.m1 = 0x80;
    map.set_tile(c(1, 1), tile).expect("tile");

    let grid = RenderGrid::from_map(&map, 4, 4);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    assert_ne!(ctx.info.tileh, 0);
    let entry = industry_gfx_entry_for_tile(11, 0x80, 0).expect("industria vanilla terminada");
    assert!(
        !industry_building_needs_client_anim(11, 0x80),
        "la fixture verifica la ruta estática"
    );
    let plan =
        openttdrs_core::foundation_draw_plan(ctx.info.tileh, openttdrs_core::FOUNDATION_LEVELED, 0);
    let surface_z = ctx.info.base_z.saturating_add(plan.surface_z_delta);

    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("industry slope");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    assert!(
        !foundation_parents.is_empty(),
        "DrawFoundation debe crear el parent del muro"
    );
    let (building_entity, building_parent) = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .find_map(|(entity, parent)| {
            (parent.sprite_id == entry.sprite_id).then_some((entity, *parent))
        })
        .expect("el edificio debe entrar como parent sortable propio");
    assert_eq!(
        building_parent.bounds,
        ParentSpriteBounds::new(
            16 + entry.sort_ox,
            16 + entry.sort_oy,
            i32::from(surface_z) * 8 + entry.sort_oz,
            16 + entry.sort_ox + entry.sort_ex - 1,
            16 + entry.sort_oy + entry.sort_ey - 1,
            i32::from(surface_z) * 8 + entry.sort_oz + entry.sort_ez - 1,
        ),
        "el prisma M() debe usar ti->z después de DrawFoundation"
    );

    let foundation_children: Vec<_> = world
        .query::<&ViewportSortableChild>()
        .iter(&world)
        .filter(|child| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        foundation_children.len(),
        1,
        "el suelo vanilla debe colgar del último parent de la fundación"
    );
    assert!(
        foundation_children
            .iter()
            .all(|child| child.parent != building_entity),
        "el suelo no puede confundirse con los children del edificio"
    );
}

#[test]
fn steep_animated_industry_keeps_foundation_contract_across_frames() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 4);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    // Norte dos niveles por encima del resto: `SLOPE_STEEP | SLOPE_N`.
    // `DrawFoundation(Leveled)` debe elevar la superficie resultante dos
    // unidades, no sólo la habitual de una pendiente simple.
    let industry_tile = |m3hi| Tile {
        kind: TileKind::Industry,
        mapt: 0x80,
        // Gfx 1 usa anim_state y cambia tanto el edificio como el suelo
        // mientras permanece terminado.
        m5: 1,
        m1: 0x80,
        m3hi,
        ..tile_template()
    };
    map.set_tile(c(1, 1), industry_tile(0))
        .expect("animated industry tile");
    // `set_tile` copia también `height`; restaurar la esquina después evita
    // convertir accidentalmente la fixture en una pendiente irregular.
    map.set_height(c(1, 1), 6).expect("steep north corner");

    let grid = RenderGrid::from_map(&map, 4, 4);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    let plan =
        openttdrs_core::foundation_draw_plan(ctx.info.tileh, openttdrs_core::FOUNDATION_LEVELED, 0);
    assert_eq!(
        plan.surface_z_delta, 2,
        "la fixture debe ejercer la elevación de una pendiente empinada"
    );
    let surface_z = ctx.info.base_z.saturating_add(plan.surface_z_delta);
    assert!(industry_building_needs_client_anim(1, 0x80));
    let initial = industry_gfx_entry_for_tile(1, 0x80, 0).expect("primer frame de torre");
    let next = industry_gfx_entry_for_tile(1, 0x80, 1).expect("segundo frame de torre");

    let mut sim_state = GameState::new(4, 4);
    sim_state
        .map
        .set_tile(c(1, 1), industry_tile(0))
        .expect("simulated animated industry tile");
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets.clone()));
    world.insert_resource(assets);
    world.insert_resource(crate::state::SimWorld {
        state: sim_state,
        loaded_file: false,
        ottdmap_extras: None,
    });
    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("animated industry slope spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    let (building_entity, building_parent) = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .find_map(|(entity, parent)| {
            (parent.sprite_id == initial.sprite_id).then_some((entity, *parent))
        })
        .expect("el edificio animado debe abrir un parent aun sobre fundación");
    assert_eq!(
        building_parent.bounds.zmin,
        i32::from(surface_z) * 8 + initial.sort_oz,
        "el prisma inicial usa la superficie efectiva empinada"
    );
    let (ground_entity, ground_child) = world
        .query::<(Entity, &ViewportSortableChild)>()
        .iter(&world)
        .find_map(|(entity, child)| {
            foundation_parents
                .contains(&child.parent)
                .then_some((entity, *child))
        })
        .expect("el suelo animado debe ser child de la fundación");
    assert!(
        !foundation_parents.contains(&building_entity),
        "el edificio no puede reutilizar el parent del muro"
    );

    world
        .resource_mut::<crate::state::SimWorld>()
        .state
        .map
        .set_tile(c(1, 1), industry_tile(1))
        .expect("segundo frame simulado");
    world
        .run_system_once(crate::render::industry_anim::animate_industry_building_layers)
        .expect("animated industry update");

    let updated_parent = world
        .get::<ViewportSortableParent>(building_entity)
        .expect("el parent del edificio debe sobrevivir al cambio de frame");
    assert_eq!(updated_parent.sprite_id, next.sprite_id);
    assert_eq!(
        updated_parent.bounds.zmin,
        i32::from(surface_z) * 8 + next.sort_oz,
        "el frame vivo no puede restaurar la altura cruda de la tesela"
    );
    let updated_ground = world
        .get::<ViewportSortableChild>(ground_entity)
        .expect("el child de suelo debe sobrevivir al cambio de frame");
    assert_eq!(updated_ground.parent, ground_child.parent);
    assert!(
        foundation_parents.contains(&updated_ground.parent),
        "el frame vivo conserva la asociación del suelo con DrawFoundation"
    );
}

#[test]
fn sloped_industry_draw_proc_layers_follow_the_building_not_foundation() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_height(c(1, 1), 7).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(c(x, y), 4).expect("h");
    }
    map.set_tile(
        c(1, 1),
        Tile {
            kind: TileKind::Industry,
            mapt: 0x80,
            // Toy Factory: combina suelo propio, edificio sortable y los
            // cuatro slots dinámicos de `IndustryDrawToyFactory`.
            m5: 143,
            m1: 0x80,
            ..tile_template()
        },
    )
    .expect("toy factory slope");
    let building = industry_gfx_entry_for_tile(143, 0x80, 0).expect("toy factory entry");
    let proc = crate::sprites::industry_draw_proc_for_tile(143, 0x80);
    let expected_slots = usize::from(crate::sprites::industry_draw_proc_layer_slot_count(proc));
    assert_eq!(proc, 4, "toy factory uses IndustryDrawToyFactory");
    assert!(expected_slots > 0, "el draw proc debe materializar slots");

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("toy factory slope spawn");

    let foundation_parents: std::collections::HashSet<_> = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .filter_map(|(entity, parent)| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
                .then_some(entity)
        })
        .collect();
    let building_parent = world
        .query::<(Entity, &ViewportSortableParent)>()
        .iter(&world)
        .find_map(|(entity, parent)| (parent.sprite_id == building.sprite_id).then_some(entity))
        .expect("toy factory building parent on slope");
    assert!(
        !foundation_parents.contains(&building_parent),
        "el edificio debe ser un parent distinto del muro de fundación"
    );

    let children: Vec<_> = world
        .query::<&ViewportSortableChild>()
        .iter(&world)
        .collect();
    assert_eq!(
        children
            .iter()
            .filter(|child| foundation_parents.contains(&child.parent))
            .count(),
        1,
        "DrawGroundSprite queda bajo la fundación"
    );
    assert_eq!(
        children
            .iter()
            .filter(|child| child.parent == building_parent)
            .count(),
        expected_slots,
        "los AddChildSpriteScreen de draw_proc quedan bajo el edificio"
    );
}

#[test]
fn sloped_newgrf_industry_overlay_is_child_of_foundation() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_height(c(1, 1), 7).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(c(x, y), 4).expect("h");
    }
    let mut tile = tile_template();
    tile.kind = TileKind::Industry;
    tile.mapt = 0x80;
    tile.m5 = 175; // primer slot IndustryTile NewGRF.
    tile.m1 = 0x80;
    map.set_tile(c(1, 1), tile).expect("tile");
    let view = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: vec![
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ],
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![view.clone()]],
        assigns: vec![TrainSpriteAssign {
            local_id: 3,
            set_id: 0,
        }],
        ..Default::default()
    };
    runtime.action2_var.insert(
        0,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x5F,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: 0xFF,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        },
    );
    let def = IndustryTileSpecDef {
        gfx: IndustryTileGfxId(175),
        subst_id: 0,
        from_newgrf: true,
        slopes_refused: 0,
        accepts_cargo_indices: Vec::new(),
        accepts_cargo_labels: Vec::new(),
        acceptance: Vec::new(),
        callback_mask: 0,
        animation_frames: 0,
        animation_status: 0,
        animation_speed: 0,
        animation_triggers: 0,
        animation_special_flags: 0,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        newgrf_local_id: 3,
        newgrf_grfid: 0,
        newgrf_preview: Some(view.clone()),
        newgrf_views: vec![view],
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let industry_catalog = vec![def];

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut company: Local<CompanyColoredSprites>,
                  mut images: Local<Assets<Image>>| {
                let mut cache = crate::render::NewGrfIndustrySpriteCache::default();
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &industry_catalog,
                    &openttdrs_core::empty_industry_tile_overrides(),
                    Some(&mut cache),
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("newgrf industry slope");

    let parents: Vec<_> = world
        .query_filtered::<Entity, With<ViewportSortableParent>>()
        .iter(&world)
        .collect();
    assert!(
        !parents.is_empty(),
        "la fundación debe crear un parent sortable"
    );
    let child_parents: Vec<_> = world
        .query::<&ViewportSortableChild>()
        .iter(&world)
        .map(|child| child.parent)
        .collect();
    assert_eq!(
        child_parents.len(),
        1,
        "el overlay NewGRF debe ser un child"
    );
    assert!(
        parents.contains(&child_parents[0]),
        "el overlay debe colgar de la fundación de industria"
    );
}

#[test]
fn flat_newgrf_industry_tile_layout_keeps_ground_in_ground_pass() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let mut map = Map::new_flat(4, 4, 0);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Industry,
            mapt: 0x80,
            m5: 175,
            m1: 0x80,
            ..tile_template()
        },
    )
    .expect("industry TileLayout tile");

    let ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [240, 10, 10, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![ground.clone()]],
        assigns: vec![TrainSpriteAssign {
            local_id: 3,
            set_id: 9,
        }],
        ..Default::default()
    };
    runtime.tile_layouts.insert(
        9,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(0),
                ..Default::default()
            },
            sequence: Vec::new(),
        },
    );
    let industry_def = IndustryTileSpecDef {
        gfx: IndustryTileGfxId(175),
        subst_id: 0,
        from_newgrf: true,
        slopes_refused: 0,
        accepts_cargo_indices: Vec::new(),
        accepts_cargo_labels: Vec::new(),
        acceptance: Vec::new(),
        callback_mask: 0,
        animation_frames: 0,
        animation_status: 0,
        animation_speed: 0,
        animation_triggers: 0,
        animation_special_flags: 0,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        newgrf_local_id: 3,
        newgrf_grfid: 0x494E_4454,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_runtime: Some(Box::new(runtime)),
    };

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfIndustrySpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut company: Local<CompanyColoredSprites>,
                  mut cache: ResMut<crate::render::NewGrfIndustrySpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    std::slice::from_ref(&industry_def),
                    &openttdrs_core::empty_industry_tile_overrides(),
                    Some(&mut cache),
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("industry TileLayout spawn");

    let expected_position = overlay_pos(
        crate::iso::iso(coord.x, coord.y),
        f32::from(ground.x_offs),
        f32::from(ground.y_offs),
        f32::from(ground.width),
        f32::from(ground.height),
        0,
        0.45,
        coord.x,
        coord.y,
    );
    let sprites: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .map(|(sprite, transform)| (sprite.image.clone(), transform.translation))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let ground_depths: Vec<_> = sprites
        .iter()
        .filter_map(|(handle, translation)| {
            (images.get(handle).and_then(|image| image.data.as_deref())
                == Some(ground.rgba.as_slice())
                && translation.truncate() == expected_position.truncate())
            .then_some(translation.z)
        })
        .collect();
    assert_eq!(
        ground_depths,
        vec![ground_draw_z(coord.x, coord.y, 0.45)],
        "DrawNewIndustryTile debe dejar el ground TileLayout en DrawGroundSprite"
    );
}

/// Los TileLayouts pueden referir el suelo del baseset directamente, sin un
/// set Action1 propio. Para los tres rombos planos auditados, el renderer usa
/// el atlas con su ancla NFO exacta y no degrada toda la tesela al fallback.
#[test]
fn flat_newgrf_industry_tile_layout_renders_audited_direct_base_ground() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    for direct_sprite in [3924_u16, 3981, 4000] {
        let assets = boot_assets_app();
        let expected = match direct_sprite {
            3924 => assets
                .industries
                .get(&3924)
                .expect("bare-land atlas sprite")
                .clone(),
            3981 => assets.grass.clone(),
            4000 => assets.rough_flat[0].clone(),
            _ => unreachable!(),
        };
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        map.set_tile(
            coord,
            Tile {
                kind: TileKind::Industry,
                mapt: 0x80,
                m5: 175,
                m1: 0x80,
                ..tile_template()
            },
        )
        .expect("industry TileLayout tile");

        let mut runtime = TrainSpriteGraphics {
            assigns: vec![TrainSpriteAssign {
                local_id: 3,
                set_id: 9,
            }],
            ..Default::default()
        };
        runtime.tile_layouts.insert(
            9,
            TileLayout {
                ground: TileLayoutSpriteRef {
                    direct_sprite,
                    ..Default::default()
                },
                sequence: Vec::new(),
            },
        );
        let industry_def = IndustryTileSpecDef {
            gfx: IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 3,
            newgrf_grfid: 0x4449_5247,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
        };

        let grid = RenderGrid::from_map(&map, 4, 4);
        let mut world = World::new();
        world.insert_resource(TsMap(map));
        world.insert_resource(TsGrid(grid));
        world.insert_resource(TsAssets(assets));
        world.insert_resource(crate::render::NewGrfIndustrySpriteCache::default());
        world.insert_resource(Assets::<Image>::default());
        world
            .run_system_once(
                move |mut commands: Commands,
                      m: Res<TsMap>,
                      g: Res<TsGrid>,
                      a: Res<TsAssets>,
                      mut company: Local<CompanyColoredSprites>,
                      mut cache: ResMut<crate::render::NewGrfIndustrySpriteCache>,
                      mut images: ResMut<Assets<Image>>| {
                    spawn_industry_tile(
                        &mut commands,
                        &a.0,
                        &m.0,
                        &TileRenderContext::new(&m.0, &g.0, 1, 1),
                        4.0,
                        &[],
                        &mut company,
                        &mut images,
                        std::slice::from_ref(&industry_def),
                        &openttdrs_core::empty_industry_tile_overrides(),
                        Some(&mut cache),
                        &[],
                        None,
                        &[],
                    );
                },
            )
            .expect("direct-base industry TileLayout spawn");

        let expected_position = overlay_pos(
            crate::iso::iso(coord.x, coord.y),
            -31.0,
            0.0,
            64.0,
            31.0,
            0,
            0.45,
            coord.x,
            coord.y,
        );
        let depths: Vec<_> = world
            .query::<(&Sprite, &Transform)>()
            .iter(&world)
            .filter_map(|(sprite, transform)| {
                (expected.matches(sprite)
                    && transform.translation.truncate() == expected_position.truncate())
                .then_some(transform.translation.z)
            })
            .collect();
        assert_eq!(
            depths,
            vec![ground_draw_z(coord.x, coord.y, 0.45)],
            "SpriteID base {direct_sprite} debe conservar atlas, ancla y ground pass"
        );
    }
}

#[test]
fn flat_newgrf_object_tile_layout_keeps_ground_in_ground_pass() {
    use openttdrs_core::newgrf_sprites::{TileLayout, TileLayoutSpriteRef};

    let assets = boot_assets_app();
    let coord = TileCoord::new(1, 1);
    let object_type = openttdrs_core::NEW_OBJECT_OFFSET;
    let mut map = Map::new_flat(4, 4, 0);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Grass,
            mapt: 0xA0,
            m5: u8::try_from(object_type).expect("NewGRF object type byte"),
            ..tile_template()
        },
    )
    .expect("object TileLayout tile");

    let ground = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [240, 10, 10, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut runtime = TrainSpriteGraphics {
        sets: vec![vec![ground.clone()]],
        assigns: vec![TrainSpriteAssign {
            local_id: 4,
            set_id: 9,
        }],
        ..Default::default()
    };
    runtime.tile_layouts.insert(
        9,
        TileLayout {
            ground: TileLayoutSpriteRef {
                action1_set: Some(0),
                ..Default::default()
            },
            sequence: Vec::new(),
        },
    );
    let object_def = ObjectSpecDef {
        id: object_type,
        class_label: "TEST".into(),
        name: "TileLayout object".into(),
        size: openttdrs_core::OBJECT_SIZE_1X1,
        from_newgrf: true,
        local_id: 4,
        grfid: 0x4F42_4A54,
        newgrf_grf_version: 8,
        climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
        build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
        clear_cost_factor: openttdrs_core::DEFAULT_OBJECT_CLEAR_COST_FACTOR,
        flags: 0,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        animation_triggers: 0,
        callback_mask: 0,
        views: vec![ground.clone()],
        newgrf_runtime: Some(Box::new(runtime)),
        associated_badges: Vec::new(),
    };

    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(crate::render::NewGrfObjectSpriteCache::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: ResMut<crate::render::NewGrfObjectSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    m.0.dimensions().0,
                    std::slice::from_ref(&object_def),
                    &[],
                    Some(&mut cache),
                    Some(&mut images),
                );
            },
        )
        .expect("object TileLayout spawn");

    let expected_position = overlay_pos(
        crate::iso::iso(coord.x, coord.y),
        f32::from(ground.x_offs),
        f32::from(ground.y_offs),
        f32::from(ground.width),
        f32::from(ground.height),
        0,
        0.55,
        coord.x,
        coord.y,
    );
    let sprites: Vec<_> = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .map(|(sprite, transform)| (sprite.image.clone(), transform.translation))
        .collect();
    let images = world.resource::<Assets<Image>>();
    let ground_depths: Vec<_> = sprites
        .iter()
        .filter_map(|(handle, translation)| {
            (images.get(handle).and_then(|image| image.data.as_deref())
                == Some(ground.rgba.as_slice())
                && translation.truncate() == expected_position.truncate())
            .then_some(translation.z)
        })
        .collect();
    assert_eq!(
        ground_depths,
        vec![ground_draw_z(coord.x, coord.y, 0.55)],
        "DrawNewObjectTile debe dejar el ground TileLayout en DrawGroundSprite"
    );
}

#[test]
fn newgrf_industry_draw_foundations_callback_can_suppress_default() {
    let assets = boot_assets_app();
    let mut map = Map::new_flat(4, 4, 0);
    let coord = TileCoord::new(1, 1);
    map.set_height(coord, 7).expect("h");
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        map.set_height(TileCoord::new(x, y), 4).expect("h");
    }
    let mut tile = tile_template();
    tile.kind = TileKind::Industry;
    tile.mapt = 0x80;
    tile.m5 = 175;
    tile.m1 = 0x80;
    map.set_tile(coord, tile).expect("industry tile");
    let view = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: -1,
        y_offs: -2,
        rgba: [40, 220, 40, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut runtime = callback_literal_runtime(3, 0);
    runtime.sets = vec![vec![view.clone()]];
    let def = IndustryTileSpecDef {
        gfx: IndustryTileGfxId(175),
        subst_id: 0,
        from_newgrf: true,
        slopes_refused: 0,
        accepts_cargo_indices: Vec::new(),
        accepts_cargo_labels: Vec::new(),
        acceptance: Vec::new(),
        callback_mask: openttdrs_core::INDUSTRY_TILE_CALLBACK_DRAW_FOUNDATIONS_MASK,
        animation_frames: 0,
        animation_status: 0,
        animation_speed: 0,
        animation_triggers: 0,
        animation_special_flags: 0,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
        newgrf_local_id: 3,
        newgrf_grfid: 0,
        newgrf_preview: Some(view.clone()),
        newgrf_views: vec![view],
        newgrf_runtime: Some(Box::new(runtime)),
    };
    let grid = RenderGrid::from_map(&map, 4, 4);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut company: Local<CompanyColoredSprites>,
                  mut images: Local<Assets<Image>>| {
                let mut cache = crate::render::NewGrfIndustrySpriteCache::default();
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    std::slice::from_ref(&def),
                    &openttdrs_core::empty_industry_tile_overrides(),
                    Some(&mut cache),
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("industry draw-foundations callback");

    let foundation_count = world
        .query::<&ViewportSortableParent>()
        .iter(&world)
        .filter(|parent| {
            (FOUNDATION_ORIGINAL_SPRITE_BASE..=FOUNDATION_ORIGINAL_SPRITE_BASE.saturating_add(14))
                .contains(&parent.sprite_id)
        })
        .count();
    assert_eq!(
        foundation_count, 0,
        "CB 0x30 = 0 debe suprimir la fundación de industria"
    );
}

/// `industry_land.h`: GFX 7 usa `s1=0xF54` / `SPR_FLAT_BARE_LAND` y el
/// edificio 2047. La omisión histórica de esa capa dejaba tierra áspera bajo
/// la planta y 36 comandos 3924 sin equivalente al contrastar Kale.
#[test]
fn industry_bare_land_ground_is_drawn_before_power_plant() {
    let assets = boot_assets_app();
    let expected_ground = assets.industries[&3924].clone();
    let expected_building = assets.industries[&2047].clone();
    let mut map = fresh_map8();
    let c = TileCoord::new(2, 2);
    let mut tile = tile_template();
    tile.kind = TileKind::Industry;
    tile.mapt = 0x80;
    tile.m5 = 7; // power plant, fila terminada de industry_land.h.
    tile.m1 = 0x80;
    map.set_tile(c, tile).expect("power plant");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("spawn power plant");

    let sprites: Vec<_> = world.query::<&Sprite>().iter(&world).collect();
    assert_eq!(
        sprites.len(),
        2,
        "suelo 3924 + edificio 2047, sin rough extra"
    );
    assert!(sprites.iter().any(|sprite| expected_ground.matches(sprite)));
    assert!(
        sprites
            .iter()
            .any(|sprite| expected_building.matches(sprite))
    );
}

#[test]
fn spawn_bridge_middle_draws_deck_over_marked_water() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // Rampa SW en (1,1), vano sobre agua en (2,1) y (3,1), rampa NE en (4,1).
    let mut ramp = tile_template();
    ramp.kind = TileKind::RoadBridge;
    ramp.mapt = 0x90;
    ramp.m5 = 0x86; // puente + dir SW + TransportType road (1)
    map.set_tile(c(1, 1), ramp).expect("ramp");
    ramp.m5 = 0x84; // puente + dir NE + road
    map.set_tile(c(4, 1), ramp).expect("ramp");
    for x in 2..=3 {
        let mut water = tile_template();
        water.kind = TileKind::Water;
        water.mapt = 0x64; // MP_WATER + bridge above eje X (bits 2–3 = 1)
        map.set_tile(c(x, 1), water).expect("water");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let dims = m.0.dimensions();
                // Vano marcado: tablero + barandilla + pilar.
                spawn_bridge_middle(
                    &mut commands,
                    &m.0,
                    dims,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 1),
                    false,
                    &[],
                    None,
                    &[],
                    None,
                    None,
                );
                // Tesela sin puente encima: no debe agregar nada.
                spawn_bridge_middle(
                    &mut commands,
                    &m.0,
                    dims,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 6, 6),
                    false,
                    &[],
                    None,
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("bridge middle");

    // Tablero + barandilla frontal + 1 pilar (deck_z 1, suelo 0).
    let sprites = world.query::<&Sprite>().iter(&world).count();
    assert_eq!(sprites, 3, "vano dibuja tablero, barandilla y pilar");
}

#[test]
fn bridge_middle_uses_south_ramp_tram_overlay_as_combined_child() {
    let assets = boot_assets_app();
    let expected_overlay = assets.tram_flat[1].clone();
    // `offset=1` para un vano X: `GetBridgeRoadCatenary` escoge las filas
    // 96/98 del bloque vanilla de tranvía (6082/6084 globales).
    let expected_catenary_back = assets.rail.get(&6082).expect("catenaria trasera").clone();
    let expected_catenary_front = assets.rail.get(&6084).expect("catenaria delantera").clone();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // El tramo intermedio es agua: sólo la rampa sur conserva los bits de
    // tranvía que `DrawBridgeRoadBits` recibe como `head_tile`.
    let mut ramp = tile_template();
    ramp.kind = TileKind::RoadBridge;
    ramp.mapt = 0x90;
    ramp.m3 = 0x05;
    ramp.m5 = 0x86; // bridge + SW + road
    map.set_tile(c(1, 1), ramp).expect("rampa oeste");
    ramp.m5 = 0x84; // bridge + NE + road
    map.set_tile(c(4, 1), ramp).expect("rampa este");
    for x in 2..=3 {
        let mut water = tile_template();
        water.kind = TileKind::Water;
        water.mapt = 0x64; // MP_WATER + bridge above eje X
        map.set_tile(c(x, 1), water).expect("vano de agua");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_bridge_middle(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 1),
                    false,
                    &[],
                    None,
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("bridge tram overlay spawn");

    let attached: Vec<_> = world
        .query::<(Entity, &ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .filter(|(_, _, sprite)| expected_overlay.matches(sprite))
        .collect();
    assert_eq!(
        attached.len(),
        1,
        "el overlay de tranvía debe aparecer una vez"
    );
    assert!(
        world
            .entity(attached[0].1.parent)
            .contains::<ViewportSortableParent>(),
        "el overlay debe colgar del parent trasero combinado"
    );

    let catenary_children: Vec<_> = world
        .query::<(Entity, &ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .filter(|(_, _, sprite)| {
            expected_catenary_back.matches(sprite) || expected_catenary_front.matches(sprite)
        })
        .collect();
    assert_eq!(
        catenary_children.len(),
        2,
        "el fallback vanilla debe emitir las dos mitades de catenaria del puente"
    );
    assert!(catenary_children.iter().all(|(_, child, _)| {
        world
            .entity(child.parent)
            .contains::<ViewportSortableParent>()
    }));
}

#[test]
fn bridge_middle_resolves_newgrf_bridge_overlay_and_catenary_groups_from_south_ramp() {
    use openttdrs_core::newgrf_sprites::{DecodedSprite, TrainSpriteAssign, TrainSpriteGraphics};

    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    let mut ramp = tile_template();
    ramp.kind = TileKind::RoadBridge;
    ramp.mapt = 0x90;
    ramp.m5 = 0x86; // rampa SW, puente de carretera
    ramp = openttdrs_core::set_road_type_on_tile(ramp, RoadType::from_u8(2));
    map.set_tile(c(1, 1), ramp).expect("rampa oeste");
    ramp.m5 = 0x84; // rampa NE
    map.set_tile(c(4, 1), ramp).expect("rampa este");
    for x in 2..=3 {
        let mut water = tile_template();
        water.kind = TileKind::Water;
        water.mapt = 0x64; // MP_WATER + puente encima eje X
        map.set_tile(c(x, 1), water).expect("vano de agua");
    }

    let red = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: 0,
        y_offs: 0,
        rgba: [255, 0, 0, 255].repeat(4),
        mask: Vec::new(),
    };
    let blue = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: 0,
        y_offs: 0,
        rgba: [0, 0, 255, 255].repeat(4),
        mask: Vec::new(),
    };
    let green = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: 0,
        y_offs: 0,
        rgba: [0, 255, 0, 255].repeat(4),
        mask: Vec::new(),
    };
    let yellow = DecodedSprite {
        width: 2,
        height: 2,
        x_offs: 0,
        y_offs: 0,
        rgba: [255, 255, 0, 255].repeat(4),
        mask: Vec::new(),
    };
    let mut graphics = TrainSpriteGraphics {
        sets: vec![vec![red], vec![blue], vec![green], vec![yellow]],
        assigns: vec![TrainSpriteAssign {
            local_id: 0,
            set_id: 0,
        }],
        ..TrainSpriteGraphics::default()
    };
    graphics.specific_assigns.insert((0, 6), 0); // ROTSG_BRIDGE
    graphics.specific_assigns.insert((0, 1), 1); // ROTSG_OVERLAY
    graphics.specific_assigns.insert((0, 5), 2); // ROTSG_CATENARY_BACK
    graphics.specific_assigns.insert((0, 4), 3); // ROTSG_CATENARY_FRONT
    let road_def = RoadTypeDef {
        id: RoadType::from_u8(2),
        class: RoadTramType::Road,
        label: "Puente NewGRF".into(),
        short_label: "NGBR".into(),
        intro_year: 0,
        max_speed: 0,
        cost_multiplier: 0,
        maintenance_multiplier: 0,
        flags: 1, // RoadTypeFlag::Catenary
        powered_mask: 0,
        badges: Vec::new(),
        from_tramtypes_feature: false,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(graphics)),
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    };
    let road_catalog = vec![road_def];
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  m: Res<TsMap>,
                  g: Res<TsGrid>,
                  a: Res<TsAssets>,
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: ResMut<Assets<Image>>| {
                spawn_bridge_middle_with_road_types(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 1),
                    false,
                    TEST_CLIMATE,
                    &road_catalog,
                    Some(&mut cache),
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                    Some(&mut images),
                );
            },
        )
        .expect("bridge NewGRF groups");

    let custom_handles: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite)>()
        .iter(&world)
        .map(|(child, sprite)| (child.parent, sprite.image.clone()))
        .collect();
    let custom_handles: Vec<_> = custom_handles
        .into_iter()
        .filter_map(|(parent, handle)| {
            let image = world.resource::<Assets<Image>>().get(&handle)?;
            let first = image.data.as_deref()?.get(0..4)?;
            (first == [255, 0, 0, 255]
                || first == [0, 0, 255, 255]
                || first == [0, 255, 0, 255]
                || first == [255, 255, 0, 255])
            .then_some((parent, first.to_vec()))
        })
        .collect();
    assert_eq!(
        custom_handles.len(),
        4,
        "bridge, overlay y ambos grupos de catenaria deben ser children"
    );
    assert!(
        custom_handles
            .iter()
            .all(|(parent, _)| world.entity(*parent).contains::<ViewportSortableParent>())
    );
    assert!(
        custom_handles
            .iter()
            .any(|(_, rgba)| rgba == &[255, 0, 0, 255])
    );
    assert!(
        custom_handles
            .iter()
            .any(|(_, rgba)| rgba == &[0, 0, 255, 255])
    );
    assert!(
        custom_handles
            .iter()
            .any(|(_, rgba)| rgba == &[0, 255, 0, 255])
    );
    assert!(
        custom_handles
            .iter()
            .any(|(_, rgba)| rgba == &[255, 255, 0, 255])
    );
}

#[test]
fn bridge_pbs_overlay_stays_attached_to_the_rear_combined_parent() {
    let assets = boot_assets_app();
    let expected_pbs = assets.pbs_rail_sprite(1005).expect("reserva PBS X").clone();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // Puente ferroviario X con reserva en el extremo norte. El vano está
    // sobre agua para recorrer el mismo camino que un puente real de Kale.
    let mut ramp = tile_template();
    ramp.kind = TileKind::RailBridge;
    ramp.mapt = 0x90;
    ramp.m5 = 0x92; // bridge + SW + HasTunnelBridgeReservation
    ramp.m8 = RailType::Rail as u16;
    ramp.m6 = openttdrs_core::set_bridge_type_m6(0, BridgeType::CantileverRed);
    map.set_tile(c(1, 1), ramp).expect("rampa oeste");
    ramp.m5 = 0x90; // bridge + NE + reserva
    map.set_tile(c(4, 1), ramp).expect("rampa este");
    for x in 2..=3 {
        let mut water = tile_template();
        water.kind = TileKind::Water;
        water.mapt = 0x64; // MP_WATER + bridge above eje X
        map.set_tile(c(x, 1), water).expect("vano de agua");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_bridge_middle(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 1),
                    true,
                    &[],
                    None,
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("bridge PBS spawn");

    let mut children = world.query::<(Entity, &ViewportSortableChild, &Sprite)>();
    let attached: Vec<_> = children
        .iter(&world)
        .filter(|(_, _, sprite)| expected_pbs.matches(sprite))
        .collect();
    assert_eq!(attached.len(), 1, "el overlay PBS debe dibujarse una vez");
    let (_, child, _) = attached[0];
    assert!(
        world
            .entity(child.parent)
            .contains::<ViewportSortableParent>(),
        "el overlay PBS debe colgar del parent trasero del bloque combinado"
    );
    assert_eq!(
        child.source_depth,
        world
            .entity(attached[0].0)
            .get::<Transform>()
            .unwrap()
            .translation
            .z
    );
}

#[test]
fn rail_under_bridge_above_is_not_skipped() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(3, 3);
    let rail_under_bridge = Tile {
        kind: TileKind::Rail,
        // MP_RAILWAY + `IsBridgeAbove` sobre eje X (bits 2--3 = 1).
        // La vía inferior sigue siendo una vía X normal en m5.
        mapt: 0x14,
        m5: 0x01,
        ..tile_template()
    };
    map.set_tile(coord, rail_under_bridge)
        .expect("rail below bridge");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let mut rail_layers = Vec::new();
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    m.0.dimensions(),
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    4.0,
                    &mut rail_layers,
                    TEST_CLIMATE,
                    false,
                    false,
                    false,
                    &[],
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &openttdrs_core::RailTypeRuntimeProps::defaults(),
                    None,
                    &[],
                    &[],
                    None,
                    None,
                    0,
                    &[],
                );
            },
        )
        .expect("spawn rail below bridge");

    assert_eq!(
        world.query::<&Sprite>().iter(&world).count(),
        1,
        "la vía inferior se pinta antes de sumar el tablero del puente"
    );
}

#[test]
fn power_plant_chimney_spawns_animated_smoke() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // GFX_POWERPLANT_CHIMNEY (gfx 8) terminada y otra en obra (sin humo).
    let mut chimney = tile_template();
    chimney.kind = TileKind::Industry;
    chimney.mapt = 0x80;
    chimney.m5 = 8;
    chimney.m1 = 0x80;
    map.set_tile(c(2, 2), chimney).expect("chimenea");
    chimney.m1 = 0x01;
    map.set_tile(c(3, 2), chimney).expect("en obra");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    let spawn_at = |world: &mut World, tx: u32| {
        world
            .run_system_once(
                move |mut commands: Commands,
                      m: Res<TsMap>,
                      g: Res<TsGrid>,
                      a: Res<TsAssets>,
                      mut company: Local<CompanyColoredSprites>,
                      mut images: Local<Assets<Image>>| {
                    spawn_industry_tile(
                        &mut commands,
                        &a.0,
                        &m.0,
                        &TileRenderContext::new(&m.0, &g.0, tx, 2),
                        4.0,
                        &[],
                        &mut company,
                        &mut images,
                        &[],
                        &openttdrs_core::empty_industry_tile_overrides(),
                        None,
                        &[],
                        None,
                        &[],
                    );
                },
            )
            .expect("spawn industry");
        world
            .query_filtered::<(), With<crate::render::smoke::ChimneySmoke>>()
            .iter(world)
            .count()
    };
    assert_eq!(spawn_at(&mut world, 2), 1, "terminada: penacho de humo");
    assert_eq!(spawn_at(&mut world, 3), 1, "en obra: sin humo nuevo");
    assert_eq!(
        world
            .query_filtered::<&ViewportSortableParent, With<crate::render::smoke::ChimneySmoke>>()
            .iter(&world)
            .count(),
        1,
        "el penacho terminado debe entrar como parent al compositor global"
    );
}

#[test]
fn copper_mine_chimney_spawns_animated_smoke() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    let mut chimney = tile_template();
    chimney.kind = TileKind::Industry;
    chimney.mapt = 0x80;
    chimney.m5 = 49;
    chimney.m1 = 0x80;
    map.set_tile(c(2, 2), chimney).expect("chimenea cobre");
    chimney.m1 = 0x01;
    map.set_tile(c(3, 2), chimney).expect("en obra");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    let spawn_at = |world: &mut World, tx: u32| {
        world
            .run_system_once(
                move |mut commands: Commands,
                      m: Res<TsMap>,
                      g: Res<TsGrid>,
                      a: Res<TsAssets>,
                      mut company: Local<CompanyColoredSprites>,
                      mut images: Local<Assets<Image>>| {
                    spawn_industry_tile(
                        &mut commands,
                        &a.0,
                        &m.0,
                        &TileRenderContext::new(&m.0, &g.0, tx, 2),
                        4.0,
                        &[],
                        &mut company,
                        &mut images,
                        &[],
                        &openttdrs_core::empty_industry_tile_overrides(),
                        None,
                        &[],
                        None,
                        &[],
                    );
                },
            )
            .expect("spawn industry");
        world
            .query_filtered::<(), With<crate::render::smoke::CopperMineSmoke>>()
            .iter(world)
            .count()
    };
    assert_eq!(spawn_at(&mut world, 2), 1, "terminada: humo mina cobre");
    assert_eq!(spawn_at(&mut world, 3), 1, "en obra: sin humo nuevo");
    assert_eq!(
        world
            .query_filtered::<&ViewportSortableParent, With<crate::render::smoke::CopperMineSmoke>>(
            )
            .iter(&world)
            .count(),
        1,
        "el humo de cobre terminado debe entrar como parent al compositor global"
    );
}

#[test]
fn palette_animated_refinery_building_joins_global_sorter() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(2, 2);
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Industry,
            mapt: 0x80,
            // GFX_OIL_REFINERY_TOWER: cambia la paleta de su llama, no su
            // sprite lógico ni el prisma M() de DrawTile_Industry.
            m5: 19,
            m1: 0x80,
            ..tile_template()
        },
    )
    .expect("refinery tile");
    let expected =
        crate::sprites::industry_gfx_entry_for_tile(19, 0x80, 0).expect("refinery entry");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("refinery spawn");

    let mut refinery = world
        .query_filtered::<
            (&crate::render::RefineryFireAnim, &ViewportSortableParent),
            With<crate::render::RefineryFireAnim>,
        >();
    let (fire, parent) = refinery.single(&world).expect("torre de fuego");
    assert_eq!(fire.sprite_id, expected.sprite_id);
    assert_eq!(parent.sprite_id, expected.sprite_id);
    assert_eq!(parent.insertion_key, viewport_insertion_key(2, 2, 2));
    assert_eq!(
        parent.bounds,
        ParentSpriteBounds::new(
            32 + expected.sort_ox,
            32 + expected.sort_oy,
            expected.sort_oz,
            32 + expected.sort_ox + expected.sort_ex - 1,
            32 + expected.sort_oy + expected.sort_ey - 1,
            expected.sort_oz + expected.sort_ez - 1,
        )
    );
}

#[test]
fn flat_industry_draw_proc_layers_follow_building_parent() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let coord = TileCoord::new(2, 2);
    // GFX_TOY_FACTORY: su `draw_proc` emite varios children de pantalla que
    // cambian con m3hi, mientras el edificio base conserva su prisma M().
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Industry,
            mapt: 0x80,
            m5: 143,
            m1: 0x80,
            ..tile_template()
        },
    )
    .expect("toy factory tile");
    let building =
        crate::sprites::industry_gfx_entry_for_tile(143, 0x80, 0).expect("toy factory entry");
    let proc = crate::sprites::industry_draw_proc_for_tile(143, 0x80);
    assert_eq!(proc, 4, "toy factory uses IndustryDrawToyFactory");
    let expected_layers = crate::sprites::industry_draw_proc_dynamic_layers(proc, 0x80, 0);
    let expected_slots = usize::from(crate::sprites::industry_draw_proc_layer_slot_count(proc));
    let (later_frame, later_layers) = (0..=u8::MAX)
        .map(|frame| {
            (
                frame,
                crate::sprites::industry_draw_proc_dynamic_layers(proc, 0x80, frame),
            )
        })
        .find(|(_, layers)| layers.len() > expected_layers.len())
        .expect("un frame posterior debe revelar una capa opcional");
    assert!(
        !expected_layers.is_empty(),
        "el frame inicial de fábrica de juguetes debe tener children draw-proc"
    );

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets.clone()));
    world.insert_resource(assets);
    let mut sim_state = GameState::new(8, 8);
    sim_state
        .map
        .set_tile(
            coord,
            Tile {
                kind: TileKind::Industry,
                mapt: 0x80,
                m5: 143,
                m1: 0x80,
                ..tile_template()
            },
        )
        .expect("toy factory simulation tile");
    world.insert_resource(crate::state::SimWorld {
        state: sim_state,
        loaded_file: false,
        ottdmap_extras: None,
    });
    world
        .run_system_once(
            |mut commands: Commands,
             m: Res<TsMap>,
             g: Res<TsGrid>,
             a: Res<TsAssets>,
             mut company: Local<CompanyColoredSprites>,
             mut images: Local<Assets<Image>>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &m.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    &[],
                    &mut company,
                    &mut images,
                    &[],
                    &openttdrs_core::empty_industry_tile_overrides(),
                    None,
                    &[],
                    None,
                    &[],
                );
            },
        )
        .expect("toy factory spawn");

    let mut parents = world.query::<(Entity, &ViewportSortableParent)>();
    let (parent_entity, parent) = parents
        .iter(&world)
        .find(|(_, parent)| parent.sprite_id == building.sprite_id)
        .expect("toy factory building parent");
    assert_eq!(parent.insertion_key, viewport_insertion_key(2, 2, 2));

    let initial_visible = {
        let mut children = world.query::<(&ViewportSortableChild, &Transform, &Visibility)>();
        let attached: Vec<_> = children.iter(&world).collect();
        assert_eq!(
            attached.len(),
            expected_slots,
            "cada slot posible de AddChildSpriteScreen debe quedar materializado"
        );
        let mut visible = 0;
        for (child, transform, visibility) in attached {
            assert_eq!(child.parent, parent_entity);
            assert_eq!(child.source_depth, transform.translation.z);
            if *visibility == Visibility::Visible {
                visible += 1;
            }
        }
        visible
    };
    assert_eq!(
        initial_visible,
        expected_layers.len(),
        "los slots ausentes del frame inicial deben quedar ocultos, no omitidos"
    );

    let mut animated_tile = tile_template();
    animated_tile.kind = TileKind::Industry;
    animated_tile.mapt = 0x80;
    animated_tile.m5 = 143;
    animated_tile.m1 = 0x80;
    animated_tile.m3hi = later_frame;
    world
        .resource_mut::<crate::state::SimWorld>()
        .state
        .map
        .set_tile(coord, animated_tile)
        .expect("toy factory later frame");
    world
        .run_system_once(crate::render::industry_draw_proc::animate_industry_draw_proc_layers)
        .expect("draw-proc animation update");
    let visible_later = world
        .query::<(&ViewportSortableChild, &Visibility)>()
        .iter(&world)
        .filter(|(child, visibility)| {
            child.parent == parent_entity && **visibility == Visibility::Visible
        })
        .count();
    assert_eq!(
        visible_later,
        later_layers.len(),
        "un slot oculto debe activarse al entrar su capa en el frame vivo"
    );
}

#[test]
fn paved_roadside_uses_paved_set_and_details_join_global_sort() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // Carretera normal recta (bits NW|SE = 0x5) con acera (Roadside::Paved = 2).
    let mut paved = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        m5: 0x05,
        m6: 2 << 3,
        ..tile_template()
    };
    map.set_tile(c(2, 2), paved).expect("paved road");

    // Misma carretera con faroles (Roadside::StreetLights = 3).
    paved.m6 = 3 << 3;
    map.set_tile(c(4, 4), paved).expect("street lights road");

    // Mismo trazado con árboles (Roadside::Trees = 5). Cada árbol debe
    // entregar su propio parent al compositor, igual que DrawRoadDetail.
    paved.m6 = 5 << 3;
    map.set_tile(c(6, 2), paved).expect("roadside trees road");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                    TEST_CLIMATE,
                    true,
                    true,
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("paved road tile");
    let paved_sprites: Vec<Sprite> = world.query::<&Sprite>().iter(&world).cloned().collect();
    assert_eq!(
        paved_sprites.len(),
        1,
        "carretera pavimentada: solo el suelo"
    );
    let fi = crate::sprites::ROAD_FLAT_OFFSET_TBL[5] as usize;
    let expected_paved = world.resource::<TsAssets>().0.road_paved[fi].clone();
    assert!(
        expected_paved.matches(&paved_sprites[0]),
        "debe usar el set pavimentado (1313..)"
    );
    let paved_ground_x = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            expected_paved
                .matches(sprite)
                .then_some(transform.translation.x)
        })
        .expect("suelo de carretera pavimentada");
    assert_eq!(
        paved_ground_x,
        crate::iso::iso(2, 2).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET,
        "road_paved conserva el xrel=-31 del sprite completo"
    );
    let paved_ground_z = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            expected_paved
                .matches(sprite)
                .then_some(transform.translation.z)
        })
        .expect("profundidad del suelo de carretera pavimentada");
    assert_eq!(
        paved_ground_z,
        ground_draw_z(2, 2, 0.02),
        "DrawRoadGroundSprites plano pertenece al pase ground, no a los parents"
    );

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                for (x, y) in [(4, 4), (6, 2)] {
                    spawn_road_tile(
                        &mut commands,
                        &m.0,
                        mw,
                        mh,
                        &a.0,
                        &TileRenderContext::new(&m.0, &g.0, x, y),
                        4.0,
                        TEST_CLIMATE,
                        true,
                        true,
                        &[],
                        None,
                        None,
                        &[],
                        &[],
                        &[],
                        None,
                        &[],
                        None,
                    );
                }
            },
        )
        .expect("roadside detail tiles");
    let total = world.query::<&Sprite>().iter(&world).count();
    // `_roadside_lamps[5]`: dos faroles; `_roadside_trees[5]`: cuatro
    // árboles. Ambos aportan también el suelo de su carretera.
    assert_eq!(total - 1, 8, "dos suelos + 2 faroles + 4 árboles");

    let mut lights: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter(|(parent, _)| matches!(parent.sprite_id, 1_406 | 1_407))
        .map(|(parent, transform)| (*parent, transform.translation.z))
        .collect();
    lights.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        lights
            .iter()
            .map(|(parent, _)| (parent.sprite_id, parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                1_407,
                ParentSpriteBounds::new(65, 72, 0, 66, 73, 15),
                viewport_insertion_key(4, 4, 16),
            ),
            (
                1_406,
                ParentSpriteBounds::new(78, 72, 0, 79, 73, 15),
                viewport_insertion_key(4, 4, 17),
            ),
        ],
        "cada farol debe conservar el prisma 2×2×16 de DrawRoadDetail"
    );
    assert!(
        lights
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "el slot fuente del parent debe ser el Transform previo al sort global"
    );

    let mut trees: Vec<_> = world
        .query::<(&ViewportSortableParent, &Transform)>()
        .iter(&world)
        .filter(|(parent, _)| parent.sprite_id == 4_626)
        .map(|(parent, transform)| (*parent, transform.translation.z))
        .collect();
    trees.sort_by_key(|(parent, _)| parent.insertion_key);
    assert_eq!(
        trees
            .iter()
            .map(|(parent, _)| (parent.bounds, parent.insertion_key))
            .collect::<Vec<_>>(),
        vec![
            (
                ParentSpriteBounds::new(96, 34, 0, 97, 35, 15),
                viewport_insertion_key(6, 2, 16),
            ),
            (
                ParentSpriteBounds::new(96, 42, 0, 97, 43, 15),
                viewport_insertion_key(6, 2, 17),
            ),
            (
                ParentSpriteBounds::new(108, 34, 0, 109, 35, 15),
                viewport_insertion_key(6, 2, 18),
            ),
            (
                ParentSpriteBounds::new(108, 42, 0, 109, 43, 15),
                viewport_insertion_key(6, 2, 19),
            ),
        ],
        "los árboles entran individualmente al sorter, no como una capa plana"
    );
    assert!(
        trees
            .iter()
            .all(|(parent, depth)| parent.source_depth == *depth),
        "cada árbol reserva un slot de profundidad antes del sort global"
    );
}

/// `SPR_ONEWAY_BASE` (Action5 0x09) pertenece al `openttd.grf` oficial, no
/// al stack de NewGRFs de una partida. Kale (118,29)/(119,29) usa exactamente
/// estas dos variantes: ROAD_Y con una dirección prohibida produce slots 3 y
/// 4 (sprites 6108/6109). Si se vuelve a condicionar al stack NewGRF, las
/// flechas desaparecen de saves vanilla y la traza deja huecos.
#[test]
fn vanilla_oneway_roads_draw_builtin_action5_overlays_without_newgrf() {
    let assets = boot_assets_app();
    let expected_southbound = assets.oneway_roads[3].clone();
    let expected_northbound = assets.oneway_roads[4].clone();
    let mut map = fresh_map8();
    let left = TileCoord::new(2, 2);
    let right = TileCoord::new(3, 2);

    map.set_tile(
        left,
        Tile {
            kind: TileKind::Road,
            mapt: 0x20,
            m5: 0x15, // ROAD_Y (0x5) + DRD=1.
            ..tile_template()
        },
    )
    .expect("oneway southbound");
    map.set_tile(
        right,
        Tile {
            kind: TileKind::Road,
            mapt: 0x20,
            m5: 0x25, // ROAD_Y (0x5) + DRD=2.
            ..tile_template()
        },
    )
    .expect("oneway northbound");

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));
    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                for (x, y) in [(2, 2), (3, 2)] {
                    spawn_road_tile(
                        &mut commands,
                        &m.0,
                        mw,
                        mh,
                        &a.0,
                        &TileRenderContext::new(&m.0, &g.0, x, y),
                        4.0,
                        TEST_CLIMATE,
                        true,
                        true,
                        &[],
                        None,
                        None,
                        &[],
                        &[],
                        &[],
                        None,
                        &[],
                        None,
                    );
                }
            },
        )
        .expect("oneway road tiles");

    let sprites: Vec<_> = world.query::<&Sprite>().iter(&world).collect();
    assert_eq!(sprites.len(), 4, "cada carretera aporta suelo + flecha");
    assert!(
        sprites
            .iter()
            .any(|sprite| expected_southbound.matches(sprite)),
        "DRD=1 debe usar el slot Action5 3 / sprite 6108"
    );
    assert!(
        sprites
            .iter()
            .any(|sprite| expected_northbound.matches(sprite)),
        "DRD=2 debe usar el slot Action5 4 / sprite 6109"
    );
}

#[test]
fn level_crossing_uses_only_the_paved_crossing_ground() {
    let assets = boot_assets_app();
    // La variante pavimentada debe estar precargada de verdad. Sustituirla en
    // el test ocultaba la regresión que dejaba los cruces de Kale sin suelo.
    let expected = assets
        .level_crossing_ground_sprite(1375)
        .expect("crossing rail Y paved")
        .clone();
    let mut map = fresh_map8();
    let crossing = Tile {
        kind: TileKind::Road,
        mapt: 0x20,
        // `RoadTileType::Crossing`, road axis X → rail axis Y.
        m5: 0x40,
        // `Roadside::Paved`: `DrawTile_Road` suma el bloque +4.
        m6: 2 << 3,
        ..tile_template()
    };
    map.set_tile(TileCoord::new(3, 3), crossing)
        .expect("crossing tile");
    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    4.0,
                    TEST_CLIMATE,
                    true,
                    true,
                    &[],
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    None,
                    &[],
                    None,
                );
            },
        )
        .expect("crossing tile");

    let sprites: Vec<Sprite> = world.query::<&Sprite>().iter(&world).cloned().collect();
    assert_eq!(
        sprites.len(),
        1,
        "el cruce no dibuja asfalto normal adicional"
    );
    assert!(
        expected.matches(&sprites[0]),
        "debe usar crossing paved 1375"
    );
    let crossing_x = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| expected.matches(sprite).then_some(transform.translation.x))
        .expect("suelo de cruce");
    assert_eq!(
        crossing_x,
        crate::iso::iso(3, 3).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET,
        "crossing completo conserva el xrel=-31"
    );
}

#[test]
fn spawn_field_tile_draws_crop_ground_and_fences() {
    let assets = boot_assets_app();
    // Oráculo directo de `DrawTile_Clear` / `DrawClearLandFence`:
    // SPR_FARMLAND_STATE_4 = 4202; las cuatro cercas salen de las tablas
    // `_fence_mod_by_tileh_*` planas de `clear_land.h`.
    let expected_field = assets.fields[4 * 19].clone();
    let expected_fences = [
        assets.fences[2 * 6 + 1].clone(), // NW: fence type 3, variant 1 = 4103.
        assets.fences[0].clone(),         // NE: bushes, variant 0 = 4090.
        assets.fences[5 * 6].clone(),     // SW: stone, variant 0 = 4120.
        assets.fences[6 + 1].clone(),     // SE: gate, variant 1 = 4097.
    ];
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // MP_CLEAR Fields (m5 bits 2-4 = 3), estado 4, cercas NE (m3 5-7),
    // NW (m6 2-4), SW (MAP4/m3hi 5-7) y SE (MAP4/m3hi 2-4).
    let mut field = tile_template();
    field.m5 = 3 << 2;
    field.m3 = 0x24; // NE = tipo 1 (bushes) + estado 4
    field.m6 = 3 << 2; // NW = tipo 3 (fence)
    field.m3hi = (6 << 5) | (2 << 2); // SW = tipo 6 (stone), SE = tipo 2
    map.set_tile(c(2, 2), field).expect("field");

    // Campo sin cercas: solo el suelo.
    let mut bare = tile_template();
    bare.m5 = 3 << 2;
    map.set_tile(c(3, 2), bare).expect("bare field");

    // Meseta plana: el suelo de campo queda visualmente 9 niveles más alto,
    // pero `DrawGroundSprite` no puede cambiar su orden de composición por
    // esa altura. Las cuatro muestras son N, W, E y S de (2, 2).
    for coord in [c(2, 2), c(3, 2), c(2, 3), c(3, 3)] {
        map.set_height(coord, 9).expect("field plateau");
    }

    let grid = RenderGrid::from_map(&map, 8, 8);
    let mut world = World::new();
    world.insert_resource(TsMap(map));
    world.insert_resource(TsGrid(grid));
    world.insert_resource(TsAssets(assets));

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("field tile");
    let with_fences = world.query::<&Sprite>().iter(&world).count();
    assert_eq!(with_fences, 5, "suelo de cultivo + 4 cercas");
    let rendered: Vec<_> = world.query::<&Sprite>().iter(&world).collect();
    assert_eq!(
        rendered
            .iter()
            .filter(|sprite| expected_field.matches(sprite))
            .count(),
        1,
        "estado 4 debe seleccionar SPR_FARMLAND_STATE_4 plano (4202)"
    );
    for (side, expected) in ["NW", "NE", "SW", "SE"].into_iter().zip(expected_fences) {
        assert_eq!(
            rendered
                .iter()
                .filter(|sprite| expected.matches(sprite))
                .count(),
            1,
            "la cerca {side} debe conservar el sprite que selecciona OpenTTD"
        );
    }
    let field_ground_z = world
        .query::<(&Sprite, &Transform)>()
        .iter(&world)
        .find_map(|(sprite, transform)| {
            expected_field
                .matches(sprite)
                .then_some(transform.translation.z)
        })
        .expect("sprite de suelo de campo");
    assert_eq!(
        field_ground_z,
        crate::iso::ground_tile_pos_half(2, 2, 9, 0.0, 4.0).z,
        "el suelo debe usar el pase Ground de OpenTTD"
    );
    assert_ne!(
        field_ground_z,
        crate::iso::tile_pos_half(2, 2, 9, 0.0, 4.0).z,
        "la elevación no puede formar parte de la profundidad del campo"
    );

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    None,
                    None,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    &m.0,
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
                    &[],
                    &[],
                    None,
                    None,
                );
            },
        )
        .expect("bare field tile");
    let total = world.query::<&Sprite>().iter(&world).count();
    assert_eq!(total - with_fences, 1, "campo sin cercas solo dibuja suelo");
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

fn callback_literal_runtime(local_id: u8, value: u8) -> TrainSpriteGraphics {
    let mut runtime = TrainSpriteGraphics {
        assigns: vec![TrainSpriteAssign {
            local_id,
            set_id: 0,
        }],
        ..Default::default()
    };
    runtime.action2_var.insert(
        0,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x1A,
                param: None,
                adjust: Action2VarAdjust {
                    and_mask: u32::from(value),
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        },
    );
    runtime.action2_to_action1.insert(0, 0);
    runtime
}
