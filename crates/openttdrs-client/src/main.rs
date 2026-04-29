//! Cliente isométrico: sprites de `OpenGFX` + gizmos de overlay para el [`GameState`] del core.
//!
//! Para cargar un mapa real de `OpenTTD`, exportar con `scripts/parse_sav.py` y
//! luego ejecutar el cliente con la variable de entorno:
//!
//! ```
//! OTTDMAP_FILE=/ruta/al/mapa.ottdmap cargo run -p openttdrs-client
//! ```

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

mod camera;
mod iso;
mod sprites;
mod state;
mod ui;

use std::collections::HashMap;
use std::path::Path;

use bevy::image::ImageSamplerDescriptor;
use bevy::math::{Affine3A, Rect};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{IndustryKind, Map, TileCoord, TileKind, Vehicle};

use camera::{CameraVelocity, move_camera};
use iso::{
    ISO_HW, ISO_QH, SLOPE_HALF_H, TILE_HALF_H, gizmo_diamond, iso, overlay_pos, shore_png_index,
    shore_tileh_for_draw_shore, tile_min_z, tile_pos, tile_pos_half, tile_slope_and_min_z,
    wang_hash,
};
use sprites::{
    HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, RAIL_SPRITE_IDS, ROAD_FLAT_HALF_H, collect_rail_sprites,
    house_draw_data_index_for_tile, rail_track_base_color, rail_trackbits_for_render,
    road_bits_for_render, road_flat_sprite_color, road_flat_sprite_index,
};
use state::SimWorld;
use ui::{SelectedTileInfo, handle_tile_click, setup_tile_info_ui, update_tile_info_text};

/// Factor de escala para los sprites de camiones (son 20×14 px nativo).
const TRUCK_SCALE: f32 = 2.0;

/// Sesgo en la componente Z de **solo** el agua animada (sin sprite `shore_*`).
/// El orden de dibujo usa `(tx+ty)`; el mar al **este/sur** tiene suma mayor y acaba
/// encima del borde costero del vecino NO/NE → sierra y rectángulos azules oscuros.
const FLAT_WATER_LAYER_FRAC: f32 = -0.014;

// ── Animación de agua ─────────────────────────────────────────────────────────

/// Marca los tiles de agua para la animación por ondas.
/// Almacena fases discretas por tile para emular el ciclado de paleta
/// (dark water 5 pasos + glitter 15 pasos).
#[derive(Component)]
struct WaterTile {
    dark_phase: u8,
    glitter_phase: u8,
}

// ── Dirección de vehículo ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum VehicleDir {
    #[default]
    Ne,
    Se,
    Sw,
    Nw,
}

fn vehicle_dir(v: &Vehicle) -> VehicleDir {
    let Some(next) = v.path.front() else {
        return VehicleDir::default();
    };
    let dx = next.x - v.pos.x;
    let dy = next.y - v.pos.y;
    match (dx.signum(), dy.signum()) {
        (1, _) => VehicleDir::Se,
        (-1, _) => VehicleDir::Nw,
        (_, 1) => VehicleDir::Sw,
        _ => VehicleDir::Ne,
    }
}

/// `true` si algún vecino ortogonal no es agua ni vacío (borde mar/tierra o río).
///
/// Los exports `.ottdmap` a veces dejan `m5=0` en toda el agua y se pierde
/// `WaterTileType::Coast` en bits 4–7; sin esto solo se pinta agua plana en la orilla.
fn water_tile_touches_land(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> bool {
    let is_land = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= mw as i32 || y >= mh as i32 {
            return false;
        }
        map.get(TileCoord::new(x, y))
            .is_some_and(|t| t.kind != TileKind::Water && t.kind != TileKind::Void)
    };
    let x = tx as i32;
    let y = ty as i32;
    is_land(x - 1, y) || is_land(x + 1, y) || is_land(x, y - 1) || is_land(x, y + 1)
}

// ── Recursos ──────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct TruckHandles {
    ne: Handle<Image>,
    se: Handle<Image>,
    sw: Handle<Image>,
    nw: Handle<Image>,
}

impl TruckHandles {
    fn for_dir(&self, dir: VehicleDir) -> Handle<Image> {
        match dir {
            VehicleDir::Ne => self.ne.clone(),
            VehicleDir::Se => self.se.clone(),
            VehicleDir::Sw => self.sw.clone(),
            VehicleDir::Nw => self.nw.clone(),
        }
    }
}

/// Índice `Vehicle.id` → posición en `GameState::vehicles` (evita `find` O(V) por sprite).
#[derive(Resource, Default)]
struct VehicleIndex {
    by_id: HashMap<u32, usize>,
}

impl VehicleIndex {
    fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.by_id.clear();
        self.by_id.reserve(vehicles.len());
        for (i, v) in vehicles.iter().enumerate() {
            self.by_id.insert(v.id, i);
        }
    }
}

fn rebuild_vehicle_index(sim: Res<SimWorld>, mut idx: ResMut<VehicleIndex>) {
    idx.rebuild(&sim.state.vehicles);
}

// ── Componentes ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct VehicleSprite(u32);

// ── App ───────────────────────────────────────────────────────────────────────

fn main() {
    let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
    if !check_required_assets(asset_root) {
        return;
    }

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "openttdrs".into(),
                        resolution: (1280_u32, 720_u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root.into(),
                    ..default()
                })
                // Nearest-neighbor: sprites pixel-art nítidos en todos los zoom levels.
                // Bevy usa bilinear por defecto, que desenfoca los sprites al hacer zoom.
                .set(ImagePlugin {
                    default_sampler: ImageSamplerDescriptor::nearest(),
                }),
        )
        .init_resource::<SimWorld>()
        .init_resource::<SelectedTileInfo>()
        .init_resource::<CameraVelocity>()
        .init_resource::<VehicleIndex>()
        .add_systems(
            Startup,
            (setup, rebuild_vehicle_index, setup_tile_info_ui).chain(),
        )
        .add_systems(
            Update,
            (
                advance_sim,
                sync_window_title,
                update_vehicles,
                animate_water,
                draw_industries,
                draw_stations,
                move_camera,
                handle_tile_click,
                update_tile_info_text,
            )
                .chain(),
        )
        .run();
}

fn check_required_assets(asset_root: &str) -> bool {
    let tiles_dir = Path::new(asset_root).join("opengfx/tiles");
    let required = [
        tiles_dir.join("grass.png"),
        tiles_dir.join("water.png"),
        tiles_dir.join("vehicle_bus_sw.png"),
    ];

    let missing: Vec<String> = required
        .iter()
        .filter(|p| !p.is_file())
        .map(|p| p.display().to_string())
        .collect();

    if missing.is_empty() {
        return true;
    }

    eprintln!(
        "No se encontraron assets OpenGFX requeridos. Faltan {} archivos.",
        missing.len()
    );
    for path in &missing {
        eprintln!("Archivo faltante: {path}");
    }
    eprintln!("Genera los assets con: ./scripts/descargar_graficos.sh");
    false
}

// ── Setup ─────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, sim: Res<SimWorld>) {
    let (mw, mh) = sim.state.map.dimensions();

    // Con iso(tx,ty) = (ty-tx)*ISO_HW, el centro del mapa está en iso(mw/2, mh/2):
    // screen_x = (mh/2 - mw/2) * ISO_HW
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;

    let target_tiles_wide: f32 = if sim.loaded_file { 64.0 } else { mw as f32 };
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);

    commands.spawn((
        Camera2d,
        Camera {
            // Tonos cercanos al mar oscuro: si hay huecos de 1px entre sprites, menos brillo que el cielo.
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        Transform::from_translation(Vec3::new(cam_x, cam_y, 999.9)),
        Projection::Orthographic(OrthographicProjection {
            scale: cam_scale,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // ── Handles de teselas de suelo ───────────────────────────────────────────
    let h_grass = asset_server.load::<Image>("opengfx/tiles/grass.png");
    let h_rough = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
    // Pendientes de grass y rough: índice 0 = tileh 1, índice 13 = tileh 14
    let grass_slopes: Vec<Handle<Image>> = (1u8..=14)
        .map(|tileh| {
            asset_server.load::<Image>(format!("opengfx/tiles/terrain_grass_slope_{tileh:02}.png"))
        })
        .collect();
    let rough_slopes: Vec<Handle<Image>> = (1u8..=14)
        .map(|tileh| {
            asset_server.load::<Image>(format!("opengfx/tiles/terrain_rough_slope_{tileh:02}.png"))
        })
        .collect();
    let h_water = asset_server.load::<Image>("opengfx/tiles/water.png");
    let shore_tex: Vec<Handle<Image>> = (0..8)
        .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/shore_{i}.png")))
        .collect();
    // Objetos estáticos del mapa (MP_OBJECT): faro (type 1) y transmisor (type 0)
    let h_lighthouse = asset_server.load::<Image>("opengfx/tiles/object_lighthouse.png");
    let h_transmitter = asset_server.load::<Image>("opengfx/tiles/object_transmitter.png");
    let road_flat: Vec<Handle<Image>> = (0..19)
        .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/road_flat_{i:02}.png")))
        .collect();
    let rail_tex: HashMap<u32, Handle<Image>> = RAIL_SPRITE_IDS
        .iter()
        .copied()
        .map(|id| {
            (
                id,
                asset_server.load::<Image>(format!("opengfx/tiles/rail_{id}.png")),
            )
        })
        .collect();

    // ── Handles de estaciones ──────────────────────────────────────────────────
    let station_grounds: Vec<Handle<Image>> = (0..4)
        .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/truck_stop_ground_{i}.png")))
        .collect();

    // ── Handles de casas urbanas ───────────────────────────────────────────────
    // house_building_tex: sprites por sprite_id (house_s{id}.png) para HouseIDs 0-127
    let house_building_tex: HashMap<u32, Handle<Image>> = {
        let mut map = HashMap::new();
        for spec in &HOUSE_DRAW_DATA {
            for &sid in &[spec.s1, spec.s2] {
                if sid != 0 {
                    let fname = sprites::house_sprite_filename(sid);
                    map.entry(sid).or_insert_with(|| {
                        asset_server.load::<Image>(format!("opengfx/tiles/{fname}"))
                    });
                }
            }
        }
        map
    };

    // ── Handles de overlays ────────────────────────────────────────────────────
    let h_tree_1 = asset_server.load::<Image>("opengfx/tiles/tree_00.png");
    let h_tree_2 = asset_server.load::<Image>("opengfx/tiles/tree_07.png");
    let h_tree_3 = asset_server.load::<Image>("opengfx/tiles/tree_14.png");
    let trees = [h_tree_1, h_tree_2, h_tree_3];

    // ── Handles de industrias ─────────────────────────────────────────────────
    // Carga dinámica: itera INDUSTRY_GFX_DATA y agrupa los sprite_ids únicos.
    let industry_tex: HashMap<u32, Handle<Image>> = {
        let mut map = HashMap::new();
        for entry in &INDUSTRY_GFX_DATA {
            if entry.sprite_id != 0 {
                map.entry(entry.sprite_id).or_insert_with(|| {
                    asset_server
                        .load::<Image>(format!("opengfx/tiles/industry_{}.png", entry.sprite_id))
                });
            }
        }
        map
    };

    // ── Handles de camiones ────────────────────────────────────────────────────
    let h_truck_ne = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    let h_truck_se = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    let h_truck_sw = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    let h_truck_nw = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    commands.insert_resource(TruckHandles {
        ne: h_truck_ne,
        se: h_truck_se,
        sw: h_truck_sw,
        nw: h_truck_nw,
    });

    let map = &sim.state.map;
    let grid_len = (mw * mh) as usize;
    let mut tileh_grid = vec![0u8; grid_len];
    let mut base_z_grid = vec![0u8; grid_len];
    let mut use_shore_grid = vec![false; grid_len];
    for ty in 0..mh {
        for tx in 0..mw {
            let idx = (ty * mw + tx) as usize;
            let (th, bz) = tile_slope_and_min_z(map, tx, ty);
            tileh_grid[idx] = th;
            base_z_grid[idx] = bz;
        }
    }
    for ty in 0..mh {
        for tx in 0..mw {
            let c = TileCoord::new(tx as i32, ty as i32);
            let tile = map.get(c);
            let kind = tile.map_or(TileKind::Grass, |t| t.kind);
            if kind != TileKind::Water {
                continue;
            }
            let idx = (ty * mw + tx) as usize;
            let m5_w = tile.map_or(0u8, |t| t.m5);
            let water_tile_type = (m5_w >> 4) & 0x0F;
            use_shore_grid[idx] = water_tile_type == 1
                || (water_tile_type == 0 && water_tile_touches_land(map, tx, ty, mw, mh));
        }
    }

    let mut batch_water: Vec<(WaterTile, Sprite, Transform)> = Vec::new();
    let mut batch_shore: Vec<(Sprite, Transform)> = Vec::new();
    let mut batch_trees: Vec<(Sprite, Transform)> = Vec::new();

    // ── Teselas de suelo ───────────────────────────────────────────────────────
    let mut rail_layers: Vec<u32> = Vec::with_capacity(8);
    for ty in 0..mh {
        for tx in 0..mw {
            let idx = (ty * mw + tx) as usize;
            let c = TileCoord::new(tx as i32, ty as i32);
            let tile = sim.state.map.get(c);
            let kind = tile.map_or(TileKind::Grass, |t| t.kind);
            let base_z = base_z_grid[idx];
            let tileh = tileh_grid[idx];
            let p = iso(tx as i32, ty as i32);

            if kind == TileKind::Void {
                continue;
            }

            let slope_half_ground = SLOPE_HALF_H[tileh as usize];

            if kind == TileKind::Road {
                let rb = road_bits_for_render(&sim.state.map, c, mw, mh);
                let fi = road_flat_sprite_index(tileh, rb);
                let road_half_h = if tileh == 0 {
                    ROAD_FLAT_HALF_H[fi]
                } else {
                    SLOPE_HALF_H[tileh as usize]
                };
                let road_paint =
                    tile.map_or(Color::WHITE, |t| road_flat_sprite_color(t.mapt, kind, t.m7));
                if tileh != 0 {
                    commands.spawn((
                        Sprite {
                            image: grass_slopes[tileh as usize - 1].clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(tile_pos_half(
                            tx as i32,
                            ty as i32,
                            base_z,
                            0.0,
                            slope_half_ground,
                        )),
                    ));
                }
                let pos_road = tile_pos_half(tx as i32, ty as i32, base_z, 0.02, road_half_h);
                commands.spawn((
                    Sprite {
                        image: road_flat[fi].clone(),
                        color: road_paint,
                        ..default()
                    },
                    Transform::from_translation(pos_road),
                ));
            } else if kind == TileKind::Rail {
                if tileh != 0 {
                    commands.spawn((
                        Sprite {
                            image: grass_slopes[tileh as usize - 1].clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(tile_pos_half(
                            tx as i32,
                            ty as i32,
                            base_z,
                            0.0,
                            slope_half_ground,
                        )),
                    ));
                }
                let rail_half_h = if tileh == 0 {
                    TILE_HALF_H
                } else {
                    SLOPE_HALF_H[tileh as usize]
                };
                collect_rail_sprites(
                    rail_trackbits_for_render(&sim.state.map, c, mw, mh),
                    &mut rail_layers,
                );
                let rail_paint = tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
                    rail_track_base_color(t.mapt, kind, t.m5, t.m3)
                });
                for (i, sid) in rail_layers.iter().copied().enumerate() {
                    let Some(img) = rail_tex.get(&sid) else {
                        continue;
                    };
                    let z = 0.02 + i as f32 * 0.0004;
                    commands.spawn((
                        Sprite {
                            image: img.clone(),
                            color: rail_paint,
                            ..default()
                        },
                        Transform::from_translation(tile_pos_half(
                            tx as i32,
                            ty as i32,
                            base_z,
                            z,
                            rail_half_h,
                        )),
                    ));
                }
            } else if kind == TileKind::House {
                // GetCleanHouseType: GB(m8, 0, 12) — el resto es datos NewGRF
                let clean_house_id = tile.map_or(0u16, |t| t.m8 & 0xFFF);
                let house_base = if tileh == 0 {
                    h_grass.clone()
                } else {
                    grass_slopes[tileh as usize - 1].clone()
                };
                commands.spawn((
                    Sprite {
                        image: house_base,
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(tile_pos_half(
                        tx as i32,
                        ty as i32,
                        base_z,
                        0.0,
                        slope_half_ground,
                    )),
                ));
                let spec_idx = house_draw_data_index_for_tile(clean_house_id, tx as i32, ty as i32);
                let spec = &HOUSE_DRAW_DATA[spec_idx];
                if spec.s1 != 0
                    && let Some(img) = house_building_tex.get(&spec.s1)
                {
                    let pos3 = overlay_pos(
                        p,
                        spec.s1_xrel,
                        spec.s1_yrel,
                        spec.s1_w,
                        spec.s1_h,
                        base_z,
                        0.4,
                        tx as i32,
                        ty as i32,
                    );
                    commands.spawn((
                        Sprite {
                            image: img.clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(pos3),
                    ));
                }
                if spec.s2 != 0
                    && let Some(img) = house_building_tex.get(&spec.s2)
                {
                    let pos3 = overlay_pos(
                        p,
                        spec.s2_xrel,
                        spec.s2_yrel,
                        spec.s2_w,
                        spec.s2_h,
                        base_z,
                        0.5,
                        tx as i32,
                        ty as i32,
                    );
                    commands.spawn((
                        Sprite {
                            image: img.clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(pos3),
                    ));
                }
            } else if kind == TileKind::Station {
                if tileh != 0 {
                    commands.spawn((
                        Sprite {
                            image: grass_slopes[tileh as usize - 1].clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(tile_pos_half(
                            tx as i32,
                            ty as i32,
                            base_z,
                            0.0,
                            slope_half_ground,
                        )),
                    ));
                }
                let dir = wang_hash(tx, ty, 0xCAFE) as usize % station_grounds.len();
                commands.spawn((
                    Sprite {
                        image: station_grounds[dir].clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, base_z, 0.01)),
                ));
            } else if kind == TileKind::Industry {
                // gfx de industria es de 9 bits: m5 (bits 0-7) | bit 2 de m6 (bit 8)
                // Fuente: GetCleanIndustryGfx() en industry_map.h de OpenTTD
                let gfx = tile.map_or(0u16, |t| {
                    u16::from(t.m5) | (u16::from((t.m6 >> 2) & 1) << 8)
                });
                let has_building = sprites::industry_sprite_for_gfx(gfx).is_some();
                let (ground_img, ground_color) = if has_building {
                    (
                        if tileh == 0 {
                            h_rough.clone()
                        } else {
                            rough_slopes[tileh as usize - 1].clone()
                        },
                        Color::srgb(0.55, 0.50, 0.45),
                    )
                } else {
                    (
                        if tileh == 0 {
                            h_grass.clone()
                        } else {
                            grass_slopes[tileh as usize - 1].clone()
                        },
                        Color::WHITE,
                    )
                };
                commands.spawn((
                    Sprite {
                        image: ground_img,
                        color: ground_color,
                        ..default()
                    },
                    Transform::from_translation(tile_pos_half(
                        tx as i32,
                        ty as i32,
                        base_z,
                        0.0,
                        slope_half_ground,
                    )),
                ));
                // Edificio de industria según gfx (m5)
                if let Some(s) = sprites::industry_sprite_for_gfx(gfx)
                    && let Some(img) = industry_tex.get(&s.sprite_id)
                {
                    let pos3 = overlay_pos(
                        p, s.xrel, s.yrel, s.w, s.h, base_z, 0.5, tx as i32, ty as i32,
                    );
                    commands.spawn((
                        Sprite {
                            image: img.clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(pos3),
                    ));
                }
            } else {
                if kind == TileKind::Water {
                    if use_shore_grid[idx] {
                        // `DrawShoreTile(tileh)` — igual que OpenTTD: pendiente real del 2×2
                        // cuando no es plana; si no, vecinos de tierra (`infer_coast`).
                        let th = shore_tileh_for_draw_shore(&sim.state.map, tx, ty, mw, mh);
                        if th == 0 {
                            let dark_phase = ((tx + 2 * ty).rem_euclid(5)) as u8;
                            let glitter_phase = (wang_hash(tx, ty, 0xA9FE) % 15) as u8;
                            batch_water.push((
                                WaterTile {
                                    dark_phase,
                                    glitter_phase,
                                },
                                Sprite {
                                    image: h_water.clone(),
                                    color: Color::WHITE,
                                    ..default()
                                },
                                Transform::from_translation(tile_pos(
                                    tx as i32,
                                    ty as i32,
                                    base_z,
                                    FLAT_WATER_LAYER_FRAC,
                                )),
                            ));
                        } else {
                            let si = shore_png_index(th);
                            let hh = SLOPE_HALF_H[th as usize];
                            batch_shore.push((
                                Sprite {
                                    image: shore_tex[si].clone(),
                                    color: Color::WHITE,
                                    ..default()
                                },
                                Transform::from_translation(tile_pos_half(
                                    tx as i32, ty as i32, base_z, 0.0, hh,
                                )),
                            ));
                        }
                    } else {
                        // Agua libre (Clear, Lock, Depot en mapas típicos: Clear).
                        let dark_phase = ((tx + 2 * ty).rem_euclid(5)) as u8;
                        let glitter_phase = (wang_hash(tx, ty, 0xA9FE) % 15) as u8;
                        batch_water.push((
                            WaterTile {
                                dark_phase,
                                glitter_phase,
                            },
                            Sprite {
                                image: h_water.clone(),
                                color: Color::WHITE,
                                ..default()
                            },
                            Transform::from_translation(tile_pos(
                                tx as i32,
                                ty as i32,
                                base_z,
                                FLAT_WATER_LAYER_FRAC,
                            )),
                        ));
                    }
                } else {
                    let ottd_type = tile.map_or(0u8, |t| (t.mapt >> 4) & 0xF);
                    let tile_m5 = tile.map_or(0u8, |t| t.m5);
                    let _tile_m6 = tile.map_or(0u8, |t| t.m6);
                    let _tile_m8 = tile.map_or(0u16, |t| t.m8);

                    // MP_CLEAR (0): distinguir subtipo de suelo via m5 bits 2-4
                    // MP_OBJECT (10): grass de base + overlay de objeto
                    let slope_half_h = slope_half_ground;

                    // Helpers para elegir sprite plano o con pendiente
                    let grass_img = || {
                        if tileh == 0 {
                            h_grass.clone()
                        } else {
                            grass_slopes[tileh as usize - 1].clone()
                        }
                    };
                    let rough_img = || {
                        if tileh == 0 {
                            h_rough.clone()
                        } else {
                            rough_slopes[tileh as usize - 1].clone()
                        }
                    };

                    let (image, color) = match kind {
                        TileKind::Grass if ottd_type == 0 => {
                            // bits 2-4 de m5 = ClearGround
                            // 0=grass, 1=rough, 2=rocky, 3=fields, 4=snow, 5=desert
                            match (tile_m5 >> 2) & 0x7 {
                                0 => (grass_img(), Color::WHITE),
                                3 => (rough_img(), Color::srgb(0.82, 0.72, 0.45)), // campos
                                _ => (rough_img(), Color::srgb(0.78, 0.73, 0.58)), // rough/rocky
                            }
                        }
                        TileKind::Grass => (grass_img(), Color::WHITE), // MP_OBJECT u otros
                        TileKind::Forest => (rough_img(), Color::srgb(0.6, 1.0, 0.45)),
                        TileKind::CoalField => (rough_img(), Color::srgb(0.55, 0.50, 0.45)),
                        TileKind::Unknown(_) => (grass_img(), Color::srgb(1.0, 0.0, 1.0)),
                        TileKind::House
                        | TileKind::Station
                        | TileKind::Road
                        | TileKind::Rail
                        | TileKind::Industry
                        | TileKind::Water
                        | TileKind::Void => unreachable!(),
                    };
                    commands.spawn((
                        Sprite {
                            image,
                            color,
                            ..default()
                        },
                        Transform::from_translation(tile_pos_half(
                            tx as i32,
                            ty as i32,
                            base_z,
                            0.0,
                            slope_half_h,
                        )),
                    ));

                    // MP_OBJECT: renderizar faro o transmisor como overlay
                    // ObjectType (m5): 0=Transmisor, 1=Faro
                    if ottd_type == 10 {
                        // ObjectType de OpenTTD: 0=Transmisor, 1=Faro
                        // m5 contiene el ObjectType real (resuelto por parse_sav.py desde OBJS)
                        let (obj_img, obj_xrel, obj_yrel, obj_w, obj_h) = match tile_m5 {
                            // OBJECT_TRANSMITTER=0: sprite 2601, 55×77, xrel=-26, yrel=-71
                            0 => (Some(h_transmitter.clone()), -26.0, -71.0, 55.0, 77.0),
                            // OBJECT_LIGHTHOUSE=1: sprite 2602, 41×61, xrel=-22, yrel=-48
                            1 => (Some(h_lighthouse.clone()), -22.0, -48.0, 41.0, 61.0),
                            _ => (None, 0.0, 0.0, 0.0, 0.0),
                        };
                        if let Some(img) = obj_img {
                            let pos3 = overlay_pos(
                                p, obj_xrel, obj_yrel, obj_w, obj_h, base_z, 0.6, tx as i32,
                                ty as i32,
                            );
                            commands.spawn((
                                Sprite {
                                    image: img,
                                    color: Color::WHITE,
                                    ..default()
                                },
                                Transform::from_translation(pos3),
                            ));
                        }
                    }
                }
            }

            if kind == TileKind::Forest {
                let h = wang_hash(tx, ty, 0xCAFE);
                let tree_idx = (h % 3) as usize;
                let ox = ((h >> 2) % 17) as f32 - 8.0;
                let pos3 = overlay_pos(
                    Vec2::new(p.x + ox, p.y),
                    -19.0,
                    -36.0,
                    35.0,
                    43.0,
                    base_z,
                    0.3,
                    tx as i32,
                    ty as i32,
                );
                batch_trees.push((
                    Sprite {
                        image: trees[tree_idx].clone(),
                        ..default()
                    },
                    Transform::from_translation(pos3),
                ));
            }
        }
    }

    commands.spawn_batch(batch_water);
    commands.spawn_batch(batch_shore);
    commands.spawn_batch(batch_trees);

    // ── Sprites de vehículos ───────────────────────────────────────────────────
    let h_truck_ne_init = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    for vehicle in &sim.state.vehicles {
        let vh = tile_min_z(&sim.state.map, vehicle.pos);
        let p = iso(vehicle.pos.x, vehicle.pos.y);
        let pos3 = overlay_pos(
            p,
            -14.0,
            -5.0,
            20.0,
            14.0,
            vh,
            1.0,
            vehicle.pos.x,
            vehicle.pos.y,
        );
        commands.spawn((
            VehicleSprite(vehicle.id),
            Sprite {
                image: h_truck_ne_init.clone(),
                ..default()
            },
            Transform::from_translation(pos3).with_scale(Vec3::splat(TRUCK_SCALE)),
        ));
    }
}

// ── Sistemas de actualización ─────────────────────────────────────────────────

fn advance_sim(
    time: Res<Time>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut acc: Local<f32>,
) {
    const TICK_HZ: f32 = 15.0;
    *acc += time.delta_secs();
    let period = 1.0 / TICK_HZ;
    let mut stepped = false;
    while *acc >= period {
        *acc -= period;
        sim.state.step();
        stepped = true;
    }
    if stepped {
        vehicle_index.rebuild(&sim.state.vehicles);
    }
}

/// Estado del título: FPS se refresca ~1 vez/s; el zoom se refleja al instante al mover la rueda.
#[derive(Default)]
struct WindowTitleSync {
    last_scale: f32,
    fps_dt: f32,
    fps_frames: u32,
    last_fps: f32,
}

fn sync_window_title(
    sim: Res<SimWorld>,
    time: Res<Time>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    cam_q: Query<&Projection, With<Camera2d>>,
    mut state: Local<WindowTitleSync>,
) {
    let scale = cam_q
        .single()
        .ok()
        .and_then(|p| match p {
            Projection::Orthographic(o) => Some(o.scale),
            _ => None,
        })
        .unwrap_or(1.0);

    state.fps_dt += time.delta_secs();
    state.fps_frames += 1;

    let scale_changed = (scale - state.last_scale).abs() > 0.000_5;
    if scale_changed {
        state.last_scale = scale;
    }

    let fps_tick = state.fps_dt >= 1.0;
    if fps_tick {
        state.last_fps = state.fps_frames as f32 / state.fps_dt;
        state.fps_dt = 0.0;
        state.fps_frames = 0;
    }

    if !scale_changed && !fps_tick {
        return;
    }

    let fps = if state.last_fps > 0.0 {
        state.last_fps
    } else {
        60.0
    };

    if let Ok(mut window) = windows.single_mut() {
        window.title = format!(
            "openttdrs — tick {} — zoom {:.2}× — {:.0} FPS",
            sim.state.tick.get(),
            scale,
            fps
        );
    }
}

fn update_vehicles(
    sim: Res<SimWorld>,
    trucks: Res<TruckHandles>,
    vehicle_index: Res<VehicleIndex>,
    mut q: Query<(&VehicleSprite, &mut Transform, &mut Sprite)>,
) {
    for (vs, mut transform, mut sprite) in &mut q {
        let Some(&i) = vehicle_index.by_id.get(&vs.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        let dir = vehicle_dir(v);
        let vh = tile_min_z(&sim.state.map, v.pos);
        let p = iso(v.pos.x, v.pos.y);

        let (xrel, yrel, w, h) = match dir {
            VehicleDir::Ne => (-14.0, -5.0, 20.0, 14.0),
            VehicleDir::Se => (-6.0, -6.0, 20.0, 15.0),
            VehicleDir::Sw => (-14.0, -6.0, 20.0, 15.0),
            VehicleDir::Nw => (-6.0, -5.0, 20.0, 14.0),
        };
        let pos3 = overlay_pos(p, xrel, yrel, w, h, vh, 1.0, v.pos.x, v.pos.y);
        transform.translation = pos3;
        sprite.image = trucks.for_dir(dir);
    }
}

fn draw_industries(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    if std::env::var("OPENTTDRS_GIZMOS").ok().as_deref() != Some("1") {
        return;
    }
    for industry in &sim.state.industries {
        let center = iso(industry.pos.x, industry.pos.y);
        let color = match industry.kind {
            IndustryKind::CoalMine => Color::srgb(1.0, 0.9, 0.1),
            IndustryKind::Forest => Color::srgb(1.0, 0.5, 0.05),
        };
        gizmo_diamond(&mut gizmos, center, 30.0, 14.0, color);

        if industry.stock > 0 {
            let fill = industry.stock as f32 / industry.capacity as f32;
            let bar_w = 56.0 * fill;
            let bar_y = center.y - 12.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::WHITE,
            );
        }
    }
}

/// Anima agua con ciclos discretos para aproximar la paleta animada de OpenTTD.
///
/// En OpenTTD clásico el agua se mueve ciclando índices de paleta:
/// - dark water: ciclo de 5 entradas
/// - glitter water: ciclo de 15 colores, muestreado de 3 en 3
///
/// Este cliente usa sprites RGBA (no indexados), por eso emulamos ese efecto
/// modulando brillo/tinte en pasos discretos sincronizados.
fn animate_water(
    time: Res<Time>,
    cam_q: Query<(&GlobalTransform, &Projection), With<Camera2d>>,
    mut query: Query<(&WaterTile, &GlobalTransform, &mut Sprite)>,
) {
    const DARK_CYCLE: [f32; 5] = [0.92, 0.95, 0.98, 1.01, 1.04];
    const GLITTER_CYCLE: [f32; 15] = [
        0.00, 0.02, 0.05, 0.01, 0.03, 0.07, 0.02, 0.00, 0.04, 0.08, 0.03, 0.01, 0.06, 0.02, 0.00,
    ];
    // Paso base del ciclo dark (5 estados).
    let dark_tick = ((time.elapsed_secs() * 3.0) as usize) % DARK_CYCLE.len();
    // Glitter en 15 estados, avanzando de 3 en 3 (como DoPaletteAnimations).
    let glitter_tick = (((time.elapsed_secs() * 3.0) as usize) * 3) % GLITTER_CYCLE.len();

    let cull: Option<(Affine3A, Rect)> = cam_q.iter().next().and_then(|(cam_gt, proj)| {
        let Projection::Orthographic(ortho) = proj else {
            return None;
        };
        Some((cam_gt.affine().inverse(), ortho.area))
    });
    let margin = ISO_HW * 4.0;

    for (water, wg, mut sprite) in &mut query {
        if let Some((world_to_view, area)) = cull.as_ref() {
            let wpos = wg.translation();
            let local = world_to_view.transform_point3(wpos);
            if local.x < area.min.x - margin
                || local.x > area.max.x + margin
                || local.y < area.min.y - margin
                || local.y > area.max.y + margin
            {
                continue;
            }
        }
        let dark_idx = (dark_tick + water.dark_phase as usize) % DARK_CYCLE.len();
        let glitter_idx = (glitter_tick + water.glitter_phase as usize) % GLITTER_CYCLE.len();
        let dark = DARK_CYCLE[dark_idx];
        let glitter = GLITTER_CYCLE[glitter_idx];

        // Mantener amplitud baja para respetar el color original del sprite.
        let v = (dark + glitter * 0.40).clamp(0.88, 1.08);
        // Leve sesgo a azul para recordar el tono del agua original.
        sprite.color = Color::srgb(v * 0.95, v * 0.99, v * 1.03);
    }
}

fn draw_stations(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    if std::env::var("OPENTTDRS_GIZMOS").ok().as_deref() != Some("1") {
        return;
    }
    for station in &sim.state.stations {
        let center = iso(station.pos.x, station.pos.y);
        gizmo_diamond(&mut gizmos, center, 26.0, 12.0, Color::srgb(0.0, 0.9, 0.9));

        if station.income > 0 {
            let fill = ((station.income as f32).log2() / 10.0).min(1.0);
            let bar_w = 48.0 * fill;
            let bar_y = center.y - 10.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
}
