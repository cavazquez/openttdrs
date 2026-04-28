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

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{IndustryKind, TileCoord, TileKind, Vehicle};

use camera::move_camera;
use iso::{
    ISO_HW, ISO_QH, TILE_HALF_H, gizmo_diamond, iso, overlay_pos, tile_pos, tile_pos_half,
    wang_hash,
};
use sprites::{
    HOUSE_META, INDUSTRY_GFX_DATA, RAIL_SPRITE_IDS, ROAD_FLAT_HALF_H, collect_rail_sprites,
    rail_trackbits_for_render, road_bits_for_render, road_flat_index,
};
use state::SimWorld;
use ui::{SelectedTileInfo, handle_tile_click, setup_tile_info_ui, update_tile_info_text};

/// Factor de escala para los sprites de camiones (son 20×14 px nativo).
const TRUCK_SCALE: f32 = 2.0;

// ── Animación de agua ─────────────────────────────────────────────────────────

/// Marca los tiles de agua para la animación por ondas.
/// Almacena la fase aleatoria por tile para que cada tile se mueva
/// de forma independiente, simulando olas.
#[derive(Component)]
struct WaterTile {
    phase: f32,
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

// ── Componentes ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct VehicleSprite(u32);

// ── App ───────────────────────────────────────────────────────────────────────

fn main() {
    let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

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
                }),
        )
        .init_resource::<SimWorld>()
        .init_resource::<SelectedTileInfo>()
        .add_systems(Startup, (setup, setup_tile_info_ui))
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
            clear_color: ClearColorConfig::Custom(Color::srgb(0.45, 0.60, 0.75)),
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
    let h_water = asset_server.load::<Image>("opengfx/tiles/water.png");
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
    let house_tex: Vec<Handle<Image>> = (0..8)
        .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/house_{i}.png")))
        .collect();

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

    // ── Teselas de suelo ───────────────────────────────────────────────────────
    let mut rail_layers: Vec<u32> = Vec::with_capacity(8);
    for ty in 0..mh {
        for tx in 0..mw {
            let c = TileCoord::new(tx as i32, ty as i32);
            let tile = sim.state.map.get(c);
            let kind = tile.map_or(TileKind::Grass, |t| t.kind);
            let height = tile.map_or(0, |t| t.height);
            let p = iso(tx as i32, ty as i32);

            if kind == TileKind::Void {
                continue;
            }

            if kind == TileKind::Road {
                let fi = road_flat_index(road_bits_for_render(&sim.state.map, c, mw, mh));
                let pos_road =
                    tile_pos_half(tx as i32, ty as i32, height, 0.0, ROAD_FLAT_HALF_H[fi]);
                commands.spawn((
                    Sprite {
                        image: road_flat[fi].clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(pos_road),
                ));
            } else if kind == TileKind::Rail {
                collect_rail_sprites(
                    rail_trackbits_for_render(&sim.state.map, c, mw, mh),
                    &mut rail_layers,
                );
                for (i, sid) in rail_layers.iter().copied().enumerate() {
                    let Some(img) = rail_tex.get(&sid) else {
                        continue;
                    };
                    let z = i as f32 * 0.0004;
                    commands.spawn((
                        Sprite {
                            image: img.clone(),
                            color: Color::srgb(0.88, 0.88, 0.97),
                            ..default()
                        },
                        Transform::from_translation(tile_pos(tx as i32, ty as i32, height, z)),
                    ));
                }
            } else if kind == TileKind::House {
                commands.spawn((
                    Sprite {
                        image: h_grass.clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                ));
                let hi = wang_hash(tx, ty, 0xBEEF) as usize % house_tex.len();
                let (w, h, xr, yr) = HOUSE_META[hi];
                let pos3 = overlay_pos(p, xr, yr, w, h, height, 0.5, tx as i32, ty as i32);
                commands.spawn((
                    Sprite {
                        image: house_tex[hi].clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(pos3),
                ));
            } else if kind == TileKind::Station {
                let dir = wang_hash(tx, ty, 0xCAFE) as usize % station_grounds.len();
                commands.spawn((
                    Sprite {
                        image: station_grounds[dir].clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                ));
            } else if kind == TileKind::Industry {
                // Tile base: con edificio→suelo marrón; sin edificio (campo/suelo)→grass
                // para evitar el cuadrado negro en campos de Farm y tiles de suelo.
                let gfx = tile.map_or(0, |t| t.m5);
                let has_building = sprites::industry_sprite_for_gfx(gfx).is_some();
                let (ground_img, ground_color) = if has_building {
                    (h_rough.clone(), Color::srgb(0.55, 0.50, 0.45))
                } else {
                    (h_grass.clone(), Color::WHITE)
                };
                commands.spawn((
                    Sprite {
                        image: ground_img,
                        color: ground_color,
                        ..default()
                    },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                ));
                // Edificio de industria según gfx (m5)
                if let Some(s) = sprites::industry_sprite_for_gfx(gfx)
                    && let Some(img) = industry_tex.get(&s.sprite_id)
                {
                    let pos3 = overlay_pos(
                        p, s.xrel, s.yrel, s.w, s.h, height, 0.5, tx as i32, ty as i32,
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
                    // Fase aleatoria por tile para desfasar las olas entre vecinos
                    let phase = wang_hash(tx, ty, 0xA9FE) as f32
                        * (std::f32::consts::TAU / u32::MAX as f32);
                    commands.spawn((
                        WaterTile { phase },
                        Sprite {
                            image: h_water.clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                    ));
                } else {
                    let ottd_type = tile.map_or(0u8, |t| (t.mapt >> 4) & 0xF);
                    let tile_m5 = tile.map_or(0u8, |t| t.m5);

                    // MP_CLEAR (0): distinguir subtipo de suelo via m5 bits 2-4
                    // MP_OBJECT (10): grass de base + overlay de objeto
                    let (image, color) = match kind {
                        TileKind::Grass if ottd_type == 0 => {
                            // bits 2-4 de m5 = ClearGround
                            // 0=grass, 1=rough, 2=rocky, 3=fields, 4=snow, 5=desert
                            match (tile_m5 >> 2) & 0x7 {
                                0 => (h_grass.clone(), Color::WHITE), // grass verde
                                3 => (h_rough.clone(), Color::srgb(0.82, 0.72, 0.45)), // campos arados
                                _ => (h_rough.clone(), Color::srgb(0.78, 0.73, 0.58)), // rough/rocky
                            }
                        }
                        TileKind::Grass => (h_grass.clone(), Color::WHITE), // MP_OBJECT u otros
                        TileKind::Forest => (h_rough.clone(), Color::srgb(0.6, 1.0, 0.45)),
                        TileKind::CoalField => (h_rough.clone(), Color::srgb(0.55, 0.50, 0.45)),
                        TileKind::Unknown(_) => (h_grass.clone(), Color::srgb(1.0, 0.0, 1.0)),
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
                        Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
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
                                p, obj_xrel, obj_yrel, obj_w, obj_h, height, 0.6, tx as i32,
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
                    height,
                    0.3,
                    tx as i32,
                    ty as i32,
                );
                commands.spawn((
                    Sprite {
                        image: trees[tree_idx].clone(),
                        ..default()
                    },
                    Transform::from_translation(pos3),
                ));
            }
        }
    }

    // ── Sprites de vehículos ───────────────────────────────────────────────────
    let h_truck_ne_init = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    for vehicle in &sim.state.vehicles {
        let vh = sim.state.map.get(vehicle.pos).map_or(0, |t| t.height);
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

fn advance_sim(time: Res<Time>, mut sim: ResMut<SimWorld>, mut acc: Local<f32>) {
    const TICK_HZ: f32 = 15.0;
    *acc += time.delta_secs();
    let period = 1.0 / TICK_HZ;
    while *acc >= period {
        *acc -= period;
        sim.state.step();
    }
}

fn sync_window_title(sim: Res<SimWorld>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.title = format!("openttdrs — tick {}", sim.state.tick.get());
    }
}

fn update_vehicles(
    sim: Res<SimWorld>,
    trucks: Res<TruckHandles>,
    mut q: Query<(&VehicleSprite, &mut Transform, &mut Sprite)>,
) {
    for (vs, mut transform, mut sprite) in &mut q {
        let Some(v) = sim.state.vehicles.iter().find(|v| v.id == vs.0) else {
            continue;
        };
        let dir = vehicle_dir(v);
        let vh = sim.state.map.get(v.pos).map_or(0, |t| t.height);
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

/// Anima los tiles de agua con una onda senoidal desfasada por tile.
///
/// El sprite de agua (OpenGFX 4061) ya tiene el color azul/textura de olas
/// baked-in. Aquí solo modulamos el brillo levemente para simular el reflejo
/// de luz en movimiento, sin teñir el color original del sprite.
fn animate_water(time: Res<Time>, mut query: Query<(&WaterTile, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (water, mut sprite) in &mut query {
        // Onda lenta: reflejo suave de luz en la superficie
        let wave = (t * 1.4 + water.phase).sin();
        // Onda rápida superpuesta: efecto de pequeñas olas/rizado
        let ripple = (t * 2.7 + water.phase * 1.3).sin() * 0.3;
        // Amplitud pequeña (±5%) para no distorsionar el color original del sprite
        let v = 0.95 + (wave + ripple) * 0.025;
        sprite.color = Color::srgb(v, v, v);
    }
}

fn draw_stations(sim: Res<SimWorld>, mut gizmos: Gizmos) {
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
