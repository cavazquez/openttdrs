//! Tests de construcción acuática (depósito, muelle, boya, acueducto).

use crate::economy::station_build_cost;
use crate::test_fixtures::SandboxMap;
use crate::{
    Command, DEPOT_BUILD_COST, GameState, StopKind, TileCoord, TileKind, VehicleKind,
    apply_command, bridge_above_axis_from_mapt,
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
    assert_eq!(
        crate::station::station_type_from_m6(s.map.get(dock).unwrap().m6),
        crate::station::STATION_TYPE_DOCK
    );
    assert!(crate::ship_movement::is_water_network_tile_at(&s.map, dock));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Dock);
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Ship));
    assert_eq!(
        s.economy.money,
        money - station_build_cost(&s.global_economy)
    );
}

#[test]
fn place_buoy_on_water_is_ship_waypoint() {
    let mut s = GameState::new(12, 12);
    let buoy = TileCoord::new(4, 4);
    s.map.set_kind(buoy, TileKind::Water).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Station));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Buoy);
    assert!(s.stations[0].is_waypoint());
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Ship));
    assert!(!s.stations[0].accepts_cargo(crate::CargoType::Goods));
    assert_eq!(
        s.economy.money,
        money - station_build_cost(&s.global_economy) / 2
    );
}

#[test]
fn clearing_buoy_restores_underlying_canal_water() {
    use crate::map::is_canal_tile;

    let mut s = GameState::new(8, 8);
    let buoy = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceCanal(buoy)).unwrap();
    apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Station));

    apply_command(&mut s, &Command::ClearTile(buoy)).unwrap();

    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Water));
    assert!(s.map.get(buoy).is_some_and(is_canal_tile));
    assert_eq!(s.map.get(buoy).map(|tile| tile.m6), Some(0));
    assert!(s.stations.is_empty());
}

#[test]
fn place_buoy_under_bridge_keeps_waterway_available() {
    use crate::BridgeType;

    let mut s = GameState::new(8, 8);
    let c = |x: i32| TileCoord::new(x, 4);
    for x in 2..=5 {
        s.map.set_kind(c(x), TileKind::Water).unwrap();
    }
    apply_command(
        &mut s,
        &Command::PlaceRoadBridge(c(1), c(6), BridgeType::Wooden),
    )
    .unwrap();

    let buoy = c(3);
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Water));
    apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Station));
    assert_eq!(s.stations[0].stop_kind, StopKind::Buoy);
}

#[test]
fn place_buoy_rejects_land() {
    let mut s = GameState::new(8, 8);
    let e = apply_command(&mut s, &Command::PlaceBuoy(TileCoord::new(2, 2))).unwrap_err();
    assert!(matches!(
        e,
        crate::CommandError::CannotPlaceStationOnOccupiedTile
    ));
}

fn set_ne_slope(map: &mut crate::Map, tx: i32, ty: i32, base: u8) {
    map.set_height(TileCoord::new(tx, ty), base + 1).unwrap();
    map.set_height(TileCoord::new(tx, ty + 1), base + 1)
        .unwrap();
    map.set_height(TileCoord::new(tx + 1, ty), base).unwrap();
    map.set_height(TileCoord::new(tx + 1, ty + 1), base)
        .unwrap();
}

fn set_sw_slope(map: &mut crate::Map, tx: i32, ty: i32, base: u8) {
    map.set_height(TileCoord::new(tx, ty), base).unwrap();
    map.set_height(TileCoord::new(tx, ty + 1), base).unwrap();
    map.set_height(TileCoord::new(tx + 1, ty), base + 1)
        .unwrap();
    map.set_height(TileCoord::new(tx + 1, ty + 1), base + 1)
        .unwrap();
}

#[test]
fn place_aqueduct_between_facing_slopes() {
    let mut s = SandboxMap::flat_rich(16, 12, 1);
    // Oeste → este: rampa SW en (3,5), rampa NE en (7,5).
    let west = TileCoord::new(3, 5);
    let east = TileCoord::new(7, 5);
    set_sw_slope(&mut s.map, west.x, west.y, 1);
    set_ne_slope(&mut s.map, east.x, east.y, 1);
    apply_command(&mut s, &Command::PlaceAqueduct(west, east)).unwrap();
    assert_eq!(s.map.get_kind(west), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(east), Some(TileKind::Water));
    let mid = s.map.get(TileCoord::new(5, 5)).unwrap();
    assert_eq!(mid.kind, TileKind::Water);
    assert!(bridge_above_axis_from_mapt(mid.mapt).is_some());
    assert!(crate::ship_movement::is_water_network_tile_at(
        &s.map,
        TileCoord::new(5, 5)
    ));
}

#[test]
fn place_aqueduct_rejects_flat_endpoints() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let e = apply_command(
        &mut s,
        &Command::PlaceAqueduct(TileCoord::new(2, 4), TileCoord::new(6, 4)),
    )
    .unwrap_err();
    assert!(matches!(e, crate::CommandError::InvalidBridgeSpan));
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

#[test]
fn ship_paths_via_buoy() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = GameState::new(16, 10);
    for x in 2..=10 {
        s.map
            .set_kind(TileCoord::new(x, 4), TileKind::Water)
            .unwrap();
    }
    apply_command(&mut s, &Command::PlaceBuoy(TileCoord::new(6, 4))).unwrap();
    let path = find_path(
        &s.map,
        TileCoord::new(2, 4),
        TileCoord::new(10, 4),
        PathNetwork::Water,
    );
    assert!(path.is_some(), "ruta agua atraviesa boya");
    assert!(
        path.unwrap().contains(&TileCoord::new(6, 4)),
        "la ruta incluye la boya"
    );
}

#[test]
fn place_river_on_flat_and_inclined() {
    use crate::map::is_river_tile;

    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let flat = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRiver(flat)).unwrap();
    assert!(s.map.get(flat).is_some_and(is_river_tile));

    // Pendiente NE en (6,4): río permitido.
    s.map.set_height(TileCoord::new(6, 4), 2).unwrap();
    s.map.set_height(TileCoord::new(6, 5), 2).unwrap();
    s.map.set_height(TileCoord::new(7, 4), 1).unwrap();
    s.map.set_height(TileCoord::new(7, 5), 1).unwrap();
    let slope = TileCoord::new(6, 4);
    apply_command(&mut s, &Command::PlaceRiver(slope)).unwrap();
    assert!(s.map.get(slope).is_some_and(is_river_tile));
    // Río en pendiente no es navegable.
    assert!(!crate::ship_movement::is_water_network_tile_at(
        &s.map, slope
    ));

    assert!(
        apply_command(&mut s, &Command::PlaceCanal(slope)).is_err(),
        "canal rechaza pendiente"
    );
}

#[test]
fn place_canal_sets_water_class_canal() {
    use crate::map::is_canal_tile;

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceCanal(c)).unwrap();
    assert!(s.map.get(c).is_some_and(is_canal_tile));
}

#[test]
fn ship_paths_on_flat_river() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = SandboxMap::flat_rich(16, 10, 1);
    for x in 2..=10 {
        apply_command(&mut s, &Command::PlaceRiver(TileCoord::new(x, 4))).unwrap();
    }
    let path = find_path(
        &s.map,
        TileCoord::new(2, 4),
        TileCoord::new(10, 4),
        PathNetwork::Water,
    );
    assert!(path.is_some(), "río plano navegable");
}
