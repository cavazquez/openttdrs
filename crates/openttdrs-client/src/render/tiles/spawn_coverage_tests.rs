//! Tests de integración: rutas principales de spawn de tiles (carretera, vía, agua, etc.).

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::image::ImagePlugin;
use bevy::prelude::*;
use openttdrs_core::{Map, Tile, TileCoord, TileKind};

use crate::render::assets::{WorldAssets, stub_opengfx_tiles_for_tests};
use crate::render::tiles::{
    flush_map_batches, push_forest_tree, push_water_tile, spawn_bridge_middle,
    spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile, spawn_rail_tile,
    spawn_road_tile, spawn_station_tile, spawn_transport_object_tile,
};
use crate::render::{MapSpriteBatches, RenderGrid, TileRenderContext};
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
    WorldAssets::load(&atlas)
}

fn fresh_map8() -> Map {
    Map::new_flat(8, 8, 0)
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
                );
                spawn_road_tile(
                    &mut commands,
                    &m.0,
                    mw,
                    mh,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 3),
                    4.0,
                );
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    4.0,
                    &mut rails,
                );
                rails.clear();
                spawn_rail_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 3, 3),
                    4.0,
                    &mut rails,
                );
                spawn_station_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 4, 2),
                    &[],
                    4.0,
                );
                for (x, y) in [(5, 2), (5, 3), (5, 4), (5, 5), (5, 6), (5, 7)] {
                    spawn_transport_object_tile(
                        &mut commands,
                        &a.0,
                        &TileRenderContext::new(&m.0, &g.0, x as u32, y as u32),
                        4.0,
                    );
                }
            },
        )
        .expect("spawn batch");
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
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                let (mw, mh) = m.0.dimensions();
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 0, 0),
                    4.0,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 0),
                    4.0,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 2, 0),
                    4.0,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 3, 0),
                    4.0,
                );
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 4, 0),
                    4.0,
                );
                spawn_house_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 0, 1),
                    4.0,
                );
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
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
                );
                push_water_tile(
                    &mut commands,
                    &m.0,
                    (mw, mh),
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 6, 6),
                    false,
                    &mut batches,
                );
                push_forest_tree(
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 4, 4),
                    &mut batches,
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
                );
                spawn_station_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    &[],
                    4.0,
                );
            },
        )
        .expect("sloped");
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
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_industry_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 1, 1),
                    4.0,
                );
            },
        )
        .expect("industry slope");
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
                );
                // Tesela sin puente encima: no debe agregar nada.
                spawn_bridge_middle(
                    &mut commands,
                    &m.0,
                    dims,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 6, 6),
                );
            },
        )
        .expect("bridge middle");

    // Tablero + barandilla frontal + 1 pilar (deck_z 1, suelo 0).
    let sprites = world.query::<&Sprite>().iter(&world).count();
    assert_eq!(sprites, 3, "vano dibuja tablero, barandilla y pilar");
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
                move |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                    spawn_industry_tile(
                        &mut commands,
                        &a.0,
                        &TileRenderContext::new(&m.0, &g.0, tx, 2),
                        4.0,
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
    let a = world.resource::<TsAssets>();
    let fi = crate::sprites::ROAD_FLAT_OFFSET_TBL[5] as usize;
    assert!(
        a.0.road_paved[fi].matches(&paved_sprites[0]),
        "debe usar el set pavimentado (1313..)"
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
                );
            },
        )
        .expect("street lights road tile");
    let total = world.query::<&Sprite>().iter(&world).count();
    // `_roadside_lamps[5]`: dos faroles además del suelo pavimentado.
    assert_eq!(total - 1, 3, "suelo pavimentado + 2 faroles");
}

#[test]
fn spawn_field_tile_draws_crop_ground_and_fences() {
    let assets = boot_assets_app();
    let mut map = fresh_map8();
    let c = |x: i32, y: i32| TileCoord::new(x, y);

    // MP_CLEAR Fields (m5 bits 2-4 = 3), estado 4, cercas NE (m3 5-7),
    // NW (m6 2-4), SW (m3hi 5-7) y SE (m3hi 2-4).
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
                    &TileRenderContext::new(&m.0, &g.0, 2, 2),
                    4.0,
                );
            },
        )
        .expect("field tile");
    let with_fences = world.query::<&Sprite>().iter(&world).count();
    assert_eq!(with_fences, 5, "suelo de cultivo + 4 cercas");

    world
        .run_system_once(
            |mut commands: Commands, m: Res<TsMap>, g: Res<TsGrid>, a: Res<TsAssets>| {
                spawn_generic_land_tile(
                    &mut commands,
                    &a.0,
                    &TileRenderContext::new(&m.0, &g.0, 3, 2),
                    4.0,
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
