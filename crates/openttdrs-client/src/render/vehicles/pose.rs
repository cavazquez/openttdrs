use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    ConstructionSettings, EngineDef, extrapolate_vehicle_pose, slope_dz_at_subtile,
    vehicle_render_direction_at_with_map, vehicle_subtile_at_with_map,
};

/// Pose de render con el lado de circulación de la partida.
#[must_use]
pub(crate) fn vehicle_pose_for_construction(
    v: &Vehicle,
    tick_alpha: f32,
    construction: ConstructionSettings,
) -> openttdrs_core::VehiclePose {
    extrapolate_vehicle_pose(v, tick_alpha).with_drive_on_right(construction.road_drive_on_right())
}

use crate::iso::{overlay_pos, road_vehicle_tile_anchor, tile_min_z, tile_slope_and_min_z};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{viewport_insertion_key, viewport_source_depth};

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
    // El grafo ferroviario representa el vano de un puente como un salto entre
    // rampas (igual que OpenTTD a nivel de topología). Para dibujo no debemos
    // teletransportar el tren: mientras consume los 16 píxeles de vía, el
    // ancla recorre de forma continua las dos rampas. El estado lógico sigue
    // ocupando la rampa de entrada, por lo que PBS y colisiones permanecen
    // autoritativos durante todo el cruce.
    if v.kind == VehicleKind::Train
        && map.get_kind(pose.pos) == Some(TileKind::RailBridge)
        && let Some(other_ramp) = openttdrs_core::rail_bridge_other_end(map, pose.pos)
        && v.movement_target() == Some(other_ramp)
    {
        let t = (pose.progress_f / 255.0).clamp(0.0, 1.0);
        let start = road_vehicle_tile_anchor(pose.pos.x, pose.pos.y, 8.0, 8.0, 0.0);
        let end = road_vehicle_tile_anchor(other_ramp.x, other_ramp.y, 8.0, 8.0, 0.0);
        return (start.lerp(end, t), base_z, pose.pos.x, pose.pos.y);
    }
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
    let (x_offs, y_offs, w, h) = if let Some(cat) = catalog
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

/// Caja `Vehicle::bounds` que OpenTTD entrega a `AddSortableSpriteToDraw`.
///
/// Las coordenadas son píxeles de mundo (16 por tesela), no coordenadas de
/// pantalla. El cliente conserva la misma caja para que un vehículo pueda
/// competir con edificios, puentes y estaciones en el sorter global. El
/// `offset` de OpenTTD sólo corrige el ancla del sprite y no forma parte del
/// prisma de sorting, por eso no aparece aquí.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn vehicle_parent_bounds(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
) -> ParentSpriteBounds {
    let (sub_x, sub_y) = vehicle_subtile_at_with_map(v, pose, Some(map));
    let world_x = pose
        .pos
        .x
        .saturating_mul(16)
        .saturating_add(sub_x.round() as i32);
    let world_y = pose
        .pos
        .y
        .saturating_mul(16)
        .saturating_add(sub_y.round() as i32);
    let world_z = i32::from(tile_min_z(map, pose.pos))
        .saturating_mul(i32::from(openttdrs_core::TILE_PIXEL_HEIGHT))
        .saturating_add(i32::from(v.altitude));

    let (origin_x, origin_y, extent_x, extent_y, extent_z) = match v.kind {
        VehicleKind::Ship => match v.direction {
            openttdrs_core::DIR_NE | openttdrs_core::DIR_SW => (-16, -3, 32, 6, 6),
            openttdrs_core::DIR_SE | openttdrs_core::DIR_NW => (-3, -16, 6, 32, 6),
            _ => (-3, -3, 6, 6, 6),
        },
        VehicleKind::Aircraft => {
            let airborne = matches!(
                v.aircraft_phase,
                openttdrs_core::AircraftPhase::Takeoff
                    | openttdrs_core::AircraftPhase::Landing
                    | openttdrs_core::AircraftPhase::Flying
            );
            if airborne {
                (-1, -1, 24, 24, 5)
            } else {
                (-1, -1, 2, 2, 5)
            }
        }
        VehicleKind::Train if v.direction % 2 == 1 => {
            let length = i32::from(v.unit_length.max(1));
            match v.direction {
                openttdrs_core::DIR_NE => (-(length + 1) / 2, -1, length, 3, 6),
                openttdrs_core::DIR_NW => (-1, -(length + 1) / 2, 3, length, 6),
                openttdrs_core::DIR_SW => (-length / 2, -1, length, 3, 6),
                openttdrs_core::DIR_SE => (-1, -length / 2, 3, length, 6),
                _ => (-1, -1, 3, 3, 6),
            }
        }
        VehicleKind::Train | VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            (-1, -1, 3, 3, 6)
        }
    };
    ParentSpriteBounds::new(
        world_x + origin_x,
        world_y + origin_y,
        world_z,
        world_x + origin_x + extent_x - 1,
        world_y + origin_y + extent_y - 1,
        world_z + extent_z - 1,
    )
}

/// Clave de inserción estable para los vehículos de `ViewportAddVehicles`.
///
/// El bit alto del ordinal deja los vehículos después de los productores de
/// terreno de la misma tesela, como en el pase separado de OpenTTD. Los siete
/// bits restantes conservan un desempate reproducible entre unidades.
#[must_use]
pub(crate) fn vehicle_insertion_key(v: &Vehicle, pose: openttdrs_core::VehiclePose) -> u64 {
    viewport_insertion_key(
        u32::try_from(pose.pos.x).unwrap_or(0),
        u32::try_from(pose.pos.y).unwrap_or(0),
        0x80 | (u8::try_from(v.id & 0x7F).unwrap_or(0)),
    )
}

/// Profundidad fuente del parent de vehículo antes de aplicar el sorter.
#[must_use]
pub(crate) fn vehicle_source_depth(
    _v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    sprite_pos: Vec3,
) -> f32 {
    viewport_source_depth(
        sprite_pos.z,
        u32::try_from(pose.pos.x).unwrap_or(0),
        map.dimensions().0,
    )
}

/// Posición de una capa auxiliar de aeronave (sombra o rotor) usando los
/// offsets NFO de esa capa contra el mismo punto continuo del vehículo.
pub(super) fn aircraft_aux_sprite_pos_at(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    layer: &VehicleLayerGfx,
    airborne: bool,
    layer_z: f32,
) -> Vec3 {
    let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(v, map, pose);
    let height = if airborne {
        base_z.saturating_add(v.altitude)
    } else {
        base_z
    };
    overlay_pos(
        anchor,
        layer.x_offs,
        layer.y_offs,
        layer.w,
        layer.h,
        height,
        layer_z,
        tx,
        ty,
    )
}
