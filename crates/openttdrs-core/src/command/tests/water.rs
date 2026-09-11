//! Tests de construcción acuática (depósito, muelle, boya, acueducto).

use crate::economy::{ship_depot_build_cost, ship_depot_clear_cost, station_build_cost};
use crate::test_fixtures::SandboxMap;
use crate::{
    Command, GameState, StopKind, TileCoord, TileKind, Vehicle, VehicleKind, WaterClass,
    apply_command, bridge_above_axis_from_mapt, command_would_fail, set_water_class_m1,
};

#[test]
fn place_ship_depot_on_water_with_water_entrance() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(3, 4); // dir 0 → (-1,0)
    let other = TileCoord::new(5, 4); // segunda parte de la huella.
    for coord in [depot, mouth, other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    let tile = s.map.get(depot).expect("depósito construido");
    let other_tile = s.map.get(other).expect("segunda parte construida");
    assert_eq!(tile.kind, TileKind::ShipDepot);
    assert_eq!(tile.mapt, 0x60, "MP_WATER conserva el tipo alto canónico");
    assert_eq!(tile.m5, 0x30, "WaterTileType::Depot vigente");
    assert_eq!(other_tile.kind, TileKind::ShipDepot);
    assert_eq!(other_tile.m5, 0x31, "parte opuesta sobre el eje X");
    assert_eq!(crate::depot::depot_id_from_tile(tile), Some(0));
    assert_eq!(crate::depot::depot_id_from_tile(other_tile), Some(0));
    assert_eq!(s.depots.len(), 1);
    assert_eq!(s.depots[0].depot_id, 0);
    assert_eq!(s.depots[0].tile, depot);
    assert_eq!(
        s.depots[0].build_date,
        crate::news::openttd_date_from_calendar_day_index(u64::from(s.calendar.date))
    );
    assert_eq!(
        s.economy.money,
        money - ship_depot_build_cost(&s.global_economy)
    );
}

#[test]
fn place_ship_depot_writes_current_raw_contract_and_active_owner() {
    let mut s = GameState::new(12, 12);
    s.ensure_rival_transcargo();
    let rival = crate::CompanyId(1);
    assert!(s.set_active_company(rival));

    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(4, 3); // dir 3 → norte en la grilla del mapa.
    let other = TileCoord::new(4, 5);
    for coord in [depot, mouth, other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let mut original = s.map.get(depot).expect("agua del depósito");
    original.mapt = 0x60 | 0x02; // MP_WATER + zona climática persistida.
    original.m1 = set_water_class_m1(original.m1 | 0x80, WaterClass::Canal);
    original.m2 = 0xFE;
    original.m2_hi = 0xAA;
    original.m3 = 0x81;
    original.m3hi = 0x91;
    original.m6 = 0xFC;
    original.m7 = 0xAB;
    original.m8 = 0xBEEF;
    s.map.set_tile(depot, original).unwrap();
    let mut other_original = s.map.get(other).expect("agua de la parte opuesta");
    other_original.mapt = 0x60 | 0x03;
    other_original.m1 = set_water_class_m1(other_original.m1, WaterClass::River);
    other_original.m6 = 0xFD;
    s.map.set_tile(other, other_original).unwrap();

    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 3)).unwrap();

    let tile = s.map.get(depot).expect("depósito construido");
    let other_tile = s.map.get(other).expect("parte opuesta construida");
    assert_eq!(tile.kind, TileKind::ShipDepot);
    assert_eq!(tile.mapt, 0x62, "se conserva la zona climática de MAPT");
    assert_eq!(tile.m5, 0x32, "tipo Depot + parte/eje de la orientación");
    assert_eq!(crate::depot::depot_id_from_tile(tile), Some(0));
    assert_eq!(crate::depot::depot_id_from_tile(other_tile), Some(0));
    assert_eq!(tile.m3, 0);
    assert_eq!(tile.m3hi, 0);
    assert_eq!(tile.m6, 0, "MakeShipDepot limpia el contador alto");
    assert_eq!(tile.m7, 0);
    assert_eq!(tile.m8, 0);
    assert_eq!(tile.m1 & 0x80, 0, "se limpia DockingTile");
    assert_eq!(
        crate::map::water_class(tile),
        Some(WaterClass::Canal),
        "SetTileOwner no debe destruir WaterClass"
    );
    assert_eq!(
        crate::CompanyId::from_tile_m1(tile.m1, s.companies.len()),
        rival,
        "el depósito queda a nombre de la compañía activa"
    );
    assert_eq!(other_tile.kind, TileKind::ShipDepot);
    assert_eq!(
        other_tile.mapt, 0x63,
        "cada parte conserva su zona climática"
    );
    assert_eq!(other_tile.m5, 0x33, "parte opuesta del eje Y");
    assert_eq!(crate::map::water_class(other_tile), Some(WaterClass::River));
    assert_eq!(other_tile.m6, 1);
    assert_eq!(
        crate::CompanyId::from_tile_m1(other_tile.m1, s.companies.len()),
        rival
    );
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
fn place_ship_depot_writes_both_native_parts_for_each_direction() {
    let cases = [
        (
            0,
            TileCoord::new(5, 5),
            TileCoord::new(4, 5),
            TileCoord::new(6, 5),
            0x30,
            0x31,
        ),
        (
            1,
            TileCoord::new(5, 5),
            TileCoord::new(5, 6),
            TileCoord::new(5, 4),
            0x33,
            0x32,
        ),
        (
            2,
            TileCoord::new(5, 5),
            TileCoord::new(6, 5),
            TileCoord::new(4, 5),
            0x31,
            0x30,
        ),
        (
            3,
            TileCoord::new(5, 5),
            TileCoord::new(5, 4),
            TileCoord::new(5, 6),
            0x32,
            0x33,
        ),
    ];

    for (dir, depot, mouth, other, depot_m5, other_m5) in cases {
        let mut s = GameState::new(12, 12);
        for coord in [depot, mouth, other] {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }

        apply_command(&mut s, &Command::PlaceShipDepotDir(depot, dir)).unwrap();

        assert_eq!(s.map.get(depot).unwrap().kind, TileKind::ShipDepot);
        assert_eq!(s.map.get(depot).unwrap().m5, depot_m5);
        assert_eq!(s.map.get(other).unwrap().kind, TileKind::ShipDepot);
        assert_eq!(s.map.get(other).unwrap().m5, other_m5);
    }
}

#[test]
fn depot_builders_share_native_pool_ids() {
    let mut s = GameState::new(16, 16);
    let road = TileCoord::new(2, 2);
    let rail = TileCoord::new(6, 2);
    let ship = TileCoord::new(10, 8);
    let ship_other = TileCoord::new(11, 8);

    s.map
        .set_kind(TileCoord::new(2, 1), TileKind::Road)
        .unwrap();
    s.map
        .set_kind(TileCoord::new(6, 1), TileKind::Rail)
        .unwrap();
    for coord in [ship, TileCoord::new(9, 8), ship_other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }

    apply_command(&mut s, &Command::PlaceRoadDepotDir(road, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceShipDepotDir(ship, 0)).unwrap();

    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(road).unwrap()),
        Some(0)
    );
    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(rail).unwrap()),
        Some(1)
    );
    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(ship).unwrap()),
        Some(2)
    );
    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(ship_other).unwrap()),
        Some(2)
    );
    assert_eq!(
        s.depots
            .iter()
            .map(|depot| depot.depot_id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(s.depots[0].tile, road);
    assert_eq!(s.depots[1].tile, rail);
    assert_eq!(s.depots[2].tile, ship);
}

#[test]
fn place_ship_depot_rejects_second_part_without_mutating_first() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(3, 4);
    let other = TileCoord::new(5, 4);
    for coord in [depot, mouth] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let money = s.economy.money;

    let error = apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap_err();

    assert_eq!(error, crate::CommandError::CannotPlaceStationOnOccupiedTile);
    assert_eq!(s.map.get_kind(depot), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(other), Some(TileKind::Grass));
    assert_eq!(s.economy.money, money);
}

#[test]
fn clear_ship_depot_from_either_section_restores_both_water_tiles() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(5, 5);
    let mouth = TileCoord::new(6, 5);
    let other = TileCoord::new(4, 5);
    for (coord, water_class) in [
        (depot, WaterClass::Canal),
        (mouth, WaterClass::Sea),
        (other, WaterClass::River),
    ] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
        let mut tile = s.map.get(coord).unwrap();
        tile.m1 = set_water_class_m1(tile.m1, water_class);
        s.map.set_tile(coord, tile).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 2)).unwrap();
    let money = s.economy.money;

    // `other` es la sección norte; se demuele desde el extremo opuesto al
    // que recibió el comando de construcción para cubrir ambos accesos.
    apply_command(&mut s, &Command::ClearTile(other)).unwrap();

    assert_eq!(s.map.get_kind(depot), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(other), Some(TileKind::Water));
    assert_eq!(
        crate::map::water_class(s.map.get(depot).unwrap()),
        Some(WaterClass::Canal)
    );
    assert_eq!(
        crate::map::water_class(s.map.get(other).unwrap()),
        Some(WaterClass::River)
    );
    assert_eq!(s.map.get(depot).unwrap().m5, 0);
    assert_eq!(s.map.get(other).unwrap().m5, 0);
    assert!(s.depots.is_empty());
    assert_eq!(
        s.economy.money,
        money - ship_depot_clear_cost(&s.global_economy)
    );
}

#[test]
fn clear_ship_depot_rejects_vehicle_on_the_other_section() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let other = TileCoord::new(5, 4);
    for coord in [depot, TileCoord::new(3, 4), other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Ship, other, other));
    let money = s.economy.money;

    assert_eq!(
        command_would_fail(&s, &Command::ClearTile(depot)),
        Some(crate::CommandError::VehicleInTheWay)
    );
    assert_eq!(
        apply_command(&mut s, &Command::ClearTile(depot)),
        Err(crate::CommandError::VehicleInTheWay)
    );
    assert_eq!(s.map.get_kind(depot), Some(TileKind::ShipDepot));
    assert_eq!(s.map.get_kind(other), Some(TileKind::ShipDepot));
    assert_eq!(s.economy.money, money);
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
    for x in 1..=10 {
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
