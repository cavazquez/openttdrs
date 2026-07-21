//! Tests de aeropuerto, canal y esclusa.

use crate::{
    AircraftPhase, Command, DEPOT_BUILD_COST, ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_TRICARIO,
    ENGINE_SHIP_FERRY, GameState, STATION_BUILD_COST, StopKind, TileCoord, TileKind, VehicleKind,
    airport_tile_is_hangar, airport_tile_is_heliport, apply_command,
};

#[test]
fn place_heliport_and_buy_helicopter() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceAirport(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Airport));
    assert!(airport_tile_is_heliport(&s.map, c));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Airport);
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Aircraft));
    assert_eq!(s.economy.money, money - DEPOT_BUILD_COST);

    // Aviones no se compran en helipuerto.
    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap_err();
    assert!(matches!(err, crate::CommandError::VehicleKindNotAllowed));

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    assert!(
        s.vehicles
            .iter()
            .any(|v| v.kind == VehicleKind::Aircraft && v.pos == c)
    );
}

#[test]
fn place_airport_small_footprint_and_hangar_buy() {
    let mut s = GameState::new(20, 20);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].airport_tiles.len(), 12);
    let hangar = s.stations[0].pos;
    assert!(airport_tile_is_hangar(&s.map, hangar));
    // Compra solo en hangar.
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    // Apron no es hangar.
    let apron = s.stations[0]
        .airport_tiles
        .iter()
        .copied()
        .find(|&c| !airport_tile_is_hangar(&s.map, c))
        .unwrap();
    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(apron, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap_err();
    assert!(matches!(err, crate::CommandError::InvalidDepotTile));
}

#[test]
fn place_canal_converts_grass_to_water() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceCanal(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
    assert_eq!(s.economy.money, money - STATION_BUILD_COST / 2);
}

#[test]
fn place_lock_requires_height_delta() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    let a = TileCoord::new(1, 2);
    let b = TileCoord::new(3, 2);
    for t in [a, c, b] {
        s.map.set_kind(t, TileKind::Water).unwrap();
    }
    // Misma altura → rechazo.
    assert!(apply_command(&mut s, &Command::PlaceLock(c, false)).is_err());
    s.map.set_height(a, 1).unwrap();
    s.map.set_height(c, 1).unwrap();
    s.map.set_height(b, 2).unwrap();
    apply_command(&mut s, &Command::PlaceLock(c, false)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.m5 >> 4, 2);
}

#[test]
fn place_city_airport_footprint() {
    let mut s = GameState::new(24, 24);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::City,
        },
    )
    .unwrap();
    assert_eq!(s.stations[0].airport_tiles.len(), 36);
    assert!(airport_tile_is_hangar(&s.map, s.stations[0].pos));
}

#[test]
fn place_international_airport_footprint() {
    let mut s = GameState::new(24, 24);
    let origin = TileCoord::new(1, 1);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::International,
        },
    )
    .unwrap();
    assert_eq!(s.stations[0].airport_tiles.len(), 49);
    let hangar = s.stations[0].pos;
    assert!(airport_tile_is_hangar(&s.map, hangar));
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
}

#[test]
fn ferry_engine_is_passenger_ship() {
    let eng = crate::engine_by_id(ENGINE_SHIP_FERRY).unwrap();
    assert_eq!(eng.kind, VehicleKind::Ship);
    assert_eq!(eng.cargo, Some(crate::CargoType::Passengers));
}

#[test]
fn aircraft_phase_starts_on_heliport_pad() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceAirport(c)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    // Heliport/Oilrig FTA: sin hangar; arranca en pad (Taxi + Helipad1).
    assert_eq!(s.vehicles[0].aircraft_phase, AircraftPhase::Taxi);
    assert!(s.vehicles[0].airport_fta_active);
    assert_eq!(s.vehicles[0].altitude, 0);
}

#[test]
fn small_airport_accepts_helicopter_and_airplane() {
    let mut s = GameState::new(20, 20);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    let hangar = s.stations[0].pos;
    assert!(!airport_tile_is_heliport(&s.map, hangar));
    // Country: Airplanes + Helicopters (+ ShortStrip).
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    assert_eq!(
        s.vehicles
            .iter()
            .filter(|v| v.kind == VehicleKind::Aircraft)
            .count(),
        2
    );
}

#[test]
fn airplane_order_to_heliport_rejected() {
    let mut s = GameState::new(24, 24);
    let heliport = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceAirport(heliport)).unwrap();
    let small_origin = TileCoord::new(8, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: small_origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    let hangar = s.stations.iter().find(|st| st.pos != heliport).unwrap().pos;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    let plane_id = s.vehicles[0].id;
    let err = apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            plane_id,
            vec![crate::vehicle::VehicleOrder::station(heliport)],
        ),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::CommandError::IncompatibleStopForVehicle
    ));

    // Hélico sí puede ordenar al helipuerto.
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(heliport, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    let heli_id = s.vehicles.iter().find(|v| v.id != plane_id).unwrap().id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            heli_id,
            vec![crate::vehicle::VehicleOrder::station(heliport)],
        ),
    )
    .unwrap();
}

#[test]
fn airplane_order_to_small_and_intercon_ok() {
    let mut s = GameState::new(40, 40);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: TileCoord::new(2, 2),
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: TileCoord::new(12, 2),
            axis_y: false,
            spec: crate::AirportSpecId::Intercontinental,
        },
    )
    .unwrap();
    let small = s
        .stations
        .iter()
        .find(|st| st.airport_spec == crate::AirportSpecId::Small)
        .unwrap()
        .pos;
    let inter = s
        .stations
        .iter()
        .find(|st| st.airport_spec == crate::AirportSpecId::Intercontinental)
        .unwrap()
        .pos;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(small, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            id,
            vec![
                crate::vehicle::VehicleOrder::station(small),
                crate::vehicle::VehicleOrder::station(inter),
            ],
        ),
    )
    .unwrap();
}
