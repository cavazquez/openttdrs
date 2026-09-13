use bevy::prelude::*;
use openttdrs_core::extrapolate_vehicle_pose;
use openttdrs_core::prelude::*;

use crate::state::SimWorld;

use super::assets::{NewGrfTrainSpriteCache, NewGrfVehicleLayer, TruckHandles};
use super::pose::vehicle_sprite_geometry_at_with_catalog;

#[derive(Clone, Copy, Debug)]
struct VehiclePickBounds {
    min: Vec2,
    max: Vec2,
}

impl VehiclePickBounds {
    fn from_sprite(center: Vec2, size: Vec2) -> Self {
        let half = size * 0.5;
        Self {
            min: center - half,
            max: center + half,
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    fn contains(self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    fn center(self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
}

fn bounds_for_layer(
    vehicle: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    layer: &NewGrfVehicleLayer,
) -> VehiclePickBounds {
    let center = super::pose::vehicle_sprite_pos_at_offsets(
        vehicle,
        map,
        pose,
        f32::from(layer.x_offs),
        f32::from(layer.y_offs),
        f32::from(layer.width),
        f32::from(layer.height),
    )
    .truncate();
    VehiclePickBounds::from_sprite(
        center,
        Vec2::new(
            f32::from(layer.width).max(1.0),
            f32::from(layer.height).max(1.0),
        ),
    )
}

fn bounds_for_layers(
    vehicle: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    layers: &[NewGrfVehicleLayer],
) -> Option<VehiclePickBounds> {
    layers
        .iter()
        .map(|layer| bounds_for_layer(vehicle, map, pose, layer))
        .reduce(VehiclePickBounds::union)
}

fn vehicle_is_hidden_from_view(
    sim: &SimWorld,
    v: &Vehicle,
    pose: openttdrs_core::VehiclePose,
) -> bool {
    openttdrs_core::vehicle_hidden_from_view(&sim.state.map, v, pose.pos, pose.progress)
}

/// Vehículo visible bajo el cursor (prioriza el sprite más cercano).
#[must_use]
pub(crate) fn pick_vehicle_id_at_world(world_pos: Vec2, sim: &SimWorld) -> Option<u32> {
    pick_vehicle_id_at_world_with_assets(world_pos, sim, None, None, None)
}

/// Hit-test que comparte la capa runtime con el renderer del mapa.
///
/// El sistema de input puede ejecutarse antes de que `update_vehicles` haya
/// escrito sus transforms del frame. Resolver aquí la primera capa evita que
/// un Action2/SpriteStack desplazado sea seleccionable sólo en su posición
/// vanilla; los argumentos opcionales conservan el contrato de los tests y
/// arneses que no instalan recursos gráficos.
#[must_use]
pub(crate) fn pick_vehicle_id_at_world_with_newgrf(
    world_pos: Vec2,
    sim: &SimWorld,
    trucks: &TruckHandles,
    cache: &mut NewGrfTrainSpriteCache,
    images: &mut Assets<Image>,
) -> Option<u32> {
    pick_vehicle_id_at_world_with_assets(world_pos, sim, Some(trucks), Some(cache), Some(images))
}

fn pick_vehicle_id_at_world_with_assets(
    world_pos: Vec2,
    sim: &SimWorld,
    trucks: Option<&TruckHandles>,
    cache: Option<&mut NewGrfTrainSpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> Option<u32> {
    let mut cache = cache;
    let mut images = images;
    let mut closest = None;
    for v in &sim.state.vehicles {
        let pose = extrapolate_vehicle_pose(v, 0.0);
        if vehicle_is_hidden_from_view(sim, v, pose) {
            continue;
        }
        let bounds = match (trucks, cache.as_deref_mut(), images.as_deref_mut()) {
            (Some(trucks), Some(cache), Some(images)) => {
                let layers = trucks.for_vehicle_with_newgrf_layers(
                    v,
                    pose,
                    None,
                    Some(super::vehicle_livery_colour(sim, v)),
                    sim,
                    cache,
                    images,
                );
                bounds_for_layers(v, &sim.state.map, pose, &layers).unwrap_or_else(|| {
                    let (center, size) = vehicle_sprite_geometry_at_with_catalog(
                        v,
                        &sim.state.map,
                        pose,
                        Some(&sim.state.engine_catalog),
                    );
                    VehiclePickBounds::from_sprite(center.truncate(), size)
                })
            }
            _ => {
                let (center, size) = vehicle_sprite_geometry_at_with_catalog(
                    v,
                    &sim.state.map,
                    pose,
                    Some(&sim.state.engine_catalog),
                );
                VehiclePickBounds::from_sprite(center.truncate(), size)
            }
        };
        if !bounds.contains(world_pos) {
            continue;
        }
        let distance_sq = bounds.center().distance_squared(world_pos);
        if closest.is_none_or(|(closest_distance, _)| distance_sq < closest_distance) {
            closest = Some((distance_sq, v.id));
        }
    }
    closest.map(|(_, id)| id)
}
