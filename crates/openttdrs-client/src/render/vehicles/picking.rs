use bevy::prelude::*;
use openttdrs_core::extrapolate_vehicle_pose;
use openttdrs_core::prelude::*;

use crate::state::SimWorld;

use super::assets::{NewGrfTrainSpriteCache, TruckHandles};
use super::pose::vehicle_sprite_pos_at_with_catalog;

const VEHICLE_PICK_RADIUS_SQ: f32 = 34.0 * 34.0;

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
        let sprite_xy = match (trucks, cache.as_deref_mut(), images.as_deref_mut()) {
            (Some(trucks), Some(cache), Some(images)) => {
                super::vehicle_world_position_with_newgrf(sim, trucks, v, cache, images).truncate()
            }
            _ => vehicle_sprite_pos_at_with_catalog(
                v,
                &sim.state.map,
                pose,
                Some(&sim.state.engine_catalog),
            )
            .truncate(),
        };
        let distance_sq = sprite_xy.distance_squared(world_pos);
        if distance_sq > VEHICLE_PICK_RADIUS_SQ {
            continue;
        }
        if closest.is_none_or(|(closest_distance, _)| distance_sq < closest_distance) {
            closest = Some((distance_sq, v.id));
        }
    }
    closest.map(|(_, id)| id)
}
