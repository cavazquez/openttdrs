use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{CargoType, Map, TileKind, Vehicle, VehicleKind};

use crate::bevy_app::UpdateSet;
use crate::iso::{
    iso, overlay_pos, road_vehicle_straight_subtile, road_vehicle_tile_anchor, tile_min_z,
};
use crate::render::MapVisualLayer;
use crate::state::{ClientScreen, SimWorld};

#[path = "../sprites/vehicle_gfx_data_generated.rs"]
mod vehicle_gfx;

use vehicle_gfx::{BUS_VEHICLE_LAYERS, TRUCK_VEHICLE_LAYERS, TRUCK_VEHICLE_LAYERS_LOADED};

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
        VehicleKind::Bus | VehicleKind::Train => &BUS_VEHICLE_LAYERS,
    }
}

fn vehicle_layer(v: &Vehicle) -> &'static vehicle_gfx::VehicleLayerGfx {
    let dir = v.render_direction().min(7) as usize;
    &vehicle_layers(v)[dir]
}

fn vehicle_draw_anchor(v: &Vehicle, map: &Map) -> (Vec2, u8, i32, i32) {
    let base_z = tile_min_z(map, v.pos);
    let dir = v.render_direction();

    // Carretera recta: sub-tesela OpenTTD (eje del carril), no diagonal entre esquinas.
    if dir & 1 == 1 {
        let (sub_x, sub_y) = road_vehicle_straight_subtile(dir, v.progress);
        let anchor = road_vehicle_tile_anchor(v.pos.x, v.pos.y, sub_x, sub_y);
        return (anchor, base_z, v.pos.x, v.pos.y);
    }

    // Giros cardinales: interpolación entre teselas (hasta tener curvas de giro).
    let base = iso(v.pos.x, v.pos.y);
    let Some(next) = v.movement_target() else {
        return (base, base_z, v.pos.x, v.pos.y);
    };
    if v.progress == 0 {
        return (base, base_z, v.pos.x, v.pos.y);
    }
    let t = f32::from(v.progress) / 255.0;
    let next_iso = iso(next.x, next.y);
    let next_z = tile_min_z(map, next);
    let pos = base.lerp(next_iso, t);
    let z = f32::from(base_z)
        .mul_add(1.0 - t, f32::from(next_z) * t)
        .round() as u8;
    let tx = (v.pos.x as f32).mul_add(1.0 - t, next.x as f32 * t).round() as i32;
    let ty = (v.pos.y as f32).mul_add(1.0 - t, next.y as f32 * t).round() as i32;
    (pos, z, tx, ty)
}

fn vehicle_sprite_pos(v: &Vehicle, map: &Map) -> Vec3 {
    let layer = vehicle_layer(v);
    let (anchor, height, tx, ty) = vehicle_draw_anchor(v, map);
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
    truck: DirHandles,
    truck_loaded: DirHandles,
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
            truck: load_set(asset_server, &TRUCK_VEHICLE_LAYERS),
            truck_loaded: load_set(asset_server, &TRUCK_VEHICLE_LAYERS_LOADED),
        }
    }

    fn for_vehicle(&self, v: &Vehicle) -> Handle<Image> {
        let i = v.render_direction().min(7) as usize;
        match v.kind {
            VehicleKind::Truck if v.uses_loaded_road_sprite() => self.truck_loaded[i].clone(),
            VehicleKind::Truck => self.truck[i].clone(),
            VehicleKind::Bus | VehicleKind::Train => self.bus[i].clone(),
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
        let pos3 = vehicle_sprite_pos(vehicle, &sim.state.map);
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

fn vehicle_tint(v: &Vehicle) -> Color {
    if v.kind == VehicleKind::Train {
        Color::srgb(0.86, 1.0, 0.86)
    } else {
        Color::WHITE
    }
}

fn vehicle_is_hidden_in_depot(sim: &SimWorld, v: &Vehicle) -> bool {
    !v.running && sim.state.map.get_kind(v.pos) == Some(TileKind::RoadDepot)
}

pub(crate) fn update_vehicles(
    sim: Res<SimWorld>,
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
        let pos3 = vehicle_sprite_pos(v, &sim.state.map);
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
        let pos3 = vehicle_sprite_pos(v, &sim.state.map);
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
            orders: Vec::new(),
            current_order: 0,
            no_network_route_to_order: false,
        }
    }

    fn default_handles() -> TruckHandles {
        TruckHandles {
            bus: Default::default(),
            truck: Default::default(),
            truck_loaded: Default::default(),
        }
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
