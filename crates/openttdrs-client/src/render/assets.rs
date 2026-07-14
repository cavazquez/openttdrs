use std::collections::HashMap;

use bevy::prelude::*;

use crate::render::atlas::{AtlasSprite, TileAtlas};
use crate::sprites::{
    BridgePaletteSprites, HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, ROAD_DEPOT_GROUND_PATH,
    StationTileClass, house_sprite_filename, rail_depot_build_layers, rail_sprite_ids_for_preload,
    rail_station_draw_layers, rail_station_ground_track_sprite, rail_waypoint_draw_layers,
    road_depot_build_layers, road_stop_build_layers, signal_sprite_texture_id,
};

#[derive(Clone, Resource)]
pub(crate) struct WorldAssets {
    pub(crate) grass: AtlasSprite,
    /// Hierba parcial (`terrain_grass_1_3.png`, densidad m5 & 0x3 == 1).
    pub(crate) grass_one_third: AtlasSprite,
    /// Hierba parcial (`terrain_grass_2_3.png`, densidad m5 & 0x3 == 2).
    pub(crate) grass_two_third: AtlasSprite,
    /// Suelo desnudo (`terrain_bare.png`, densidad m5 & 0x3 == 0).
    pub(crate) bare: AtlasSprite,
    pub(crate) rough: AtlasSprite,
    /// Variantes planas de suelo rocoso (`terrain_rocky_1/2.png`).
    pub(crate) rocky: [AtlasSprite; 2],
    /// Suelo ártico plano (`terrain_snow_full.png`).
    pub(crate) snow: AtlasSprite,
    pub(crate) bought_land: AtlasSprite,
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
    /// Árbol de acera (`Roadside::Trees`, sprite 0x1212).
    pub(crate) roadside_tree: AtlasSprite,
    /// Cercas de vía `track_fence_0..7.png` (`SPR_TRACK_FENCE_*`).
    pub(crate) track_fences: [AtlasSprite; 8],
    /// Frames faro/estadio (`object_lighthouse_anim_*` / `house_s148x_anim_*`).
    pub(crate) lighthouse_anim_frames: HashMap<u32, Vec<AtlasSprite>>,
    /// Ascensor Large Office (`SPR_LIFT` / `house_lift.png`, atlas id ~1443).
    pub(crate) house_lift: AtlasSprite,
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
    /// Depósito naval por dirección (`m5 & 3`).
    pub(crate) ship_depot: [AtlasSprite; 4],
    /// Muelle plano: índice 0 = eje X, 1 = eje Y.
    pub(crate) dock_flat: [AtlasSprite; 2],
    /// Boya (`buoy.png`).
    pub(crate) buoy: AtlasSprite,
    /// Helipuerto / hangar 1×1.
    pub(crate) airport_heliport: AtlasSprite,
    pub(crate) airport_hangar: AtlasSprite,
    pub(crate) airport_apron: AtlasSprite,
    pub(crate) airport_terminal: AtlasSprite,
    pub(crate) airport_runway: AtlasSprite,
    pub(crate) airport_taxiway: AtlasSprite,
    pub(crate) airport_tower: AtlasSprite,
    pub(crate) airport_stand: AtlasSprite,
    /// Radar vanilla: `airport_radar_00` … `_11`.
    pub(crate) airport_radar: [AtlasSprite; 12],
    /// Esclusa: [NS, EW] × [lower, middle, upper].
    pub(crate) water_lock: [[AtlasSprite; 3]; 2],
    /// Portales de túnel por dirección diagonal (0=NE … 3=NW).
    pub(crate) road_tunnels: [AtlasSprite; 4],
    pub(crate) rail_tunnels: [AtlasSprite; 4],
    /// Sprites de puente por id OpenGFX (`bridge_{id}.png` o alias madera).
    pub(crate) bridge_by_id: std::collections::HashMap<u32, AtlasSprite>,
    /// Variantes recoloreadas (`PALETTE_TO_STRUCT_*`) fuera del atlas.
    pub(crate) bridge_palettes: BridgePaletteSprites,
    pub(crate) houses: HashMap<u32, AtlasSprite>,
    /// `tree_{NN}.png` (NN = sprite − 1576): 19 especies × 7 etapas.
    pub(crate) trees: Vec<AtlasSprite>,
    /// `field_{estado}_{tileh:02}.png`: índice = estado × 15 + tileh (0..14).
    pub(crate) fields: Vec<AtlasSprite>,
    /// `fence_{tipo}_{var}.png`: índice = tipo (0..5) × 6 + variante (0..5).
    pub(crate) fences: Vec<AtlasSprite>,
    /// `chimney_smoke_{i}.png`: 8 frames del humo de la central eléctrica.
    pub(crate) chimney_smoke: Vec<AtlasSprite>,
    /// `mine_smoke_{i}.png`: 5 frames del humo de mina de cobre.
    pub(crate) copper_mine_smoke: Vec<AtlasSprite>,
    /// `steam_smoke_{i}.png`: humo locomotoras vapor (SPR_STEAM_SMOKE_0..4).
    pub(crate) steam_smoke: Vec<AtlasSprite>,
    /// `diesel_smoke_{i}.png`: humo diésel (SPR_DIESEL_SMOKE_0..5).
    pub(crate) diesel_smoke: Vec<AtlasSprite>,
    /// `electric_spark_{i}.png`: chispas eléctricas (SPR_ELECTRIC_SPARK_0..5).
    pub(crate) electric_spark: Vec<AtlasSprite>,
    /// `explosion_large_{i}.png`: explosión grande (SPR_EXPLOSION_LARGE_0..F).
    pub(crate) explosion_large: Vec<AtlasSprite>,
    /// `breakdown_smoke_{i}.png`: humo de avería (SPR_BREAKDOWN_SMOKE_0..3).
    pub(crate) breakdown_smoke: Vec<AtlasSprite>,
    pub(crate) industries: HashMap<u32, AtlasSprite>,
    /// Llama refinería: `industry_{id}_fire_anim_{f}.png` (7 frames por sprite).
    pub(crate) refinery_fire_frames: HashMap<u32, Vec<AtlasSprite>>,
    /// Bebidas gaseosas: `industry_{id}_fizzy_anim_{f}.png` (5 frames por sprite).
    pub(crate) fizzy_drink_frames: HashMap<u32, Vec<AtlasSprite>>,
    /// Cimientos nivelados (`foundation_01..14.png`, SPR_FOUNDATION_BASE + tileh).
    pub(crate) foundations: Vec<AtlasSprite>,
}

/// Nombre en el atlas para un sprite de industria o suelo vanilla (`SPR_FLAT_GRASS_TILE` = 3981).
#[must_use]
fn industry_sprite_atlas_name(id: u32) -> String {
    match id {
        3981 => "grass.png".into(),
        3982..=3995 => format!("terrain_grass_slope_{:02}.png", id - 3981),
        _ => format!("industry_{id}.png"),
    }
}

impl WorldAssets {
    /// Resuelve todos los sprites del mapa contra el [`TileAtlas`]; no toca
    /// el filesystem (la tabla de rects es metadata compilada).
    pub(crate) fn load(atlas: &TileAtlas, images: &mut Assets<Image>) -> Self {
        let grass = atlas.get("grass.png");
        let grass_one_third = atlas.get("terrain_grass_1_3.png");
        let grass_two_third = atlas.get("terrain_grass_2_3.png");
        let bare = atlas.get("terrain_bare.png");
        let rough = atlas.get("grass_rough.png");
        let rocky = [
            atlas.get("terrain_rocky_1.png"),
            atlas.get("terrain_rocky_2.png"),
        ];
        let snow = atlas.get("terrain_snow_full.png");
        let bought_land = atlas.get("object_bought_land.png");
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
        let roadside_tree = atlas.get("roadside_tree.png");
        let track_fences = std::array::from_fn(|i| atlas.get(&format!("track_fence_{i}.png")));
        let mut lighthouse_anim_frames = HashMap::new();
        for &id in &[2602u32, 1483, 1484, 1485, 1486] {
            let frames: Vec<_> = (0..4)
                .filter_map(|f| {
                    let name = if id == 2602 {
                        format!("object_lighthouse_anim_{f:02}.png")
                    } else {
                        format!("house_s{id}_anim_{f:02}.png")
                    };
                    atlas.try_get(&name)
                })
                .collect();
            if frames.len() == 4 {
                lighthouse_anim_frames.insert(id, frames);
            }
        }
        let house_lift = atlas.get("house_lift.png");
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
        let mut rail = std::collections::HashMap::new();
        for id in rail_ids {
            let tex_id = signal_sprite_texture_id(id);
            let sprite = crate::sprites::rail_sprite_atlas_keys(tex_id)
                .into_iter()
                .find_map(|k| atlas.try_get(&k))
                .unwrap_or_else(|| atlas.get(&format!("rail_{tex_id}.png")));
            rail.insert(tex_id, sprite.clone());
            if tex_id != id {
                rail.insert(id, sprite);
            }
        }
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
        let ship_depot = [
            atlas.get("ship_depot_ne.png"),
            atlas.get("ship_depot_se_front.png"),
            atlas.get("ship_depot_sw_front.png"),
            atlas.get("ship_depot_nw.png"),
        ];
        let dock_flat = [atlas.get("dock_flat_x.png"), atlas.get("dock_flat_y.png")];
        let buoy = atlas.get("buoy.png");
        let airport_heliport = atlas.get("airport_heliport.png");
        let airport_hangar = atlas.get("airport_hangar_front.png");
        let airport_apron = atlas.get("airport_apron.png");
        let airport_terminal = atlas.get("airport_terminal_a.png");
        let airport_runway = atlas.get("airport_runway_0.png");
        let airport_taxiway = atlas.get("airport_taxiway_0.png");
        let airport_tower = atlas.get("airport_tower.png");
        let airport_stand = atlas.get("airport_stand.png");
        let airport_radar: [AtlasSprite; 12] =
            std::array::from_fn(|i| atlas.get(&format!("airport_radar_{i:02}.png")));
        let water_lock = [
            [
                atlas.get("water_lock_ns_lower.png"),
                atlas.get("water_lock_ns_middle.png"),
                atlas.get("water_lock_ns_upper.png"),
            ],
            [
                atlas.get("water_lock_ew_lower.png"),
                atlas.get("water_lock_ew_middle.png"),
                atlas.get("water_lock_ew_upper.png"),
            ],
        ];
        use crate::sprites::{tunnel_rear_atlas_name, tunnel_rear_legacy_atlas_name};
        let road_tunnels = std::array::from_fn(|dir| {
            atlas
                .try_get(&tunnel_rear_atlas_name(false, dir as u8))
                .or_else(|| atlas.try_get(tunnel_rear_legacy_atlas_name(false)))
                .unwrap_or_else(|| {
                    error!("Sprite de túnel carretera dir {dir} no encontrado");
                    atlas.get("grass.png")
                })
        });
        let rail_tunnels = std::array::from_fn(|dir| {
            atlas
                .try_get(&tunnel_rear_atlas_name(true, dir as u8))
                .or_else(|| atlas.try_get(tunnel_rear_legacy_atlas_name(true)))
                .unwrap_or_else(|| {
                    error!("Sprite de túnel ferrocarril dir {dir} no encontrado");
                    atlas.get("grass.png")
                })
        });
        let mut bridge_by_id = std::collections::HashMap::new();
        use crate::sprites::{BridgeDeckSpriteIds, bridge_deck_sprite_ids};
        use openttdrs_core::{BridgePiece, BridgeType};
        for bt in 0..13u8 {
            let Some(bridge_type) = BridgeType::from_u8(bt) else {
                continue;
            };
            for (pi, piece) in [
                (0, BridgePiece::North),
                (1, BridgePiece::South),
                (2, BridgePiece::InnerNorth),
                (3, BridgePiece::InnerSouth),
                (4, BridgePiece::MiddleOdd),
                (5, BridgePiece::MiddleEven),
            ] {
                let _ = pi;
                let ids = bridge_deck_sprite_ids(bridge_type, piece);
                for sid in ids
                    .rear_rail
                    .iter()
                    .chain(ids.rear_road.iter())
                    .chain(ids.front.iter())
                    .chain(ids.pillar.iter())
                    .copied()
                    .filter(|id| *id != 0)
                {
                    bridge_by_id.entry(sid).or_insert_with(|| {
                        let name = BridgeDeckSpriteIds::atlas_name(sid);
                        atlas.try_get(&name).unwrap_or_else(|| {
                            error!("Sprite de puente no encontrado en atlas: {name}");
                            atlas.get("bridge_wood_road_x.png")
                        })
                    });
                }
            }
        }

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
        let copper_mine_smoke = (0..crate::sprites::COPPER_MINE_SMOKE_FRAMES)
            .map(|i| atlas.get(&format!("mine_smoke_{i}.png")))
            .collect();
        let steam_smoke = (0..crate::sprites::STEAM_SMOKE_FRAMES)
            .map(|i| atlas.get(&format!("steam_smoke_{i}.png")))
            .collect();
        let diesel_smoke = (0..crate::sprites::DIESEL_SMOKE_FRAMES)
            .map(|i| atlas.get(&format!("diesel_smoke_{i}.png")))
            .collect();
        let electric_spark = (0..crate::sprites::ELECTRIC_SPARK_FRAMES)
            .map(|i| atlas.get(&format!("electric_spark_{i}.png")))
            .collect();
        let explosion_large = (0..crate::sprites::EXPLOSION_LARGE_FRAMES)
            .map(|i| atlas.get(&format!("explosion_large_{i}.png")))
            .collect();
        let breakdown_smoke = (0..crate::sprites::BREAKDOWN_SMOKE_FRAMES)
            .map(|i| atlas.get(&format!("breakdown_smoke_{i}.png")))
            .collect();

        let mut industries = HashMap::new();
        for entry in &INDUSTRY_GFX_DATA {
            for &id in &[entry.sprite_id, entry.ground_sprite_id] {
                if id == 0 || industries.contains_key(&id) {
                    continue;
                }
                let name = industry_sprite_atlas_name(id);
                if let Some(sprite) = atlas.try_get(&name) {
                    industries.insert(id, sprite);
                }
            }
        }
        for id in crate::sprites::INDUSTRY_DRAW_PROC_SPRITE_IDS {
            if let Some(img) = atlas.try_get(&format!("industry_{id}.png")) {
                industries.entry(id).or_insert(img);
            }
        }

        let mut refinery_fire_frames = HashMap::new();
        for &id in &crate::sprites::REFINERY_FIRE_SPRITE_IDS {
            let frames: Vec<_> = (0..7)
                .filter_map(|f| atlas.try_get(&format!("industry_{id}_fire_anim_{f:02}.png")))
                .collect();
            if frames.len() == 7 {
                refinery_fire_frames.insert(id, frames);
            }
        }

        let mut fizzy_drink_frames = HashMap::new();
        for &id in &crate::sprites::FIZZY_DRINK_SPRITE_IDS {
            let frames: Vec<_> = (0..5)
                .filter_map(|f| atlas.try_get(&format!("industry_{id}_fizzy_anim_{f:02}.png")))
                .collect();
            if frames.len() == 5 {
                fizzy_drink_frames.insert(id, frames);
            }
        }

        let mut bridge_palettes = BridgePaletteSprites::default();
        bridge_palettes.build_all(images);

        Self {
            grass,
            grass_one_third,
            grass_two_third,
            bare,
            rough,
            rocky,
            snow,
            bought_land,
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
            roadside_tree,
            track_fences,
            lighthouse_anim_frames,
            house_lift,
            tram_flat,
            rail,
            station_grounds,
            bus_stop_grounds,
            bus_stop_builds,
            truck_stop_builds,
            road_depot_ground,
            road_depot_builds,
            rail_depot_builds,
            ship_depot,
            dock_flat,
            buoy,
            airport_heliport,
            airport_hangar,
            airport_apron,
            airport_terminal,
            airport_runway,
            airport_taxiway,
            airport_tower,
            airport_stand,
            airport_radar,
            water_lock,
            road_tunnels,
            rail_tunnels,
            bridge_by_id,
            bridge_palettes,
            houses,
            trees,
            fields,
            fences,
            chimney_smoke,
            copper_mine_smoke,
            steam_smoke,
            diesel_smoke,
            electric_spark,
            explosion_large,
            breakdown_smoke,
            industries,
            refinery_fire_frames,
            fizzy_drink_frames,
            foundations,
        }
    }

    pub(crate) fn airport_piece_sprite(&self, piece: openttdrs_core::AirportPiece) -> &AtlasSprite {
        use openttdrs_core::AirportPiece;
        match piece {
            AirportPiece::Heliport => &self.airport_heliport,
            AirportPiece::Hangar => &self.airport_hangar,
            AirportPiece::Apron => &self.airport_apron,
            AirportPiece::Terminal => &self.airport_terminal,
            AirportPiece::Runway => &self.airport_runway,
            AirportPiece::Taxiway => &self.airport_taxiway,
            AirportPiece::Tower => &self.airport_tower,
            AirportPiece::Stand => &self.airport_stand,
        }
    }

    #[must_use]
    pub(crate) fn bridge_sprite(&self, id: u32) -> Option<&AtlasSprite> {
        self.bridge_by_id.get(&id)
    }

    #[must_use]
    pub(crate) fn tunnel_portal_sprite(&self, rail: bool, dir: u8) -> &AtlasSprite {
        let d = dir as usize & 3;
        if rail {
            &self.rail_tunnels[d]
        } else {
            &self.road_tunnels[d]
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
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let _assets = WorldAssets::load(&atlas, &mut images);
    }
}
