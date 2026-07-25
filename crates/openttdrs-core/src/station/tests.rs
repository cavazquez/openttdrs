#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod coherence_tests {
    use super::super::*;
    use crate::test_fixtures::SimHarness;
    use crate::{
        CargoType, Command, CompanyId, GameState, Industry, IndustryKind, PathNetwork, TileCoord,
        TileKind, Vehicle, VehicleKind, command::apply_command, find_path,
    };

    #[test]
    fn rail_station_stop_tile_targets_platform_not_approach() {
        use crate::command::{Command, apply_command};
        let mut state = GameState::new(16, 12);
        for x in 2..=5 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 6))).unwrap();
        }
        let station = TileCoord::new(1, 6);
        apply_command(&mut state, &Command::PlaceRailStation(station, 2)).unwrap();
        assert_eq!(
            rail_station_approach_tile(&state.map, station),
            Some(TileCoord::new(2, 6))
        );
        assert_eq!(
            rail_station_stop_tile(&state.map, station),
            Some(station),
            "destino de orden = plataforma"
        );
        assert_eq!(
            resolve_order_destination(
                &state.map,
                VehicleKind::Train,
                crate::vehicle::VehicleOrder::station(station)
            ),
            station
        );
    }

    #[test]
    fn pick_stop_tile_respects_order_stop_location() {
        use crate::vehicle::OrderStopLocation;
        let platform = vec![
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
            TileCoord::new(4, 0),
        ];
        assert_eq!(
            pick_stop_tile(&platform, OrderStopLocation::NearEnd, 8),
            TileCoord::new(1, 0)
        );
        assert_eq!(
            pick_stop_tile(&platform, OrderStopLocation::Middle, 8),
            TileCoord::new(3, 0)
        );
        assert_eq!(
            pick_stop_tile(&platform, OrderStopLocation::FarEnd, 8),
            TileCoord::new(4, 0)
        );
        // Tren más largo que el andén → FarEnd.
        assert_eq!(
            pick_stop_tile(&platform, OrderStopLocation::NearEnd, 80),
            TileCoord::new(4, 0)
        );
    }

    #[test]
    fn dual_platform_stop_prefers_approach_track() {
        use crate::command::{Command, apply_command};
        let mut state = GameState::new(20, 20);
        apply_command(
            &mut state,
            &Command::PlaceRailStationArea {
                origin: TileCoord::new(4, 4),
                axis_y: true,
                platforms: 2,
                length: 4,
            },
        )
        .unwrap();
        let anchor = state.stations[0].pos;
        let left = rail_station_stop_tile_for_approach(&state.map, anchor, TileCoord::new(4, 8));
        let right = rail_station_stop_tile_for_approach(&state.map, anchor, TileCoord::new(5, 8));
        assert_ne!(left, right, "andenes paralelos no deben compartir parada");
        assert_eq!(left.map(|c| c.x), Some(4));
        assert_eq!(right.map(|c| c.x), Some(5));
        let cands = rail_station_stop_candidates(&state.map, anchor, TileCoord::new(4, 8));
        assert_eq!(cands.first().map(|c| c.x), Some(4));
        assert!(cands.iter().any(|c| c.x == 5), "incluye andén paralelo");
    }

    #[test]
    fn stop_kind_from_m6_maps_openttd_station_types() {
        assert_eq!(stop_kind_from_m6(2 << 3), StopKind::TruckStop);
        assert_eq!(stop_kind_from_m6(3 << 3), StopKind::BusStop);
        assert_eq!(stop_kind_from_m6(0), StopKind::RailStation);
        assert_eq!(stop_kind_from_m6(7 << 3), StopKind::RailWaypoint);
    }

    #[test]
    fn station_map_coherence_flags_orphan_tile_and_state() {
        let mut state = GameState::new(6, 6);
        state
            .map
            .set_kind(TileCoord::new(1, 1), TileKind::Station)
            .unwrap();
        state.stations.push(Station::new(TileCoord::new(3, 3)));
        let report = station_map_coherence(&state);
        assert_eq!(report.tiles_without_station, vec![TileCoord::new(1, 1)]);
        assert_eq!(report.stations_without_tile, vec![TileCoord::new(3, 3)]);
    }

    #[test]
    fn place_station_dir_keeps_map_and_state_aligned() {
        let mut state = GameState::new(8, 8);
        let road = TileCoord::new(4, 5);
        let stop = TileCoord::new(4, 4);
        apply_command(&mut state, &Command::PlaceRoad(road)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(stop, 1)).unwrap();
        let report = station_map_coherence(&state);
        assert!(report.tiles_without_station.is_empty());
        assert!(report.stations_without_tile.is_empty());
        assert_eq!(state.map.get_kind(stop), Some(TileKind::Station));
        assert_eq!(state.stations.len(), 1);
        assert_eq!(state.stations[0].pos, stop);
    }

    #[test]
    fn truck_does_not_reload_coal_at_deliver_on_load_order() {
        let mut state = GameState::new(16, 12);
        let load_stop = TileCoord::new(3, 5);
        let deliver_stop = TileCoord::new(10, 5);
        let deliver_road = TileCoord::new(10, 6);
        for x in 2..=12_i32 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(load_stop, 1)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();
        let deliver_idx = state
            .stations
            .iter()
            .position(|s| s.pos == deliver_stop)
            .expect("parada descarga");
        state.stations[deliver_idx].cargo_stock.coal = 20;

        let mut truck = Vehicle::new(9010, VehicleKind::Truck, deliver_road, load_stop);
        truck.running = true;
        truck.set_station_orders(vec![load_stop, deliver_stop]);
        truck.sync_order_destination(&state.map);
        state.vehicles.push(truck);

        state.step();
        assert_eq!(
            state.vehicles[0].cargo, 0,
            "orden de carga en mina: no tomar carbón en parada de descarga"
        );
    }

    #[test]
    fn truck_unloads_from_road_tile_adjacent_to_stop() {
        let mut state = GameState::new(16, 12);
        let load_road = TileCoord::new(3, 6);
        let load_stop = TileCoord::new(3, 5);
        let deliver_road = TileCoord::new(10, 6);
        let deliver_stop = TileCoord::new(10, 5);
        for x in 2..=12_i32 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(load_stop, 1)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();
        assert_eq!(
            road_stop_approach_tile(&state.map, load_stop),
            Some(load_road)
        );
        assert_eq!(
            road_stop_approach_tile(&state.map, deliver_stop),
            Some(deliver_road)
        );

        let mut mine = Industry::new(TileCoord::new(2, 3), IndustryKind::CoalMine);
        mine.stock = 64;
        state.industries.push(mine);

        let mut truck = Vehicle::new(9010, VehicleKind::Truck, load_road, load_stop);
        truck.running = true;
        truck.set_station_orders(vec![load_stop, deliver_stop]);
        truck.sync_order_destination(&state.map);
        assert_eq!(
            truck.dest, load_stop,
            "entra a la tesela de la bahía (Fase 2), no para en el acceso"
        );
        if let Some(path) = find_path(&state.map, load_road, truck.dest, PathNetwork::Road) {
            truck.path = path.into();
        }
        state.vehicles.push(truck);

        for t in 1..=2_000 {
            state.step();
            if state.stats.cargo_units_delivered > 0 && state.vehicles[0].cargo == 0 {
                assert!(
                    !state.vehicles[0].cargo_transfer_active(),
                    "sin recarga tras completar la entrega (t={t})"
                );
                return;
            }
        }
        panic!("camión debe descargar en parada de destino");
    }

    #[test]
    fn truck_does_not_pick_up_wood_at_deliver_stop_after_unload() {
        let mut state = GameState::new(16, 12);
        let load_stop = TileCoord::new(3, 5);
        let deliver_stop = TileCoord::new(10, 5);
        let deliver_road = TileCoord::new(10, 6);
        for x in 2..=12_i32 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(load_stop, 1)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();
        let deliver_idx = state
            .stations
            .iter()
            .position(|s| s.pos == deliver_stop)
            .expect("parada descarga");
        state.stations[deliver_idx].cargo_stock.wood = 160;

        // Fase 2: el camión descarga DENTRO de la bahía, no en el acceso.
        let mut truck = Vehicle::new(9010, VehicleKind::Truck, deliver_stop, deliver_stop);
        truck.running = true;
        truck.direction = crate::vehicle::DIR_NW;
        truck.cargo_type = Some(CargoType::Coal);
        truck.cargo = 20;
        truck.mark_cargo_loaded(TileCoord::new(2, 3));
        truck.ensure_packets_from_legacy();
        truck.set_station_orders(vec![load_stop, deliver_stop]);
        truck.current_order = 1;
        truck.sync_order_destination(&state.map);
        truck.progress = 255;
        state.vehicles.push(truck);
        let _ = deliver_road;

        SimHarness::until_vehicle_cargo(&mut state, 0, 0, 16);
        assert_eq!(
            state.vehicles[0].cargo, 0,
            "debe descargar carbón en parada de entrega"
        );
        assert_eq!(
            state.vehicles[0].cargo_type, None,
            "sin recargar madera de stock de fábrica en el mismo tick"
        );
        assert_eq!(
            state.vehicles[0].current_order, 0,
            "tras entregar, la orden activa debe ser la de carga en mina"
        );
    }

    #[test]
    fn truck_unloads_wood_at_deliver_even_when_cargo_source_is_station() {
        let mut state = GameState::new(16, 12);
        let deliver_stop = TileCoord::new(10, 5);
        let deliver_road = TileCoord::new(10, 6);
        apply_command(
            &mut state,
            &Command::PlaceRoadBits(TileCoord::new(10, 6), 0x0A),
        )
        .unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();

        // Fase 2: el camión descarga DENTRO de la bahía, no en el acceso.
        let mut truck = Vehicle::new(9010, VehicleKind::Truck, deliver_stop, deliver_stop);
        truck.running = true;
        truck.direction = crate::vehicle::DIR_NW;
        truck.cargo_type = Some(CargoType::Wood);
        truck.cargo = 20;
        truck.mark_cargo_loaded(deliver_stop);
        truck.ensure_packets_from_legacy();
        truck.set_station_orders(vec![deliver_stop]);
        truck.sync_order_destination(&state.map);
        truck.progress = 255;
        state.vehicles.push(truck);
        let _ = deliver_road;

        SimHarness::until_vehicle_cargo(&mut state, 0, 0, 16);
        assert_eq!(
            state.vehicles[0].cargo, 0,
            "parada de entrega debe aceptar descarga aunque cargo_source sea la misma tesela"
        );
    }

    fn sweep(station: &mut Station, times: usize, selectgoods: bool) {
        let mut rng = crate::cargodist::parity::Randomizer::new(1);
        for _ in 0..times {
            update_station_ratings(std::slice::from_mut(station), selectgoods, &mut rng);
        }
    }

    #[test]
    fn station_rating_decays_with_waiting_cargo() {
        let mut station = Station::new(TileCoord::new(0, 0));
        station.cargo_stock.coal = 50;
        station.ensure_packets_from_stock();
        sweep(&mut station, 1, true);
        assert_eq!(station.time_since_pickup.coal, 1);
        assert_eq!(
            station.rating,
            INITIAL_STATION_RATING - 2,
            "sin ningún vehículo que la sirva la estación ya empieza a bajar"
        );

        sweep(&mut station, 253, true);
        assert_eq!(station.time_since_pickup.coal, 254);
        assert!(
            station.rating < INITIAL_STATION_RATING,
            "254 barridos sin recoger tienen que hundir el rating"
        );

        // Barrido 255: la carga caduca y el tipo pierde su rating.
        sweep(&mut station, 1, true);
        assert_eq!(station.cargo_stock.coal, 0);
        assert!(station.cargo_packets.is_empty());
        assert_eq!(station.time_since_pickup.coal, 0);
        assert!(!station.goods.get(CargoType::Coal).has_rating);
        assert_eq!(station.rating, INITIAL_STATION_RATING);
    }

    #[test]
    fn station_cargo_truncate_at_max_pickup_age() {
        let mut station = Station::new(TileCoord::new(1, 1));
        station.add_waiting_cargo(CargoType::Wood, 40);
        station.time_since_pickup.set(CargoType::Wood, 254);
        sweep(&mut station, 1, true);
        assert_eq!(station.cargo_stock.wood, 0);
        station.add_waiting_cargo(CargoType::Wood, 10);
        assert_eq!(station.time_since_pickup.wood, 0);
        assert_eq!(station.cargo_stock.wood, 10);
    }

    /// Una estación recién construida no rinde como perfecta: nace en 175 y solo sube si
    /// alguien la sirve (`INITIAL_STATION_RATING`, `station_base.h:23`).
    #[test]
    fn new_station_starts_at_initial_rating() {
        let station = Station::new(TileCoord::new(2, 2));
        assert_eq!(station.rating, INITIAL_STATION_RATING);
        assert_eq!(
            station_rating_for_cargo(&station, CargoType::Coal),
            INITIAL_STATION_RATING
        );
        assert_eq!(INITIAL_STATION_RATING, 175);
    }

    /// Servir con material rápido y nuevo sube el objetivo, pero el rating solo se mueve de
    /// dos en dos por barrido: hay que sostener el servicio para llegar arriba.
    #[test]
    fn good_service_raises_rating_two_points_per_sweep() {
        let mut station = Station::new(TileCoord::new(3, 3));
        let express = StationVisit {
            vehicle_kind: crate::vehicle::VehicleKind::Train,
            last_speed: 200,
            last_age: 0,
        };
        // La industria deja carga y el tren se lleva casi toda: poca cola acumulada.
        let serve = |station: &mut Station| {
            station.add_waiting_cargo(CargoType::Coal, 20);
            let _ = station.take_waiting_cargo(CargoType::Coal, 18);
            on_station_cargo_pickup(station, CargoType::Coal, CompanyId::PLAYER, express);
        };

        serve(&mut station);
        let before = station_rating_for_cargo(&station, CargoType::Coal);
        assert_eq!(before, INITIAL_STATION_RATING);
        sweep(&mut station, 1, true);
        assert_eq!(
            station_rating_for_cargo(&station, CargoType::Coal),
            before + 2,
            "convergencia de ±2 por barrido"
        );

        for _ in 0..30 {
            serve(&mut station);
            sweep(&mut station, 1, true);
        }
        let sustained = station_rating_for_cargo(&station, CargoType::Coal);
        assert!(
            sustained > 220,
            "servicio sostenido con material rápido y nuevo debe acercarse al máximo, fue {sustained}"
        );

        // Al dejar de servirla, la misma estación se desinfla al mismo ritmo.
        sweep(&mut station, 30, true);
        assert!(station_rating_for_cargo(&station, CargoType::Coal) < sustained);
    }

    /// Sin haber movido nunca esa carga, el rating sube de uno en uno y se queda en 175.
    #[test]
    fn unserved_cargo_creeps_back_to_initial_rating() {
        let mut station = Station::new(TileCoord::new(4, 4));
        station.goods.get_mut(CargoType::Wood).rating = 100;
        sweep(&mut station, 1, true);
        assert_eq!(station_rating_for_cargo(&station, CargoType::Wood), 101);
        sweep(&mut station, 200, true);
        assert_eq!(
            station_rating_for_cargo(&station, CargoType::Wood),
            INITIAL_STATION_RATING,
            "sin servicio el rating no pasa del inicial"
        );
    }

    #[test]
    fn load_amount_for_rating_scales_down() {
        assert_eq!(load_amount_for_rating(100, 255), 100);
        assert_eq!(load_amount_for_rating(100, 128), 50);
        assert_eq!(load_amount_for_rating(100, 0), 0);
    }

    /// El rating es de la estación, igual para todas las compañías, como en `OpenTTD`.
    /// Lo que sí se sigue midiendo por separado es cuánto hace que cada una no recoge, que
    /// es el dato que necesitará el reparto de producción.
    #[test]
    fn rating_is_shared_but_pickup_delay_is_per_company() {
        let mut station = Station::new(TileCoord::new(0, 0));
        station.add_waiting_cargo(CargoType::Passengers, 20);
        sweep(&mut station, 140, false);
        assert!(
            station_rating_for_company_cargo(&station, CompanyId::PLAYER, CargoType::Passengers)
                < TOWN_CARGO_MIN_OWNER_RATING,
            "nadie sirve la estación: el rating cae para todos"
        );

        let rival = CompanyId(1);
        on_station_cargo_pickup(
            &mut station,
            CargoType::Passengers,
            rival,
            StationVisit {
                vehicle_kind: crate::vehicle::VehicleKind::Bus,
                last_speed: 90,
                last_age: 1,
            },
        );
        assert_eq!(
            station.company_pickup_days(rival, CargoType::Passengers),
            0,
            "el rival acaba de recoger"
        );
        assert!(
            station.company_pickup_days(CompanyId::PLAYER, CargoType::Passengers) > 100,
            "el dueño no se beneficia de la recogida rival"
        );
        assert_eq!(
            station_rating_for_company_cargo(&station, rival, CargoType::Passengers),
            station_rating_for_company_cargo(&station, CompanyId::PLAYER, CargoType::Passengers)
        );
    }
}
