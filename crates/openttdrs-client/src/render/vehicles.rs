use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::Vehicle;

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
                ..default()
            },
            Transform::from_translation(pos3).with_scale(Vec3::splat(TRUCK_SCALE)),
        ));
    }
}

#[derive(Component)]
pub(crate) struct VehicleSprite(u32);

pub(crate) fn update_vehicles(
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

        let (xrel, yrel, w, h) = vehicle_sprite_bounds(dir);
        let pos3 = overlay_pos(p, xrel, yrel, w, h, vh, 1.0, v.pos.x, v.pos.y);
        transform.translation = pos3;
        sprite.image = trucks.for_dir(dir);
    }
}
