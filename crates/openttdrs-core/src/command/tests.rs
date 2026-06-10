use super::{Command, CommandError, apply_command, command_error_message, command_would_fail};
use crate::{
    BRIDGE_BUILD_COST_PER_TILE, CLEAR_TILE_COST, GameState, IndustryKind, IndustrySpec,
    ROAD_BUILD_COST, STATION_BUILD_COST, StopKind, TileCoord, TileKind, Vehicle, VehicleKind,
    industry_template,
};

#[test]
fn place_road_mutates_tile_kind() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 4);
    let money_before = s.economy.money;
    assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
    apply_command(&mut s, &Command::PlaceRoad(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Road));
    assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x05);
    assert_eq!((s.map.get(c).unwrap().mapt >> 4) & 0x0F, 2);
    assert_eq!(s.economy.money, money_before - ROAD_BUILD_COST);
}

#[test]
fn place_road_bits_combines_directions() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 4);
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x05)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0A)).unwrap();
    assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x0F);
}

#[test]
fn set_road_bits_replaces_existing_directions() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 4);
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0F)).unwrap();
    apply_command(&mut s, &Command::SetRoadBits(c, 0x0A)).unwrap();
    assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x0A);
}

#[test]
fn set_road_bits_clears_forest_auxiliary_planes() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 4);
    let mut tile = s.map.get(c).unwrap();
    tile.kind = TileKind::Forest;
    tile.mapt = 0x40;
    tile.m5 = 0x83;
    tile.m3 = 0x06;
    tile.m7 = 0x20;
    tile.m8 = 0x1234;
    s.map.set_tile(c, tile).unwrap();

    apply_command(&mut s, &Command::SetRoadBits(c, 0x0A)).unwrap();

    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Road);
    assert_eq!(tile.mapt, 0x20);
    assert_eq!(tile.m5, 0x0A);
    assert_eq!(tile.m3, 0);
    assert_eq!(tile.m7, 0);
    assert_eq!(tile.m8, 0);
}

#[test]
fn place_road_on_water_returns_error() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 1);
    let money_before = s.economy.money;
    s.map.set_kind(c, TileKind::Water).unwrap();
    let e = apply_command(&mut s, &Command::PlaceRoad(c)).unwrap_err();
    assert_eq!(e, CommandError::CannotPlaceRoadOnWater);
    assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
    assert_eq!(s.economy.money, money_before);
}

#[test]
fn command_sequence_is_deterministic() {
    let cmds = [
        Command::PlaceRoad(TileCoord::new(0, 0)),
        Command::PlaceRail(TileCoord::new(0, 1)),
        Command::PlaceRoad(TileCoord::new(1, 0)),
        Command::PlaceStation(TileCoord::new(2, 0)),
        Command::ClearTile(TileCoord::new(1, 0)),
    ];
    let mut a = GameState::new(8, 8);
    let mut b = GameState::new(8, 8);
    for cmd in &cmds {
        apply_command(&mut a, cmd).unwrap();
        apply_command(&mut b, cmd).unwrap();
    }
    let ja = a.save_json().unwrap();
    let jb = b.save_json().unwrap();
    assert_eq!(ja, jb);
}

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
fn place_rail_station_sets_m6_and_axis_in_m5() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(c, 0)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Station);
    assert_eq!((tile.m6 >> 3) & 0x0F, 0);
    assert_eq!(
        tile.m5, 3,
        "vía vecina aislada es eje Y → gfx 3 con edificio"
    );
    assert_eq!(s.stations[0].stop_kind, StopKind::RailStation);
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
fn place_bus_stop_links_adjacent_road() {
    let mut s = GameState::new(8, 8);
    let stop = TileCoord::new(1, 1);
    let road = TileCoord::new(1, 0);
    apply_command(&mut s, &Command::PlaceRoad(road)).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(stop, 3)).unwrap();
    assert_eq!(s.map.get(stop).unwrap().m3 & 0x0F, 0x01);
    assert!(
        s.map.get(road).unwrap().m5 & 0x04 != 0,
        "carretera con bit hacia la parada"
    );
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
fn place_rail_station_rejects_entrance_away_from_rail() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
    let e = apply_command(&mut s, &Command::PlaceRailStation(c, 2)).unwrap_err();
    assert_eq!(e, CommandError::StationNotAdjacentToTransport);
}

#[test]
fn build_road_vehicle_at_depot_creates_stopped_bus() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    let money_before = s.economy.money;
    apply_command(
        &mut s,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].kind, VehicleKind::Bus);
    assert!(!s.vehicles[0].running);
    assert_eq!(
        s.economy.money,
        money_before - crate::vehicle_purchase_cost(VehicleKind::Bus)
    );
}

#[test]
fn place_road_depot_dir_preserves_orientation_in_m5() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    let exit = TileCoord::new(2, 1);
    apply_command(&mut s, &Command::PlaceRoad(exit)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap();
    let tile = s.map.get(depot).unwrap();
    assert_eq!(tile.kind, TileKind::RoadDepot);
    assert_eq!(tile.m5 & 0x03, 3);
    assert_eq!((tile.m5 >> 6) & 0x03, 2, "RoadTileType::Depot en bits 6–7");
    assert_eq!(s.map.get_kind(exit), Some(TileKind::Road));
    assert_ne!(
        s.map.get(exit).unwrap().m5 & 0x04,
        0,
        "boca NW hacia el depósito"
    );
}

#[test]
fn place_road_depot_dir_requires_road_at_entrance() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    let e = apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap_err();
    assert_eq!(e, CommandError::StationNotAdjacentToTransport);
    assert_eq!(s.map.get_kind(depot), Some(TileKind::Grass));
}

#[test]
fn toggle_road_vehicle_running_targets_depot_exit() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    let exit = TileCoord::new(3, 2);
    apply_command(&mut s, &Command::PlaceRoad(exit)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 2)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Truck),
    )
    .unwrap();

    apply_command(&mut s, &Command::ToggleVehicleRunning(1)).unwrap();

    assert!(s.vehicles[0].running);
    assert_eq!(s.vehicles[0].dest, exit);
}

#[test]
fn toggle_road_vehicle_running_targets_reachable_road_not_depot_mouth() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    let far = TileCoord::new(5, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(3, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 2)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(4, 2), 0x0A)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadBits(far, 0x0A)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Truck),
    )
    .unwrap();

    apply_command(&mut s, &Command::ToggleVehicleRunning(1)).unwrap();

    assert_eq!(s.vehicles[0].dest, far);
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
fn place_rail_mutates_tile_kind() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 3);
    assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Rail));
}

#[test]
fn place_rail_sets_mapt_and_trackbits_for_horizontal_line() {
    let mut s = GameState::new(12, 8);
    for x in 2..=5 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let mid = s.map.get(TileCoord::new(3, 4)).unwrap();
    assert_eq!(mid.mapt, 0x10);
    assert_eq!(mid.m5 & 0x3F, 0x01, "tramo horizontal: Track X");
    assert_eq!((mid.m5 >> 6) & 0x3, 0);
}

#[test]
fn set_rail_bits_places_horz_and_vert() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(2, 2), 0x0C)).unwrap();
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(4, 2), 0x30)).unwrap();
    assert_eq!(s.map.get_kind(TileCoord::new(2, 2)), Some(TileKind::Rail));
    assert_eq!(s.map.get(TileCoord::new(2, 2)).unwrap().m5 & 0x3F, 0x0C);
    assert_eq!(s.map.get(TileCoord::new(4, 2)).unwrap().m5 & 0x3F, 0x30);
}

#[test]
fn place_rail_bits_merges_trackbits_on_existing_rail() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(3, 3), 0x01)).unwrap();
    apply_command(&mut s, &Command::PlaceRailBits(TileCoord::new(3, 3), 0x02)).unwrap();
    assert_eq!(s.map.get(TileCoord::new(3, 3)).unwrap().m5 & 0x3F, 0x03);
}

#[test]
fn bridge_cost_scales_with_line_length() {
    let mut s = GameState::new(8, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for x in 2..=3 {
        s.map.set_kind(c(x, 1), TileKind::Water).unwrap();
    }
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::PlaceRoadBridge(c(1, 1), c(4, 1))).unwrap();
    assert_eq!(
        s.economy.money,
        money_before - BRIDGE_BUILD_COST_PER_TILE * 4
    );
}

#[test]
fn set_vehicle_orders_assigns_circular_route() {
    let mut s = GameState::new(8, 8);
    s.vehicles.push(crate::Vehicle::new(
        7,
        crate::VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    ));
    apply_command(
        &mut s,
        &Command::SetVehicleOrders(7, vec![TileCoord::new(2, 0), TileCoord::new(2, 2)]),
    )
    .unwrap();
    assert_eq!(s.vehicles[0].dest, TileCoord::new(2, 0));
    assert_eq!(s.vehicles[0].orders.len(), 2);
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
fn sandbox_commands_place_visible_tile_kinds() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceHouse(TileCoord::new(1, 1))).unwrap();
    apply_command(&mut s, &Command::PlaceIndustry(TileCoord::new(2, 1))).unwrap();
    apply_command(&mut s, &Command::PlaceForest(TileCoord::new(3, 1))).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceIndustryKind(TileCoord::new(4, 1), IndustryKind::CoalMine),
    )
    .unwrap();
    assert_eq!(s.map.get_kind(TileCoord::new(1, 1)), Some(TileKind::House));
    assert_eq!(
        s.map.get_kind(TileCoord::new(2, 1)),
        Some(TileKind::Industry)
    );
    assert_eq!(s.map.get_kind(TileCoord::new(3, 1)), Some(TileKind::Forest));
    assert_eq!(
        s.map.get_kind(TileCoord::new(4, 1)),
        Some(TileKind::Industry)
    );
    // CoalMine ahora ocupa múltiples tiles (2x2).
    assert_eq!(
        s.map.get_kind(TileCoord::new(5, 1)),
        Some(TileKind::Industry)
    );
    assert_eq!(
        s.map.get_kind(TileCoord::new(4, 2)),
        Some(TileKind::Industry)
    );
    assert_eq!(
        s.map.get_kind(TileCoord::new(5, 2)),
        Some(TileKind::Industry)
    );
    assert!(s.industries.iter().any(|industry| {
        industry.pos == TileCoord::new(4, 1) && industry.kind == IndustryKind::CoalMine
    }));
}

#[test]
fn place_industry_spec_marks_tiles_completed() {
    let mut s = GameState::new(16, 16);
    let origin = TileCoord::new(5, 5);
    apply_command(
        &mut s,
        &Command::PlaceIndustrySpec(origin, IndustrySpec::Sawmill),
    )
    .unwrap();
    for (coord, _) in industry_template(origin, IndustrySpec::Sawmill) {
        let Some(tile) = s.map.get(coord) else {
            panic!("tesela del footprint {coord:?}");
        };
        assert_eq!(tile.kind, TileKind::Industry);
        assert_ne!(tile.m1 & 0x80, 0, "IsIndustryCompleted en {coord:?}");
    }
}

#[test]
fn clear_any_industry_tile_removes_whole_industry_footprint() {
    let mut s = GameState::new(10, 10);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceIndustryKind(origin, IndustryKind::Factory),
    )
    .unwrap();
    assert_eq!(s.industries.len(), 1);
    let target_inside = TileCoord::new(3, 2);
    apply_command(&mut s, &Command::ClearTile(target_inside)).unwrap();
    assert!(s.industries.is_empty());
    // Factory template cubre también (4,3).
    assert_eq!(s.map.get_kind(TileCoord::new(4, 3)), Some(TileKind::Grass));
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
fn bridge_axis_y_sets_m5_flag() {
    let mut s = GameState::new(8, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for y in 2..=4 {
        s.map.set_kind(c(2, y), TileKind::Water).unwrap();
    }
    let a = TileCoord::new(2, 1);
    let b = TileCoord::new(2, 5);
    apply_command(&mut s, &Command::PlaceRoadBridge(a, b)).unwrap();
    assert_eq!(s.map.get(a).unwrap().m5 & 0x10, 0x10);
    let mut s2 = GameState::new(8, 8);
    for x in 1..=5 {
        s2.map.set_kind(c(x, 2), TileKind::Water).unwrap();
    }
    let a2 = TileCoord::new(0, 2);
    let b2 = TileCoord::new(6, 2);
    apply_command(&mut s2, &Command::PlaceRoadBridge(a2, b2)).unwrap();
    assert_eq!(s2.map.get(a2).unwrap().m5 & 0x10, 0);
}

#[test]
fn bridge_rejects_flat_grass_without_gap() {
    let s = GameState::new(8, 8);
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(4, 1);
    assert_eq!(
        command_would_fail(&s, &Command::PlaceRoadBridge(a, b)),
        Some(CommandError::InvalidBridgeSpan)
    );
}

#[test]
fn bridge_accepts_span_over_water() {
    let mut s = GameState::new(16, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for x in 2..=5 {
        s.map.set_kind(c(x, 4), TileKind::Water).unwrap();
    }
    assert!(command_would_fail(&s, &Command::PlaceRoadBridge(c(1, 4), c(6, 4))).is_none());
}

#[test]
fn sell_vehicle_requires_depot_tile() {
    let mut s = GameState::new(8, 8);
    let road = TileCoord::new(2, 2);
    s.map.set_kind(road, TileKind::Road).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Truck, road, road));
    assert_eq!(
        apply_command(&mut s, &Command::SellVehicle(1)),
        Err(CommandError::VehicleNotInDepot)
    );
}

#[test]
fn sell_vehicle_in_road_depot_succeeds() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(1, 1);
    s.map.set_kind(depot, TileKind::RoadDepot).unwrap();
    let vehicle = Vehicle::new(1, VehicleKind::Bus, depot, depot);
    let refund = crate::vehicle_sell_refund(&vehicle);
    s.vehicles.push(vehicle);
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::SellVehicle(1)).unwrap();
    assert!(s.vehicles.is_empty());
    assert_eq!(s.economy.money, money_before + refund);
}

#[test]
fn every_command_error_has_user_message() {
    const ERRORS: [CommandError; 18] = [
        CommandError::OutOfBounds,
        CommandError::CannotPlaceRoadOnWater,
        CommandError::CannotPlaceRoadOnVoid,
        CommandError::CannotPlaceRailOnWater,
        CommandError::CannotPlaceRailOnVoid,
        CommandError::CannotPlaceStationOnWater,
        CommandError::CannotPlaceStationOnVoid,
        CommandError::CannotPlaceStationOnOccupiedTile,
        CommandError::StationNotAdjacentToTransport,
        CommandError::StationAlreadyExists,
        CommandError::StationNotFound,
        CommandError::VehicleNotFound,
        CommandError::VehicleNotInDepot,
        CommandError::InvalidDepotTile,
        CommandError::VehicleKindNotAllowed,
        CommandError::IncompatibleStopForVehicle,
        CommandError::InvalidTunnelEndpoints,
        CommandError::InvalidBridgeSpan,
    ];
    for err in ERRORS {
        let msg = command_error_message(err);
        assert!(!msg.is_empty(), "{err:?}");
        assert!(
            msg.chars().any(char::is_alphabetic),
            "mensaje sin letras para {err:?}: {msg}"
        );
    }
}
