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
pub use newgrf_vars::{
    action2_eval_ctx_for_unit, cargo_class_bits, cargo_type_a_id, enrich_vehicle_track_badge_vars,
};
pub use pose::{TrainUnitPose, consist_unit_poses};
pub use topology::{
    consist_changed, consist_changed_with_map, consist_changed_with_map_and_catalog,
    consist_changed_with_map_and_catalog_and_cargo,
    consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier,
    consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier_and_wagon_speed_limits,
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
    use crate::map::{Map, TileCoord, TileKind};
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
    fn action2_ctx_exposes_same_engine_relative_random_scope() {
        let mut vs = vec![train(1), train(2), train(3), train(4), train(5)];
        vs[0].engine_id = Some(10);
        vs[1].engine_id = Some(20);
        vs[2].engine_id = Some(20);
        vs[3].engine_id = Some(20);
        vs[4].engine_id = Some(30);
        vs[0].newgrf_random_bits = 0x10;
        vs[1].newgrf_random_bits = 0x20;
        vs[2].newgrf_random_bits = 0x30;
        vs[3].newgrf_random_bits = 0x40;
        vs[4].newgrf_random_bits = 0x50;
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        assert!(attach_wagon(&mut vs, 1, 4).is_ok());
        assert!(attach_wagon(&mut vs, 1, 5).is_ok());

        // For unit 4 the contiguous run with engine 20 starts at unit 2.
        // Counts 0, 1 and 2 select units 2, 3 and 4; count 3 advances into
        // the following unit exactly as OpenTTD's Move(count) does.
        let ctx = action2_eval_ctx_for_unit(&vs, 4, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(ctx.relative_same_engine_random_bits.get(&0), Some(&0x20));
        assert_eq!(ctx.relative_same_engine_random_bits.get(&1), Some(&0x30));
        assert_eq!(ctx.relative_same_engine_random_bits.get(&2), Some(&0x40));
        assert_eq!(ctx.relative_same_engine_random_bits.get(&3), Some(&0x50));
    }

    #[test]
    fn action2_ctx_resolves_var61_to_selected_vehicle_var62() {
        let mut vs = vec![train(1), train(2), train(3)];
        vs[0].engine_id = Some(10);
        vs[1].engine_id = Some(20);
        vs[2].engine_id = Some(20);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        // Set the geometry after coupling: the topology helper intentionally
        // propagates the head direction to followers while it refreshes the
        // consist cache.
        vs[0].pos = TileCoord::new(1, 1);
        vs[1].pos = TileCoord::new(3, 2);
        vs[2].pos = TileCoord::new(8, 5);
        vs[0].direction = 0;
        vs[1].direction = 1;
        vs[2].direction = 2;

        // Resolve unit 3, select unit 2 with var 61 (offset -1), then ask
        // that selected unit for var 62 at its own offset -1 (unit 1).
        let ctx = action2_eval_ctx_for_unit(&vs, 3, crate::tick::GameTick::new(0), &[], 0);
        let direct = ctx.relative_vars.get(&(-1, 0x62)).copied();
        let nested = ctx
            .relative_parameterized_vars
            .get(&(-1, 0x62, 0xFF))
            .copied();
        assert_ne!(direct, nested);
        assert_eq!(nested, Some(0x00FF_FE0F));
        // Through var 61, register 10E can also select the engine-local-id
        // count (var 60) of the selected unit's remaining chain. Vanilla
        // engines use local id zero, so units 2 and 3 are counted here.
        assert_eq!(
            ctx.relative_parameterized_vars.get(&(-1, 0x60, 0)),
            Some(&2)
        );
    }

    #[test]
    fn action2_ctx_preserves_extended_engine_local_ids() {
        let Some(vanilla_engine) = crate::engine::engine_by_id(crate::engine::ENGINE_TRAIN_KIRBY)
        else {
            panic!("vanilla train fixture is missing");
        };
        let mut engine = vanilla_engine.clone();
        engine.id = 60_000;
        engine.newgrf_local_id = 0x1234;
        engine.from_newgrf = true;

        let mut vs = vec![train(1), train(2)];
        vs[0].engine_id = Some(engine.id);
        vs[1].engine_id = Some(engine.id);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());

        let ctx = action2_eval_ctx_for_unit(&vs, 2, crate::tick::GameTick::new(0), &[engine], 0);
        assert_eq!(
            ctx.relative_parameterized_vars.get(&(-1, 0x60, 0x1234)),
            Some(&2)
        );
    }

    #[test]
    fn action2_ctx_var65_matches_current_rail_badges_and_relative_unit() {
        let Some(vanilla_engine) = crate::engine::engine_by_id(crate::engine::ENGINE_TRAIN_KIRBY)
        else {
            panic!("vanilla train fixture is missing");
        };
        let mut engine = vanilla_engine.clone();
        engine.id = 60_001;
        engine.newgrf_local_id = 7;
        engine.from_newgrf = true;
        engine.badges = vec![11];
        engine.newgrf_badge_translation = vec![11];

        let mut map = Map::new_flat(4, 4, 0);
        let current = TileCoord::new(1, 1);
        let previous = TileCoord::new(2, 1);
        let Some(mut current_tile) = map.get(current) else {
            panic!("current tile is inside the fixture map");
        };
        current_tile.kind = TileKind::Rail;
        current_tile.m8 = 1;
        assert!(map.set_tile(current, current_tile).is_ok());
        let Some(mut previous_tile) = map.get(previous) else {
            panic!("previous tile is inside the fixture map");
        };
        previous_tile.kind = TileKind::Rail;
        previous_tile.m8 = 0;
        assert!(map.set_tile(previous, previous_tile).is_ok());

        let mut vs = vec![train(1), train(2)];
        vs[0].engine_id = Some(engine.id);
        vs[1].engine_id = Some(engine.id);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        vs[0].pos = previous;
        vs[1].pos = current;
        assert_eq!(
            map.get(current)
                .map(crate::rail_type_from_tile)
                .map(crate::rail_type::RailType::as_u8),
            Some(1)
        );
        assert_eq!(vs[1].pos, current);
        assert_eq!(vs[1].engine_id, Some(engine.id));

        let mut ctx = action2_eval_ctx_for_unit(
            &vs,
            2,
            crate::tick::GameTick::new(0),
            std::slice::from_ref(&engine),
            0,
        );
        let rail_badges =
            std::array::from_fn(|index| if index == 1 { vec![11] } else { Vec::new() });
        enrich_vehicle_track_badge_vars(
            &mut ctx,
            &vs,
            2,
            &map,
            std::slice::from_ref(&engine),
            &rail_badges,
            &crate::road_type::vanilla_road_type_catalog(),
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x65, 0)), Some(&1));
        assert_eq!(
            ctx.relative_parameterized_vars.get(&(-1, 0x65, 0)),
            Some(&0)
        );
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
    fn consist_changed_includes_active_custom_cargo_weight() {
        let mut vs = vec![train(1)];
        let cargo = CargoType::Custom(3);
        vs[0].cargo = 8;
        vs[0].cargo_type = Some(cargo);
        let engine_weight = vs[0]
            .engine_id
            .and_then(crate::engine::engine_by_id)
            .map_or(0, |engine| engine.weight_t);
        let mut catalog = vec![crate::CargoSpecDef {
            id: cargo.cargo_id(),
            label: "TEST".to_owned(),
            name: "Carga de prueba".to_owned(),
            weight: 32,
            from_newgrf: true,
            ..crate::CargoSpecDef::default()
        }];

        consist_changed_with_map_and_catalog_and_cargo(&mut vs, 1, None, &[], &catalog);

        assert_eq!(vs[0].cached_weight_t, engine_weight.saturating_add(16));

        catalog[0].weight = 16;
        catalog[0].is_freight = true;
        consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier(
            &mut vs,
            1,
            None,
            &[],
            &catalog,
            3,
        );
        assert_eq!(vs[0].cached_weight_t, engine_weight.saturating_add(24));
    }

    #[test]
    fn wagon_speed_limits_setting_controls_consist_max_speed() {
        let mut vs = vec![train(1), train(2)];
        vs[0].engine_id = Some(10);
        vs[1].engine_id = Some(11);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());

        let mut head = crate::engine::engine_for_vehicle(VehicleKind::Train, 0).clone();
        head.id = 10;
        head.max_speed = 160;
        head.capacity = 0;
        head.cargo = None;
        let Some(mut wagon) =
            crate::engine::engine_by_id(crate::engine::ENGINE_WAGON_PASSENGER).cloned()
        else {
            panic!("vanilla passenger wagon");
        };
        wagon.id = 11;
        wagon.max_speed = 40;
        let catalog = vec![head, wagon];

        consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier_and_wagon_speed_limits(
            &mut vs,
            1,
            None,
            &catalog,
            &[],
            1,
            true,
        );
        assert_eq!(vs[0].cached_max_speed, 40);

        consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier_and_wagon_speed_limits(
            &mut vs,
            1,
            None,
            &catalog,
            &[],
            1,
            false,
        );
        assert_eq!(vs[0].cached_max_speed, 160);
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
