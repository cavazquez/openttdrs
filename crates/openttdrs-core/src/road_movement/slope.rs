//! `RoadZPosAffectSpeed` — corrección de velocidad por pendiente en carretera.

use crate::map::{Map, slope_pixel_z};
use crate::road_movement::traffic::is_road_vehicle_kind;
use crate::vehicle::Vehicle;

/// Factor al subir (`cur_speed * 232 / 256`, ≈ −10 %).
pub const ROAD_Z_UP_NUM: u32 = 232;
pub const ROAD_Z_UP_DEN: u32 = 256;
/// Empuje al bajar (`+2`), acotado por el techo de vía.
pub const ROAD_Z_DOWN_BOOST: u16 = 2;

/// Aplica `RoadZPosAffectSpeed` (`roadveh_cmd.cpp:859-868`).
#[must_use]
pub fn road_z_pos_affect_speed(
    cur_speed: u16,
    old_z: i16,
    new_z: i16,
    max_track_speed: u16,
) -> u16 {
    if old_z == new_z {
        return cur_speed;
    }
    if old_z < new_z {
        u16::try_from((u32::from(cur_speed) * ROAD_Z_UP_NUM) / ROAD_Z_UP_DEN).unwrap_or(cur_speed)
    } else {
        let spd = cur_speed.saturating_add(ROAD_Z_DOWN_BOOST);
        if spd <= max_track_speed {
            spd
        } else {
            cur_speed
        }
    }
}

/// Sincroniza `z_pos` y corrige `cur_speed` tras un tick de carretera.
pub fn sync_road_slope_speed(v: &mut Vehicle, map: &Map) {
    if !is_road_vehicle_kind(v.kind) {
        return;
    }
    let (sub_x, sub_y) = crate::road_movement::vehicle_subtile(v);
    let new_z = slope_pixel_z(map, v.pos, sub_x, sub_y);
    let Some(old_z) = v.z_pos else {
        v.z_pos.replace(new_z);
        return;
    };
    v.z_pos = Some(new_z);
    let mut max_speed = v.effective_engine().max_speed;
    if let Some(cap) = crate::bridge_spec::bridge_max_speed_for_tile(map, v.pos) {
        max_speed = max_speed.min(cap);
    }
    // Techo de vía cacheado si existe; si no, el del motor.
    let track_cap = if v.cached_max_track_speed > 0 {
        v.cached_max_track_speed.min(max_speed)
    } else {
        max_speed
    };
    v.cur_speed = road_z_pos_affect_speed(v.cur_speed, old_z, new_z, track_cap);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uphill_slows_by_232_over_256() {
        assert_eq!(road_z_pos_affect_speed(256, 0, 8, 500), 232);
    }

    #[test]
    fn downhill_adds_two_within_cap() {
        assert_eq!(road_z_pos_affect_speed(10, 8, 0, 500), 12);
        assert_eq!(road_z_pos_affect_speed(10, 8, 0, 11), 10);
    }
}
