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
    pub(crate) rail: HashMap<u32, Handle<Image>>,
    pub(crate) station_grounds: Vec<Handle<Image>>,
    pub(crate) houses: HashMap<u32, Handle<Image>>,
    pub(crate) trees: [Handle<Image>; 3],
    pub(crate) industries: HashMap<u32, Handle<Image>>,
}

impl WorldAssets {
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        let grass = asset_server.load::<Image>("opengfx/tiles/grass.png");
        let rough = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
        let grass_slopes = (1u8..=14)
            .map(|tileh| {
                asset_server
                    .load::<Image>(format!("opengfx/tiles/terrain_grass_slope_{tileh:02}.png"))
            })
            .collect();
        let rough_slopes = (1u8..=14)
            .map(|tileh| {
                asset_server
                    .load::<Image>(format!("opengfx/tiles/terrain_rough_slope_{tileh:02}.png"))
            })
            .collect();
        let water = asset_server.load::<Image>("opengfx/tiles/water.png");
        let shore = (0..8)
            .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/shore_{i}.png")))
            .collect();
        let lighthouse = asset_server.load::<Image>("opengfx/tiles/object_lighthouse.png");
        let transmitter = asset_server.load::<Image>("opengfx/tiles/object_transmitter.png");
        let road_flat = (0..19)
            .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/road_flat_{i:02}.png")))
            .collect();
        let rail = rail_sprite_ids_for_preload()
            .into_iter()
            .map(|id| {
                (
                    id,
                    asset_server.load::<Image>(format!("opengfx/tiles/rail_{id}.png")),
                )
            })
            .collect();
        let station_grounds = (0..4)
            .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/truck_stop_ground_{i}.png")))
            .collect();

        let mut houses = HashMap::new();
        for spec in &HOUSE_DRAW_DATA {
            for &sid in &[spec.s1, spec.s2] {
                if sid != 0 {
                    let fname = house_sprite_filename(sid);
                    houses.entry(sid).or_insert_with(|| {
                        asset_server.load::<Image>(format!("opengfx/tiles/{fname}"))
                    });
                }
            }
        }

        let trees = [
            asset_server.load::<Image>("opengfx/tiles/tree_00.png"),
            asset_server.load::<Image>("opengfx/tiles/tree_07.png"),
            asset_server.load::<Image>("opengfx/tiles/tree_14.png"),
        ];

        let mut industries = HashMap::new();
        for entry in &INDUSTRY_GFX_DATA {
            for &id in &[entry.sprite_id, entry.ground_sprite_id] {
                if id != 0 {
                    industries.entry(id).or_insert_with(|| {
                        asset_server.load::<Image>(format!("opengfx/tiles/industry_{id}.png"))
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
            rail,
            station_grounds,
            houses,
            trees,
            industries,
        }
    }
}
