use bevy::prelude::*;

mod batches;
mod land;
mod objects;
mod transport;
mod water;

pub(crate) use batches::flush_map_batches;
pub(crate) use land::{
    push_forest_tree, spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile,
};
pub(crate) use objects::{spawn_station_tile, spawn_transport_object_tile};
pub(crate) use transport::{spawn_rail_tile, spawn_road_tile};
pub(crate) use water::push_water_tile;

use super::{MapVisualLayer, TileRenderContext, WaterTile};
use crate::iso::{TILE_HALF_H, tile_pos, tile_pos_half, wang_hash};

/// Sesgo en la componente Z de **solo** el agua animada (sin sprite `shore_*`).
/// El orden de dibujo usa `(tx+ty)`; el mar al **este/sur** tiene suma mayor y acaba
/// encima del borde costero del vecino NO/NE → sierra y rectángulos azules oscuros.
const FLAT_WATER_LAYER_FRAC: f32 = -0.030;
/// Costa entre tierra y agua: debe tapar agua vecina, pero no pintar su parte azul
/// encima de la tierra que queda del lado interior de la orilla.
const SHORE_LAYER_FRAC: f32 = -0.015;
/// Solape mínimo para ocultar costuras finas entre tiles adyacentes.
const TILE_OVERLAP_SCALE: f32 = 1.002;
/// Capa de tranvía (`tram_flat_*`, SPR_TRAMWAY_OVERLAY) por encima del asfalto.
const TRAM_OVERLAY_LAYER_FRAC: f32 = 0.028;

fn sloped_or_flat_image(
    tileh: u8,
    flat: &Handle<Image>,
    slopes: &[Handle<Image>],
) -> Handle<Image> {
    if tileh == 0 {
        flat.clone()
    } else {
        slopes[tileh as usize - 1].clone()
    }
}

fn spawn_ground_sprite(
    commands: &mut Commands,
    image: Handle<Image>,
    color: Color,
    ctx: &TileRenderContext,
    half_h: f32,
) {
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            0.0,
            half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

fn water_phases(tx: u32, ty: u32) -> WaterTile {
    WaterTile {
        dark_phase: ((tx + 2 * ty).rem_euclid(5)) as u8,
        glitter_phase: (wang_hash(tx, ty, 0xA9FE) % 15) as u8,
    }
}

fn push_water_sprite(
    batch_water: &mut Vec<(WaterTile, Sprite, Transform)>,
    h_water: &Handle<Image>,
    ctx: &TileRenderContext,
) {
    batch_water.push((
        water_phases(ctx.tx, ctx.ty),
        Sprite {
            image: h_water.clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            FLAT_WATER_LAYER_FRAC,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

fn spawn_coast_debug_label(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    raw: u8,
    tileh: u8,
    shore_index: usize,
) {
    let label = format!("r{raw}/t{tileh}/s{shore_index}");
    commands.spawn((
        MapVisualLayer,
        Text2d::new(label),
        TextFont {
            font_size: 9.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.4)),
        Transform::from_translation(Vec3::new(
            ctx.iso_pos.x - 18.0,
            ctx.iso_pos.y - TILE_HALF_H + f32::from(ctx.info.base_z) * 8.0 - 3.0,
            (ctx.tx + ctx.ty) as f32 * 0.01 + f32::from(ctx.info.base_z) * 0.001 + 0.95,
        )),
    ));
}

#[cfg(test)]
mod tests {
    use super::{FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC};
    use crate::iso::{TILE_HALF_H, tile_pos, tile_pos_half};

    #[test]
    fn shore_z_sits_between_neighbor_land_and_water() {
        let tx = 10;
        let ty = 10;
        let shore = tile_pos_half(tx, ty, 0, SHORE_LAYER_FRAC, TILE_HALF_H).z;
        let inner_land = tile_pos(tx - 1, ty, 0, 0.0).z;
        let outer_water = tile_pos(tx + 1, ty, 0, FLAT_WATER_LAYER_FRAC).z;

        assert!(shore < inner_land);
        assert!(shore > outer_water);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod spawn_coverage_tests {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;
    use openttdrs_core::{Map, Tile, TileCoord, TileKind};

    use super::{
        flush_map_batches, push_forest_tree, push_water_tile, spawn_generic_land_tile,
        spawn_house_tile, spawn_industry_tile, spawn_rail_tile, spawn_road_tile,
        spawn_station_tile, spawn_transport_object_tile,
    };
    use crate::render::assets::{WorldAssets, stub_opengfx_tiles_for_tests};
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
        app.update();
        WorldAssets::load(app.world().resource::<AssetServer>())
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
