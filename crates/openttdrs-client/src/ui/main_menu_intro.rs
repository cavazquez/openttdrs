//! Fondo del menú: mapa procedural isométrico con paneo suave y tráfico decorativo.

use bevy::prelude::*;
use openttdrs_core::{Map, VehicleKind};

use crate::iso::{road_vehicle_tile_anchor, tile_min_z, tile_pos};
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
    start_year: 1950,
    world_gen: true,
    island: true,
    preserve_demo: false,
    seed: 0x4F54_4452, // "OTDR"
    town_density: PopulationDensity::Normal,
    industry_density: PopulationDensity::Normal,
    starting_money: STARTING_MONEY_OPTIONS[1],
    rival_ai: false,
    disasters_enabled: false,
    terrain_roughness: crate::state::bootstrap::TerrainRoughness::Normal,
};

const INTRO_PAN_AMPLITUDE_X: f32 = 48.0;
const INTRO_PAN_AMPLITUDE_Y: f32 = 28.0;
const INTRO_PAN_PERIOD_SECS: f32 = 42.0;

#[derive(Clone, Copy)]
enum IntroVehicleKind {
    Bus,
    Truck,
    Train,
    Ship,
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

const INTRO_TRAFFIC_ROUTES: [IntroTrafficRoute; 8] = [
    IntroTrafficRoute {
        from: (16, 34),
        to: (48, 34),
        speed: 0.11,
        direction: 4,
        kind: IntroVehicleKind::Bus,
        start_progress: 0.1,
    },
    IntroTrafficRoute {
        from: (48, 28),
        to: (18, 28),
        speed: 0.09,
        direction: 0,
        kind: IntroVehicleKind::Bus,
        start_progress: 0.55,
    },
    IntroTrafficRoute {
        from: (20, 40),
        to: (44, 40),
        speed: 0.1,
        direction: 4,
        kind: IntroVehicleKind::Truck,
        start_progress: 0.3,
    },
    IntroTrafficRoute {
        from: (44, 20),
        to: (22, 20),
        speed: 0.08,
        direction: 0,
        kind: IntroVehicleKind::Truck,
        start_progress: 0.8,
    },
    IntroTrafficRoute {
        from: (26, 22),
        to: (40, 38),
        speed: 0.07,
        direction: 6,
        kind: IntroVehicleKind::Train,
        start_progress: 0.25,
    },
    IntroTrafficRoute {
        from: (40, 38),
        to: (26, 22),
        speed: 0.06,
        direction: 2,
        kind: IntroVehicleKind::Train,
        start_progress: 0.7,
    },
    IntroTrafficRoute {
        from: (12, 30),
        to: (12, 44),
        speed: 0.05,
        direction: 6,
        kind: IntroVehicleKind::Ship,
        start_progress: 0.15,
    },
    IntroTrafficRoute {
        from: (52, 44),
        to: (52, 26),
        speed: 0.045,
        direction: 2,
        kind: IntroVehicleKind::Ship,
        start_progress: 0.65,
    },
];

pub(crate) fn setup_main_menu_intro(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layout_assets: ResMut<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
) {
    let intro_sim = SimWorld::from_new_game(&INTRO_SETTINGS);
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
        IntroVehicleKind::Ship => trucks.intro_sprite(VehicleKind::Ship, actor.direction),
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
    let anchor = road_vehicle_tile_anchor(tile_x, tile_y, sub_x, sub_y, 0.0);
    let base = tile_pos(tile_x, tile_y, height, 1.0);
    Vec3::new(anchor.x, anchor.y, base.z + 0.2)
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
    #[test]
    fn intro_traffic_covers_road_rail_and_water() {
        let kinds: Vec<_> = super::INTRO_TRAFFIC_ROUTES
            .iter()
            .map(|r| std::mem::discriminant(&r.kind))
            .collect();
        assert_eq!(super::INTRO_TRAFFIC_ROUTES.len(), 8);
        assert_eq!(kinds.len(), 8);
        let unique: std::collections::HashSet<_> = kinds.into_iter().collect();
        assert_eq!(unique.len(), 4, "bus, truck, train y ship");
    }
}
