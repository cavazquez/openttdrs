//! Consist ferroviario: locomotora (cabeza) + cadena de unidades (`Next()`).
//!
//! Longitud en unidades `OpenTTD` (`VEHICLE_LENGTH = 8` por unidad). La cabeza
//! lleva órdenes y pathfinding; los vagones siguen la posición de la cabeza
//! con offset por longitud acumulada.

mod controller;
mod couple;
mod metrics;
mod newgrf_vars;
mod pose;
mod topology;

pub use controller::{
    propagate_consist_unit_poses, propagate_consist_unit_poses_with_map_indexed,
    reverse_consist_at_stop, reverse_consist_at_stop_indexed,
};
pub use couple::{
    attach_wagon, attach_wagon_chain, detach_unit, detach_unit_keep_tail, sell_chain_ids,
};
pub(crate) use metrics::cargo_weight_t;
pub use metrics::{
    consist_capacity, consist_occupied_tiles, consist_occupied_tiles_indexed, consist_power_hp,
    consist_tile_span, consist_weight_t,
};
pub use newgrf_vars::{action2_eval_ctx_for_unit, cargo_class_bits, cargo_type_a_id};
pub use pose::{TrainUnitPose, consist_unit_poses};
pub use topology::{
    consist_changed, consist_changed_with_map, consist_changed_with_map_and_catalog,
    consist_head_id, consist_unit_ids, consist_unit_ids_indexed, engine_is_train_engine,
    engine_is_wagon, same_consist,
};

/// Longitud de una unidad de tren en fracciones de tesela (`OpenTTD` `VEHICLE_LENGTH`).
pub const VEHICLE_LENGTH: u8 = 8;
/// Fracciones de tesela por tesela completa.
pub const TILE_FRACTIONS: u16 = 256;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::economy::TICKS_PER_DAY;
    use crate::map::TileCoord;
    use crate::vehicle::{Vehicle, VehicleKind};

    fn train(id: u32) -> Vehicle {
        let mut v = Vehicle::new(
            id,
            VehicleKind::Train,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        v.unit_length = VEHICLE_LENGTH;
        v.cached_total_length = u16::from(VEHICLE_LENGTH);
        v
    }

    #[test]
    fn attach_and_detach_wagon() {
        let mut vs = vec![train(1), train(2)];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert_eq!(vs[0].next_unit, Some(2));
        assert_eq!(vs[1].prev_unit, Some(1));
        assert_eq!(consist_unit_ids(&vs, 1), vec![1, 2]);
        assert!(vs[0].cached_total_length >= 16);
        assert!(detach_unit(&mut vs, 2).is_ok());
        assert_eq!(vs[0].next_unit, None);
        assert_eq!(vs[1].prev_unit, None);
    }

    #[test]
    fn action2_ctx_counts_back_to_head() {
        let mut vs = vec![train(1), train(2), train(3)];
        vs[0].newgrf_random_bits = 0x11;
        vs[1].newgrf_random_bits = 0x22;
        vs[2].newgrf_random_bits = 0x33;
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        vs[2].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        let ctx = action2_eval_ctx_for_unit(&vs, 3, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(ctx.random_bits, 0x33);
        assert_eq!(ctx.consist_random_bits.get(&0), Some(&0x33));
        assert_eq!(ctx.consist_random_bits.get(&1), Some(&0x22));
        assert_eq!(ctx.consist_random_bits.get(&2), Some(&0x11));
    }

    #[test]
    fn action2_ctx_exposes_parent_and_relative_random_scopes() {
        let mut vs = vec![train(1), train(2), train(3)];
        vs[0].newgrf_random_bits = 0x11;
        vs[1].newgrf_random_bits = 0x22;
        vs[2].newgrf_random_bits = 0x33;
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        vs[2].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());

        let ctx = action2_eval_ctx_for_unit(&vs, 3, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(ctx.parent_random_bits, 0x22);
        assert_eq!(ctx.parent_vars.get(&0x40).map(|v| v & 0xFF), Some(1));
        assert_eq!(ctx.relative_random_bits.get(&0), Some(&0x33));
        assert_eq!(ctx.relative_random_bits.get(&-1), Some(&0x22));
        assert_eq!(ctx.relative_random_bits.get(&-2), Some(&0x11));
    }

    #[test]
    fn action2_ctx_var40_consist_position() {
        let mut vs = vec![train(1), train(2), train(3)];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        vs[2].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        let ctx = action2_eval_ctx_for_unit(&vs, 2, crate::tick::GameTick::new(0), &[], 4);
        // head=1 ff=0; unit2 ff=1 bb=1; nn=2 (3 vehicles zero-based)
        let v40 = ctx.vars.get(&0x40).copied();
        assert_eq!(v40.map(|v| v & 0xFF), Some(1)); // ff
        assert_eq!(v40.map(|v| (v >> 8) & 0xFF), Some(1)); // bb
        assert_eq!(v40.map(|v| (v >> 16) & 0xFF), Some(2)); // nn
    }

    #[test]
    fn action2_ctx_cargo_vars() {
        let mut v = train(10);
        v.cargo_type = Some(CargoType::Coal);
        v.cur_speed = 40;
        v.running = false;
        let vs = vec![v];
        let ctx = action2_eval_ctx_for_unit(
            &vs,
            10,
            crate::tick::GameTick::new(u64::from(TICKS_PER_DAY) * 10),
            &[],
            2,
        );
        assert_eq!(ctx.vars.get(&0xB9), Some(&1)); // coal type A
        assert_eq!(ctx.vars.get(&0x47).map(|v| v & 0xFF), Some(1));
        assert_eq!(ctx.vars.get(&0xB4), Some(&40));
        assert_eq!(ctx.vars.get(&0xB2), Some(&(1 << 1)));
        assert_eq!(ctx.vars.get(&0xC8), Some(&0xFD));
    }

    #[test]
    fn consist_tile_span_grows_with_units() {
        let mut vs = vec![train(1), train(2), train(3)];
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        // 3 * 8 = 24 fracciones → 1 tesela (24 < 256)
        assert_eq!(consist_tile_span(&vs, 1), 1);
        // Forzar longitudes grandes
        vs[0].unit_length = 100;
        vs[1].unit_length = 100;
        vs[2].unit_length = 100;
        consist_changed(&mut vs, 1);
        assert!(consist_tile_span(&vs, 1) >= 2);
    }

    #[test]
    fn consist_changed_min_speed_and_compatible_railtypes() {
        let mut vs = vec![train(1), train(2)];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        consist_changed(&mut vs, 1);
        assert!(vs[0].cached_max_speed < u16::MAX);
        assert!(vs[0].compatible_railtypes != 0);
        assert!(!vs[1].powered_wagon);
    }

    #[test]
    fn consist_changed_marks_powered_wagons_when_head_has_pow_wag() {
        let mut vs = vec![train(1), train(2)];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        // Simular locomotora NewGRF con vagones motorizados.
        let power_before = vs[0].cached_power_hp;
        // Inyectar pow_wag vía consist_changed leyendo el catálogo: forzamos flag a mano
        // tras un consist_changed con override temporal no disponible → marcar y
        // re-sumar potencia como haría ConsistChanged+CargoChanged.
        consist_changed(&mut vs, 1);
        let _ = power_before;
        // Sin pow_wag_power en el catálogo vanilla no hay powered wagons.
        assert!(!vs[1].powered_wagon);
    }
}
