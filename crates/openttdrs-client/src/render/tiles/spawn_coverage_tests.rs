//! Tests de integración: rutas principales de spawn de tiles (carretera, vía, agua, etc.).

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::image::ImagePlugin;
use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    Action2VarAdjust, Action2VarEntry, Action2VarTerm, BridgeType, Climate, DecodedSprite,
    FOUNDATION_ORIGINAL_SPRITE_BASE, HouseSpecDef, IndustryTileGfxId, IndustryTileSpecDef,
    RailType, RoadStopSpecDef, RoadTramType, RoadType, RoadTypeDef, StationClassId, StationSpecDef,
    StationSpecId, TrainSpriteAssign, TrainSpriteGraphics, WaterClass, set_water_class_m1,
    vanilla_road_type_catalog,
};

const TEST_CLIMATE: Climate = Climate::Temperate;
const TEST_WORLD_SEED: u64 = 0;

use crate::iso::ground_draw_z;
use crate::render::assets::{WorldAssets, stub_opengfx_tiles_for_tests};
use crate::render::tiles::{
    HouseSpawnResources, flush_map_batches, push_forest_tree, push_water_tile, spawn_bridge_middle,
    spawn_bridge_middle_with_road_types, spawn_generic_land_tile, spawn_house_tile,
    spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
    spawn_transport_object_tile,
};
use crate::render::{
    CompanyColoredSprites, MapSpriteBatches, MapVisualLayer, RenderGrid, TileRenderContext,
    ViewportSortableChild, ViewportSortableParent,
};
use crate::sprites::{RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS};

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
    let mut station = Station::new_with_kind(oilrig, StopKind::Airport);
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
fn drive_through_tram_stop_draws_vanilla_catenary_after_stop_layers() {
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
        callback_mask: 0,
        animation_status: 0xFF,
        animation_frames: 0,
        animation_speed: 2,
        animation_triggers: 0,
        newgrf_views: Vec::new(),
        newgrf_runtime: None,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
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
    map.set_tile(
        coord,
        Tile {
            kind: TileKind::Station,
            mapt: 0x50,
            m3: 0x04, // Roadside::Grass.
            m5: openttdrs_core::RSV_DRIVE_THROUGH_X,
            m6: openttdrs_core::station::STATION_TYPE_ROAD_WAYPOINT << 3,
            ..tile_template()
        },
    )
    .expect("sloped road waypoint");

    let grid = RenderGrid::from_map(&map, 3, 3);
    let ctx = TileRenderContext::new(&map, &grid, 1, 1);
    assert_ne!(ctx.info.tileh, 0, "the fixture must remain sloped");
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
    tram.from_newgrf = true;
    tram.from_tramtypes_feature = true;
    tram.newgrf_views = vec![openttdrs_core::DecodedSprite {
        width: 8,
        height: 8,
        x_offs: 0,
        y_offs: 0,
        rgba: vec![255; 8 * 8 * 4],
        mask: Vec::new(),
    }];
    let road_catalog = vec![tram];
    let grid = RenderGrid::from_map(&map, 8, 8);
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
                  mut cache: Local<crate::render::NewGrfRoadSpriteCache>,
                  mut images: Local<Assets<Image>>| {
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
}

#[test]
fn sloped_road_stop_grounds_attach_to_their_foundation_parent() {
    let assets = boot_assets_app();
    let bay_ground = assets.bus_stop_grounds[0].clone();
    let drive_through_ground =
        assets.road_paved[crate::sprites::road_flat_sprite_index(0, 0x0A)].clone();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    let bay = c(1, 1);
    let drive_through = c(5, 1);
    for (coord, m5) in [
        (bay, 0),
        (drive_through, openttdrs_core::RSV_DRIVE_THROUGH_X),
    ] {
        map.set_tile(
            coord,
            Tile {
                kind: TileKind::Station,
                mapt: 0x50,
                m5,
                m6: 3 << 3, // StationType::Bus.
                ..tile_template()
            },
        )
        .expect("parada vial inclinada");
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

    let attached: Vec<_> = world
        .query::<(&ViewportSortableChild, &Sprite, &Transform)>()
        .iter(&world)
        .filter(|(child, _, _)| foundation_parents.contains(&child.parent))
        .collect();
    assert_eq!(
        attached.len(),
        2,
        "cada suelo vial debe ser child del cimiento"
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
}

#[test]
fn airport_pier_tile_seq_layers_spawn_for_both_import_paths() {
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
    assert_eq!(jetways, vec![expected_jetway_pos]);
    assert_eq!(tunnels, vec![expected_tunnel_pos]);
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
        accepts_cargo_indices: Vec::new(),
        accepts_cargo_labels: Vec::new(),
        acceptance: Vec::new(),
        callback_mask: 0,
        animation_frames: 0,
        animation_status: 0,
        animation_speed: 0,
        animation_triggers: 0,
        animation_special_flags: 0,
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
}

#[test]
fn paved_roadside_uses_paved_set_and_streetlights_spawn_lamps() {
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
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 4, 4),
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
        .expect("street lights road tile");
    let total = world.query::<&Sprite>().iter(&world).count();
    // `_roadside_lamps[5]`: dos faroles además del suelo pavimentado.
    assert_eq!(total - 1, 3, "suelo pavimentado + 2 faroles");
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
                    4.0,
                    TEST_CLIMATE,
                    TEST_WORLD_SEED,
                    8,
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
