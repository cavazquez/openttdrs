//! Cliente mínimo: ventana Bevy, cámara 2D y rejilla de depuración del [`GameState`] del core.

#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

use bevy::color::palettes::css::{DARK_GRAY, LIMEGREEN};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{GameState, Industry, IndustryKind, TileCoord, TileKind, Vehicle, VehicleKind};

const TILE_WORLD: f32 = 20.0;
const MAP_W: u32 = 24;
const MAP_H: u32 = 18;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "openttdrs — vista debug".into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<SimWorld>()
        .add_systems(Startup, setup_camera)
        .add_systems(
            Update,
            (advance_sim, sync_window_title, draw_map_debug, draw_industries, draw_vehicles).chain(),
        )
        .run();
}

/// Copia del estado de simulación expuesta al motor (se avanza a ritmo fijo por simplicidad).
#[derive(Resource)]
struct SimWorld {
    state: GameState,
}

impl Default for SimWorld {
    fn default() -> Self {
        let mut state = GameState::new(MAP_W, MAP_H);
        distribute_tile_kinds(&mut state, 0xDEAD_BEEF_CAFE_1234);
        place_industries(&mut state);
        place_vehicles(&mut state);
        Self { state }
    }
}

/// Distribuye tipos de tesela con una semilla fija (solo visual; sin RNG en core).
///
/// Usa un hash multiplicativo de Wang para producir una distribución determinista:
/// ~20 % agua, ~20 % bosque, ~10 % carbón, resto prado.
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

/// Coloca una industria en teselas CoalField y Forest (una de cada N para no saturar el mapa).
fn place_industries(state: &mut GameState) {
    const STRIDE: u32 = 4; // una industria cada 4 teselas del mismo tipo
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

/// Coloca un truck entre cada par consecutivo de industrias.
fn place_vehicles(state: &mut GameState) {
    let positions: Vec<(TileCoord, TileCoord)> = state
        .industries
        .chunks(2)
        .filter(|pair| pair.len() == 2)
        .map(|pair| (pair[0].pos, pair[1].pos))
        .collect();

    for (i, (a, b)) in positions.into_iter().enumerate() {
        state.vehicles.push(Vehicle::new(i as u32, VehicleKind::Truck, a, b));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn sync_window_title(sim: Res<SimWorld>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.title = format!("openttdrs — tick {}", sim.state.tick.get());
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

fn tile_color(kind: TileKind) -> Color {
    match kind {
        TileKind::Grass => Color::srgb(0.45, 0.75, 0.25),
        TileKind::Water => Color::srgb(0.15, 0.40, 0.80),
        TileKind::Forest => Color::srgb(0.10, 0.45, 0.15),
        TileKind::CoalField => Color::srgb(0.22, 0.20, 0.20),
    }
}

fn draw_map_debug(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    let (mw, mh) = sim.state.map.dimensions();
    let ox = -(mw as f32) * TILE_WORLD * 0.5;
    let oy = -(mh as f32) * TILE_WORLD * 0.5;

    for y in 0..mh {
        for x in 0..mw {
            let xi = i32::try_from(x).expect("map index fits i32");
            let yi = i32::try_from(y).expect("map index fits i32");
            let c = TileCoord::new(xi, yi);
            let kind = sim.state.map.get_kind(c).unwrap_or(TileKind::Grass);
            let wx = ox + (x as f32) * TILE_WORLD;
            let wy = oy + (y as f32) * TILE_WORLD;
            gizmos.rect_2d(
                Isometry2d::from_translation(Vec2::new(wx, wy)),
                Vec2::splat(TILE_WORLD - 1.0),
                tile_color(kind),
            );
        }
    }

    // Contorno del mapa (solo trazo, sin relleno adicional encima de las teselas).
    let half_w = (mw as f32) * TILE_WORLD * 0.5;
    let half_h = (mh as f32) * TILE_WORLD * 0.5;
    let a = Vec2::new(ox - TILE_WORLD * 0.5, oy - TILE_WORLD * 0.5);
    let b = Vec2::new(ox + half_w * 2.0 - TILE_WORLD * 0.5, oy - TILE_WORLD * 0.5);
    let c = Vec2::new(
        ox + half_w * 2.0 - TILE_WORLD * 0.5,
        oy + half_h * 2.0 - TILE_WORLD * 0.5,
    );
    let d = Vec2::new(ox - TILE_WORLD * 0.5, oy + half_h * 2.0 - TILE_WORLD * 0.5);
    gizmos.line_2d(a, b, LIMEGREEN);
    gizmos.line_2d(b, c, LIMEGREEN);
    gizmos.line_2d(c, d, LIMEGREEN);
    gizmos.line_2d(d, a, LIMEGREEN);

    gizmos.line_2d(Vec2::ZERO, Vec2::new(80.0, 40.0), DARK_GRAY);
}

fn draw_vehicles(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    let (mw, mh) = sim.state.map.dimensions();
    let ox = -(mw as f32) * TILE_WORLD * 0.5;
    let oy = -(mh as f32) * TILE_WORLD * 0.5;

    for vehicle in &sim.state.vehicles {
        let wx = ox + (vehicle.pos.x as f32) * TILE_WORLD;
        let wy = oy + (vehicle.pos.y as f32) * TILE_WORLD;
        gizmos.rect_2d(
            Isometry2d::from_translation(Vec2::new(wx, wy)),
            Vec2::splat(TILE_WORLD * 0.3),
            Color::WHITE,
        );
    }
}

fn draw_industries(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    let (mw, mh) = sim.state.map.dimensions();
    let ox = -(mw as f32) * TILE_WORLD * 0.5;
    let oy = -(mh as f32) * TILE_WORLD * 0.5;

    for industry in &sim.state.industries {
        let wx = ox + (industry.pos.x as f32) * TILE_WORLD;
        let wy = oy + (industry.pos.y as f32) * TILE_WORLD;

        // Color base según tipo de industria.
        let base_color = match industry.kind {
            IndustryKind::CoalMine => Color::srgb(0.9, 0.85, 0.1),  // amarillo
            IndustryKind::Forest   => Color::srgb(0.8, 0.4, 0.05),  // naranja
        };

        // Cuadrado central que representa la industria.
        let icon_size = TILE_WORLD * 0.55;
        gizmos.rect_2d(
            Isometry2d::from_translation(Vec2::new(wx, wy)),
            Vec2::splat(icon_size),
            base_color,
        );

        // Barra de stock: rectángulo estrecho en el borde inferior de la tesela,
        // cuya anchura escala con el nivel de stock.
        let fill = industry.stock as f32 / industry.capacity as f32;
        // Solo dibujar la barra cuando hay stock producido, cada INDUSTRY_PRODUCE_TICKS ticks.
        if fill > 0.0 {
            let bar_w = (TILE_WORLD - 2.0) * fill;
            let bar_h = 3.0;
            let bar_x = wx - (TILE_WORLD - 2.0) * 0.5 + bar_w * 0.5;
            let bar_y = wy - TILE_WORLD * 0.5 + bar_h * 0.5 + 1.0;
            gizmos.rect_2d(
                Isometry2d::from_translation(Vec2::new(bar_x, bar_y)),
                Vec2::new(bar_w, bar_h),
                Color::WHITE,
            );
        }
    }
}
