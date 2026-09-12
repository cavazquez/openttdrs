//! Fondo del menú: showcase isométrico determinista con paneo suave y tráfico.

use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::iso::{road_vehicle_tile_anchor, tile_min_z, tile_pos};
use crate::network::NetCli;
use crate::render::{
    MapVisualLayer, ShoreTile, TruckHandles, WaterTile, initial_map_camera_pose,
    spawn_intro_map_render,
};
use crate::state::SimWorld;
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, STARTING_MONEY_OPTIONS,
};

use super::main_menu::MainMenuCamera;
use super::main_menu::MainMenuPanel;

/// Cámara del fondo intro (también lleva [`MainMenuCamera`] para limpieza al salir).
#[derive(Component)]
pub(crate) struct MainMenuIntroCamera;

#[derive(Resource, Clone)]
pub(crate) struct MainMenuIntroMap(Map);

#[derive(Resource)]
pub(crate) struct MainMenuIntroState {
    pub(crate) base_pos: Vec2,
}

const INTRO_SETTINGS: NewGameSettings = NewGameSettings {
    climate: openttdrs_core::Climate::Temperate,
    map_size: MapSizePreset::SMALL,
    // 1961 deja visibles todos los vehículos del showcase (incluidos
    // helicóptero y ferry) sin conceder excepciones de preview al jugador.
    start_year: 1961,
    // El menú necesita una composición legible y repetible, no el ruido visual
    // de un mapa aleatorio. `preserve_demo` reutiliza el showcase 64×64 que ya
    // contiene ciudad, industria, vías, puerto y aeropuertos.
    world_gen: false,
    island: false,
    preserve_demo: true,
    seed: 0x4F54_4452, // "OTDR"
    town_density: PopulationDensity::Normal,
    industry_density: PopulationDensity::Normal,
    starting_money: STARTING_MONEY_OPTIONS[1],
    rival_ai: false,
    disasters_enabled: false,
    terrain_roughness: crate::state::bootstrap::TerrainRoughness::Normal,
    gamescript_demo: false,
};

const INTRO_PAN_AMPLITUDE_X: f32 = 30.0;
const INTRO_PAN_AMPLITUDE_Y: f32 = 18.0;
const INTRO_PAN_PERIOD_SECS: f32 = 42.0;

const INTRO_MAGLEV_X0: i32 = 30;
const INTRO_MAGLEV_X1: i32 = 58;
const INTRO_MAGLEV_Y: i32 = 53;

#[derive(Clone, Copy)]
enum IntroVehicleKind {
    Bus,
    Truck,
    Train,
    Maglev,
    Ship,
    Aircraft,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuIntroTrafficActor {
    from: (i32, i32),
    to: (i32, i32),
    progress: f32,
    speed: f32,
    direction: usize,
    kind: IntroVehicleKind,
}

struct IntroTrafficRoute {
    from: (i32, i32),
    to: (i32, i32),
    speed: f32,
    direction: usize,
    kind: IntroVehicleKind,
    start_progress: f32,
}

const INTRO_TRAFFIC_ROUTES: [IntroTrafficRoute; 12] = [
    IntroTrafficRoute {
        from: (14, 4),
        to: (46, 4),
        speed: 0.11,
        direction: 4,
        kind: IntroVehicleKind::Bus,
        start_progress: 0.1,
    },
    IntroTrafficRoute {
        from: (46, 4),
        to: (14, 4),
        speed: 0.09,
        direction: 0,
        kind: IntroVehicleKind::Bus,
        start_progress: 0.55,
    },
    IntroTrafficRoute {
        from: (15, 10),
        to: (22, 10),
        speed: 0.1,
        direction: 4,
        kind: IntroVehicleKind::Truck,
        start_progress: 0.3,
    },
    IntroTrafficRoute {
        from: (22, 12),
        to: (15, 12),
        speed: 0.08,
        direction: 0,
        kind: IntroVehicleKind::Truck,
        start_progress: 0.8,
    },
    IntroTrafficRoute {
        from: (14, 36),
        to: (48, 36),
        speed: 0.075,
        direction: 4,
        kind: IntroVehicleKind::Train,
        start_progress: 0.25,
    },
    IntroTrafficRoute {
        from: (48, 36),
        to: (14, 36),
        speed: 0.065,
        direction: 0,
        kind: IntroVehicleKind::Train,
        start_progress: 0.7,
    },
    IntroTrafficRoute {
        from: (INTRO_MAGLEV_X0, INTRO_MAGLEV_Y),
        to: (INTRO_MAGLEV_X1, INTRO_MAGLEV_Y),
        speed: 0.085,
        direction: 4,
        kind: IntroVehicleKind::Maglev,
        start_progress: 0.42,
    },
    IntroTrafficRoute {
        from: (INTRO_MAGLEV_X1, INTRO_MAGLEV_Y),
        to: (INTRO_MAGLEV_X0, INTRO_MAGLEV_Y),
        speed: 0.075,
        direction: 0,
        kind: IntroVehicleKind::Maglev,
        start_progress: 0.82,
    },
    IntroTrafficRoute {
        from: (12, 24),
        to: (51, 26),
        speed: 0.05,
        direction: 4,
        kind: IntroVehicleKind::Ship,
        start_progress: 0.15,
    },
    IntroTrafficRoute {
        from: (51, 26),
        to: (12, 24),
        speed: 0.045,
        direction: 0,
        kind: IntroVehicleKind::Ship,
        start_progress: 0.65,
    },
    IntroTrafficRoute {
        from: (9, 50),
        to: (44, 50),
        speed: 0.04,
        direction: 4,
        kind: IntroVehicleKind::Aircraft,
        start_progress: 0.33,
    },
    IntroTrafficRoute {
        from: (44, 50),
        to: (9, 50),
        speed: 0.035,
        direction: 0,
        kind: IntroVehicleKind::Aircraft,
        start_progress: 0.76,
    },
];

pub(crate) fn setup_main_menu_intro(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layout_assets: ResMut<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
    net_cli: Res<NetCli>,
) {
    // `--client`: no generar mapa intro; el Welcome trae el mundo del servidor.
    if matches!(*net_cli, NetCli::Client { .. }) {
        commands.insert_resource(MainMenuIntroState {
            base_pos: Vec2::ZERO,
        });
        commands.insert_resource(MainMenuIntroMap(Map::new_flat(1, 1, 1)));
        commands.insert_resource(TruckHandles::load(&asset_server));
        commands.spawn((
            Camera2d,
            MainMenuCamera,
            MainMenuIntroCamera,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.18, 0.26)),
                ..default()
            },
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        return;
    }

    let mut intro_sim = SimWorld::from_new_game(&INTRO_SETTINGS);
    decorate_intro_maglev(&mut intro_sim.state);
    let (cam_pos, cam_scale) = initial_map_camera_pose(&intro_sim);
    let base_pos = cam_pos.truncate();

    commands.insert_resource(MainMenuIntroState { base_pos });
    commands.insert_resource(MainMenuIntroMap(intro_sim.state.map.clone()));

    commands.spawn((
        Camera2d,
        MainMenuCamera,
        MainMenuIntroCamera,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        Transform::from_translation(cam_pos),
        Projection::Orthographic(OrthographicProjection {
            scale: cam_scale * 0.92,
            ..OrthographicProjection::default_2d()
        }),
    ));

    spawn_intro_map_render(
        &mut commands,
        &asset_server,
        &mut layout_assets,
        &mut images,
        &intro_sim,
    );

    let truck_handles = TruckHandles::load(&asset_server);
    spawn_intro_traffic(&mut commands, &intro_sim.state.map, &truck_handles);
    commands.insert_resource(truck_handles);
}

fn spawn_intro_traffic(commands: &mut Commands, map: &Map, trucks: &TruckHandles) {
    for route in INTRO_TRAFFIC_ROUTES {
        let actor = MainMenuIntroTrafficActor {
            from: route.from,
            to: route.to,
            progress: route.start_progress,
            speed: route.speed,
            direction: route.direction,
            kind: route.kind,
        };
        let pos = actor_world_pos(map, &actor);
        let image = intro_sprite_handle(trucks, &actor);
        commands.spawn((
            MapVisualLayer,
            actor,
            Sprite {
                image,
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(pos),
            Visibility::Visible,
        ));
    }
}

fn intro_sprite_handle(trucks: &TruckHandles, actor: &MainMenuIntroTrafficActor) -> Handle<Image> {
    match actor.kind {
        IntroVehicleKind::Bus => trucks.intro_sprite(VehicleKind::Bus, actor.direction),
        IntroVehicleKind::Truck => trucks.intro_sprite(VehicleKind::Truck, actor.direction),
        IntroVehicleKind::Train => trucks.intro_sprite(VehicleKind::Train, actor.direction),
        IntroVehicleKind::Maglev => trucks.intro_maglev_sprite(actor.direction),
        IntroVehicleKind::Ship => trucks.intro_sprite(VehicleKind::Ship, actor.direction),
        IntroVehicleKind::Aircraft => trucks.intro_sprite(VehicleKind::Aircraft, actor.direction),
    }
}

/// Agrega una vía maglev corta al showcase del menú.
///
/// El showcase jugable conserva su red ferroviaria normal para que sus
/// servicios de carga sigan siendo válidos. Esta línea está aislada y sólo
/// pertenece a la escena del título, donde permite identificar visualmente el
/// transporte avanzado sin alterar ninguna partida.
fn decorate_intro_maglev(state: &mut GameState) {
    for x in INTRO_MAGLEV_X0..=INTRO_MAGLEV_X1 {
        let coord = TileCoord::new(x, INTRO_MAGLEV_Y);
        let Some(mut tile) = state.map.get(coord) else {
            continue;
        };
        tile.kind = TileKind::Rail;
        tile.mapt = 0x10;
        tile.m5 = openttdrs_core::RAIL_TB_X;
        tile = openttdrs_core::set_rail_type_on_tile(tile, openttdrs_core::RailType::Maglev);
        let _ = state.map.set_tile(coord, tile);
    }
}

fn actor_world_pos(map: &Map, actor: &MainMenuIntroTrafficActor) -> Vec3 {
    use openttdrs_core::TileCoord;
    let (from_x, from_y) = actor.from;
    let (to_x, to_y) = actor.to;
    let t = actor.progress.clamp(0.0, 1.0);
    let tx_f = from_x as f32 + (to_x - from_x) as f32 * t;
    let ty_f = from_y as f32 + (to_y - from_y) as f32 * t;
    let tile_x = tx_f.floor() as i32;
    let tile_y = ty_f.floor() as i32;
    let sub_x = tx_f - tile_x as f32;
    let sub_y = ty_f - tile_y as f32;
    let height = tile_min_z(map, TileCoord::new(tile_x, tile_y));
    let base = tile_pos(tile_x, tile_y, height, 1.0);
    let (x, y) = match actor.kind {
        IntroVehicleKind::Ship | IntroVehicleKind::Aircraft => (base.x, base.y),
        IntroVehicleKind::Bus
        | IntroVehicleKind::Truck
        | IntroVehicleKind::Train
        | IntroVehicleKind::Maglev => {
            let anchor = road_vehicle_tile_anchor(tile_x, tile_y, sub_x, sub_y, 0.0);
            (anchor.x, anchor.y)
        }
    };
    Vec3::new(x, y, base.z + 0.2)
}

pub(crate) fn animate_main_menu_intro_traffic(
    time: Res<Time>,
    map: Res<MainMenuIntroMap>,
    trucks: Res<TruckHandles>,
    mut q: Query<(&mut MainMenuIntroTrafficActor, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (mut actor, mut transform, mut sprite) in &mut q {
        actor.progress += actor.speed * dt;
        if actor.progress >= 1.0 {
            actor.progress -= 1.0;
            let (new_from, new_to) = (actor.to, actor.from);
            actor.from = new_from;
            actor.to = new_to;
            actor.direction = reverse_intro_direction(actor.direction);
        }
        transform.translation = actor_world_pos(&map.0, &actor);
        sprite.image = intro_sprite_handle(&trucks, &actor);
    }
}

fn reverse_intro_direction(dir: usize) -> usize {
    match dir {
        0 => 4,
        1 => 5,
        2 => 6,
        3 => 7,
        4 => 0,
        5 => 1,
        6 => 2,
        7 => 3,
        _ => dir,
    }
}

pub(crate) fn pan_main_menu_intro_camera(
    time: Res<Time>,
    state: Res<MainMenuIntroState>,
    mut cam_q: Query<&mut Transform, With<MainMenuIntroCamera>>,
) {
    let Ok(mut transform) = cam_q.single_mut() else {
        return;
    };
    let phase = time.elapsed_secs() * std::f32::consts::TAU / INTRO_PAN_PERIOD_SECS;
    transform.translation.x = state.base_pos.x + phase.sin() * INTRO_PAN_AMPLITUDE_X;
    transform.translation.y = state.base_pos.y + (phase * 0.7 + 1.1).cos() * INTRO_PAN_AMPLITUDE_Y;
}

pub(crate) fn despawn_main_menu_intro_layers(
    commands: &mut Commands,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
) {
    for entity in intro_layers {
        commands.entity(entity).despawn();
    }
}

/// Recursos del intro/menú; se ejecuta en `OnExit(MainMenu)` tras los sistemas del frame.
pub(crate) fn cleanup_main_menu_on_exit(mut commands: Commands) {
    commands.remove_resource::<MainMenuPanel>();
    commands.remove_resource::<MainMenuIntroState>();
    commands.remove_resource::<MainMenuIntroMap>();
    commands.remove_resource::<TruckHandles>();
}

#[cfg(test)]
mod tests {
    use openttdrs_core::{GameState, TileCoord, TileKind};

    #[test]
    fn intro_traffic_covers_road_rail_and_water() {
        let kinds: Vec<_> = super::INTRO_TRAFFIC_ROUTES
            .iter()
            .map(|r| std::mem::discriminant(&r.kind))
            .collect();
        assert_eq!(super::INTRO_TRAFFIC_ROUTES.len(), 12);
        assert_eq!(kinds.len(), 12);
        let unique: std::collections::HashSet<_> = kinds.into_iter().collect();
        assert_eq!(
            unique.len(),
            6,
            "bus, truck, train, maglev, ship y aircraft"
        );
    }

    #[test]
    fn intro_uses_deterministic_showcase_settings() {
        assert!(!super::INTRO_SETTINGS.world_gen);
        assert!(super::INTRO_SETTINGS.preserve_demo);
    }

    #[test]
    fn intro_maglev_line_is_typed_and_isolated() {
        let mut state = GameState::new(64, 64);
        super::decorate_intro_maglev(&mut state);
        for x in super::INTRO_MAGLEV_X0..=super::INTRO_MAGLEV_X1 {
            let tile = state
                .map
                .get(TileCoord::new(x, super::INTRO_MAGLEV_Y))
                .expect("maglev dentro del mapa");
            assert_eq!(tile.kind, TileKind::Rail);
            assert_eq!(tile.m5, openttdrs_core::RAIL_TB_X);
            assert_eq!(
                openttdrs_core::rail_type_from_tile(tile),
                openttdrs_core::RailType::Maglev
            );
        }
    }
}
