//! Cliente isométrico: sprites de OpenGFX + gizmos de overlay para el [`GameState`] del core.

#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{
    find_path, GameState, Industry, IndustryKind, Map, Station, TileCoord, TileKind, Vehicle,
    VehicleKind,
};

// ── Constantes isométricas ────────────────────────────────────────────────────
const MAP_W: u32 = 24;
const MAP_H: u32 = 18;
/// Desplazamiento horizontal por tesela en pantalla (la tesela mide 64 px de ancho).
const ISO_HW: f32 = 32.0;
/// Desplazamiento vertical por tesela en pantalla (ratio 2:1 isométrico).
const ISO_QH: f32 = 16.0;
/// Factor de escala para los sprites de camiones (son 20×14 px nativo).
const TRUCK_SCALE: f32 = 2.0;

// ── Utilidades de proyección ──────────────────────────────────────────────────

/// Convierte coordenadas de tesela a posición del vértice superior del rombo (Bevy Y-up).
fn iso(tx: i32, ty: i32) -> Vec2 {
    Vec2::new(
        (tx - ty) as f32 * ISO_HW,
        (tx + ty) as f32 * -ISO_QH,
    )
}

/// La mitad de la altura de los sprites de tesela (64×31 → 15.5 px).
const TILE_HALF_H: f32 = 15.5;

/// Vec3 para teselas de suelo (painter's algorithm: mayor tx+ty = al frente).
///
/// Equivalente a Anchor::TopCenter: posiciona el centro del sprite 15.5 px por debajo
/// del vértice superior del rombo, que es la convención de referencia de OpenTTD.
fn tile_pos(tx: i32, ty: i32, layer: f32) -> Vec3 {
    let p = iso(tx, ty);
    Vec3::new(
        p.x,
        p.y - TILE_HALF_H,
        (tx + ty) as f32 * 0.01 + layer,
    )
}

/// Calcula la posición del centro de un sprite overlay a partir del xrel/yrel del NFO.
///
/// - `ref_pos`: vértice superior del rombo de la tesela (salida de `iso()`).
/// - `xrel`, `yrel`: offsets del NFO (en coords pantalla Y-down).
/// - `w`, `h`: dimensiones del sprite en píxeles.
fn overlay_pos(ref_pos: Vec2, xrel: f32, yrel: f32, w: f32, h: f32, layer: f32, tx: i32, ty: i32) -> Vec3 {
    Vec3::new(
        ref_pos.x + xrel + w / 2.0,
        ref_pos.y - yrel - h / 2.0,
        (tx + ty) as f32 * 0.01 + layer,
    )
}

/// Dibuja el contorno de un rombo isométrico.
fn gizmo_diamond(gizmos: &mut Gizmos, center: Vec2, hw: f32, hh: f32, color: Color) {
    let t = center + Vec2::new(0.0, hh);
    let r = center + Vec2::new(hw, 0.0);
    let b = center + Vec2::new(0.0, -hh);
    let l = center + Vec2::new(-hw, 0.0);
    gizmos.line_2d(t, r, color);
    gizmos.line_2d(r, b, color);
    gizmos.line_2d(b, l, color);
    gizmos.line_2d(l, t, color);
}

// ── Dirección de carretera ────────────────────────────────────────────────────

/// Dirección predominante de un tramo de carretera.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RoadDir {
    /// Conecta en la dirección tx (±1, 0) → sprite NE-SW.
    Tx,
    /// Conecta en la dirección ty (0, ±1) → sprite NW-SE.
    Ty,
    /// Conecta en ambas → cruce o esquina.
    Both,
}

/// Detecta la dirección de una tesela de carretera mirando sus vecinas.
fn road_dir(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> RoadDir {
    let is_road = |c: TileCoord| -> bool {
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
            return false;
        }
        matches!(map.get_kind(c), Some(TileKind::Road | TileKind::Rail))
    };
    let has_tx = is_road(TileCoord::new(pos.x - 1, pos.y))
        || is_road(TileCoord::new(pos.x + 1, pos.y));
    let has_ty = is_road(TileCoord::new(pos.x, pos.y - 1))
        || is_road(TileCoord::new(pos.x, pos.y + 1));
    match (has_tx, has_ty) {
        (true, false) => RoadDir::Tx,
        (false, true) => RoadDir::Ty,
        (true, true) => RoadDir::Both,
        (false, false) => RoadDir::Ty,
    }
}

// ── Dirección de vehículo ─────────────────────────────────────────────────────

/// Dirección de movimiento de un vehículo en pantalla isométrica.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum VehicleDir {
    #[default]
    Ne,
    Se,
    Sw,
    Nw,
}

/// Deduce la dirección del vehículo a partir del siguiente paso en su path.
fn vehicle_dir(v: &Vehicle) -> VehicleDir {
    let Some(next) = v.path.front() else {
        return VehicleDir::default();
    };
    let dx = next.x - v.pos.x;
    let dy = next.y - v.pos.y;
    // En isométrico: +tx = SE, -tx = NW, +ty = SW, -ty = NE
    match (dx.signum(), dy.signum()) {
        (1, _) => VehicleDir::Se,
        (-1, _) => VehicleDir::Nw,
        (_, 1) => VehicleDir::Sw,
        _ => VehicleDir::Ne,
    }
}

// ── Recursos ──────────────────────────────────────────────────────────────────

/// Handles de los sprites de camiones en las 4 direcciones isométricas.
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

/// Marca la entidad sprite de un vehículo; el valor es el `id` del vehículo.
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
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                advance_sim,
                sync_window_title,
                update_vehicles,
                draw_industries,
                draw_stations,
            )
                .chain(),
        )
        .run();
}

// ── Simulación ────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct SimWorld {
    state: GameState,
}

impl Default for SimWorld {
    fn default() -> Self {
        let mut state = GameState::new(MAP_W, MAP_H);
        distribute_tile_kinds(&mut state, 0xDEAD_BEEF_CAFE_1234);
        place_industries(&mut state);
        place_stations(&mut state);
        place_roads(&mut state);
        place_vehicles(&mut state);
        Self { state }
    }
}

fn distribute_tile_kinds(state: &mut GameState, seed: u64) {
    let (mw, mh) = state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let kind = tile_kind_hash(x, y, seed);
            let c = TileCoord::new(x as i32, y as i32);
            let _ = state.map.set_kind(c, kind);
        }
    }
}

fn tile_kind_hash(x: u32, y: u32, seed: u64) -> TileKind {
    let mut h = seed
        .wrapping_add(u64::from(x).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(u64::from(y).wrapping_mul(0x6C62_272E_07BB_0142));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    match h % 10 {
        0 | 1 => TileKind::Water,
        2 | 3 => TileKind::Forest,
        4 => TileKind::CoalField,
        _ => TileKind::Grass,
    }
}

fn place_industries(state: &mut GameState) {
    const STRIDE: u32 = 4;
    let (mw, mh) = state.map.dimensions();
    let mut coal_n = 0u32;
    let mut forest_n = 0u32;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            match state.map.get_kind(c) {
                Some(TileKind::CoalField) => {
                    if coal_n % STRIDE == 0 {
                        state.industries.push(Industry::new(c, IndustryKind::CoalMine));
                    }
                    coal_n += 1;
                }
                Some(TileKind::Forest) => {
                    if forest_n % STRIDE == 0 {
                        state.industries.push(Industry::new(c, IndustryKind::Forest));
                    }
                    forest_n += 1;
                }
                _ => {}
            }
        }
    }
}

fn place_stations(state: &mut GameState) {
    let (mw, mh) = state.map.dimensions();
    let positions: Vec<TileCoord> = state
        .industries
        .iter()
        .enumerate()
        .map(|(i, ind)| {
            let dy = if i % 2 == 0 { 3i32 } else { -3i32 };
            TileCoord::new(
                (ind.pos.x + 3).clamp(0, mw as i32 - 1),
                (ind.pos.y + dy).clamp(0, mh as i32 - 1),
            )
        })
        .collect();
    for pos in positions {
        state.stations.push(Station::new(pos));
    }
}

fn place_roads(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (from, to) in routes {
        let mut cur = from;
        while cur.x != to.x {
            cur.x += (to.x - cur.x).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
        while cur.y != to.y {
            cur.y += (to.y - cur.y).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
    }
}

fn place_vehicles(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (i, (a, b)) in routes.into_iter().enumerate() {
        let mut v = Vehicle::new(i as u32, VehicleKind::Truck, a, b);
        if let Some(path) = find_path(&state.map, a, b) {
            v.path = path.into_iter().collect();
        }
        state.vehicles.push(v);
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, sim: Res<SimWorld>) {
    // Centro del mapa isométrico.
    let cam_x = ((MAP_W as i32 - 1) - (MAP_H as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((MAP_W as i32 - 1) + (MAP_H as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.45, 0.60, 0.75)),
            ..default()
        },
        Transform::from_translation(Vec3::new(cam_x, cam_y, 999.9)),
    ));

    // ── Handles de teselas de suelo ───────────────────────────────────────────
    let h_grass = asset_server.load::<Image>("opengfx/tiles/grass.png");
    let h_rough = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
    let h_water = asset_server.load::<Image>("opengfx/tiles/water.png");
    // SPR_ROAD_X (1333): carretera en dirección tx (NE-SW en pantalla).
    let h_road_tx = asset_server.load::<Image>("opengfx/tiles/road_tx.png");
    // SPR_ROAD_Y (1332): carretera en dirección ty (NW-SE en pantalla).
    let h_road_ty = asset_server.load::<Image>("opengfx/tiles/road_ty.png");
    // Cruce de carretera (sprite 1338).
    let h_road_cross = asset_server.load::<Image>("opengfx/tiles/road_cross.png");

    // ── Handles de overlays ───────────────────────────────────────────────────
    let h_tree_1 = asset_server.load::<Image>("opengfx/tiles/tree_1.png");
    let h_tree_2 = asset_server.load::<Image>("opengfx/tiles/tree_2.png");
    let h_tree_3 = asset_server.load::<Image>("opengfx/tiles/tree_3.png");
    // sid=2013 (58×50): headframe principal de la Coal Mine (IT_COAL_MINE = 0).
    // El sprite 2179 que usábamos antes era de la Printing Works (IT_PRINT = 7).
    let h_coal = asset_server.load::<Image>("opengfx/tiles/coal_mine_hq.png");
    let trees = [h_tree_1, h_tree_2, h_tree_3];

    // ── Handles de camiones ───────────────────────────────────────────────────
    let h_truck_ne = asset_server.load::<Image>("opengfx/tiles/truck_ne.png");
    let h_truck_se = asset_server.load::<Image>("opengfx/tiles/truck_se.png");
    let h_truck_sw = asset_server.load::<Image>("opengfx/tiles/truck_sw.png");
    let h_truck_nw = asset_server.load::<Image>("opengfx/tiles/truck_nw.png");
    commands.insert_resource(TruckHandles {
        ne: h_truck_ne,
        se: h_truck_se,
        sw: h_truck_sw,
        nw: h_truck_nw,
    });

    // ── Teselas de suelo ──────────────────────────────────────────────────────
    let (mw, mh) = sim.state.map.dimensions();
    for ty in 0..mh {
        for tx in 0..mw {
            let c = TileCoord::new(tx as i32, ty as i32);
            let kind = sim.state.map.get_kind(c).unwrap_or(TileKind::Grass);
            let p = iso(tx as i32, ty as i32);

            let (image, color) = match kind {
                TileKind::Grass => (h_grass.clone(), Color::WHITE),
                TileKind::Forest => (h_rough.clone(), Color::srgb(0.6, 1.0, 0.45)),
                TileKind::CoalField => (h_rough.clone(), Color::srgb(0.55, 0.50, 0.45)),
                TileKind::Water => (h_water.clone(), Color::WHITE),
                TileKind::Road => {
                    let img = match road_dir(&sim.state.map, c, mw, mh) {
                        RoadDir::Tx => h_road_tx.clone(),
                        RoadDir::Ty => h_road_ty.clone(),
                        RoadDir::Both => h_road_cross.clone(),
                    };
                    (img, Color::WHITE)
                }
                TileKind::Rail => (h_road_tx.clone(), Color::srgb(0.75, 0.75, 1.0)),
            };

            commands.spawn((
                Sprite { image, color, ..default() },
                Transform::from_translation(tile_pos(tx as i32, ty as i32, 0.0)),
            ));

            // ── Árbol sobre teselas Forest ────────────────────────────────────
            // 1 árbol cuya variante y pequeño offset X se derivan del hash de la posición.
            if kind == TileKind::Forest {
                let h = wang_hash(tx, ty, 0xCAFE);
                let tree_idx = (h % 3) as usize;
                // Offset X determinista dentro del rombo (±8 px máx).
                let ox = ((h >> 2) % 17) as f32 - 8.0;
                // NFO: tree_1-3 → xrel=-19 yrel=-36 w=35 h=43
                let pos3 = overlay_pos(
                    Vec2::new(p.x + ox, p.y),
                    -19.0, -36.0, 35.0, 43.0,
                    0.3,
                    tx as i32, ty as i32,
                );
                commands.spawn((
                    Sprite { image: trees[tree_idx].clone(), ..default() },
                    Transform::from_translation(pos3),
                ));
            }
        }
    }

    // ── Edificios de industrias ───────────────────────────────────────────────
    for industry in &sim.state.industries {
        if industry.kind == IndustryKind::CoalMine {
            let p = iso(industry.pos.x, industry.pos.y);
            // sid=2013: headframe de la Coal Mine, 58×50, xrel=-16, yrel=-33
            let pos3 = overlay_pos(p, -16.0, -33.0, 58.0, 50.0, 0.6, industry.pos.x, industry.pos.y);
            commands.spawn((
                Sprite { image: h_coal.clone(), ..default() },
                Transform::from_translation(pos3),
            ));
        }
    }

    // ── Sprites de vehículos ──────────────────────────────────────────────────
    // Se spawnean aquí y se actualizan en update_vehicles cada tick.
    // TruckHandles se insertó arriba; los accedemos desde el resource en update_vehicles.
    // Por ahora cargamos el handle NE como placeholder; update_vehicles lo corrige en el primer frame.
    let h_truck_ne_init = asset_server.load::<Image>("opengfx/tiles/truck_ne.png");
    for vehicle in &sim.state.vehicles {
        let p = iso(vehicle.pos.x, vehicle.pos.y);
        // NFO truck_ne: xrel=-14 yrel=-5 w=20 h=14
        let pos3 = overlay_pos(p, -14.0, -5.0, 20.0, 14.0, 1.0, vehicle.pos.x, vehicle.pos.y);
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

/// Actualiza posición y dirección de cada sprite de vehículo.
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
        let p = iso(v.pos.x, v.pos.y);

        // Offset según dirección (xrel/yrel del NFO para cada vista del camión).
        let (xrel, yrel, w, h) = match dir {
            VehicleDir::Ne => (-14.0, -5.0, 20.0, 14.0),
            VehicleDir::Se => (-6.0, -6.0, 20.0, 15.0),
            VehicleDir::Sw => (-14.0, -6.0, 20.0, 15.0),
            VehicleDir::Nw => (-6.0, -5.0, 20.0, 14.0),
        };
        let pos3 = overlay_pos(p, xrel, yrel, w, h, 1.0, v.pos.x, v.pos.y);
        transform.translation = pos3;
        sprite.image = trucks.for_dir(dir);
    }
}

/// Dibuja contorno de rombo para cada industria + barra de stock.
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

/// Dibuja contorno cian para cada estación + barra de income.
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

// ── Utilidades ────────────────────────────────────────────────────────────────

/// Hash de Wang para generar variación determinista (sin RNG en el core).
fn wang_hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = seed
        .wrapping_add(x.wrapping_mul(0x9E37_79B9))
        .wrapping_add(y.wrapping_mul(0x6C62_272E));
    h ^= h >> 16;
    h = h.wrapping_mul(0x45D9_F3B);
    h ^= h >> 16;
    h
}
