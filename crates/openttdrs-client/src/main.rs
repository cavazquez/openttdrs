//! Cliente mínimo: ventana Bevy, cámara 2D y rejilla de depuración del [`GameState`] del core.

#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

use bevy::color::palettes::css::{DARK_GRAY, LIMEGREEN};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{GameState, TileCoord, TileKind};

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
            (advance_sim, sync_window_title, draw_map_debug).chain(),
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
