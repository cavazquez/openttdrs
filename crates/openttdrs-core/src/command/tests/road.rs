use crate::command::{Command, CommandError, apply_command, command_would_fail};
use crate::{
    GameState, ROAD_BUILD_COST, ROAD_PLACE_FORCE_AXIS, TileCoord, TileKind, Vehicle, VehicleKind,
    infer_road_drag_axis, road_bits_for_autoroute, tile_slope_and_z,
};

use super::helpers::set_w_only_slope;

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
fn road_drag_line_on_row_below_network() {
    let mut s = GameState::new(12, 12);
    for x in 3..=6 {
        apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A)).unwrap();
    }
    for x in 8..=11 {
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
        assert_eq!(
            s.map.get(TileCoord::new(x, 6)).unwrap().m5 & 0x0F,
            0x0A,
            "x={x}"
        );
    }
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
fn place_road_bits_extends_horizontal_when_neighbor_west() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(3, 4), 0x0A)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(4, 4), 0x05)).unwrap();
    assert_eq!(
        s.map.get(TileCoord::new(4, 4)).unwrap().m5 & 0x0F,
        0x0A,
        "al continuar al este, ignorar tool Y y alinear eje horizontal"
    );
}

#[test]
fn place_road_bits_force_axis_on_isolated_tile() {
    let mut s = GameState::new(8, 8);
    apply_command(
        &mut s,
        &Command::PlaceRoadBits(TileCoord::new(2, 2), 0x05 | ROAD_PLACE_FORCE_AXIS),
    )
    .unwrap();
    assert_eq!(
        s.map.get(TileCoord::new(2, 2)).unwrap().m5 & 0x0F,
        0x05,
        "arrastre Y fuerza vertical aunque no haya vecinos"
    );
    apply_command(
        &mut s,
        &Command::PlaceRoadBits(TileCoord::new(5, 5), 0x0A | ROAD_PLACE_FORCE_AXIS),
    )
    .unwrap();
    assert_eq!(s.map.get(TileCoord::new(5, 5)).unwrap().m5 & 0x0F, 0x0A);
}

#[test]
fn road_bits_for_autoroute_follows_neighbors() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(4, 4);
    assert_eq!(road_bits_for_autoroute(&s.map, c), 0x0A);
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(3, 4), 0x0A)).unwrap();
    assert_eq!(road_bits_for_autoroute(&s.map, c), 0x0A);
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(4, 3), 0x05)).unwrap();
    assert_eq!(road_bits_for_autoroute(&s.map, c), 0x0F);
}

#[test]
fn place_road_bits_force_axis_ignores_single_cardinal_neighbor() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(4, 3), 0x05)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRoadBits(TileCoord::new(4, 4), 0x0A | ROAD_PLACE_FORCE_AXIS),
    )
    .unwrap();
    assert_eq!(
        s.map.get(TileCoord::new(4, 4)).unwrap().m5 & 0x0F,
        0x0A,
        "eje horizontal forzado no inventa bit N hacia el vecino (#181)"
    );
}

#[test]
fn place_road_bits_reinforce_same_axis_keeps_bits_with_parallel() {
    let mut s = GameState::new(12, 12);
    // Dos rectas horizontales (FORCE evita que la 2.ª fila se incline al tocar la 1.ª).
    for x in 3..=6 {
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
    }
    for x in 3..=6 {
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
    }
    for x in 3..=6 {
        let c = TileCoord::new(x, 5);
        assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x0A, "precondición x={x}");
        apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0A)).unwrap();
        assert_eq!(
            s.map.get(c).unwrap().m5 & 0x0F,
            0x0A,
            "reforzar RoadX sin FORCE no añade N/S; x={x}"
        );
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(c, 0x0A | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
        assert_eq!(
            s.map.get(c).unwrap().m5 & 0x0F,
            0x0A,
            "reforzar RoadX con FORCE no añade N/S; x={x}"
        );
    }
}

#[test]
fn place_road_bits_reinforce_same_axis_vertical_keeps_bits() {
    let mut s = GameState::new(12, 12);
    for y in 3..=6 {
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(TileCoord::new(5, y), 0x05 | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(TileCoord::new(6, y), 0x05 | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
    }
    for y in 3..=6 {
        let c = TileCoord::new(5, y);
        assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x05, "precondición y={y}");
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(c, 0x05 | ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
        assert_eq!(
            s.map.get(c).unwrap().m5 & 0x0F,
            0x05,
            "reforzar RoadY con FORCE no añade E/O; y={y}"
        );
    }
}

#[test]
fn infer_road_drag_axis_continues_colinear_network() {
    let mut s = GameState::new(12, 12);
    for x in 3..=6 {
        apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A)).unwrap();
    }
    let start = TileCoord::new(8, 6);
    let end = TileCoord::new(11, 8);
    assert_eq!(
        infer_road_drag_axis(&s.map, start, end, 0x05),
        0x0A,
        "cerca de línea horizontal: ignorar tool Y"
    );
}

#[test]
fn infer_road_drag_axis_branches_perpendicular_from_road_tile() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(4, 4), 0x0A)).unwrap();
    assert_eq!(
        infer_road_drag_axis(&s.map, TileCoord::new(4, 4), TileCoord::new(4, 7), 0x0A),
        0x05,
        "sobre recta horizontal, arrastre vertical → rama"
    );
}

#[test]
fn road_y_force_keeps_tool_axis_without_cardinal_neighbor() {
    let mut s = GameState::new(8, 8);
    for x in 3..=5 {
        apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A)).unwrap();
    }
    apply_command(
        &mut s,
        &Command::PlaceRoadBits(TileCoord::new(6, 4), 0x05 | ROAD_PLACE_FORCE_AXIS),
    )
    .unwrap();
    assert_eq!(s.map.get(TileCoord::new(6, 4)).unwrap().m5 & 0x0F, 0x05);
}

#[test]
fn generic_inferred_axis_placed_on_colinear_row() {
    let mut s = GameState::new(8, 8);
    for x in 3..=5 {
        apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A)).unwrap();
    }
    let c = TileCoord::new(6, 4);
    let axis = infer_road_drag_axis(&s.map, c, TileCoord::new(8, 4), 0x05);
    assert_eq!(axis, 0x0A);
    apply_command(
        &mut s,
        &Command::PlaceRoadBits(c, axis | ROAD_PLACE_FORCE_AXIS),
    )
    .unwrap();
    assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x0A);
}

#[test]
fn place_road_bits_links_perpendicular_neighbor() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(3, 4), 0x0A)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(3, 3), 0x05)).unwrap();
    assert_eq!(s.map.get(TileCoord::new(3, 3)).unwrap().m5 & 0x0F, 0x05);
    assert_eq!(
        s.map.get(TileCoord::new(3, 4)).unwrap().m5 & 0x0F,
        0x0B,
        "la horizontal recibe NW al unir rama vertical en tesela adyacente"
    );
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
fn raise_land_rejects_road() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(c)).unwrap();
    assert_eq!(
        apply_command(&mut s, &Command::RaiseLand(c)),
        Err(CommandError::TileNotTerraformable)
    );
    assert_eq!(
        command_would_fail(&s, &Command::RaiseLand(c)),
        Some(CommandError::TileNotTerraformable)
    );
}

#[test]
fn place_road_autoslopes_grass_slope() {
    let mut s = GameState::new(8, 8);
    set_w_only_slope(&mut s.map, 2, 2, 1);
    let c = TileCoord::new(2, 2);
    assert_ne!(tile_slope_and_z(&s.map, c).unwrap().0, 0);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0A)).unwrap();
    assert_eq!(tile_slope_and_z(&s.map, c).unwrap().0, 0);
    assert_eq!(s.map.get_kind(c), Some(TileKind::Road));
    assert!(s.economy.money < money - ROAD_BUILD_COST);
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
fn build_vehicle_at_depot_rejects_train_in_road_depot() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    let e = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_KIRBY),
    )
    .unwrap_err();
    assert_eq!(e, CommandError::InvalidDepotTile);
}

#[test]
fn place_tram_bits_sets_m3_and_m8() {
    use crate::road_type::{RoadType, tram_road_type_from_tile, tram_track_bits};
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceTramBits(c, 0x05)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Road);
    assert_eq!(tram_track_bits(&tile), 0x05);
    assert_eq!(tram_road_type_from_tile(&tile), Some(RoadType::Tram));
}

#[test]
fn remove_tram_bits_clears_overlay_keeps_road() {
    use crate::road_type::{tram_road_type_from_tile, tram_track_bits};
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x05)).unwrap();
    apply_command(&mut s, &Command::PlaceTramBits(c, 0x05)).unwrap();
    apply_command(&mut s, &Command::RemoveTramBits(c)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Road);
    assert_eq!(tile.m5 & 0x0F, 0x05);
    assert_eq!(tram_track_bits(&tile), 0);
    assert_eq!(tram_road_type_from_tile(&tile), None);
}

#[test]
fn place_road_preserves_existing_tram_overlay() {
    use crate::road_type::{RoadType, tram_road_type_from_tile, tram_track_bits};
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceTramBits(c, 0x0A)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x05)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.m5 & 0x0F, 0x05);
    assert_eq!(tram_track_bits(&tile), 0x0A);
    assert_eq!(tram_road_type_from_tile(&tile), Some(RoadType::Tram));
}

#[test]
fn place_tram_on_existing_road_keeps_road_bits() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0A)).unwrap();
    apply_command(&mut s, &Command::PlaceTramBits(c, 0x05)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.m5 & 0x0F, 0x0A);
    assert_eq!(tile.m3 & 0x0F, 0x05);
}

#[test]
fn build_tram_at_depot_and_toggle_uses_tram_network() {
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::road_type::tram_track_bits;

    let mut s = GameState::new(10, 10);
    let depot = TileCoord::new(2, 2);
    let exit = TileCoord::new(1, 2);
    let mid = TileCoord::new(1, 3);
    let end = TileCoord::new(1, 4);
    apply_command(&mut s, &Command::PlaceRoad(exit)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    // Red de tranvía desde la boca hacia el sur.
    apply_command(&mut s, &Command::PlaceTramBits(exit, 0x05)).unwrap();
    apply_command(&mut s, &Command::PlaceTramBits(mid, 0x05)).unwrap();
    apply_command(&mut s, &Command::PlaceTramBits(end, 0x05)).unwrap();

    assert!(
        find_path(&s.map, exit, end, PathNetwork::Tram).is_some(),
        "pathfinder Tram debe conectar overlay m3"
    );

    apply_command(
        &mut s,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Tram),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].kind, VehicleKind::Tram);
    assert!(!s.vehicles[0].running);

    let id = s.vehicles[0].id;
    apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();
    assert!(s.vehicles[0].running);
    assert_ne!(s.vehicles[0].dest, depot);
    // Al salir, la boca debe tener overlay tram (ya lo tenía o se aseguró).
    assert_ne!(tram_track_bits(&s.map.get(exit).unwrap()), 0);
}

#[test]
fn place_road_waypoint_on_straight_road() {
    use crate::WAYPOINT_BUILD_COST;
    use crate::station::{STATION_TYPE_ROAD_WAYPOINT, StopKind, station_type_from_m6};

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0A)).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceRoadWaypoint(c)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Station);
    assert_eq!(station_type_from_m6(tile.m6), STATION_TYPE_ROAD_WAYPOINT);
    assert_eq!(tile.m3 & 0x0F, 0x0A);
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::RoadWaypoint);
    assert!(s.stations[0].is_waypoint());
    assert_eq!(s.economy.money, money - WAYPOINT_BUILD_COST);
}

#[test]
fn place_road_waypoint_rejects_crossing() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0F)).unwrap();
    assert_eq!(
        apply_command(&mut s, &Command::PlaceRoadWaypoint(c)),
        Err(CommandError::CannotPlaceWaypointOnTrack)
    );
}

#[test]
fn place_road_writes_newgrf_road_type_m8() {
    use crate::{RoadTramType, RoadType, RoadTypeDef, road_type_from_tile};
    let mut s = GameState::new(8, 8);
    let ngrf = RoadType::from_u8(2);
    s.road_type_catalog.push(RoadTypeDef {
        id: ngrf,
        class: RoadTramType::Road,
        label: "Adoquines".into(),
        short_label: "COBB".into(),
        intro_year: 0,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
        newgrf_type_tables: None,
    });
    s.current_road_type = ngrf;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRoad(c)).unwrap();
    assert_eq!(road_type_from_tile(&s.map.get(c).unwrap()), ngrf);
}
