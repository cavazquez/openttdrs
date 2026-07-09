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
fn ferry_engine_is_passenger_ship() {
    let eng = crate::engine_by_id(ENGINE_SHIP_FERRY).unwrap();
    assert_eq!(eng.kind, VehicleKind::Ship);
    assert_eq!(eng.cargo, Some(crate::CargoType::Passengers));
}

#[test]
fn aircraft_phase_starts_in_hangar() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceAirport(c)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    assert_eq!(s.vehicles[0].aircraft_phase, AircraftPhase::InHangar);
    assert_eq!(s.vehicles[0].altitude, 0);
}

#[test]
fn small_airport_rejects_helicopter() {
    let mut s = GameState::new(20, 20);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
        },
    )
    .unwrap();
    let hangar = s.stations[0].pos;
    assert!(!airport_tile_is_heliport(&s.map, hangar));
    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap_err();
    assert!(matches!(err, crate::CommandError::VehicleKindNotAllowed));
}
