use std::collections::HashMap;

use bevy::prelude::*;

use crate::sprites::{
    HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, ROAD_DEPOT_GROUND_PATH, StationTileClass,
    house_sprite_filename, rail_sprite_ids_for_preload, rail_station_draw_layers,
    rail_station_ground_track_sprite, road_depot_build_layers, road_stop_build_layers,
};

pub(crate) struct WorldAssets {
    pub(crate) grass: Handle<Image>,
    pub(crate) rough: Handle<Image>,
    pub(crate) grass_slopes: Vec<Handle<Image>>,
    pub(crate) rough_slopes: Vec<Handle<Image>>,
    pub(crate) water: Handle<Image>,
    /// Set completo `SPR_SHORE_BASE + 0..17` (`shore_full_{i:02}.png`).
    pub(crate) shore: Vec<Handle<Image>>,
    /// 15 frames del ciclo de paleta del agua (`water_anim_{f}.png`).
    pub(crate) water_frames: Vec<Handle<Image>>,
    /// 18 orillas × 15 frames (`shore_full_{i:02}_anim_{f}.png`).
    pub(crate) shore_frames: Vec<Vec<Handle<Image>>>,
    pub(crate) lighthouse: Handle<Image>,
    pub(crate) transmitter: Handle<Image>,
    pub(crate) road_flat: Vec<Handle<Image>>,
    /// Set pavimentado (`SPR_ROAD_Y - 19` = 1313..1331), mismo orden que `road_flat`.
    pub(crate) road_paved: Vec<Handle<Image>>,
    /// Faroles de `Roadside::StreetLights` (sprites 0x57E/0x57F).
    pub(crate) road_streetlights: [Handle<Image>; 2],
    /// OpenGFX `tram_flat_*` (SPR_TRAMWAY_OVERLAY+0..18); mismo índice que `road_flat_*`.
    pub(crate) tram_flat: Vec<Handle<Image>>,
    pub(crate) rail: HashMap<u32, Handle<Image>>,
    pub(crate) station_grounds: Vec<Handle<Image>>,
    pub(crate) bus_stop_grounds: Vec<Handle<Image>>,
    pub(crate) bus_stop_builds: [[Handle<Image>; 3]; 4],
    pub(crate) truck_stop_builds: [[Handle<Image>; 3]; 4],
    pub(crate) road_depot_ground: Handle<Image>,
    pub(crate) road_depot_builds: [Vec<Handle<Image>>; 4],
    pub(crate) rail_depot: Handle<Image>,
    pub(crate) road_tunnel: Handle<Image>,
    pub(crate) rail_tunnel: Handle<Image>,
    pub(crate) road_bridge: Handle<Image>,
    pub(crate) road_bridge_y: Handle<Image>,
    pub(crate) rail_bridge: Handle<Image>,
    pub(crate) rail_bridge_y: Handle<Image>,
    /// Barandilla frontal del tramo intermedio de puente, por eje `[x, y]`.
    pub(crate) bridge_front: [Handle<Image>; 2],
    /// Pilar del tramo intermedio de puente, por eje `[x, y]`.
    pub(crate) bridge_pillar: [Handle<Image>; 2],
    pub(crate) houses: HashMap<u32, Handle<Image>>,
    /// `tree_{NN}.png` (NN = sprite − 1576): 19 especies × 7 etapas.
    pub(crate) trees: Vec<Handle<Image>>,
    /// `field_{estado}_{tileh:02}.png`: índice = estado × 15 + tileh (0..14).
    pub(crate) fields: Vec<Handle<Image>>,
    /// `fence_{tipo}_{var}.png`: índice = tipo (0..5) × 6 + variante (0..5).
    pub(crate) fences: Vec<Handle<Image>>,
    /// `chimney_smoke_{i}.png`: 8 frames del humo de la central eléctrica.
    pub(crate) chimney_smoke: Vec<Handle<Image>>,
    pub(crate) industries: HashMap<u32, Handle<Image>>,
    /// Cimientos nivelados (`foundation_01..14.png`, SPR_FOUNDATION_BASE + tileh).
    pub(crate) foundations: Vec<Handle<Image>>,
}

impl WorldAssets {
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        let grass = asset_server.load::<Image>("assets/opengfx/tiles/grass.png");
        let rough = asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png");
        let grass_slopes = (1u8..=14)
            .map(|tileh| {
                asset_server.load::<Image>(format!(
                    "assets/opengfx/tiles/terrain_grass_slope_{tileh:02}.png"
                ))
            })
            .collect();
        let rough_slopes = (1u8..=14)
            .map(|tileh| {
                asset_server.load::<Image>(format!(
                    "assets/opengfx/tiles/terrain_rough_slope_{tileh:02}.png"
                ))
            })
            .collect();
        let foundations = (1u8..=14)
            .map(|tileh| {
                asset_server
                    .load::<Image>(format!("assets/opengfx/tiles/foundation_{tileh:02}.png"))
            })
            .collect();
        let water = asset_server.load::<Image>("assets/opengfx/tiles/water.png");
        let shore: Vec<Handle<Image>> = (0..crate::sprites::SHORE_SPRITE_COUNT)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/shore_full_{i:02}.png"))
            })
            .collect();
        let water_frames = (0..15)
            .map(|f| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/water_anim_{f:02}.png"))
            })
            .collect();
        let shore_frames = (0..crate::sprites::SHORE_SPRITE_COUNT)
            .map(|i| {
                (0..15)
                    .map(|f| {
                        asset_server.load::<Image>(format!(
                            "assets/opengfx/tiles/shore_full_{i:02}_anim_{f:02}.png"
                        ))
                    })
                    .collect()
            })
            .collect();
        let lighthouse = asset_server.load::<Image>("assets/opengfx/tiles/object_lighthouse.png");
        let transmitter = asset_server.load::<Image>("assets/opengfx/tiles/object_transmitter.png");
        let road_flat = (0..19)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/road_flat_{i:02}.png"))
            })
            .collect();
        let road_paved = (0..19)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/road_paved_{i:02}.png"))
            })
            .collect();
        let road_streetlights = [
            asset_server.load::<Image>("assets/opengfx/tiles/road_streetlight_0.png"),
            asset_server.load::<Image>("assets/opengfx/tiles/road_streetlight_1.png"),
        ];
        let tram_flat = (0..19)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/tram_flat_{i:02}.png"))
            })
            .collect();
        let mut rail_ids: std::collections::BTreeSet<_> =
            rail_sprite_ids_for_preload().into_iter().collect();
        for gfx in 0..=7 {
            rail_ids.insert(rail_station_ground_track_sprite(gfx, 0));
            for layer in rail_station_draw_layers(gfx) {
                rail_ids.insert(layer.sprite_id);
            }
        }
        let rail = rail_ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    asset_server.load::<Image>(format!("assets/opengfx/tiles/rail_{id}.png")),
                )
            })
            .collect();
        let station_grounds = (0..4)
            .map(|i| {
                asset_server
                    .load::<Image>(format!("assets/opengfx/tiles/truck_stop_ground_{i}.png"))
            })
            .collect();
        let bus_stop_grounds = [
            "assets/opengfx/tiles/bus_stop_ne_ground.png",
            "assets/opengfx/tiles/bus_stop_se_ground.png",
            "assets/opengfx/tiles/bus_stop_sw_ground.png",
            "assets/opengfx/tiles/bus_stop_nw_ground.png",
        ]
        .into_iter()
        .map(|path| asset_server.load::<Image>(path))
        .collect();
        let bus_stop_builds = std::array::from_fn(|dir| {
            std::array::from_fn(|layer| {
                asset_server
                    .load::<Image>(road_stop_build_layers(StationTileClass::Bus, dir)[layer].path)
            })
        });
        let truck_stop_builds = std::array::from_fn(|dir| {
            std::array::from_fn(|layer| {
                asset_server
                    .load::<Image>(road_stop_build_layers(StationTileClass::Truck, dir)[layer].path)
            })
        });
        let road_depot_ground = asset_server.load::<Image>(ROAD_DEPOT_GROUND_PATH);
        let road_depot_builds = std::array::from_fn(|dir| {
            road_depot_build_layers(dir)
                .iter()
                .map(|layer| asset_server.load::<Image>(layer.path))
                .collect()
        });
        let rail_depot = asset_server.load::<Image>("assets/opengfx/tiles/rail_depot_ne.png");
        let road_tunnel = asset_server.load::<Image>("assets/opengfx/tiles/tunnel_road_rear.png");
        let rail_tunnel = asset_server.load::<Image>("assets/opengfx/tiles/tunnel_rail_rear.png");
        let road_bridge = asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_road_x.png");
        let road_bridge_y =
            asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_road_y.png");
        let rail_bridge = asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_rail_x.png");
        let rail_bridge_y =
            asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_rail_y.png");
        let bridge_front = [
            asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_x_front.png"),
            asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_y_front.png"),
        ];
        let bridge_pillar = [
            asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_x_pillar.png"),
            asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_y_pillar.png"),
        ];

        let mut houses = HashMap::new();
        for spec in &HOUSE_DRAW_DATA {
            for &sid in &[spec.s1, spec.s2] {
                if sid != 0 {
                    let fname = house_sprite_filename(sid);
                    houses.entry(sid).or_insert_with(|| {
                        asset_server.load::<Image>(format!("assets/opengfx/tiles/{fname}"))
                    });
                }
            }
        }

        let trees = (0..crate::sprites::TREE_SPRITE_COUNT)
            .map(|i| asset_server.load::<Image>(format!("assets/opengfx/tiles/tree_{i:02}.png")))
            .collect();
        let mut fields = Vec::with_capacity(crate::sprites::FIELD_STATES * 15);
        for state in 0..crate::sprites::FIELD_STATES {
            for tileh in 0..15 {
                fields.push(
                    asset_server.load::<Image>(format!(
                        "assets/opengfx/tiles/field_{state}_{tileh:02}.png"
                    )),
                );
            }
        }
        let mut fences = Vec::with_capacity(36);
        for ftype in 0..6 {
            for var in 0..6 {
                fences.push(
                    asset_server
                        .load::<Image>(format!("assets/opengfx/tiles/fence_{ftype}_{var}.png")),
                );
            }
        }
        let chimney_smoke = (0..crate::sprites::CHIMNEY_SMOKE_FRAMES)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/chimney_smoke_{i}.png"))
            })
            .collect();

        let mut industries = HashMap::new();
        for entry in &INDUSTRY_GFX_DATA {
            for &id in &[entry.sprite_id, entry.ground_sprite_id] {
                if id != 0 {
                    industries.entry(id).or_insert_with(|| {
                        asset_server
                            .load::<Image>(format!("assets/opengfx/tiles/industry_{id}.png"))
                    });
                }
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
            rail_depot,
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

#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) fn stub_opengfx_tiles_for_tests(root: &std::path::Path) {
    use std::fs;

    use crate::sprites::{
        HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, house_sprite_filename, rail_sprite_ids_for_preload,
    };

    const ONE_PX_PNG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/one_pixel.png"
    ));

    fn write_png(root: &std::path::Path, rel: &str) {
        let p = root.join(rel);
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir).expect("mkdir");
        }
        fs::write(&p, ONE_PX_PNG).expect("write png");
    }

    write_png(root, "assets/opengfx/tiles/grass.png");
    write_png(root, "assets/opengfx/tiles/grass_rough.png");
    for tileh in 1u8..=14 {
        write_png(
            root,
            &format!("assets/opengfx/tiles/terrain_grass_slope_{tileh:02}.png"),
        );
        write_png(
            root,
            &format!("assets/opengfx/tiles/terrain_rough_slope_{tileh:02}.png"),
        );
    }
    for tileh in 1..=14 {
        write_png(
            root,
            &format!("assets/opengfx/tiles/foundation_{tileh:02}.png"),
        );
    }
    write_png(root, "assets/opengfx/tiles/water.png");
    for f in 0..15 {
        write_png(root, &format!("assets/opengfx/tiles/water_anim_{f:02}.png"));
        for i in 0..crate::sprites::SHORE_SPRITE_COUNT {
            write_png(
                root,
                &format!("assets/opengfx/tiles/shore_full_{i:02}_anim_{f:02}.png"),
            );
        }
    }
    for i in 0..crate::sprites::SHORE_SPRITE_COUNT {
        write_png(root, &format!("assets/opengfx/tiles/shore_full_{i:02}.png"));
    }
    write_png(root, "assets/opengfx/tiles/object_lighthouse.png");
    write_png(root, "assets/opengfx/tiles/object_transmitter.png");
    for i in 0..19 {
        write_png(root, &format!("assets/opengfx/tiles/road_flat_{i:02}.png"));
        write_png(root, &format!("assets/opengfx/tiles/road_paved_{i:02}.png"));
        write_png(root, &format!("assets/opengfx/tiles/tram_flat_{i:02}.png"));
    }
    for i in 0..2 {
        write_png(
            root,
            &format!("assets/opengfx/tiles/road_streetlight_{i}.png"),
        );
    }
    for id in rail_sprite_ids_for_preload() {
        write_png(root, &format!("assets/opengfx/tiles/rail_{id}.png"));
    }
    write_png(root, "assets/opengfx/tiles/road_depot_ground.png");
    for i in 0..4 {
        write_png(
            root,
            &format!("assets/opengfx/tiles/truck_stop_ground_{i}.png"),
        );
        write_png(root, &format!("assets/opengfx/tiles/road_depot_{i}.png"));
    }
    write_png(root, "assets/opengfx/tiles/rail_1412.png");
    write_png(root, "assets/opengfx/tiles/rail_1413.png");
    for dir in ["ne", "se", "sw", "nw"] {
        write_png(
            root,
            &format!("assets/opengfx/tiles/bus_stop_{dir}_ground.png"),
        );
        for layer in ["a", "b", "c"] {
            write_png(
                root,
                &format!("assets/opengfx/tiles/bus_stop_{dir}_build_{layer}.png"),
            );
            write_png(
                root,
                &format!("assets/opengfx/tiles/truck_stop_{dir}_build_{layer}.png"),
            );
        }
    }
    write_png(root, "assets/opengfx/tiles/rail_depot_ne.png");
    write_png(root, "assets/opengfx/tiles/tunnel_road_rear.png");
    write_png(root, "assets/opengfx/tiles/tunnel_rail_rear.png");
    write_png(root, "assets/opengfx/tiles/bridge_wood_road_x.png");
    write_png(root, "assets/opengfx/tiles/bridge_wood_road_y.png");
    write_png(root, "assets/opengfx/tiles/bridge_wood_rail_x.png");
    write_png(root, "assets/opengfx/tiles/bridge_wood_rail_y.png");
    for axis in ["x", "y"] {
        write_png(
            root,
            &format!("assets/opengfx/tiles/bridge_wood_{axis}_front.png"),
        );
        write_png(
            root,
            &format!("assets/opengfx/tiles/bridge_wood_{axis}_pillar.png"),
        );
    }
    for spec in &HOUSE_DRAW_DATA {
        for &sid in &[spec.s1, spec.s2] {
            if sid != 0 {
                let fname = house_sprite_filename(sid);
                write_png(root, &format!("assets/opengfx/tiles/{fname}"));
            }
        }
    }
    for i in 0..crate::sprites::TREE_SPRITE_COUNT {
        write_png(root, &format!("assets/opengfx/tiles/tree_{i:02}.png"));
    }
    for state in 0..crate::sprites::FIELD_STATES {
        for tileh in 0..15 {
            write_png(
                root,
                &format!("assets/opengfx/tiles/field_{state}_{tileh:02}.png"),
            );
        }
    }
    for ftype in 0..6 {
        for var in 0..6 {
            write_png(
                root,
                &format!("assets/opengfx/tiles/fence_{ftype}_{var}.png"),
            );
        }
    }
    for i in 0..crate::sprites::CHIMNEY_SMOKE_FRAMES {
        write_png(root, &format!("assets/opengfx/tiles/chimney_smoke_{i}.png"));
    }
    for entry in &INDUSTRY_GFX_DATA {
        for &id in &[entry.sprite_id, entry.ground_sprite_id] {
            if id != 0 {
                write_png(root, &format!("assets/opengfx/tiles/industry_{id}.png"));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod world_assets_tests {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;

    use super::{WorldAssets, stub_opengfx_tiles_for_tests};

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
        app.update();
        let asset_server = app.world().resource::<AssetServer>();
        let _assets = WorldAssets::load(asset_server);
    }
}
