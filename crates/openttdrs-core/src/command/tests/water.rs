//! Tests de construcción acuática (depósito + muelle).

use crate::{
    Command, DEPOT_BUILD_COST, GameState, STATION_BUILD_COST, StopKind, TileCoord, TileKind,
    VehicleKind, apply_command,
};

#[test]
fn place_ship_depot_on_water_with_water_entrance() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(3, 4); // dir 0 → (-1,0)
    s.map.set_kind(depot, TileKind::Water).unwrap();
    s.map.set_kind(mouth, TileKind::Water).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    assert_eq!(s.map.get_kind(depot), Some(TileKind::ShipDepot));
    assert_eq!(s.economy.money, money - DEPOT_BUILD_COST);
}

#[test]
fn place_ship_depot_rejects_land() {
    let mut s = GameState::new(8, 8);
    let e =
        apply_command(&mut s, &Command::PlaceShipDepotDir(TileCoord::new(2, 2), 0)).unwrap_err();
    assert!(matches!(
        e,
        crate::CommandError::CannotPlaceStationOnOccupiedTile
    ));
}

#[test]
fn place_dock_on_coast_and_serves_ship() {
    let mut s = GameState::new(12, 12);
    let dock = TileCoord::new(5, 5);
    let land = TileCoord::new(5, 4);
    let water = TileCoord::new(6, 5);
    s.map.set_kind(dock, TileKind::Water).unwrap();
    s.map.set_kind(water, TileKind::Water).unwrap();
    s.map.set_kind(land, TileKind::Grass).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceDock(dock, 0)).unwrap();
    assert_eq!(s.map.get_kind(dock), Some(TileKind::Station));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Dock);
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Ship));
    assert_eq!(s.economy.money, money - STATION_BUILD_COST);
}

#[test]
fn ship_buys_at_depot_and_paths_to_dock() {
    use crate::engine::ENGINE_SHIP_MPS;
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::vehicle::VehicleOrder;

    let mut s = GameState::new(16, 10);
    for x in 2..=10 {
        s.map
            .set_kind(TileCoord::new(x, 4), TileKind::Water)
            .unwrap();
    }
    s.map
        .set_kind(TileCoord::new(10, 3), TileKind::Grass)
        .unwrap();
    apply_command(&mut s, &Command::PlaceShipDepotDir(TileCoord::new(2, 4), 2)).unwrap(); // boca +x hacia agua
    apply_command(&mut s, &Command::PlaceDock(TileCoord::new(10, 4), 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(TileCoord::new(2, 4), ENGINE_SHIP_MPS),
    )
    .unwrap();
    let ship = s
        .vehicles
        .iter_mut()
        .find(|v| v.kind == VehicleKind::Ship)
        .unwrap();
    ship.running = true;
    ship.set_vehicle_orders(vec![VehicleOrder::station(TileCoord::new(10, 4))]);
    ship.sync_order_destination(&s.map);
    let path = find_path(&s.map, ship.pos, ship.dest, PathNetwork::Water);
    assert!(path.is_some(), "ruta agua depósito → muelle");
}
