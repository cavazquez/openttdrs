//! Tests de aeropuerto, canal y esclusa.

use crate::{
    Command, DEPOT_BUILD_COST, ENGINE_AIRCRAFT_DAKOTA, ENGINE_SHIP_FERRY, GameState,
    STATION_BUILD_COST, StopKind, TileCoord, TileKind, VehicleKind, apply_command,
};

#[test]
fn place_airport_and_buy_aircraft() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceAirport(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Airport));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Airport);
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Aircraft));
    assert_eq!(s.economy.money, money - DEPOT_BUILD_COST);

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    assert!(
        s.vehicles
            .iter()
            .any(|v| v.kind == VehicleKind::Aircraft && v.pos == c)
    );
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
fn place_lock_marks_water_subtype() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    s.map.set_kind(c, TileKind::Water).unwrap();
    apply_command(&mut s, &Command::PlaceLock(c, true)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.m5 >> 4, 2);
    assert_eq!(tile.m5 & 1, 1);
}

#[test]
fn ferry_engine_is_passenger_ship() {
    let eng = crate::engine_by_id(ENGINE_SHIP_FERRY).unwrap();
    assert_eq!(eng.kind, VehicleKind::Ship);
    assert_eq!(eng.cargo, Some(crate::CargoType::Passengers));
}
