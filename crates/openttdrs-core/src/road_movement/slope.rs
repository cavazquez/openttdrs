//! `RoadZPosAffectSpeed` — corrección de velocidad por pendiente en carretera.

use crate::map::{Map, slope_pixel_z};
use crate::road_movement::traffic::is_road_vehicle_kind;
use crate::vehicle::Vehicle;

const REVERSING_TRACKDIRS: [u8; 4] = [6, 7, 14, 15];

/// Factor al subir (`cur_speed * 232 / 256`, ≈ −10 %).
pub const ROAD_Z_UP_NUM: u32 = 232;
pub const ROAD_Z_UP_DEN: u32 = 256;
/// Empuje al bajar (`+2`), acotado por el techo de vía.
pub const ROAD_Z_DOWN_BOOST: u16 = 2;

/// Techo equivalente a `RoadVehicle::GetCurrentMaxSpeed` para el estado actual.
#[must_use]
pub fn current_road_max_speed(v: &Vehicle, map: Option<&Map>) -> u16 {
    let engine_speed = v.effective_engine().max_speed;
    current_road_max_speed_for_engine(v, map, engine_speed)
}

/// Como [`current_road_max_speed`], consultando CB36 para el techo del motor.
///
/// Se mantiene la variante inmutable para callers de diagnóstico/render que
/// no pueden escribir los registros persistentes del vehículo; el controlador
/// de movimiento usa esta versión mutable.
pub fn current_road_max_speed_with_callbacks(v: &mut Vehicle, map: Option<&Map>) -> u16 {
    current_road_max_speed_with_callbacks_in_catalog(v, map, &[])
}

/// Variante de [`current_road_max_speed_with_callbacks`] que resuelve el
/// motor contra el catálogo activo de la partida (incluidos ids Action0).
pub fn current_road_max_speed_with_callbacks_in_catalog(
    v: &mut Vehicle,
    map: Option<&Map>,
    engine_catalog: &[crate::engine::EngineDef],
) -> u16 {
    let engine = crate::newgrf_callback::engine_for_vehicle_catalog(engine_catalog, v);
    let engine_speed = crate::newgrf_callback::vehicle_max_speed(engine, v);
    current_road_max_speed_for_engine(v, map, engine_speed)
}

fn current_road_max_speed_for_engine(v: &Vehicle, map: Option<&Map>, engine_speed: u16) -> u16 {
    let mut max_speed = if v.cached_max_track_speed > 0 {
        v.cached_max_track_speed.min(engine_speed)
    } else {
        engine_speed
    };

    let trackdir = v.road_state & crate::road_movement::rvsb::RVSB_TRACKDIR_MASK;
    if REVERSING_TRACKDIRS.contains(&trackdir) {
        max_speed /= 2;
    } else if v.direction & 1 == 0 {
        max_speed = max_speed.saturating_mul(3) / 4;
    }

    if let Some(map) = map
        && let Some(cap) = crate::bridge_spec::bridge_max_speed_for_tile(map, v.pos)
    {
        max_speed = max_speed.min(cap);
    }
    if let Some(order) = v.current_order_ref() {
        let cap = order.max_speed_limit();
        if cap > 0 {
            max_speed = max_speed.min(cap);
        }
    }
    max_speed.max(1)
}

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
    sync_road_slope_speed_with_catalog(v, map, &[]);
}

/// Variante de [`sync_road_slope_speed`] que resuelve CB36 contra el
/// catálogo activo de la partida (incluidos motores creados por Action0).
pub fn sync_road_slope_speed_with_catalog(
    v: &mut Vehicle,
    map: &Map,
    engine_catalog: &[crate::engine::EngineDef],
) {
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
    let engine = crate::newgrf_callback::engine_for_vehicle_catalog(engine_catalog, v);
    let mut max_speed = crate::newgrf_callback::vehicle_max_speed(engine, v);
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
    use crate::map::TileCoord;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::vehicle::{DIR_E, DIR_NE, VehicleKind};

    fn callback_literal(value: u16) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::from(value),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn road_vehicle() -> Vehicle {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.cached_max_track_speed = 120;
        v.direction = DIR_NE;
        v.road_state = 0;
        v
    }

    #[test]
    fn current_max_speed_uses_track_cap_on_straight() {
        assert_eq!(current_road_max_speed(&road_vehicle(), None), 112);
    }

    #[test]
    fn current_max_speed_caps_curves_at_three_quarters() {
        let mut v = road_vehicle();
        v.direction = DIR_E;
        assert_eq!(current_road_max_speed(&v, None), 84);
    }

    #[test]
    fn current_max_speed_caps_reversing_trackdir_at_half() {
        let mut v = road_vehicle();
        v.road_state = 6;
        assert_eq!(current_road_max_speed(&v, None), 56);
    }

    #[test]
    fn current_max_speed_uses_lowest_order_cap() {
        let mut v = road_vehicle();
        v.set_station_orders(vec![TileCoord::new(1, 0)]);
        v.orders[0] = v.orders[0].with_max_speed(80);
        assert_eq!(current_road_max_speed(&v, None), 80);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn current_max_speed_uses_active_catalog_and_cb36() {
        let mut engine = crate::engine::engine_by_id(crate::engine::ENGINE_BUS_MPS)
            .unwrap()
            .clone();
        engine.id = crate::engine::NEWGRF_ENGINE_ID_BASE;
        engine.newgrf_grfid = 0x524F_4144;
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(callback_literal(44)));
        let catalog = vec![engine];
        let mut v = road_vehicle();
        v.engine_id = Some(crate::engine::NEWGRF_ENGINE_ID_BASE);

        assert_eq!(
            current_road_max_speed_with_callbacks_in_catalog(&mut v, None, &catalog),
            44
        );
    }

    #[test]
    fn uphill_slows_by_232_over_256() {
        assert_eq!(road_z_pos_affect_speed(256, 0, 8, 500), 232);
    }

    #[test]
    fn downhill_adds_two_within_cap() {
        assert_eq!(road_z_pos_affect_speed(10, 8, 0, 500), 12);
        assert_eq!(road_z_pos_affect_speed(10, 8, 0, 11), 10);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn slope_sync_uses_active_catalog_speed_callback() {
        let mut engine = crate::engine::engine_by_id(crate::engine::ENGINE_BUS_MPS)
            .unwrap()
            .clone();
        engine.id = crate::engine::NEWGRF_ENGINE_ID_BASE + 1;
        engine.newgrf_grfid = 0x534C_4F50;
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(callback_literal(21)));
        let catalog = vec![engine];

        let map = Map::new_flat(8, 8, 0);
        let mut v = road_vehicle();
        v.engine_id = Some(crate::engine::NEWGRF_ENGINE_ID_BASE + 1);
        v.cur_speed = 20;
        v.z_pos = Some(8);

        sync_road_slope_speed_with_catalog(&mut v, &map, &catalog);

        // Con el techo CB36=21, la bajada no puede aplicar +2 (22); el
        // catálogo vanilla del autobús sí habría producido 22.
        assert_eq!(v.cur_speed, 20);
        assert_eq!(v.z_pos, Some(0));
    }
}
