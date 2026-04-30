use std::collections::HashMap;

use bevy::prelude::*;

use crate::sprites::{
    HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, house_sprite_filename, rail_sprite_ids_for_preload,
};

pub(crate) struct WorldAssets {
    pub(crate) grass: Handle<Image>,
    pub(crate) rough: Handle<Image>,
    pub(crate) grass_slopes: Vec<Handle<Image>>,
    pub(crate) rough_slopes: Vec<Handle<Image>>,
    pub(crate) water: Handle<Image>,
    pub(crate) shore: Vec<Handle<Image>>,
    pub(crate) lighthouse: Handle<Image>,
    pub(crate) transmitter: Handle<Image>,
    pub(crate) road_flat: Vec<Handle<Image>>,
    /// OpenGFX `tram_flat_*` (SPR_TRAMWAY_OVERLAY+0..18); mismo índice que `road_flat_*`.
    pub(crate) tram_flat: Vec<Handle<Image>>,
    pub(crate) rail: HashMap<u32, Handle<Image>>,
    pub(crate) station_grounds: Vec<Handle<Image>>,
    pub(crate) road_depots: Vec<Handle<Image>>,
    pub(crate) rail_depot: Handle<Image>,
    pub(crate) road_tunnel: Handle<Image>,
    pub(crate) rail_tunnel: Handle<Image>,
    pub(crate) road_bridge: Handle<Image>,
    pub(crate) rail_bridge: Handle<Image>,
    pub(crate) houses: HashMap<u32, Handle<Image>>,
    pub(crate) trees: [Handle<Image>; 3],
    pub(crate) industries: HashMap<u32, Handle<Image>>,
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
        let water = asset_server.load::<Image>("assets/opengfx/tiles/water.png");
        let shore = (0..8)
            .map(|i| asset_server.load::<Image>(format!("assets/opengfx/tiles/shore_{i}.png")))
            .collect();
        let lighthouse = asset_server.load::<Image>("assets/opengfx/tiles/object_lighthouse.png");
        let transmitter = asset_server.load::<Image>("assets/opengfx/tiles/object_transmitter.png");
        let road_flat = (0..19)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/road_flat_{i:02}.png"))
            })
            .collect();
        let tram_flat = (0..19)
            .map(|i| {
                asset_server.load::<Image>(format!("assets/opengfx/tiles/tram_flat_{i:02}.png"))
            })
            .collect();
        let rail = rail_sprite_ids_for_preload()
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
        let road_depots = (0..4)
            .map(|i| asset_server.load::<Image>(format!("assets/opengfx/tiles/road_depot_{i}.png")))
            .collect();
        let rail_depot = asset_server.load::<Image>("assets/opengfx/tiles/rail_depot_ne.png");
        let road_tunnel = asset_server.load::<Image>("assets/opengfx/tiles/tunnel_road_rear.png");
        let rail_tunnel = asset_server.load::<Image>("assets/opengfx/tiles/tunnel_rail_rear.png");
        let road_bridge = asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_road_x.png");
        let rail_bridge = asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_rail_x.png");

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

        let trees = [
            asset_server.load::<Image>("assets/opengfx/tiles/tree_00.png"),
            asset_server.load::<Image>("assets/opengfx/tiles/tree_07.png"),
            asset_server.load::<Image>("assets/opengfx/tiles/tree_14.png"),
        ];

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
            lighthouse,
            transmitter,
            road_flat,
            tram_flat,
            rail,
            station_grounds,
            road_depots,
            rail_depot,
            road_tunnel,
            rail_tunnel,
            road_bridge,
            rail_bridge,
            houses,
            trees,
            industries,
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
    write_png(root, "assets/opengfx/tiles/water.png");
    for i in 0..8 {
        write_png(root, &format!("assets/opengfx/tiles/shore_{i}.png"));
    }
    write_png(root, "assets/opengfx/tiles/object_lighthouse.png");
    write_png(root, "assets/opengfx/tiles/object_transmitter.png");
    for i in 0..19 {
        write_png(root, &format!("assets/opengfx/tiles/road_flat_{i:02}.png"));
        write_png(root, &format!("assets/opengfx/tiles/tram_flat_{i:02}.png"));
    }
    for id in rail_sprite_ids_for_preload() {
        write_png(root, &format!("assets/opengfx/tiles/rail_{id}.png"));
    }
    for i in 0..4 {
        write_png(
            root,
            &format!("assets/opengfx/tiles/truck_stop_ground_{i}.png"),
        );
        write_png(root, &format!("assets/opengfx/tiles/road_depot_{i}.png"));
    }
    write_png(root, "assets/opengfx/tiles/rail_depot_ne.png");
    write_png(root, "assets/opengfx/tiles/tunnel_road_rear.png");
    write_png(root, "assets/opengfx/tiles/tunnel_rail_rear.png");
    write_png(root, "assets/opengfx/tiles/bridge_wood_road_x.png");
    write_png(root, "assets/opengfx/tiles/bridge_wood_rail_x.png");
    for spec in &HOUSE_DRAW_DATA {
        for &sid in &[spec.s1, spec.s2] {
            if sid != 0 {
                let fname = house_sprite_filename(sid);
                write_png(root, &format!("assets/opengfx/tiles/{fname}"));
            }
        }
    }
    for name in ["tree_00.png", "tree_07.png", "tree_14.png"] {
        write_png(root, &format!("assets/opengfx/tiles/{name}"));
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
