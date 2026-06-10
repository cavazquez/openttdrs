use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{CargoType, Map, TileKind, Vehicle, VehicleKind};

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, road_vehicle_tile_anchor, tile_min_z, tile_slope_and_min_z};
use crate::render::MapVisualLayer;
use crate::state::{ClientScreen, SimWorld};
use openttdrs_core::{
    slope_dz_at_subtile, vehicle_render_direction, vehicle_render_progress,
    vehicle_subtile_with_progress,
};

use crate::simulation::SimClock;

#[path = "../sprites/vehicle_gfx_data_generated.rs"]
mod vehicle_gfx;

use vehicle_gfx::{
    BUS_VEHICLE_LAYERS, BUS_VEHICLE_LAYERS_LOADED, TRAIN_VEHICLE_LAYERS, TRUCK_VEHICLE_LAYERS,
    TRUCK_VEHICLE_LAYERS_LOADED,
};

pub(crate) struct VehicleRenderPlugin;

impl Plugin for VehicleRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIndex>()
            .add_systems(OnEnter(ClientScreen::InGame), rebuild_vehicle_index)
            .add_systems(
                Update,
                update_vehicles
                    .in_set(UpdateSet::Visuals)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

fn vehicle_layers(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
    match v.kind {
        VehicleKind::Truck if v.uses_loaded_road_sprite() => &TRUCK_VEHICLE_LAYERS_LOADED,
        VehicleKind::Truck => &TRUCK_VEHICLE_LAYERS,
        VehicleKind::Bus if v.uses_loaded_road_sprite() => &BUS_VEHICLE_LAYERS_LOADED,
        VehicleKind::Bus => &BUS_VEHICLE_LAYERS,
        VehicleKind::Train => &TRAIN_VEHICLE_LAYERS,
    }
}

fn vehicle_layer(v: &Vehicle, render_progress: u8) -> &'static vehicle_gfx::VehicleLayerGfx {
    let dir = vehicle_render_direction(v, render_progress).min(7) as usize;
    &vehicle_layers(v)[dir]
}

fn vehicle_draw_anchor(v: &Vehicle, map: &Map, tick_alpha: f32) -> (Vec2, u8, i32, i32) {
    let (tileh, _) = tile_slope_and_min_z(map, v.pos.x as u32, v.pos.y as u32);
    let base_z = tile_min_z(map, v.pos);
    let render_progress = vehicle_render_progress(v, tick_alpha);
    let (sub_x, sub_y) = vehicle_subtile_with_progress(v, render_progress);
    let sub_z = slope_dz_at_subtile(sub_x, sub_y, tileh);
    let anchor = road_vehicle_tile_anchor(v.pos.x, v.pos.y, sub_x, sub_y, sub_z);
    (anchor, base_z, v.pos.x, v.pos.y)
}

/// Posición mundo del sprite del vehículo (para cámara de seguimiento).
#[must_use]
pub(crate) fn vehicle_world_position(v: &Vehicle, map: &Map) -> Vec3 {
    vehicle_sprite_pos(v, map, 0.0)
}

fn vehicle_sprite_pos(v: &Vehicle, map: &Map, tick_alpha: f32) -> Vec3 {
    let render_progress = vehicle_render_progress(v, tick_alpha);
    let layer = vehicle_layer(v, render_progress);
    let (anchor, height, tx, ty) = vehicle_draw_anchor(v, map, tick_alpha);
    overlay_pos(
        anchor,
        layer.x_offs,
        layer.y_offs,
        layer.w,
        layer.h,
        height,
        1.0,
        tx,
        ty,
    )
}

type DirHandles = [Handle<Image>; 8];

#[derive(Resource)]
pub(crate) struct TruckHandles {
    bus: DirHandles,
    bus_loaded: DirHandles,
    truck: DirHandles,
    truck_loaded: DirHandles,
    train: DirHandles,
}

impl TruckHandles {
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        fn load_set(
            server: &AssetServer,
            layers: &[vehicle_gfx::VehicleLayerGfx; 8],
        ) -> DirHandles {
            [
                server.load(layers[0].path),
                server.load(layers[1].path),
                server.load(layers[2].path),
                server.load(layers[3].path),
                server.load(layers[4].path),
                server.load(layers[5].path),
                server.load(layers[6].path),
                server.load(layers[7].path),
            ]
        }
        Self {
            bus: load_set(asset_server, &BUS_VEHICLE_LAYERS),
            bus_loaded: load_set(asset_server, &BUS_VEHICLE_LAYERS_LOADED),
            truck: load_set(asset_server, &TRUCK_VEHICLE_LAYERS),
            truck_loaded: load_set(asset_server, &TRUCK_VEHICLE_LAYERS_LOADED),
            train: load_set(asset_server, &TRAIN_VEHICLE_LAYERS),
        }
    }

    fn for_vehicle(&self, v: &Vehicle) -> Handle<Image> {
        let i = v.render_direction().min(7) as usize;
        match v.kind {
            VehicleKind::Truck if v.uses_loaded_road_sprite() => self.truck_loaded[i].clone(),
            VehicleKind::Truck => self.truck[i].clone(),
            VehicleKind::Bus if v.uses_loaded_road_sprite() => self.bus_loaded[i].clone(),
            VehicleKind::Bus => self.bus[i].clone(),
            VehicleKind::Train => self.train[i].clone(),
        }
    }
}

/// Índice `Vehicle.id` → posición en `GameState::vehicles` (evita `find` O(V) por sprite).
#[derive(Resource, Default)]
pub(crate) struct VehicleIndex {
    by_id: HashMap<u32, usize>,
}

impl VehicleIndex {
    pub(crate) fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.by_id.clear();
        self.by_id.reserve(vehicles.len());
        for (i, v) in vehicles.iter().enumerate() {
            self.by_id.insert(v.id, i);
        }
    }
}

pub(crate) fn rebuild_vehicle_index(sim: Res<SimWorld>, mut idx: ResMut<VehicleIndex>) {
    idx.rebuild(&sim.state.vehicles);
}

pub(crate) fn spawn_initial_vehicles(
    commands: &mut Commands,
    sim: &SimWorld,
    trucks: &TruckHandles,
) {
    for vehicle in &sim.state.vehicles {
        let pos3 = vehicle_sprite_pos(vehicle, &sim.state.map, 0.0);
        commands.spawn((
            MapVisualLayer,
            VehicleSprite(vehicle.id),
            Sprite {
                image: trucks.for_vehicle(vehicle),
                color: vehicle_tint(vehicle),
                ..default()
            },
            Transform::from_translation(pos3),
            Visibility::Visible,
        ));
        commands.spawn((
            MapVisualLayer,
            VehicleCargoLabel(vehicle.id),
            Text2d::new(vehicle_cargo_label(vehicle)),
            TextFont {
                font_size: 8.0,
                ..default()
            },
            TextColor(vehicle_cargo_color(vehicle)),
            Transform::from_translation(vehicle_cargo_label_pos(pos3)),
            Visibility::Visible,
        ));
    }
}

#[derive(Component)]
pub(crate) struct VehicleSprite(u32);

#[derive(Component)]
pub(crate) struct VehicleCargoLabel(u32);

fn vehicle_cargo_label(v: &Vehicle) -> String {
    let cargo = match v.cargo_type {
        Some(CargoType::Passengers) => "PAX",
        Some(CargoType::Mail) => "MAIL",
        Some(CargoType::Goods) => "GOODS",
        Some(CargoType::Coal) => "COAL",
        Some(CargoType::Wood) => "WOOD",
        Some(CargoType::Oil) => "OIL",
        None => "ANY",
    };
    format!("{cargo} {}/{}", v.cargo, v.capacity)
}

fn vehicle_cargo_color(v: &Vehicle) -> Color {
    if v.cargo > 0 {
        Color::srgb(0.95, 0.9, 0.35)
    } else {
        Color::srgba(0.8, 0.85, 0.9, 0.72)
    }
}

fn vehicle_cargo_label_pos(vehicle_pos: Vec3) -> Vec3 {
    Vec3::new(vehicle_pos.x, vehicle_pos.y + 21.0, vehicle_pos.z + 0.35)
}

fn vehicle_tint(_v: &Vehicle) -> Color {
    Color::WHITE
}

fn vehicle_is_hidden_in_depot(sim: &SimWorld, v: &Vehicle) -> bool {
    !v.running && sim.state.map.get_kind(v.pos) == Some(TileKind::RoadDepot)
}

/// Radio de picking en unidades mundo (clic sobre el sprite del vehículo).
const VEHICLE_PICK_RADIUS_SQ: f32 = 34.0 * 34.0;

/// Vehículo visible bajo el cursor (prioriza el sprite más cercano).
#[must_use]
pub(crate) fn pick_vehicle_id_at_world(world_pos: Vec2, sim: &SimWorld) -> Option<u32> {
    sim.state
        .vehicles
        .iter()
        .filter(|v| !vehicle_is_hidden_in_depot(sim, v))
        .filter_map(|v| {
            let sprite_xy = vehicle_sprite_pos(v, &sim.state.map, 0.0).truncate();
            let dist_sq = sprite_xy.distance_squared(world_pos);
            (dist_sq <= VEHICLE_PICK_RADIUS_SQ).then_some((dist_sq, v.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

pub(crate) fn update_vehicles(
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    trucks: Res<TruckHandles>,
    vehicle_index: Res<VehicleIndex>,
    mut q: Query<(&VehicleSprite, &mut Transform, &mut Sprite, &mut Visibility)>,
    mut labels: Query<
        (
            &VehicleCargoLabel,
            &mut Transform,
            &mut Text2d,
            &mut TextColor,
            &mut Visibility,
        ),
        Without<VehicleSprite>,
    >,
) {
    for (vs, mut transform, mut sprite, mut visibility) in &mut q {
        let Some(&i) = vehicle_index.by_id.get(&vs.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        if vehicle_is_hidden_in_depot(&sim, v) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pos3 = vehicle_sprite_pos(v, &sim.state.map, sim_clock.tick_alpha);
        transform.translation = pos3;
        sprite.image = trucks.for_vehicle(v);
        sprite.color = vehicle_tint(v);
    }

    for (label, mut transform, mut text, mut color, mut visibility) in &mut labels {
        let Some(&i) = vehicle_index.by_id.get(&label.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        if vehicle_is_hidden_in_depot(&sim, v) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pos3 = vehicle_sprite_pos(v, &sim.state.map, sim_clock.tick_alpha);
        transform.translation = vehicle_cargo_label_pos(pos3);
        **text = vehicle_cargo_label(v);
        color.0 = vehicle_cargo_color(v);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{DIR_S, DIR_SW, GameState, TileCoord, VehicleKind};

    fn sample_vehicle(id: u32) -> Vehicle {
        Vehicle {
            id,
            kind: VehicleKind::Truck,
            pos: TileCoord::new(1, 1),
            origin: TileCoord::new(1, 1),
            dest: TileCoord::new(2, 1),
            path: VecDeque::from([TileCoord::new(2, 1)]),
            cargo: 0,
            cargo_type: None,
            capacity: 30,
            running: true,
            progress: 0,
            direction: DIR_SW,
            engine_id: Some(openttdrs_core::ENGINE_TRUCK_MPS),
            cur_speed: 96,
            subspeed: 0,
            orders: Vec::new(),
            current_order: 0,
            no_network_route_to_order: false,
            cargo_source: None,
            cargo_transit_ticks: 0,
            depart_turn: 0,
        }
    }

    fn default_handles() -> TruckHandles {
        TruckHandles {
            bus: Default::default(),
            bus_loaded: Default::default(),
            truck: Default::default(),
            truck_loaded: Default::default(),
            train: Default::default(),
        }
    }

    #[test]
    fn pick_vehicle_prefers_closest_sprite() {
        let mut sim = SimWorld {
            state: openttdrs_core::GameState::new(16, 16),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let on_road = TileCoord::new(4, 4);
        sim.state
            .map
            .set_kind(on_road, TileKind::Road)
            .expect("road tile");
        sim.state
            .vehicles
            .push(Vehicle::new(42, VehicleKind::Bus, on_road, on_road));
        let anchor = vehicle_sprite_pos(&sim.state.vehicles[0], &sim.state.map, 0.0).truncate();
        assert_eq!(pick_vehicle_id_at_world(anchor, &sim), Some(42));
        assert_eq!(
            pick_vehicle_id_at_world(anchor + Vec2::new(200.0, 0.0), &sim),
            None
        );
    }

    #[test]
    fn vehicle_index_and_sprite_helpers_work() {
        let mut idx = VehicleIndex::default();
        let v = sample_vehicle(7);
        idx.rebuild(std::slice::from_ref(&v));
        assert_eq!(idx.by_id.get(&7), Some(&0));
        assert_eq!(v.render_direction(), DIR_SW);
        assert_ne!(vehicle_layers(&v)[1].path, vehicle_layers(&v)[3].path);
        assert!(!v.uses_loaded_road_sprite());
        let loaded = Vehicle {
            cargo: 15,
            ..sample_vehicle(1)
        };
        assert!(loaded.uses_loaded_road_sprite());
        let empty_bus = Vehicle {
            kind: VehicleKind::Bus,
            ..sample_vehicle(2)
        };
        let loaded_bus = Vehicle {
            kind: VehicleKind::Bus,
            cargo: 15,
            ..sample_vehicle(3)
        };
        assert!(loaded_bus.uses_loaded_road_sprite());
        assert_ne!(
            vehicle_layers(&empty_bus)[5].path,
            vehicle_layers(&loaded_bus)[5].path
        );
    }

    #[test]
    fn rebuild_and_update_systems_run() {
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            loaded_file: false,
            ottdmap_extras: None,
        };
        sim.state.vehicles.push(sample_vehicle(11));

        let mut world = World::new();
        world.insert_resource(sim);
        world.insert_resource(crate::simulation::SimClock::default());
        world.insert_resource(default_handles());
        world.insert_resource(VehicleIndex::default());

        world.spawn((
            VehicleSprite(11),
            Transform::default(),
            Sprite::default(),
            Visibility::Visible,
        ));
        world.spawn((
            VehicleCargoLabel(11),
            Transform::default(),
            Text2d::new(""),
            TextColor(Color::WHITE),
            Visibility::Visible,
        ));

        world.run_system_once(rebuild_vehicle_index).unwrap();
        world.run_system_once(update_vehicles).unwrap();

        let mut labels = world.query_filtered::<&Text2d, With<VehicleCargoLabel>>();
        assert_eq!(labels.single(&world).unwrap().to_string(), "ANY 0/30");
    }

    #[test]
    fn train_layers_differ_from_bus() {
        assert_ne!(
            TRAIN_VEHICLE_LAYERS[DIR_SW as usize].path,
            BUS_VEHICLE_LAYERS[DIR_SW as usize].path
        );
    }

    #[test]
    fn render_direction_cardinal_layer_differs_from_diagonal() {
        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Bus;
        v.pos = TileCoord::new(0, 0);
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(1, 1)]);
        v.progress = 200;
        assert_eq!(v.render_direction(), DIR_S);
        assert_ne!(
            BUS_VEHICLE_LAYERS[DIR_S as usize].path,
            BUS_VEHICLE_LAYERS[openttdrs_core::DIR_SE as usize].path
        );
    }
}
