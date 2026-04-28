//! Cliente isométrico: sprites de OpenGFX + gizmos de overlay para el [`GameState`] del core.

#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{
    find_path, GameState, Industry, IndustryKind, Station, TileCoord, TileKind, Vehicle,
    VehicleKind,
};

// ── Constantes isométricas ────────────────────────────────────────────────────
const MAP_W: u32 = 24;
const MAP_H: u32 = 18;
/// Desplazamiento horizontal por tesela en pantalla (la tesela mide 64 px de ancho).
const ISO_HW: f32 = 32.0;
/// Desplazamiento vertical por tesela en pantalla (ratio 2:1 isométrico).
const ISO_QH: f32 = 16.0;

// ── Utilidades de proyección ──────────────────────────────────────────────────

/// Convierte coordenadas de tesela (tx, ty) a posición 2D de pantalla isométrica.
///
/// El eje X del mapa va hacia la derecha-abajo, el Y hacia la izquierda-abajo.
fn iso(tx: i32, ty: i32) -> Vec2 {
    Vec2::new(
        (tx - ty) as f32 * ISO_HW,
        (tx + ty) as f32 * -ISO_QH,
    )
}

/// La mitad de la altura de los sprites de tesela (64×31 → 15.5 px).
const TILE_HALF_H: f32 = 15.5;

/// Vec3 para la tesela incluyendo la capa Z (painter's algorithm: mayor tx+ty = al frente).
///
/// Posiciona el centro del sprite 15.5 px por debajo del vértice superior del rombo,
/// equivalente a un anchor top-center. Esto es la convención de referencia de OpenTTD
/// (xrel=-31, yrel=0 en el NFO) donde el punto de referencia es el vértice superior.
fn tile_pos(tx: i32, ty: i32, layer: f32) -> Vec3 {
    let p = iso(tx, ty);
    Vec3::new(
        p.x,
        p.y - TILE_HALF_H,
        (tx + ty) as f32 * 0.01 + layer,
    )
}

/// Dibuja el contorno de un rombo isométrico alineado con la tesela.
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

// ── App ───────────────────────────────────────────────────────────────────────

fn main() {
    // La carpeta assets/ vive en la raíz del workspace, dos niveles arriba del crate.
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
                draw_industries,
                draw_stations,
                draw_vehicles,
            )
                .chain(),
        )
        .run();
}

// ── Simulación ────────────────────────────────────────────────────────────────

/// Copia del estado de simulación expuesta al motor (avance a ritmo fijo).
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

/// Distribuye tipos de tesela con una semilla fija (sin RNG en core).
///
/// Usa un hash multiplicativo de Wang: ~20 % agua, ~20 % bosque, ~10 % carbón, resto prado.
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

/// Coloca una industria cada STRIDE teselas del mismo tipo para no saturar el mapa.
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

/// Coloca una estación por industria con offset (+3 X, ±3 Y) alternado según índice.
///
/// Genera rutas en L con movimiento horizontal y vertical.
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

/// Traza carretera Manhattan entre cada industria y su estación pareada.
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

/// Coloca un truck por cada par (industria[i], estación[i]).
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

// ── Sistemas de Bevy ──────────────────────────────────────────────────────────

/// Genera la cámara y los sprites de tesela isométricos al arrancar.
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, sim: Res<SimWorld>) {
    // Centro del mapa isométrico (para 24×18 teselas).
    // Con Anchor::TopCenter, el mapa se extiende hacia abajo desde los vértices superiores,
    // por eso desplazamos la cámara media altura de tesela extra (TILE_HALF_H).
    let cam_x = ((MAP_W as i32 - 1) - (MAP_H as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((MAP_W as i32 - 1) + (MAP_H as i32 - 1)) as f32 / 2.0 * ISO_QH
        - TILE_HALF_H;

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.45, 0.60, 0.75)),
            ..default()
        },
        Transform::from_translation(Vec3::new(cam_x, cam_y, 999.9)),
    ));

    // Handles de sprites de tesela (todos desde assets/opengfx/tiles/).
    let h_grass = asset_server.load::<Image>("opengfx/tiles/grass.png");
    let h_rough = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
    let h_water = asset_server.load::<Image>("opengfx/tiles/water.png");
    let h_road = asset_server.load::<Image>("opengfx/tiles/road_x.png");

    let (mw, mh) = sim.state.map.dimensions();
    for ty in 0..mh {
        for tx in 0..mw {
            let c = TileCoord::new(tx as i32, ty as i32);
            let kind = sim.state.map.get_kind(c).unwrap_or(TileKind::Grass);

            let (image, color) = match kind {
                TileKind::Grass => (h_grass.clone(), Color::WHITE),
                TileKind::Forest => (h_rough.clone(), Color::srgb(0.6, 1.0, 0.45)),
                TileKind::CoalField => (h_rough.clone(), Color::srgb(0.55, 0.50, 0.45)),
                TileKind::Water => (h_water.clone(), Color::WHITE),
                TileKind::Road => (h_road.clone(), Color::WHITE),
                TileKind::Rail => (h_road.clone(), Color::srgb(0.75, 0.75, 1.0)),
            };

            commands.spawn((
                Sprite { image, color, ..default() },
                Transform::from_translation(tile_pos(tx as i32, ty as i32, 0.0)),
            ));
        }
    }
}

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

/// Dibuja un rombo de contorno por cada industria.
fn draw_industries(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    for industry in &sim.state.industries {
        let center = iso(industry.pos.x, industry.pos.y);
        let color = match industry.kind {
            IndustryKind::CoalMine => Color::srgb(1.0, 0.9, 0.1),
            IndustryKind::Forest => Color::srgb(1.0, 0.5, 0.05),
        };
        // Rombo exterior (borde de la tesela).
        gizmo_diamond(&mut gizmos, center, 30.0, 14.0, color);

        // Barra de stock en el interior del rombo.
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

/// Dibuja un rombo cian por cada estación.
fn draw_stations(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    for station in &sim.state.stations {
        let center = iso(station.pos.x, station.pos.y);
        gizmo_diamond(&mut gizmos, center, 26.0, 12.0, Color::srgb(0.0, 0.9, 0.9));

        // Barra de income (escala logarítmica).
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

/// Dibuja un pequeño rombo blanco/amarillo por cada vehículo.
fn draw_vehicles(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    for vehicle in &sim.state.vehicles {
        let center = iso(vehicle.pos.x, vehicle.pos.y);
        let color = if vehicle.cargo > 0 {
            Color::srgb(1.0, 0.9, 0.1)
        } else {
            Color::WHITE
        };
        gizmo_diamond(&mut gizmos, center, 8.0, 4.0, color);
    }
}
