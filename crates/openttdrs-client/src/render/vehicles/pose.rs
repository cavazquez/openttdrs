use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    EngineDef, extrapolate_vehicle_pose, slope_dz_at_subtile, vehicle_render_direction_at_with_map,
    vehicle_subtile_at_with_map,
};

use crate::iso::{overlay_pos, road_vehicle_tile_anchor, tile_min_z, tile_slope_and_min_z};

use super::assets::{VehicleLayerGfx, vehicle_layers};

pub(super) fn vehicle_layer(
    v: &Vehicle,
    map: Option<&Map>,
    pose: openttdrs_core::VehiclePose,
) -> &'static VehicleLayerGfx {
    let dir = vehicle_render_direction_at_with_map(v, pose, map).min(7) as usize;
    &vehicle_layers(v)[dir]
}

pub(crate) fn vehicle_draw_anchor_from_pose(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
) -> (Vec2, u8, i32, i32) {
    let (tileh, _) = tile_slope_and_min_z(map, pose.pos.x as u32, pose.pos.y as u32);
    let base_z = tile_min_z(map, pose.pos);
    let (sub_x, sub_y) = vehicle_subtile_at_with_map(v, pose, Some(map));
    let sub_z = slope_dz_at_subtile(sub_x, sub_y, tileh);
    let anchor = road_vehicle_tile_anchor(pose.pos.x, pose.pos.y, sub_x, sub_y, sub_z);
    (anchor, base_z, pose.pos.x, pose.pos.y)
}

/// Posición mundo del sprite del vehículo (para cámara de seguimiento).
#[must_use]
pub(crate) fn vehicle_world_position(v: &Vehicle, map: &Map) -> Vec3 {
    vehicle_sprite_pos(v, map, 0.0)
}

pub(crate) fn vehicle_sprite_pos_at(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
) -> Vec3 {
    vehicle_sprite_pos_at_with_catalog(v, map, pose, None)
}

pub(crate) fn vehicle_sprite_pos_at_with_catalog(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    catalog: Option<&[EngineDef]>,
) -> Vec3 {
    let dir = vehicle_render_direction_at_with_map(v, pose, Some(map)).min(7) as usize;
    let (x_offs, y_offs, w, h) = if v.kind == VehicleKind::Train
        && let Some(cat) = catalog
        && let Some(eid) = v.engine_id
        && let Some(eng) = openttdrs_core::engine_in_catalog(cat, eid)
        && let Some(view) = eng.newgrf_view(dir)
    {
        (
            f32::from(view.x_offs),
            f32::from(view.y_offs),
            f32::from(view.width),
            f32::from(view.height),
        )
    } else {
        let layer = vehicle_layer(v, Some(map), pose);
        (layer.x_offs, layer.y_offs, layer.w, layer.h)
    };
    let (anchor, height, tx, ty) = vehicle_draw_anchor_from_pose(v, map, pose);
    let height = height.saturating_add(v.altitude);
    overlay_pos(anchor, x_offs, y_offs, w, h, height, 1.0, tx, ty)
}

pub(crate) fn vehicle_sprite_pos(v: &Vehicle, map: &Map, tick_alpha: f32) -> Vec3 {
    vehicle_sprite_pos_at(v, map, extrapolate_vehicle_pose(v, tick_alpha))
}
