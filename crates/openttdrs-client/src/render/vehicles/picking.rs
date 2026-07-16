use bevy::prelude::*;
use openttdrs_core::extrapolate_vehicle_pose;
use openttdrs_core::prelude::*;

use crate::state::SimWorld;

use super::pose::vehicle_sprite_pos;

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
    sim.state
        .vehicles
        .iter()
        .filter(|v| {
            let pose = extrapolate_vehicle_pose(v, 0.0);
            !vehicle_is_hidden_from_view(sim, v, pose)
        })
        .filter_map(|v| {
            let sprite_xy = vehicle_sprite_pos(v, &sim.state.map, 0.0).truncate();
            let dist_sq = sprite_xy.distance_squared(world_pos);
            (dist_sq <= VEHICLE_PICK_RADIUS_SQ).then_some((dist_sq, v.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}
