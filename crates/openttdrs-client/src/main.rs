//! Cliente isométrico: sprites de `OpenGFX` + gizmos de overlay para el [`GameState`] del core.
//!
//! Para cargar un mapa real de `OpenTTD`, exportar con `scripts/parse_sav.py` y
//! luego ejecutar el cliente con la variable de entorno:
//!
//! ```
//! OTTDMAP_FILE=/ruta/al/mapa.ottdmap cargo run -p openttdrs-client
//! ```
//!
//! Persistencia JSON (`openttdrs_core::save`, versión + `state` o legado plano):
//! `OTTDJSON_LOAD=/ruta/estado.json` al arranque, o **F5** / **Ctrl+S** para guardar y **F9** / **Ctrl+L** para
//! cargar (archivo por defecto `openttdrs_sim.json`, o `OPENTTDRS_JSON_SAVE`). Tras cargar se
//! redibuja todo el mapa y se reajusta la cámara (también si cambia el tamaño del mapa en el JSON).
//! **P** pausa el tick de simulación; **F4** alterna la ruta de guardado entre `openttdrs_sim.json` y
//! `openttdrs_autosave.json` (visible en el HUD). **Clic en el mapa** selecciona tesela; **panel Construir**
//! (esquina inferior izquierda) aplica carretera / estación en esa tesela.
//! Bases de sprites de señal: `OPENTTDRS_SIGNAL_BASE` / `OPENTTDRS_SIGNAL_ALT_BASE` (512–4096).

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

mod camera;
mod iso;
mod render;
mod sprites;
mod state;
mod ui;

use std::collections::HashMap;
use std::path::Path;

use bevy::image::ImageSamplerDescriptor;
use bevy::math::{Affine3A, Rect};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::save;
use openttdrs_core::{IndustryKind, TileKind, Vehicle};

use camera::{CameraVelocity, move_camera};
use iso::{ISO_HW, ISO_QH, SLOPE_HALF_H, TILE_HALF_H, gizmo_diamond, iso, overlay_pos, tile_min_z};
use render::{
    MapSpriteBatches, MapVisualLayer, RenderGrid, TileRenderContext, WaterTile, WorldAssets,
    flush_map_batches, push_forest_tree, push_water_tile, spawn_generic_land_tile,
    spawn_house_tile, spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
};
use state::SimWorld;
use ui::{
    SelectedTileInfo, SimHudControls, build_menu_interaction, cycle_json_save_path_hotkey,
    handle_pause_toggle, handle_tile_click, setup_build_menu, setup_tile_info_ui,
    update_tile_info_text,
};

/// Factor de escala para los sprites de camiones (son 20×14 px nativo).
const TRUCK_SCALE: f32 = 2.0;

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
    fn load(asset_server: &AssetServer) -> Self {
        let bus = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
        Self {
            ne: bus.clone(),
            se: bus.clone(),
            sw: bus.clone(),
            nw: bus,
        }
    }

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

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn spawn_initial_vehicles(commands: &mut Commands, sim: &SimWorld, trucks: &TruckHandles) {
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
            MapVisualLayer,
            VehicleSprite(vehicle.id),
            Sprite {
                image: trucks.for_dir(VehicleDir::default()),
                ..default()
            },
            Transform::from_translation(pos3).with_scale(Vec3::splat(TRUCK_SCALE)),
        ));
    }
}

// ── Componentes ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct VehicleSprite(u32);

/// Petición de redibujo del mapa. `sync_camera`: solo tras F9 / cambio de tamaño (no al editar teselas con clic).
#[derive(Resource, Default)]
pub(crate) struct RemapMapVisualsPending {
    pub(crate) pending: bool,
    pub(crate) sync_camera: bool,
}

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
        .init_resource::<RemapMapVisualsPending>()
        .init_resource::<SimHudControls>()
        .add_systems(
            Startup,
            (
                setup,
                rebuild_vehicle_index,
                setup_tile_info_ui,
                setup_build_menu,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                handle_pause_toggle,
                cycle_json_save_path_hotkey,
                advance_sim,
                handle_sim_json_hotkeys,
                apply_remap_map_visuals,
                sync_window_title,
                update_vehicles,
                animate_water,
                draw_industries,
                draw_stations,
                move_camera,
                build_menu_interaction,
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

    spawn_world_layer(&mut commands, &asset_server, &sim);
}

#[allow(clippy::too_many_lines)]
fn spawn_world_layer(commands: &mut Commands, asset_server: &AssetServer, sim: &SimWorld) {
    let (mw, mh) = sim.state.map.dimensions();
    let debug_coast = env_flag("OPENTTDRS_DEBUG_COAST");

    let assets = WorldAssets::load(asset_server);

    let truck_handles = TruckHandles::load(asset_server);

    let map = &sim.state.map;
    let render_grid = RenderGrid::from_map(map, mw, mh);

    let mut batches = MapSpriteBatches::default();

    // ── Teselas de suelo ───────────────────────────────────────────────────────
    let mut rail_layers: Vec<u32> = Vec::with_capacity(8);
    for ty in 0..mh {
        for tx in 0..mw {
            let ctx = TileRenderContext::new(map, &render_grid, tx, ty);
            let kind = ctx.kind;
            let tileh = ctx.info.tileh;

            if kind == TileKind::Void {
                continue;
            }

            let slope_half_ground = SLOPE_HALF_H[tileh as usize];

            match kind {
                TileKind::Road => {
                    spawn_road_tile(commands, map, mw, mh, &assets, &ctx, slope_half_ground);
                }
                TileKind::Rail => {
                    spawn_rail_tile(
                        commands,
                        map,
                        (mw, mh),
                        &assets,
                        &ctx,
                        slope_half_ground,
                        &mut rail_layers,
                    );
                }
                TileKind::House => {
                    spawn_house_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Station => {
                    spawn_station_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Industry => {
                    spawn_industry_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Water => {
                    push_water_tile(
                        commands,
                        map,
                        (mw, mh),
                        &assets,
                        &ctx,
                        debug_coast,
                        &mut batches,
                    );
                }
                TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::Unknown(_) => {
                    spawn_generic_land_tile(commands, &assets, &ctx, slope_half_ground);
                }
                TileKind::Void => unreachable!(),
            }

            if kind == TileKind::Forest {
                push_forest_tree(&assets, &ctx, &mut batches);
            }
        }
    }

    flush_map_batches(commands, batches);

    spawn_initial_vehicles(commands, sim, &truck_handles);
    commands.insert_resource(truck_handles);
}

// ── Sistemas de actualización ─────────────────────────────────────────────────

fn sync_camera_for_sim(
    q_cam: &mut Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    sim: &SimWorld,
) {
    let (mw, mh) = sim.state.map.dimensions();
    let cam_x = ((mh as i32 - 1) - (mw as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;
    let target_tiles_wide: f32 = if sim.loaded_file { 64.0 } else { mw as f32 };
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);
    let Ok((mut tf, mut proj)) = q_cam.single_mut() else {
        return;
    };
    tf.translation = Vec3::new(cam_x, cam_y, 999.9);
    let Projection::Orthographic(ref mut o) = *proj else {
        return;
    };
    o.scale = cam_scale;
}

fn apply_remap_map_visuals(
    mut commands: Commands,
    mut pending: ResMut<RemapMapVisualsPending>,
    q_vis: Query<Entity, With<MapVisualLayer>>,
    mut q_cam: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
) {
    if !pending.pending {
        return;
    }
    let do_sync_camera = pending.sync_camera;
    pending.pending = false;
    pending.sync_camera = false;
    let to_remove: Vec<Entity> = q_vis.iter().collect();
    for e in to_remove {
        commands.entity(e).despawn();
    }
    spawn_world_layer(&mut commands, &asset_server, &sim);
    if do_sync_camera {
        sync_camera_for_sim(&mut q_cam, &sim);
    }
}

fn handle_sim_json_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    hud: Res<SimHudControls>,
) {
    let save_path = hud.json_save_path.clone();
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let save_shortcut =
        keyboard.just_pressed(KeyCode::F5) || (ctrl && keyboard.just_pressed(KeyCode::KeyS));
    let load_shortcut =
        keyboard.just_pressed(KeyCode::F9) || (ctrl && keyboard.just_pressed(KeyCode::KeyL));

    if save_shortcut {
        match save::save(&sim.state, std::path::Path::new(&save_path)) {
            Ok(()) => info!("Guardado en {save_path}"),
            Err(e) => error!("No se pudo guardar en {save_path}: {e}"),
        }
    }
    if load_shortcut {
        match std::fs::read_to_string(&save_path) {
            Ok(text) => match save::load_from_str(&text) {
                Ok(loaded) => {
                    let prev = sim.state.map.dimensions();
                    let nw = loaded.map.dimensions();
                    sim.state = loaded;
                    sim.ottdmap_extras = None;
                    sim.loaded_file = true;
                    vehicle_index.rebuild(&sim.state.vehicles);
                    remap.pending = true;
                    remap.sync_camera = true;
                    if prev != nw {
                        info!("Mapa {prev:?} → {nw:?}; recarga visual y cámara.");
                    } else {
                        info!("Estado cargado desde {save_path}; recarga visual.");
                    }
                }
                Err(e) => error!("Carga: JSON inválido ({save_path}): {e}"),
            },
            Err(e) => error!("Carga: no se pudo leer {save_path}: {e}"),
        }
    }
}

fn advance_sim(
    time: Res<Time>,
    hud: Res<SimHudControls>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut acc: Local<f32>,
) {
    if hud.paused {
        return;
    }
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
        let indp_n = sim
            .ottdmap_extras
            .as_ref()
            .map(|e| e.industry_types.len())
            .unwrap_or(0);
        let indp_tag = if indp_n > 0 {
            format!(" — INDP {indp_n}")
        } else {
            String::new()
        };
        window.title = format!(
            "openttdrs — tick {} — cargas {}/{}{indp_tag} — zoom {:.2}× — {:.0} FPS",
            sim.state.tick.get(),
            sim.state.stats.cargo_pickups,
            sim.state.stats.cargo_deliveries,
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
            IndustryKind::OilWell => Color::srgb(0.35, 0.55, 1.0),
            IndustryKind::Factory => Color::srgb(0.85, 0.45, 0.95),
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
