use std::collections::HashMap;

use bevy::prelude::*;

use crate::render::atlas::{AtlasSprite, TileAtlas};
use crate::sprites::{
    AIRPORT_STATION_SPRITES, BridgePaletteSprites, HOUSE_DRAW_DATA, HousePaletteSprites,
    INDUSTRY_GFX_DATA, RAIL_DEPOT_VISUAL_TYPE_COUNT, ROAD_DEPOT_GROUND_PATH, StationTileClass,
    airport_station_base_for_gfx, house_sprite_asset_filename, rail_depot_build_layers,
    rail_pbs_sprite_ids_for_preload, rail_sprite_ids_for_preload, rail_station_draw_layers,
    rail_station_ground_track_sprite_for_type, rail_station_layer_for_type,
    rail_waypoint_draw_layers, road_depot_build_layers, road_stop_build_layers,
    road_stop_drive_through_layers, signal_sprite_texture_id,
};

#[derive(Clone, Resource)]
pub(crate) struct WorldAssets {
    pub(crate) grass: AtlasSprite,
    pub(crate) rough: AtlasSprite,
    /// Variantes planas que `DrawHillyLandTile` escoge con `TileHash(x, y)`.
    /// La primera coincide con `terrain_rough.png` / `grass_rough.png`.
    pub(crate) rough_flat: [AtlasSprite; 5],
    /// Las dos series completas `SPR_FLAT_ROCKY_LAND_{1,2}` × los 19 offsets
    /// de `SlopeToSpriteOffset`. OpenGFX y OpenGFX2 usan hoy la primera,
    /// pero el atlas conserva ambas para que un futuro baseset que active
    /// `SecondRockyTileSet` no vuelva a degradar una ladera a rough.
    pub(crate) rocky: [[AtlasSprite; 19]; 2],
    pub(crate) bought_land: AtlasSprite,
    /// `SPR_CONCRETE_GROUND`, base de la estatua de compañía (`MP_OBJECT`).
    pub(crate) object_concrete: AtlasSprite,
    /// `DrawClearLandTile`: densidad 0..3 × `SlopeToSpriteOffset` 0..18.
    /// También lo usa `DrawTile_Trees` para `TreeGround::Grass`.
    pub(crate) grass_density: [[AtlasSprite; 19]; 4],
    pub(crate) grass_slopes: Vec<AtlasSprite>,
    pub(crate) rough_slopes: Vec<AtlasSprite>,
    /// `_clear_land_sprites_snow_desert`: densidad 0..3 × pendiente 0..18.
    /// `TreeGround::SnowDesert` y `TreeGround::RoughSnow` comparten este set.
    pub(crate) snow_desert: [[AtlasSprite; 19]; 4],
    pub(crate) water: AtlasSprite,
    /// `SPR_FLAT_WATER_TILE + SlopeToSpriteOffset`, usado por los bordes
    /// `Void` cuando `construction.freeform_edges` está desactivado.
    pub(crate) water_slopes: [AtlasSprite; 19],
    /// Set completo `SPR_SHORE_BASE + 0..17` (`shore_full_{i:02}.png`).
    pub(crate) shore: Vec<AtlasSprite>,
    /// 5 fases dark × 15 glitter (`water_anim_d{d}_g{g}.png`).
    pub(crate) water_frames: Vec<AtlasSprite>,
    /// 18 orillas × 5 fases dark × 15 glitter.
    pub(crate) shore_frames: Vec<Vec<AtlasSprite>>,
    pub(crate) lighthouse: AtlasSprite,
    pub(crate) transmitter: AtlasSprite,
    pub(crate) company_statue: AtlasSprite,
    pub(crate) road_flat: Vec<AtlasSprite>,
    /// Set pavimentado (`SPR_ROAD_Y - 19` = 1313..1331), mismo orden que `road_flat`.
    pub(crate) road_paved: Vec<AtlasSprite>,
    /// Flechas base `Action5 0x09` de `openttd.grf` (`SPR_ONEWAY_BASE` + slot).
    ///
    /// Son parte del fallback oficial de OpenTTD, no de un `NewGRF` de la
    /// partida; un Action5 real puede reemplazarlas en runtime.
    pub(crate) oneway_roads: [AtlasSprite; crate::sprites::ONEWAY_ROAD_SPRITE_COUNT],
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
    /// Copias de vía remapeadas exactamente con `PALETTE_CRASH=804`.
    ///
    /// El atlas es RGBA y ya no conoce los índices DOS originales, por eso no
    /// se puede reproducir este recolor con un `Color` de Bevy.
    rail_pbs: HashMap<u32, AtlasSprite>,
    pub(crate) station_grounds: Vec<AtlasSprite>,
    pub(crate) bus_stop_grounds: Vec<AtlasSprite>,
    pub(crate) bus_stop_builds: [[AtlasSprite; 3]; 4],
    pub(crate) truck_stop_builds: [[AtlasSprite; 3]; 4],
    /// Tiras de señalización de parada pasante: [eje X/Y][lado W/E].
    pub(crate) bus_stop_drive_through: [[AtlasSprite; 2]; 2],
    pub(crate) truck_stop_drive_through: [[AtlasSprite; 2]; 2],
    pub(crate) road_depot_ground: AtlasSprite,
    pub(crate) road_depot_builds: [Vec<AtlasSprite>; 4],
    /// Capas del depósito de vía: [rail/electric, mono, maglev][dirección].
    pub(crate) rail_depot_builds: [[Vec<AtlasSprite>; 4]; RAIL_DEPOT_VISUAL_TYPE_COUNT],
    /// Capas del depósito naval vanilla, en orden de sprite OpenTTD 4070..4075.
    ///
    /// Cada depósito ocupa dos teselas: `m5 & 1` selecciona la parte norte/sur
    /// y `m5 & 2` el eje. La parte sur suma una capa posterior pequeña.
    pub(crate) ship_depot: [AtlasSprite; 6],
    /// Muelle: las cuatro piezas de tierra se indexan por `DiagDirection`
    /// (NE, SE, SW, NW); las dos planas quedan sobre agua por eje X/Y.
    pub(crate) dock_slope: [AtlasSprite; 4],
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
    /// Todos los sprites que usa el `StationGfx` airport vanilla, indexados
    /// por el ID lógico de OpenTTD. Incluye Action5 (helipads) y los frames
    /// de radar/bandera, para que un save no vuelva al icono genérico.
    airport_station_sprites: HashMap<u32, AtlasSprite>,
    /// Esclusa: [NS, EW] × [lower, middle, upper].
    pub(crate) water_lock: [[AtlasSprite; 3]; 2],
    /// Capas traseras de túnel por dirección diagonal (0=NE … 3=NW): suelo.
    pub(crate) road_tunnels: [AtlasSprite; 4],
    pub(crate) rail_tunnels: [AtlasSprite; 4],
    pub(crate) monorail_tunnels: [AtlasSprite; 4],
    pub(crate) maglev_tunnels: [AtlasSprite; 4],
    /// Capas frontales/techo del túnel; OpenTTD las dibuja como sortable.
    pub(crate) road_tunnel_fronts: [AtlasSprite; 4],
    pub(crate) rail_tunnel_fronts: [AtlasSprite; 4],
    pub(crate) monorail_tunnel_fronts: [AtlasSprite; 4],
    pub(crate) maglev_tunnel_fronts: [AtlasSprite; 4],
    /// Sprites de puente por id OpenGFX (`bridge_{id}.png` o alias madera).
    pub(crate) bridge_by_id: std::collections::HashMap<u32, AtlasSprite>,
    /// Variantes recoloreadas (`PALETTE_TO_STRUCT_*`) fuera del atlas.
    pub(crate) bridge_palettes: BridgePaletteSprites,
    /// Variantes de casas con `PaletteID` vanilla (estructura, iglesia o
    /// compañía) aplicada fuera del atlas RGBA.
    pub(crate) house_palettes: HousePaletteSprites,
    pub(crate) houses: HashMap<u32, AtlasSprite>,
    /// `tree_{NN}.png` (NN = sprite − 1576): 19 especies × 7 etapas.
    pub(crate) trees: Vec<AtlasSprite>,
    /// `field_{estado}_{offset:02}.png`: índice = estado × 19 +
    /// `SlopeToSpriteOffset` (0..18).
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
    /// `bubble_{i}.png`: EV_BUBBLE (base, generación, burst y absorción).
    pub(crate) bubble: Vec<AtlasSprite>,
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
        3982..=3999 => format!("terrain_grass_slope_{:02}.png", id - 3981),
        4000 => "terrain_rough.png".into(),
        4001..=4018 => format!("terrain_rough_slope_{:02}.png", id - 4000),
        _ => format!("industry_{id}.png"),
    }
}

impl WorldAssets {
    /// Resuelve todos los sprites del mapa contra el [`TileAtlas`]; no toca
    /// el filesystem (la tabla de rects es metadata compilada).
    pub(crate) fn load(atlas: &TileAtlas, images: &mut Assets<Image>) -> Self {
        let grass = atlas.get("grass.png");
        let rough = atlas.get("grass_rough.png");
        let rough_flat = std::array::from_fn(|variant| {
            if variant == 0 {
                atlas.get("terrain_rough.png")
            } else {
                atlas.get(&format!("terrain_rough_{variant}.png"))
            }
        });
        let rocky = std::array::from_fn(|variant| {
            std::array::from_fn(|offset| {
                atlas.get(&format!("terrain_rocky_{}_{}.png", variant + 1, offset))
            })
        });
        let grass_density = std::array::from_fn(|density| {
            std::array::from_fn(|offset| {
                atlas.get(&format!("terrain_grass_density_{density}_{offset:02}.png"))
            })
        });
        let snow_desert = std::array::from_fn(|density| {
            std::array::from_fn(|offset| {
                atlas.get(&format!("terrain_snow_desert_{density}_{offset:02}.png"))
            })
        });
        let bought_land = atlas.get("object_bought_land.png");
        let object_concrete = atlas.get("object_concrete.png");
        // `SlopeToSpriteOffset` puede devolver 15..18 para las cuatro
        // pendientes empinadas. El vector se indexa por offset - 1.
        let grass_slopes = (1u8..=18)
            .map(|tileh| atlas.get(&format!("terrain_grass_slope_{tileh:02}.png")))
            .collect();
        let rough_slopes = (1u8..=18)
            .map(|tileh| atlas.get(&format!("terrain_rough_slope_{tileh:02}.png")))
            .collect();
        let foundations = (1u8..=14)
            .map(|tileh| atlas.get(&format!("foundation_{tileh:02}.png")))
            .collect();
        let water = atlas.get("water.png");
        let water_slopes =
            std::array::from_fn(|offset| atlas.get(&format!("terrain_water_{offset:02}.png")));
        let shore: Vec<AtlasSprite> = (0..crate::sprites::SHORE_SPRITE_COUNT)
            .map(|i| atlas.get(&format!("shore_full_{i:02}.png")))
            .collect();
        let water_frames = (0..crate::sprites::DARK_WATER_FRAME_COUNT)
            .flat_map(|d| {
                (0..crate::sprites::GLITTER_WATER_FRAME_COUNT)
                    .map(move |g| atlas.get(&format!("water_anim_d{d:02}_g{g:02}.png")))
            })
            .collect();
        let shore_frames = (0..crate::sprites::SHORE_SPRITE_COUNT)
            .map(|i| {
                (0..crate::sprites::DARK_WATER_FRAME_COUNT)
                    .flat_map(|d| {
                        (0..crate::sprites::GLITTER_WATER_FRAME_COUNT).map(move |g| {
                            atlas.get(&format!("shore_full_{i:02}_anim_d{d:02}_g{g:02}.png"))
                        })
                    })
                    .collect()
            })
            .collect();
        let lighthouse = atlas.get("object_lighthouse.png");
        let transmitter = atlas.get("object_transmitter.png");
        let company_statue = atlas.get("object_statue_company.png");
        let road_flat = (0..19)
            .map(|i| atlas.get(&format!("road_flat_{i:02}.png")))
            .collect();
        let road_paved = (0..19)
            .map(|i| atlas.get(&format!("road_paved_{i:02}.png")))
            .collect();
        let oneway_roads = std::array::from_fn(|i| atlas.get(&format!("oneway_{i:02}.png")));
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
        for rail_type in [
            openttdrs_core::RailType::Rail,
            openttdrs_core::RailType::Electric,
            openttdrs_core::RailType::Monorail,
            openttdrs_core::RailType::Maglev,
        ] {
            for gfx in 0..=7 {
                rail_ids.insert(rail_station_ground_track_sprite_for_type(gfx, 0, rail_type));
                for layer in rail_station_draw_layers(gfx) {
                    rail_ids.insert(rail_station_layer_for_type(*layer, rail_type).sprite_id);
                }
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
        let rail_pbs = rail_pbs_sprite_ids_for_preload()
            .into_iter()
            .filter_map(|id| {
                atlas
                    .try_get(&format!("rail_pbs_{id}.png"))
                    .map(|sprite| (id, sprite))
            })
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
        let bus_stop_drive_through = std::array::from_fn(|axis| {
            std::array::from_fn(|layer| {
                atlas.get_path(
                    road_stop_drive_through_layers(StationTileClass::Bus, 4 + axis as u8)[layer]
                        .path,
                )
            })
        });
        let truck_stop_drive_through = std::array::from_fn(|axis| {
            std::array::from_fn(|layer| {
                atlas.get_path(
                    road_stop_drive_through_layers(StationTileClass::Truck, 4 + axis as u8)[layer]
                        .path,
                )
            })
        });
        let road_depot_ground = atlas.get_path(ROAD_DEPOT_GROUND_PATH);
        let road_depot_builds = std::array::from_fn(|dir| {
            road_depot_build_layers(dir)
                .iter()
                .map(|layer| atlas.get_path(layer.path))
                .collect()
        });
        let rail_depot_builds = std::array::from_fn(|variant| {
            let rail_type = match variant {
                1 => openttdrs_core::RailType::Monorail,
                2 => openttdrs_core::RailType::Maglev,
                _ => openttdrs_core::RailType::Rail,
            };
            std::array::from_fn(|dir| {
                rail_depot_build_layers(rail_type, dir)
                    .iter()
                    .map(|layer| atlas.get_path(layer.path))
                    .collect()
            })
        });
        let ship_depot = [
            atlas.get("ship_depot_se_front.png"),
            atlas.get("ship_depot_sw_front.png"),
            atlas.get("ship_depot_nw.png"),
            atlas.get("ship_depot_ne.png"),
            atlas.get("ship_depot_se_rear.png"),
            atlas.get("ship_depot_sw_rear.png"),
        ];
        let dock_slope = [
            atlas.get("dock_slope_ne.png"),
            atlas.get("dock_slope_se.png"),
            atlas.get("dock_slope_sw.png"),
            atlas.get("dock_slope_nw.png"),
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
        let airport_station_sprites = AIRPORT_STATION_SPRITES
            .iter()
            .filter_map(|spec| {
                let name = spec.path.rsplit('/').next().unwrap_or(spec.path);
                atlas.try_get(name).map(|sprite| (spec.sprite_id, sprite))
            })
            .collect();
        // Esclusas: `scripts/gen_water_lock_tiles.py` (Action5 canals SPR_LOCK_*).
        // Fallback a agua plana si faltan PNGs / atlas desactualizado.
        let water_lock_fallback = atlas
            .try_get("water_flat.png")
            .unwrap_or_else(|| atlas.get("water.png"));
        let lock_names = [
            "water_lock_ns_lower.png",
            "water_lock_ns_middle.png",
            "water_lock_ns_upper.png",
            "water_lock_ew_lower.png",
            "water_lock_ew_middle.png",
            "water_lock_ew_upper.png",
        ];
        let missing_locks: Vec<&str> = lock_names
            .iter()
            .copied()
            .filter(|n| atlas.try_get(n).is_none())
            .collect();
        if !missing_locks.is_empty() {
            warn!(
                "Sprites de esclusa ausentes en atlas ({}): fallback a agua plana — corré scripts/gen_water_lock_tiles.py && gen_tile_atlas.py",
                missing_locks.len()
            );
        }
        let water_lock_sprite = |name: &str| {
            atlas
                .try_get(name)
                .unwrap_or_else(|| water_lock_fallback.clone())
        };
        let water_lock = [
            [
                water_lock_sprite(lock_names[0]),
                water_lock_sprite(lock_names[1]),
                water_lock_sprite(lock_names[2]),
            ],
            [
                water_lock_sprite(lock_names[3]),
                water_lock_sprite(lock_names[4]),
                water_lock_sprite(lock_names[5]),
            ],
        ];
        use crate::sprites::{
            rail_tunnel_front_atlas_name, rail_tunnel_rear_atlas_name, tunnel_front_atlas_name,
            tunnel_rear_atlas_name, tunnel_rear_legacy_atlas_name,
        };
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
        let monorail_tunnels = std::array::from_fn(|dir| {
            atlas
                .try_get(&rail_tunnel_rear_atlas_name(
                    openttdrs_core::RailType::Monorail,
                    dir as u8,
                ))
                // Compatibilidad con atlas generados antes de los cuatro portales mono.
                .or_else(|| atlas.try_get("tunnel_mono_rear.png"))
                .unwrap_or_else(|| rail_tunnels[dir].clone())
        });
        let maglev_tunnels = std::array::from_fn(|dir| {
            atlas
                .try_get(&rail_tunnel_rear_atlas_name(
                    openttdrs_core::RailType::Maglev,
                    dir as u8,
                ))
                // Compatibilidad con atlas generados antes de los cuatro portales maglev.
                .or_else(|| atlas.try_get("tunnel_mglv_rear.png"))
                .unwrap_or_else(|| rail_tunnels[dir].clone())
        });
        let road_tunnel_fronts = std::array::from_fn(|dir| {
            atlas
                .try_get(&tunnel_front_atlas_name(false, dir as u8))
                .unwrap_or_else(|| {
                    error!("Sprite frontal de túnel carretera dir {dir} no encontrado");
                    road_tunnels[dir].clone()
                })
        });
        let rail_tunnel_fronts = std::array::from_fn(|dir| {
            atlas
                .try_get(&tunnel_front_atlas_name(true, dir as u8))
                .unwrap_or_else(|| {
                    error!("Sprite frontal de túnel ferrocarril dir {dir} no encontrado");
                    rail_tunnels[dir].clone()
                })
        });
        let monorail_tunnel_fronts = std::array::from_fn(|dir| {
            atlas
                .try_get(&rail_tunnel_front_atlas_name(
                    openttdrs_core::RailType::Monorail,
                    dir as u8,
                ))
                .unwrap_or_else(|| rail_tunnel_fronts[dir].clone())
        });
        let maglev_tunnel_fronts = std::array::from_fn(|dir| {
            atlas
                .try_get(&rail_tunnel_front_atlas_name(
                    openttdrs_core::RailType::Maglev,
                    dir as u8,
                ))
                .unwrap_or_else(|| rail_tunnel_fronts[dir].clone())
        });
        let mut bridge_by_id = std::collections::HashMap::new();
        use crate::sprites::{BridgeDeckSpriteIds, bridge_deck_sprite_ids, bridge_ramp_sprite_id};
        use openttdrs_core::{BridgePiece, BridgeType, RailType};
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
                    .chain(ids.rear_mono.iter())
                    .chain(ids.rear_maglev.iter())
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
            for rail in [false, true] {
                for rail_type in [
                    RailType::Rail,
                    RailType::Electric,
                    RailType::Monorail,
                    RailType::Maglev,
                ] {
                    if !rail && rail_type != RailType::Rail {
                        continue;
                    }
                    for tileh in [0, 1] {
                        for dir in 0..4 {
                            let sid =
                                bridge_ramp_sprite_id(bridge_type, rail, rail_type, tileh, dir);
                            bridge_by_id.entry(sid).or_insert_with(|| {
                                let name = BridgeDeckSpriteIds::atlas_name(sid);
                                atlas.try_get(&name).unwrap_or_else(|| {
                                    error!(
                                        "Sprite de rampa de puente no encontrado en atlas: {name}"
                                    );
                                    atlas.get("bridge_wood_road_x.png")
                                })
                            });
                        }
                    }
                }
            }
        }

        let mut houses = HashMap::new();
        for spec in &HOUSE_DRAW_DATA {
            for &sid in &[spec.s1, spec.s2] {
                if sid != 0 {
                    // Los dos suelos que aparecen más veces en
                    // `_town_draw_tile_data` son sprites generales de
                    // terreno, no assets exclusivos de casas. Resolverlos
                    // por su nombre canónico evita que el extractor los
                    // trate como un overlay `house_s*` y mantiene el atlas
                    // válido aun antes de una regeneración completa.
                    let name = house_sprite_asset_filename(sid);
                    houses.entry(sid).or_insert_with(|| atlas.get(&name));
                }
            }
        }

        let trees = (0..crate::sprites::TREE_SPRITE_COUNT)
            .map(|i| atlas.get(&format!("tree_{i:02}.png")))
            .collect();
        let mut fields = Vec::with_capacity(crate::sprites::FIELD_STATES * 19);
        for state in 0..crate::sprites::FIELD_STATES {
            for offset in 0..19 {
                fields.push(atlas.get(&format!("field_{state}_{offset:02}.png")));
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
        let bubble = (0..crate::sprites::BUBBLE_FRAMES)
            .map(|i| atlas.get(&format!("bubble_{i}.png")))
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
            } else {
                bevy::log::warn!(
                    "Faltan frames de fuego refinería para sprite {id} ({}/7 en atlas)",
                    frames.len()
                );
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
        let mut house_palettes = HousePaletteSprites::default();
        house_palettes.build_all(images);

        Self {
            grass,
            rough,
            rough_flat,
            rocky,
            bought_land,
            object_concrete,
            grass_density,
            grass_slopes,
            rough_slopes,
            snow_desert,
            water,
            water_slopes,
            shore,
            water_frames,
            shore_frames,
            lighthouse,
            transmitter,
            company_statue,
            road_flat,
            road_paved,
            oneway_roads,
            road_streetlights,
            roadside_tree,
            track_fences,
            lighthouse_anim_frames,
            house_lift,
            tram_flat,
            rail,
            rail_pbs,
            station_grounds,
            bus_stop_grounds,
            bus_stop_builds,
            truck_stop_builds,
            bus_stop_drive_through,
            truck_stop_drive_through,
            road_depot_ground,
            road_depot_builds,
            rail_depot_builds,
            ship_depot,
            dock_slope,
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
            airport_station_sprites,
            water_lock,
            road_tunnels,
            rail_tunnels,
            monorail_tunnels,
            maglev_tunnels,
            road_tunnel_fronts,
            rail_tunnel_fronts,
            monorail_tunnel_fronts,
            maglev_tunnel_fronts,
            bridge_by_id,
            bridge_palettes,
            house_palettes,
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
            bubble,
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

    /// Sprite más específico disponible para un `StationGfx` vanilla.
    #[must_use]
    pub(crate) fn airport_station_gfx_sprite(&self, gfx: u8) -> &AtlasSprite {
        if let Some(base) = airport_station_base_for_gfx(gfx)
            && let Some(sprite) = self.airport_station_sprite(base.sprite_id)
        {
            return sprite;
        }
        self.airport_piece_sprite(openttdrs_core::AirportPiece::from_station_gfx(gfx))
    }

    /// Sprite exacto de la tabla airport o `None` si el atlas no contiene el
    /// recurso solicitado. El renderer usa esto para señalar un fallback sin
    /// fingir que el sprite de otra tesela es equivalente.
    #[must_use]
    pub(crate) fn airport_station_sprite(&self, sprite_id: u32) -> Option<&AtlasSprite> {
        self.airport_station_sprites.get(&sprite_id)
    }

    #[must_use]
    pub(crate) fn bridge_sprite(&self, id: u32) -> Option<&AtlasSprite> {
        self.bridge_by_id.get(&id)
    }

    /// Sprite de reserva PBS. Ante assets antiguos conserva una vía visible,
    /// pero la traza lo marca como fallback porque no tiene el remapeo exacto.
    #[must_use]
    pub(crate) fn pbs_rail_sprite(&self, id: u32) -> Option<&AtlasSprite> {
        self.rail_pbs.get(&id).or_else(|| self.rail.get(&id))
    }

    #[must_use]
    pub(crate) fn has_exact_pbs_rail_sprite(&self, id: u32) -> bool {
        self.rail_pbs.contains_key(&id)
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

    #[must_use]
    pub(crate) fn rail_tunnel_portal_sprite(
        &self,
        rail_type: openttdrs_core::RailType,
        dir: u8,
    ) -> &AtlasSprite {
        let d = dir as usize & 3;
        match rail_type {
            openttdrs_core::RailType::Monorail => &self.monorail_tunnels[d],
            openttdrs_core::RailType::Maglev => &self.maglev_tunnels[d],
            openttdrs_core::RailType::Rail | openttdrs_core::RailType::Electric => {
                &self.rail_tunnels[d]
            }
        }
    }

    /// Capa frontal/techo de un túnel de carretera.
    #[must_use]
    pub(crate) fn tunnel_portal_front_sprite(&self, rail: bool, dir: u8) -> &AtlasSprite {
        let d = dir as usize & 3;
        if rail {
            &self.rail_tunnel_fronts[d]
        } else {
            &self.road_tunnel_fronts[d]
        }
    }

    /// Capa frontal/techo de un túnel de ferrocarril tipado.
    #[must_use]
    pub(crate) fn rail_tunnel_portal_front_sprite(
        &self,
        rail_type: openttdrs_core::RailType,
        dir: u8,
    ) -> &AtlasSprite {
        let d = dir as usize & 3;
        match rail_type {
            openttdrs_core::RailType::Monorail => &self.monorail_tunnel_fronts[d],
            openttdrs_core::RailType::Maglev => &self.maglev_tunnel_fronts[d],
            openttdrs_core::RailType::Rail | openttdrs_core::RailType::Electric => {
                &self.rail_tunnel_fronts[d]
            }
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
    use openttdrs_core::RailType;

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
        let assets = WorldAssets::load(&atlas, &mut images);
        // Torres terminadas (2083/2086/2089) deben tener ciclo oil_refinery.
        for id in [2083u32, 2086, 2089, 2120] {
            let frames = assets
                .refinery_fire_frames
                .get(&id)
                .unwrap_or_else(|| panic!("faltan frames de fuego para {id}"));
            assert_eq!(frames.len(), 7, "sprite {id}");
        }
        assert_eq!(
            assets.rail_tunnel_portal_sprite(RailType::Monorail, 2),
            &atlas.get("tunnel_mono_rear_sw.png")
        );
        assert_eq!(
            assets.rail_tunnel_portal_sprite(RailType::Maglev, 3),
            &atlas.get("tunnel_mglv_rear_nw.png")
        );
        assert_eq!(
            assets.rail_tunnel_portal_front_sprite(RailType::Monorail, 2),
            &atlas.get("tunnel_mono_front_sw.png")
        );
        // `station_land.h`: los StationGfx 27/28 parten de SPR_AIRPORT_APRON;
        // el jetway/túnel se añaden como capas TILE_SEQ en objects.rs.
        assert_eq!(
            assets.airport_station_gfx_sprite(27),
            &atlas.get("airport_apron.png")
        );
        assert_eq!(
            assets.airport_station_gfx_sprite(28),
            &atlas.get("airport_apron.png")
        );
        assert_ne!(
            assets.airport_station_gfx_sprite(28),
            &atlas.get("airport_concourse.png")
        );
        // La tabla vanilla no es el enum abreviado `AirportPiece`: los 74
        // StationGfx deben resolver su base exacta, incluidos helipads y
        // mitades Action5. Este test usa el atlas mínimo y evita que una
        // precarga incompleta vuelva al icono genérico.
        for gfx in 0..=73 {
            let base = crate::sprites::airport_station_base_for_gfx(gfx)
                .unwrap_or_else(|| panic!("falta base airport gfx={gfx}"));
            assert_eq!(
                assets.airport_station_gfx_sprite(gfx),
                assets
                    .airport_station_sprite(base.sprite_id)
                    .unwrap_or_else(|| panic!("falta sprite airport={}", base.sprite_id)),
                "gfx={gfx}"
            );
        }
        for sprite_id in [2095, 2601, 2633, 2650, 2662, 2668, 4982, 5966, 5967, 5968]
            .into_iter()
            .chain(2676..=2691)
        {
            assert!(
                assets.airport_station_sprite(sprite_id).is_some(),
                "falta sprite airport={sprite_id}"
            );
        }
        // Los depósitos usan tres bloques de sprites distintos en OpenTTD.
        // Así la precarga no puede volver a degradar mono/maglev al edificio
        // de vía normal aun si el renderer selecciona la capa correcta.
        assert_eq!(assets.rail_depot_builds[0][1].len(), 2); // rail SE
        assert_eq!(assets.rail_depot_builds[1][2].len(), 2); // mono SW
        assert_eq!(assets.rail_depot_builds[2][2].len(), 2); // maglev SW
        for id in crate::sprites::rail_pbs_sprite_ids_for_preload() {
            assert!(
                assets.has_exact_pbs_rail_sprite(id),
                "falta copia PALETTE_CRASH para rail_{id}"
            );
        }
        assert!(
            assets.house_palettes.covers_all_generated_pairs(),
            "falta una copia recoloreada de casa vanilla; verificar assets 8/32 bpp y PaletteID"
        );
    }
}
