use crate::command::{Command, CommandError, apply_command, command_would_fail};
use crate::{
    CLEAR_TILE_COST, GameState, ROAD_BUILD_COST, STATION_BUILD_COST, TileCoord, TileKind, Vehicle,
    VehicleKind,
};

#[test]
fn place_station_requires_adjacent_transport() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(4, 4);
    let e = apply_command(&mut s, &Command::PlaceStation(c)).unwrap_err();
    assert_eq!(e, CommandError::StationNotAdjacentToTransport);
}

#[test]
fn place_station_duplicate_errors() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(2, 1))).unwrap();
    apply_command(&mut s, &Command::PlaceStation(c)).unwrap();
    let e = apply_command(&mut s, &Command::PlaceStation(c)).unwrap_err();
    assert_eq!(e, CommandError::StationAlreadyExists);
    assert_eq!(s.stations.len(), 1);
}

#[test]
fn place_station_dir_preserves_orientation_in_m5() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 1);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceStationDir(c, 3)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Station);
    assert_eq!((tile.mapt >> 4) & 0x0F, 5);
    assert_eq!(tile.m5 & 0x03, 3);
    assert_eq!(tile.m3 & 0x0F, 0x01, "boca de parada hacia la carretera");
}

#[test]
fn place_station_on_forest_clears_and_builds_when_entrance_faces_road() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 1);
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::PlaceForest(c)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceStationDir(c, 3)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Station));
    assert_eq!(
        s.economy.money,
        money_before - 30 - CLEAR_TILE_COST - ROAD_BUILD_COST - STATION_BUILD_COST
    );
}

#[test]
fn place_station_on_road_tile_fails() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 1);
    apply_command(&mut s, &Command::PlaceRoad(c)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    let e = apply_command(&mut s, &Command::PlaceStationDir(c, 3)).unwrap_err();
    assert_eq!(e, CommandError::CannotPlaceStationOnOccupiedTile);
    assert_eq!(s.map.get_kind(c), Some(TileKind::Road));
}

#[test]
fn place_station_dir_rejects_entrance_away_from_road() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 1);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    let e = apply_command(&mut s, &Command::PlaceStationDir(c, 1)).unwrap_err();
    assert_eq!(e, CommandError::StationNotAdjacentToTransport);
}

#[test]
fn set_vehicle_station_orders_rejects_incompatible_stop_kind() {
    let mut s = GameState::new(8, 8);
    let stop = TileCoord::new(1, 1);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(stop, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(10, VehicleKind::Truck, stop, stop));
    let e = apply_command(&mut s, &Command::SetVehicleStationOrders(10, vec![stop])).unwrap_err();
    assert_eq!(e, CommandError::IncompatibleStopForVehicle);
}

#[test]
fn clear_tile_sets_grass_and_removes_station() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(2, 1))).unwrap();
    apply_command(&mut s, &Command::PlaceStation(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Station));
    assert_eq!(s.stations.len(), 1);
    apply_command(&mut s, &Command::ClearTile(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
    assert!(s.stations.is_empty());
}

#[test]
fn set_vehicle_station_orders_requires_existing_stations() {
    let mut s = GameState::new(8, 8);
    s.vehicles.push(crate::Vehicle::new(
        7,
        crate::VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    ));
    let missing = apply_command(
        &mut s,
        &Command::SetVehicleStationOrders(7, vec![TileCoord::new(2, 0)]),
    )
    .unwrap_err();
    assert_eq!(missing, CommandError::StationNotFound);

    s.stations.push(crate::Station::new(TileCoord::new(2, 0)));
    apply_command(
        &mut s,
        &Command::SetVehicleStationOrders(7, vec![TileCoord::new(2, 0)]),
    )
    .unwrap();
    assert!(matches!(
        s.vehicles[0].orders[0],
        crate::VehicleOrder::Station { .. }
    ));
    assert_eq!(s.vehicles[0].dest, TileCoord::new(2, 0));
}

#[test]
fn command_would_fail_matches_apply_for_road_water_and_station() {
    let mut s = GameState::new(8, 8);
    let water = TileCoord::new(1, 1);
    s.map.set_kind(water, TileKind::Water).unwrap();
    assert_eq!(
        command_would_fail(&s, &Command::PlaceRoadBits(water, 0x0F)),
        Some(CommandError::CannotPlaceRoadOnWater)
    );
    assert_eq!(
        apply_command(&mut s, &Command::PlaceRoadBits(water, 0x0F)).unwrap_err(),
        CommandError::CannotPlaceRoadOnWater
    );

    let mut s2 = GameState::new(8, 8);
    let road = TileCoord::new(2, 2);
    apply_command(&mut s2, &Command::PlaceRoad(road)).unwrap();
    assert_eq!(
        command_would_fail(&s2, &Command::PlaceStationDir(road, 0)),
        Some(CommandError::CannotPlaceStationOnOccupiedTile)
    );

    let mut s3 = GameState::new(8, 8);
    let a = TileCoord::new(0, 0);
    let b = TileCoord::new(2, 0);
    s3.map.set_height(b, 2).unwrap();
    assert_eq!(
        command_would_fail(&s3, &Command::PlaceRoadTunnel(a, b)),
        Some(CommandError::InvalidTunnelEndpoints)
    );

    let mut ridge = GameState::new(12, 12);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    ridge.map.set_height(c(5, 5), 2).unwrap();
    ridge.map.set_height(c(5, 6), 2).unwrap();
    ridge.map.set_height(c(6, 5), 1).unwrap();
    ridge.map.set_height(c(6, 6), 1).unwrap();
    ridge.map.set_height(c(3, 5), 1).unwrap();
    ridge.map.set_height(c(3, 6), 1).unwrap();
    ridge.map.set_height(c(4, 5), 2).unwrap();
    ridge.map.set_height(c(4, 6), 2).unwrap();
    assert!(
        command_would_fail(&ridge, &Command::PlaceRoadTunnel(c(5, 5), c(3, 5))).is_none(),
        "túnel NE→SW a mismo GetTileZ"
    );
    apply_command(&mut ridge, &Command::PlaceRoadTunnel(c(5, 5), c(3, 5))).unwrap();
    assert_eq!(ridge.map.get(c(5, 5)).unwrap().m5 & 0x03, 0);
    assert_eq!(ridge.map.get(c(3, 5)).unwrap().m5 & 0x03, 2);
    assert_eq!(ridge.map.get(c(4, 5)).unwrap().m5, 0);
}

#[test]
fn join_stations_merges_adjacent_bus_stops() {
    use crate::VehicleOrder;
    let mut s = GameState::new(8, 8);
    let keep = TileCoord::new(1, 1);
    let merge = TileCoord::new(2, 1);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(2, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(keep, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(merge, 3)).unwrap();
    assert_eq!(s.stations.len(), 2);

    let mut veh = Vehicle::new(1, VehicleKind::Bus, keep, merge);
    veh.orders = vec![VehicleOrder::station(merge)];
    s.vehicles.push(veh);

    apply_command(&mut s, &Command::JoinStations { keep, merge }).unwrap();
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, keep);
    assert!(s.stations[0].joined_tiles.contains(&merge));
    assert!(s.stations[0].covers_tile(merge));
    match &s.vehicles[0].orders[0] {
        VehicleOrder::Station { station, .. } => assert_eq!(*station, keep),
        other => panic!("expected station order, got {other:?}"),
    }
}

#[test]
fn join_stations_rejects_non_adjacent() {
    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(3, 1);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(3, 0))).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(a, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(b, 3)).unwrap();
    let e = apply_command(&mut s, &Command::JoinStations { keep: a, merge: b }).unwrap_err();
    assert_eq!(e, CommandError::CannotJoinStations);
}

#[test]
fn join_stations_merges_adjacent_rail_1x1() {
    use crate::VehicleOrder;
    let mut s = GameState::new(16, 16);
    let keep = TileCoord::new(4, 4);
    let merge = TileCoord::new(5, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(keep, 0)).unwrap();
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(6, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(merge, 0)).unwrap();
    assert_eq!(s.stations.len(), 2);

    let mut veh = Vehicle::new(1, VehicleKind::Train, keep, merge);
    veh.orders = vec![VehicleOrder::station(merge)];
    s.vehicles.push(veh);

    apply_command(&mut s, &Command::JoinStations { keep, merge }).unwrap();
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, keep);
    assert!(s.stations[0].joined_tiles.contains(&merge));
    match &s.vehicles[0].orders[0] {
        VehicleOrder::Station { station, .. } => assert_eq!(*station, keep),
        other => panic!("expected station order, got {other:?}"),
    }
    assert_eq!(
        crate::station_at_tile(&s.map, &s.stations, merge).map(|st| st.pos),
        Some(keep)
    );
}

#[test]
fn join_stations_merges_multi_tile_rail_footprints() {
    let mut s = GameState::new(24, 24);
    // Dos huellas 3×1 en eje X, adyacentes por el borde (origen (2,4) y (5,4)).
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(2, 4),
            axis_y: false,
            platforms: 1,
            length: 3,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(5, 4),
            axis_y: false,
            platforms: 1,
            length: 3,
        },
    )
    .unwrap();
    assert_eq!(s.stations.len(), 2);
    let keep = s.stations[0].pos;
    let merge = s.stations[1].pos;
    assert!((keep.x - merge.x).abs() + (keep.y - merge.y).abs() > 1);

    apply_command(&mut s, &Command::JoinStations { keep, merge }).unwrap();
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, keep);
    let footprint = crate::station_footprint_tiles(&s.map, keep);
    assert!(footprint.len() >= 6, "huella unificada tras join");
    assert!(crate::station_at_tile(&s.map, &s.stations, TileCoord::new(7, 4)).is_some());
}

#[test]
fn join_stations_rejects_rail_with_bus() {
    let mut s = GameState::new(16, 16);
    let rail = TileCoord::new(4, 4);
    let bus = TileCoord::new(5, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(rail, 0)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(5, 3))).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(bus, 3)).unwrap();
    let e = apply_command(
        &mut s,
        &Command::JoinStations {
            keep: rail,
            merge: bus,
        },
    )
    .unwrap_err();
    assert_eq!(e, CommandError::CannotJoinStations);
}

#[test]
fn join_stations_rejects_rail_perpendicular_axis() {
    let mut s = GameState::new(16, 16);
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(2, 4),
            axis_y: false,
            platforms: 1,
            length: 2,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(4, 4),
            axis_y: true,
            platforms: 1,
            length: 2,
        },
    )
    .unwrap();
    let keep = s.stations[0].pos;
    let merge = s.stations[1].pos;
    let e = apply_command(&mut s, &Command::JoinStations { keep, merge }).unwrap_err();
    assert_eq!(e, CommandError::CannotJoinStations);
}

#[test]
fn join_stations_merges_rail_cargo_packets() {
    let mut s = GameState::new(16, 16);
    let keep = TileCoord::new(4, 4);
    let merge = TileCoord::new(5, 4);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(3, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(keep, 0)).unwrap();
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(6, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(merge, 0)).unwrap();
    let merge_idx = s.stations.iter().position(|st| st.pos == merge).unwrap();
    s.stations[merge_idx].add_waiting_cargo(crate::CargoType::Coal, 40);
    apply_command(&mut s, &Command::JoinStations { keep, merge }).unwrap();
    assert_eq!(s.stations.len(), 1);
    assert!(s.stations[0].cargo_stock.get(crate::CargoType::Coal) >= 40);
}
