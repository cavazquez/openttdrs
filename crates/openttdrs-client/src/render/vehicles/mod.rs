mod assets;
mod picking;
mod plugin;
mod pose;
mod spawn;
mod sync;

use openttdrs_core::EngineDef;

use crate::state::SimWorld;

pub(crate) use assets::{NewGrfTrainSpriteCache, TruckHandles};
pub(crate) use picking::pick_vehicle_id_at_world;
pub(crate) use plugin::VehicleRenderPlugin;
pub(crate) use pose::{vehicle_draw_anchor_from_pose, vehicle_sprite_pos_at, vehicle_world_position};
pub(crate) use spawn::spawn_initial_vehicles;
pub(crate) use sync::VehicleIndex;

fn engine_in_sim(sim: &SimWorld, engine_id: u16) -> Option<&EngineDef> {
    openttdrs_core::engine_in_catalog(&sim.state.engine_catalog, engine_id)
        .or_else(|| openttdrs_core::engine_by_id(engine_id))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use openttdrs_core::{DIR_S, DIR_SW, GameState, TileCoord, TileKind, Vehicle, VehicleKind};

    use assets::{vehicle_layers, TruckHandles};
    use assets::vehicle_gfx::{BUS_VEHICLE_LAYERS, TRAIN_VEHICLE_LAYERS};
    use picking::pick_vehicle_id_at_world;
    use pose::{vehicle_sprite_pos, vehicle_sprite_pos_at, vehicle_sprite_pos_at_with_catalog};
    use spawn::{vehicle_cargo_color, vehicle_cargo_label};
    use sync::{rebuild_vehicle_index, update_vehicles, vehicle_tint, VehicleIndex};

    fn sample_vehicle(id: u32) -> Vehicle {
        let dest = TileCoord::new(2, 1);
        let mut v = Vehicle::new(id, VehicleKind::Truck, TileCoord::new(1, 1), dest);
        v.path = VecDeque::from([dest]);
        v.direction = DIR_SW;
        v.engine_id = Some(openttdrs_core::ENGINE_TRUCK_MPS);
        v.cur_speed = 96;
        v
    }

    fn default_handles() -> TruckHandles {
        TruckHandles {
            bus: Default::default(),
            bus_loaded: Default::default(),
            truck: Default::default(),
            truck_loaded: Default::default(),
            ship: Default::default(),
            ship_oil: Default::default(),
            ship_coal: Default::default(),
            ship_ferry: Default::default(),
            aircraft: Default::default(),
            aircraft_fokker: Default::default(),
            aircraft_tricario: Default::default(),
            train_groups: Default::default(),
        }
    }

    #[test]
    fn pick_vehicle_prefers_closest_sprite() {
        let mut sim = SimWorld {
            state: openttdrs_core::GameState::new(16, 16),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let on_road = TileCoord::new(4, 4);
        sim.state
            .map
            .set_kind(on_road, TileKind::Road)
            .expect("road tile");
        sim.state
            .vehicles
            .push(Vehicle::new(42, VehicleKind::Bus, on_road, on_road));
        let anchor = vehicle_sprite_pos(&sim.state.vehicles[0], &sim.state.map, 0.0).truncate();
        assert_eq!(pick_vehicle_id_at_world(anchor, &sim), Some(42));
        assert_eq!(
            pick_vehicle_id_at_world(anchor + Vec2::new(200.0, 0.0), &sim),
            None
        );
    }

    #[test]
    fn vehicle_index_and_sprite_helpers_work() {
        let mut idx = VehicleIndex::default();
        let v = sample_vehicle(7);
        idx.rebuild(std::slice::from_ref(&v));
        assert_eq!(idx.by_id.get(&7), Some(&0));
        assert_eq!(v.render_direction(), DIR_SW);
        assert_ne!(vehicle_layers(&v)[1].path, vehicle_layers(&v)[3].path);
        assert!(!v.uses_loaded_road_sprite());
        let mut loaded = sample_vehicle(1);
        loaded.cargo = 15;
        assert!(loaded.uses_loaded_road_sprite());
        let mut empty_bus = sample_vehicle(2);
        empty_bus.kind = VehicleKind::Bus;
        let mut loaded_bus = sample_vehicle(3);
        loaded_bus.kind = VehicleKind::Bus;
        loaded_bus.cargo = 15;
        assert!(loaded_bus.uses_loaded_road_sprite());
        assert_ne!(
            vehicle_layers(&empty_bus)[5].path,
            vehicle_layers(&loaded_bus)[5].path
        );
    }

    #[test]
    fn vehicle_tint_amber_when_pbs_stuck() {
        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Train;
        assert_eq!(vehicle_tint(&v), Color::WHITE);
        v.pbs_stuck = true;
        assert_eq!(vehicle_tint(&v), Color::srgb(1.0, 0.75, 0.35));
    }

    #[test]
    fn stopped_train_in_rail_depot_is_hidden_from_pick() {
        let mut sim = SimWorld {
            state: openttdrs_core::GameState::new(16, 16),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let depot = TileCoord::new(5, 5);
        sim.state
            .map
            .set_kind(depot, TileKind::RailDepot)
            .expect("rail depot");
        let mut train = Vehicle::new(9, VehicleKind::Train, depot, depot);
        train.running = false;
        sim.state.vehicles.push(train);
        let anchor = vehicle_sprite_pos(&sim.state.vehicles[0], &sim.state.map, 0.0).truncate();
        let pose = openttdrs_core::extrapolate_vehicle_pose(&sim.state.vehicles[0], 0.0);
        assert!(openttdrs_core::vehicle_hidden_from_view(
            &sim.state.map,
            &sim.state.vehicles[0],
            pose.pos,
            pose.progress
        ));
        assert_eq!(pick_vehicle_id_at_world(anchor, &sim), None);
    }

    #[test]
    fn rebuild_and_update_systems_run() {
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            loaded_file: false,
            ottdmap_extras: None,
        };
        sim.state.vehicles.push(sample_vehicle(11));

        let mut world = World::new();
        world.insert_resource(sim);
        world.insert_resource(crate::simulation::SimClock::default());
        world.insert_resource(default_handles());
        world.insert_resource(crate::render::CompanyColoredSprites::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(crate::render::NewGrfTrainSpriteCache::default());
        world.init_resource::<Assets<Image>>();

        world.spawn((
            sync::VehicleSprite(11),
            Transform::default(),
            Sprite::default(),
            Visibility::Visible,
        ));
        world.spawn((
            sync::VehicleCargoLabel(11),
            Transform::default(),
            Text2d::new(""),
            TextColor(Color::WHITE),
            Visibility::Visible,
        ));

        world.run_system_once(rebuild_vehicle_index).unwrap();
        world.run_system_once(update_vehicles).unwrap();

        let mut labels = world.query_filtered::<&Text2d, With<sync::VehicleCargoLabel>>();
        assert_eq!(labels.single(&world).unwrap().to_string(), "ANY 0/20");
    }

    #[test]
    fn newgrf_train_sprite_cache_and_pos_use_decoded_views() {
        use openttdrs_core::{
            apply_newgrf_vehicles_trains, build_action0_train_payload,
            build_grf_v2_train_with_preview_sprite, extrapolate_vehicle_pose,
        };
        use crate::sprites::CompanyColour;

        let a0 = build_action0_train_payload(1960, 100, 800, "InWorld Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'I', 0, 1],
            "tinworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("tinworld.grf"), &bytes).expect("write grf");
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("tinworld.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[dir.path()]);
        let eng = state
            .engine_catalog
            .iter()
            .find(|e| e.from_newgrf)
            .expect("newgrf engine")
            .clone();
        assert!(!eng.newgrf_views.is_empty());

        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfTrainSpriteCache::default();
        let handle = cache
            .handle_for(&eng, 0, CompanyColour::DarkBlue, &mut images)
            .expect("newgrf texture");
        let handle_again = cache
            .handle_for(&eng, 3, CompanyColour::DarkBlue, &mut images)
            .expect("reuse single view");
        assert_eq!(handle, handle_again);
        assert_eq!(cache.len(), 1);

        let mut v = sample_vehicle(99);
        v.kind = VehicleKind::Train;
        v.engine_id = Some(eng.id);
        let pose = extrapolate_vehicle_pose(&v, 0.0);
        let map = &state.map;
        let pos_vanilla = vehicle_sprite_pos_at(&v, map, pose);
        let pos_newgrf =
            vehicle_sprite_pos_at_with_catalog(&v, map, pose, Some(&state.engine_catalog));
        assert_ne!(
            (pos_vanilla.x, pos_vanilla.y),
            (pos_newgrf.x, pos_newgrf.y),
            "offsets NewGRF deben mover el sprite vs OpenGFX"
        );

        let sim = SimWorld {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        };
        let trucks = default_handles();
        let selected = assets::TruckHandles::for_vehicle_with_newgrf(
            &trucks,
            &v,
            pose,
            None,
            None,
            &sim,
            &mut cache,
            &mut images,
        );
        assert_eq!(selected, handle);
    }

    #[test]
    fn train_layers_differ_from_bus() {
        assert_ne!(
            TRAIN_VEHICLE_LAYERS[DIR_SW as usize].path,
            BUS_VEHICLE_LAYERS[DIR_SW as usize].path
        );
    }

    #[test]
    fn sprite_selection_uses_extrapolated_pose_not_logical_direction() {
        use openttdrs_core::extrapolate_vehicle_pose;
        use pose::vehicle_layer;

        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Bus;
        v.pos = TileCoord::new(1, 1);
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(0, 2)]);
        v.set_cruise_speed();
        v.progress = 100;

        let logical_dir = v.render_direction().min(7) as usize;
        assert_eq!(logical_dir, openttdrs_core::DIR_NE as usize);

        let pose = extrapolate_vehicle_pose(&v, 1.0);
        assert!(
            pose.progress >= 128,
            "la extrapolación cruza el punto medio"
        );
        let render_dir =
            openttdrs_core::vehicle_render_direction_at(&v, pose).min(7) as usize;
        assert_eq!(render_dir, openttdrs_core::DIR_E as usize);

        assert_eq!(
            vehicle_layer(&v, None, pose).path,
            vehicle_layers(&v)[render_dir].path
        );
        assert_ne!(
            vehicle_layer(&v, None, pose).path,
            vehicle_layers(&v)[logical_dir].path
        );

        let handles = default_handles();
        let selected = assets::TruckHandles::for_vehicle(&handles, &v, pose, None, None);
        assert_eq!(selected, handles.bus[render_dir]);
    }

    #[test]
    fn sprite_selection_uses_extrapolated_pose_for_train() {
        use openttdrs_core::{extrapolate_vehicle_pose, vehicle_subtile_at};
        use pose::vehicle_layer;

        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Train;
        v.pos = TileCoord::new(5, 6);
        v.path = VecDeque::from([TileCoord::new(6, 6)]);
        v.direction = openttdrs_core::DIR_NE;
        v.set_cruise_speed();
        v.progress = 40;

        let logical_pose = extrapolate_vehicle_pose(&v, 0.0);
        let extrap_pose = extrapolate_vehicle_pose(&v, 1.0);
        assert!(
            extrap_pose.progress > logical_pose.progress || extrap_pose.pos != logical_pose.pos,
            "la extrapolación avanza el tren entre ticks"
        );
        let logical_sub = vehicle_subtile_at(&v, logical_pose);
        let extrap_sub = vehicle_subtile_at(&v, extrap_pose);
        assert_ne!(
            logical_sub, extrap_sub,
            "sub-tesela extrapolada distinta de la lógica"
        );
        assert_eq!(
            vehicle_layer(&v, None, extrap_pose).path,
            vehicle_layers(&v)
                [openttdrs_core::vehicle_render_direction_at(&v, extrap_pose).min(7) as usize]
                .path
        );
    }

    #[test]
    fn render_direction_cardinal_layer_differs_from_diagonal() {
        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Bus;
        v.pos = TileCoord::new(0, 0);
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(1, 1)]);
        v.progress = 200;
        assert_eq!(v.render_direction(), DIR_S);
        assert_ne!(
            BUS_VEHICLE_LAYERS[DIR_S as usize].path,
            BUS_VEHICLE_LAYERS[openttdrs_core::DIR_SE as usize].path
        );
    }

    #[test]
    fn cargo_label_formats_correctly() {
        let mut v = sample_vehicle(1);
        v.cargo_type = Some(openttdrs_core::CargoType::Coal);
        v.cargo = 5;
        v.capacity = 20;
        assert_eq!(vehicle_cargo_label(&v), "COAL 5/20");
        assert_eq!(vehicle_cargo_color(&v), Color::srgb(0.95, 0.9, 0.35));

        v.cargo = 0;
        assert_eq!(vehicle_cargo_color(&v), Color::srgba(0.8, 0.85, 0.9, 0.72));
    }
}
