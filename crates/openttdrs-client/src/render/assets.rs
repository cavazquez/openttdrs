use std::collections::HashMap;

use bevy::prelude::*;

use crate::render::atlas::{AtlasSprite, TileAtlas};
use crate::sprites::{
    HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, ROAD_DEPOT_GROUND_PATH, StationTileClass,
    house_sprite_filename, rail_depot_build_layers, rail_sprite_ids_for_preload,
    rail_station_draw_layers, rail_station_ground_track_sprite, rail_waypoint_draw_layers,
    road_depot_build_layers, road_stop_build_layers,
};

#[derive(Clone, Resource)]
pub(crate) struct WorldAssets {
    pub(crate) grass: AtlasSprite,
    pub(crate) rough: AtlasSprite,
    pub(crate) grass_slopes: Vec<AtlasSprite>,
    pub(crate) rough_slopes: Vec<AtlasSprite>,
    pub(crate) water: AtlasSprite,
    /// Set completo `SPR_SHORE_BASE + 0..17` (`shore_full_{i:02}.png`).
    pub(crate) shore: Vec<AtlasSprite>,
    /// 15 frames del ciclo de paleta del agua (`water_anim_{f}.png`).
    pub(crate) water_frames: Vec<AtlasSprite>,
    /// 18 orillas × 15 frames (`shore_full_{i:02}_anim_{f}.png`).
    pub(crate) shore_frames: Vec<Vec<AtlasSprite>>,
    pub(crate) lighthouse: AtlasSprite,
    pub(crate) transmitter: AtlasSprite,
    pub(crate) road_flat: Vec<AtlasSprite>,
    /// Set pavimentado (`SPR_ROAD_Y - 19` = 1313..1331), mismo orden que `road_flat`.
    pub(crate) road_paved: Vec<AtlasSprite>,
    /// Faroles de `Roadside::StreetLights` (sprites 0x57E/0x57F).
    pub(crate) road_streetlights: [AtlasSprite; 2],
    /// OpenGFX `tram_flat_*` (SPR_TRAMWAY_OVERLAY+0..18); mismo índice que `road_flat_*`.
    pub(crate) tram_flat: Vec<AtlasSprite>,
    pub(crate) rail: HashMap<u32, AtlasSprite>,
    pub(crate) station_grounds: Vec<AtlasSprite>,
    pub(crate) bus_stop_grounds: Vec<AtlasSprite>,
    pub(crate) bus_stop_builds: [[AtlasSprite; 3]; 4],
    pub(crate) truck_stop_builds: [[AtlasSprite; 3]; 4],
    pub(crate) road_depot_ground: AtlasSprite,
    pub(crate) road_depot_builds: [Vec<AtlasSprite>; 4],
    /// Capas del depósito de vía por dirección (`m5 & 3`: NE/SE/SW/NW).
    pub(crate) rail_depot_builds: [Vec<AtlasSprite>; 4],
    pub(crate) road_tunnel: AtlasSprite,
    pub(crate) rail_tunnel: AtlasSprite,
    pub(crate) road_bridge: AtlasSprite,
    pub(crate) road_bridge_y: AtlasSprite,
    pub(crate) rail_bridge: AtlasSprite,
    pub(crate) rail_bridge_y: AtlasSprite,
    /// Barandilla frontal del tramo intermedio de puente, por eje `[x, y]`.
    pub(crate) bridge_front: [AtlasSprite; 2],
    /// Pilar del tramo intermedio de puente, por eje `[x, y]`.
    pub(crate) bridge_pillar: [AtlasSprite; 2],
    pub(crate) houses: HashMap<u32, AtlasSprite>,
    /// `tree_{NN}.png` (NN = sprite − 1576): 19 especies × 7 etapas.
    pub(crate) trees: Vec<AtlasSprite>,
    /// `field_{estado}_{tileh:02}.png`: índice = estado × 15 + tileh (0..14).
    pub(crate) fields: Vec<AtlasSprite>,
    /// `fence_{tipo}_{var}.png`: índice = tipo (0..5) × 6 + variante (0..5).
    pub(crate) fences: Vec<AtlasSprite>,
    /// `chimney_smoke_{i}.png`: 8 frames del humo de la central eléctrica.
    pub(crate) chimney_smoke: Vec<AtlasSprite>,
    pub(crate) industries: HashMap<u32, AtlasSprite>,
    /// Cimientos nivelados (`foundation_01..14.png`, SPR_FOUNDATION_BASE + tileh).
    pub(crate) foundations: Vec<AtlasSprite>,
}

impl WorldAssets {
    /// Resuelve todos los sprites del mapa contra el [`TileAtlas`]; no toca
    /// el filesystem (la tabla de rects es metadata compilada).
    pub(crate) fn load(atlas: &TileAtlas) -> Self {
        let grass = atlas.get("grass.png");
        let rough = atlas.get("grass_rough.png");
        let grass_slopes = (1u8..=14)
            .map(|tileh| atlas.get(&format!("terrain_grass_slope_{tileh:02}.png")))
            .collect();
        let rough_slopes = (1u8..=14)
            .map(|tileh| atlas.get(&format!("terrain_rough_slope_{tileh:02}.png")))
            .collect();
        let foundations = (1u8..=14)
            .map(|tileh| atlas.get(&format!("foundation_{tileh:02}.png")))
            .collect();
        let water = atlas.get("water.png");
        let shore: Vec<AtlasSprite> = (0..crate::sprites::SHORE_SPRITE_COUNT)
            .map(|i| atlas.get(&format!("shore_full_{i:02}.png")))
            .collect();
        let water_frames = (0..15)
            .map(|f| atlas.get(&format!("water_anim_{f:02}.png")))
            .collect();
        let shore_frames = (0..crate::sprites::SHORE_SPRITE_COUNT)
            .map(|i| {
                (0..15)
                    .map(|f| atlas.get(&format!("shore_full_{i:02}_anim_{f:02}.png")))
                    .collect()
            })
            .collect();
        let lighthouse = atlas.get("object_lighthouse.png");
        let transmitter = atlas.get("object_transmitter.png");
        let road_flat = (0..19)
            .map(|i| atlas.get(&format!("road_flat_{i:02}.png")))
            .collect();
        let road_paved = (0..19)
            .map(|i| atlas.get(&format!("road_paved_{i:02}.png")))
            .collect();
        let road_streetlights = [
            atlas.get("road_streetlight_0.png"),
            atlas.get("road_streetlight_1.png"),
        ];
        let tram_flat = (0..19)
            .map(|i| atlas.get(&format!("tram_flat_{i:02}.png")))
            .collect();
        let mut rail_ids: std::collections::BTreeSet<_> =
            rail_sprite_ids_for_preload().into_iter().collect();
        for gfx in 0..=7 {
            rail_ids.insert(rail_station_ground_track_sprite(gfx, 0));
            for layer in rail_station_draw_layers(gfx) {
                rail_ids.insert(layer.sprite_id);
            }
        }
        for axis_y in [false, true] {
            let m5 = u8::from(axis_y);
            for layer in rail_waypoint_draw_layers(m5) {
                rail_ids.insert(layer.sprite_id);
            }
        }
        let rail = rail_ids
            .into_iter()
            .map(|id| (id, atlas.get(&format!("rail_{id}.png"))))
            .collect();
        let station_grounds = (0..4)
            .map(|i| atlas.get(&format!("truck_stop_ground_{i}.png")))
            .collect();
        let bus_stop_grounds = [
            "bus_stop_ne_ground.png",
            "bus_stop_se_ground.png",
            "bus_stop_sw_ground.png",
            "bus_stop_nw_ground.png",
        ]
        .into_iter()
        .map(|name| atlas.get(name))
        .collect();
        let bus_stop_builds = std::array::from_fn(|dir| {
            std::array::from_fn(|layer| {
                atlas.get_path(road_stop_build_layers(StationTileClass::Bus, dir)[layer].path)
            })
        });
        let truck_stop_builds = std::array::from_fn(|dir| {
            std::array::from_fn(|layer| {
                atlas.get_path(road_stop_build_layers(StationTileClass::Truck, dir)[layer].path)
            })
        });
        let road_depot_ground = atlas.get_path(ROAD_DEPOT_GROUND_PATH);
        let road_depot_builds = std::array::from_fn(|dir| {
            road_depot_build_layers(dir)
                .iter()
                .map(|layer| atlas.get_path(layer.path))
                .collect()
        });
        let rail_depot_builds = std::array::from_fn(|dir| {
            rail_depot_build_layers(dir)
                .iter()
                .map(|layer| atlas.get_path(layer.path))
                .collect()
        });
        let road_tunnel = atlas.get("tunnel_road_rear.png");
        let rail_tunnel = atlas.get("tunnel_rail_rear.png");
        let road_bridge = atlas.get("bridge_wood_road_x.png");
        let road_bridge_y = atlas.get("bridge_wood_road_y.png");
        let rail_bridge = atlas.get("bridge_wood_rail_x.png");
        let rail_bridge_y = atlas.get("bridge_wood_rail_y.png");
        let bridge_front = [
            atlas.get("bridge_wood_x_front.png"),
            atlas.get("bridge_wood_y_front.png"),
        ];
        let bridge_pillar = [
            atlas.get("bridge_wood_x_pillar.png"),
            atlas.get("bridge_wood_y_pillar.png"),
        ];

        let mut houses = HashMap::new();
        for spec in &HOUSE_DRAW_DATA {
            for &sid in &[spec.s1, spec.s2] {
                if sid != 0 {
                    houses
                        .entry(sid)
                        .or_insert_with(|| atlas.get(&house_sprite_filename(sid)));
                }
            }
        }

        let trees = (0..crate::sprites::TREE_SPRITE_COUNT)
            .map(|i| atlas.get(&format!("tree_{i:02}.png")))
            .collect();
        let mut fields = Vec::with_capacity(crate::sprites::FIELD_STATES * 15);
        for state in 0..crate::sprites::FIELD_STATES {
            for tileh in 0..15 {
                fields.push(atlas.get(&format!("field_{state}_{tileh:02}.png")));
            }
        }
        let mut fences = Vec::with_capacity(36);
        for ftype in 0..6 {
            for var in 0..6 {
                fences.push(atlas.get(&format!("fence_{ftype}_{var}.png")));
            }
        }
        let chimney_smoke = (0..crate::sprites::CHIMNEY_SMOKE_FRAMES)
            .map(|i| atlas.get(&format!("chimney_smoke_{i}.png")))
            .collect();

        let mut industries = HashMap::new();
        for entry in &INDUSTRY_GFX_DATA {
            for &id in &[entry.sprite_id, entry.ground_sprite_id] {
                if id != 0 {
                    industries
                        .entry(id)
                        .or_insert_with(|| atlas.get(&format!("industry_{id}.png")));
                }
            }
        }
        for id in crate::sprites::INDUSTRY_DRAW_PROC_SPRITE_IDS {
            if let Some(img) = atlas.try_get(&format!("industry_{id}.png")) {
                industries.entry(id).or_insert(img);
            }
        }

        Self {
            grass,
            rough,
            grass_slopes,
            rough_slopes,
            water,
            shore,
            water_frames,
            shore_frames,
            lighthouse,
            transmitter,
            road_flat,
            road_paved,
            road_streetlights,
            tram_flat,
            rail,
            station_grounds,
            bus_stop_grounds,
            bus_stop_builds,
            truck_stop_builds,
            road_depot_ground,
            road_depot_builds,
            rail_depot_builds,
            road_tunnel,
            rail_tunnel,
            road_bridge,
            road_bridge_y,
            rail_bridge,
            rail_bridge_y,
            bridge_front,
            bridge_pillar,
            houses,
            trees,
            fields,
            fences,
            chimney_smoke,
            industries,
            foundations,
        }
    }
}

/// Escribe stubs de las páginas del atlas (1 px); la tabla de rects es
/// metadata compilada, así que los tests no necesitan los PNGs reales.
#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) fn stub_opengfx_tiles_for_tests(root: &std::path::Path) {
    use std::fs;

    const ONE_PX_PNG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/one_pixel.png"
    ));

    for p in 0..crate::sprites::TILE_ATLAS_PAGE_COUNT {
        let path = root.join(format!("assets/opengfx/atlas/tiles_atlas_{p}.png"));
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).expect("mkdir");
        }
        fs::write(&path, ONE_PX_PNG).expect("write png");
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod world_assets_tests {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;

    use super::{TileAtlas, WorldAssets, stub_opengfx_tiles_for_tests};

    #[test]
    fn world_assets_load_hits_all_paths() {
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
                TileAtlas::build(world.resource::<AssetServer>(), &mut layouts)
            })
        };
        let _assets = WorldAssets::load(&atlas);
    }
}
