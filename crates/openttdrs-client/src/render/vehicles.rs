use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{CargoType, TileKind, Vehicle, VehicleKind};

use crate::bevy_app::UpdateSet;
use crate::iso::{iso, overlay_pos, tile_min_z};
use crate::render::MapVisualLayer;
use crate::state::{ClientScreen, SimWorld};

/// Factor de escala para los sprites de camiones (son 20×14 px nativo).
const TRUCK_SCALE: f32 = 2.0;

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

fn vehicle_sprite_bounds(dir: VehicleDir) -> (f32, f32, f32, f32) {
    // Mantener una base visual estable evita “saltos” cuando cambia la orientación.
    let w = 20.0;
    let h = 15.0;
    let yrel = -6.0;
    let xrel = match dir {
        VehicleDir::Ne | VehicleDir::Sw => -14.0,
        VehicleDir::Se | VehicleDir::Nw => -6.0,
    };
    (xrel, yrel, w, h)
}

#[derive(Resource)]
pub(crate) struct TruckHandles {
    ne: Handle<Image>,
    se: Handle<Image>,
    sw: Handle<Image>,
    nw: Handle<Image>,
}

impl TruckHandles {
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        let bus = asset_server.load::<Image>("assets/opengfx/tiles/vehicle_bus_sw.png");
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
        let dir = vehicle_dir(vehicle);
        let vh = tile_min_z(&sim.state.map, vehicle.pos);
        let p = iso(vehicle.pos.x, vehicle.pos.y);
        let (xrel, yrel, w, h) = vehicle_sprite_bounds(dir);
        let pos3 = overlay_pos(p, xrel, yrel, w, h, vh, 1.0, vehicle.pos.x, vehicle.pos.y);
        commands.spawn((
            MapVisualLayer,
            VehicleSprite(vehicle.id),
            Sprite {
                image: trucks.for_dir(dir),
                color: vehicle_tint(vehicle),
                ..default()
            },
            Transform::from_translation(pos3).with_scale(Vec3::splat(TRUCK_SCALE)),
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
    match v.kind {
        VehicleKind::Bus => Color::srgb(0.95, 0.95, 1.0),
        VehicleKind::Truck => Color::srgb(1.0, 0.9, 0.8),
        VehicleKind::Train => Color::srgb(0.86, 1.0, 0.86),
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
        let dir = vehicle_dir(v);
        let vh = tile_min_z(&sim.state.map, v.pos);
        let p = iso(v.pos.x, v.pos.y);

        let (xrel, yrel, w, h) = vehicle_sprite_bounds(dir);
        let pos3 = overlay_pos(p, xrel, yrel, w, h, vh, 1.0, v.pos.x, v.pos.y);
        transform.translation = pos3;
        sprite.image = trucks.for_dir(dir);
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
        let dir = vehicle_dir(v);
        let vh = tile_min_z(&sim.state.map, v.pos);
        let p = iso(v.pos.x, v.pos.y);
        let (xrel, yrel, w, h) = vehicle_sprite_bounds(dir);
        let pos3 = overlay_pos(p, xrel, yrel, w, h, vh, 1.0, v.pos.x, v.pos.y);
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
    use openttdrs_core::{GameState, TileCoord, VehicleKind};

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
            orders: Vec::new(),
            current_order: 0,
        }
    }

    #[test]
    fn vehicle_index_and_direction_helpers_work() {
        let mut idx = VehicleIndex::default();
        let v = sample_vehicle(7);
        idx.rebuild(std::slice::from_ref(&v));
        assert_eq!(idx.by_id.get(&7), Some(&0));
        assert!(matches!(vehicle_dir(&v), VehicleDir::Se));
        assert_ne!(
            vehicle_sprite_bounds(VehicleDir::Ne),
            vehicle_sprite_bounds(VehicleDir::Se)
        );
        assert_eq!(vehicle_cargo_label(&v), "ANY 0/30");
        assert_ne!(
            vehicle_cargo_color(&v),
            vehicle_cargo_color(&Vehicle { cargo: 5, ..v })
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
        world.insert_resource(TruckHandles {
            ne: Handle::default(),
            se: Handle::default(),
            sw: Handle::default(),
            nw: Handle::default(),
        });
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
        world.spawn((
            VehicleSprite(99),
            Transform::default(),
            Sprite::default(),
            Visibility::Visible,
        ));

        world.run_system_once(rebuild_vehicle_index).unwrap();
        world.run_system_once(update_vehicles).unwrap();

        let mut labels = world.query_filtered::<&Text2d, With<VehicleCargoLabel>>();
        assert_eq!(labels.single(&world).unwrap().to_string(), "ANY 0/30");
    }
}
