//! Integración ligera: mismos comandos del core que aplicaría la toolbar sobre [`SimWorld`].

#![allow(clippy::expect_used)]

use std::collections::HashSet;

use openttdrs_core::Command;
use openttdrs_core::diag_dir_offset;
use openttdrs_core::prelude::*;

use super::SimWorld;

/// `DiagDirection` cuya entrada apunta desde `station` hacia `transport`.
fn entrance_dir_toward_neighbor(station: TileCoord, transport: TileCoord) -> u8 {
    let dir = (0..4).find(|&dir| {
        let (dx, dy) = diag_dir_offset(dir);
        station.x + dx == transport.x && station.y + dy == transport.y
    });
    assert!(
        dir.is_some(),
        "teselas ortogonalmente adyacentes: {station:?} / {transport:?}"
    );
    dir.unwrap_or(0)
}

fn first_tile_with_kind(sim: &SimWorld, kind: TileKind) -> Option<TileCoord> {
    let (mw, mh) = sim.state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if sim.state.map.get_kind(c) == Some(kind) {
                return Some(c);
            }
        }
    }
    None
}

/// Dos teselas de hierba ortogonalmente adyacentes, sin posición ya reservada en `state.stations`
/// (el bootstrap crea estaciones en hierba sin mutar el tile kind).
fn adjacent_grass_pair(sim: &SimWorld) -> Option<(TileCoord, TileCoord)> {
    let reserved: HashSet<TileCoord> = sim.state.stations.iter().map(|s| s.pos).collect();
    let (mw, mh) = sim.state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if reserved.contains(&c) || sim.state.map.get_kind(c) != Some(TileKind::Grass) {
                continue;
            }
            for (dx, dy) in [(0i32, -1), (1, 0), (0, 1), (-1, 0)] {
                let n = TileCoord::new(x as i32 + dx, y as i32 + dy);
                if reserved.contains(&n) {
                    continue;
                }
                if sim.state.map.get_kind(n) == Some(TileKind::Grass) {
                    return Some((c, n));
                }
            }
        }
    }
    None
}

#[test]
fn place_road_on_grass_matches_toolbar_command_stack() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoad(c)).is_ok(),
        "PlaceRoad sobre hierba (como la toolbar)"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Road));
}

#[test]
fn place_road_bits_autoroute_on_grass_uses_horizontal_axis() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    let bits = openttdrs_core::road_bits_for_autoroute(&sim.state.map, c);
    assert_eq!(bits, 0x0A);
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoadBits(c, bits)).is_ok(),
        "PlaceRoadBits con autoroute en hierba aislada"
    );
    assert_eq!(
        sim.state.map.get(c).expect("tesela colocada").m5 & 0x0F,
        0x0A
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Road));
}

#[test]
fn place_rail_on_grass_matches_toolbar_rail_tool() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRail(c)).is_ok(),
        "PlaceRail sobre hierba (BuildMenuAction::Rail)"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Rail));
}

#[test]
fn station_after_adjacent_road_matches_toolbar_station_tool() {
    let mut sim = SimWorld::default();
    let Some((station_tile, road_tile)) = adjacent_grass_pair(&sim) else {
        panic!("se esperan dos hierbas adyacentes para carretera + estación");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoad(road_tile)).is_ok(),
        "carretera en tesela vecina para estación"
    );
    let entrance_dir = entrance_dir_toward_neighbor(station_tile, road_tile);
    assert!(
        apply_command(
            &mut sim.state,
            &Command::PlaceStationDir(station_tile, entrance_dir)
        )
        .is_ok(),
        "PlaceStationDir con entrada hacia la carretera"
    );
    assert_eq!(
        sim.state.map.get_kind(station_tile),
        Some(TileKind::Station)
    );
}

#[test]
fn road_depot_dir_on_grass_matches_toolbar_depot_tool() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    let exit = TileCoord::new(c.x + 1, c.y);
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoad(exit)).is_ok(),
        "carretera en la boca del depósito"
    );
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoadDepotDir(c, 2)).is_ok(),
        "PlaceRoadDepotDir (Road depot + orientación)"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::RoadDepot));
}

#[test]
fn set_vehicle_station_orders_on_demo_truck_matches_toolbar() {
    use crate::state::bootstrap::{DEMO_ECONOMY_DELIVER_STATION, DEMO_ECONOMY_LOAD_STATION};

    let mut sim = SimWorld::default();
    let truck_id = 9010;
    assert!(
        apply_command(
            &mut sim.state,
            &Command::SetVehicleStationOrders(
                truck_id,
                vec![DEMO_ECONOMY_LOAD_STATION, DEMO_ECONOMY_DELIVER_STATION],
            )
        )
        .is_ok(),
        "SetVehicleStationOrders como el panel de órdenes"
    );
    let truck = sim
        .state
        .vehicles
        .iter()
        .find(|v| v.id == truck_id)
        .expect("camión demo");
    assert_eq!(truck.orders.len(), 2);
}

#[test]
fn place_bus_stop_matches_toolbar() {
    let mut sim = SimWorld::default();
    let Some((stop_tile, road_tile)) = adjacent_grass_pair(&sim) else {
        panic!("se esperan dos hierbas adyacentes para carretera + parada bus");
    };
    assert!(apply_command(&mut sim.state, &Command::PlaceRoad(road_tile)).is_ok());
    let entrance_dir = entrance_dir_toward_neighbor(stop_tile, road_tile);
    assert!(
        apply_command(
            &mut sim.state,
            &Command::PlaceBusStop(stop_tile, entrance_dir)
        )
        .is_ok(),
        "PlaceBusStop con entrada hacia la carretera"
    );
    assert_eq!(sim.state.map.get_kind(stop_tile), Some(TileKind::Station));
}

#[test]
fn place_rail_station_matches_toolbar() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    let rail = TileCoord::new(c.x + 1, c.y);
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRail(rail)).is_ok(),
        "vía vecina para estación de tren"
    );
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRailStation(c, 2)).is_ok(),
        "PlaceRailStation con entrada hacia la vía"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Station));
}

#[test]
fn build_road_vehicle_at_depot_matches_toolbar() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    let exit = TileCoord::new(c.x + 1, c.y);
    assert!(apply_command(&mut sim.state, &Command::PlaceRoad(exit)).is_ok());
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoadDepotDir(c, 2)).is_ok(),
        "depósito con boca hacia carretera"
    );
    let before = sim.state.vehicles.len();
    assert!(
        apply_command(
            &mut sim.state,
            &Command::BuildRoadVehicleAtDepot(c, openttdrs_core::VehicleKind::Truck)
        )
        .is_ok(),
        "BuildRoadVehicleAtDepot como panel depósito"
    );
    assert_eq!(sim.state.vehicles.len(), before + 1);
    assert!(!sim.state.vehicles.last().expect("vehículo nuevo").running);
}

#[test]
fn place_coal_mine_on_grass_matches_toolbar() {
    use openttdrs_core::{IndustryKind, IndustrySpec};

    let mut sim = SimWorld::default();
    let before = sim.state.industries.len();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(
            &mut sim.state,
            &Command::PlaceIndustrySpec(c, IndustrySpec::CoalMine)
        )
        .is_ok(),
        "PlaceIndustrySpec mina carbón"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Industry));
    assert_eq!(sim.state.industries.len(), before + 1);
    assert!(
        sim.state
            .industries
            .iter()
            .any(|i| i.pos == c && i.kind == IndustryKind::CoalMine),
        "mina de carbón en la tesela colocada"
    );
}

#[test]
fn set_vehicle_tile_orders_matches_order_panel() {
    let mut sim = SimWorld::default();
    let truck_id = 9010;
    let a = TileCoord::new(3, 6);
    let b = TileCoord::new(10, 6);
    assert!(
        apply_command(
            &mut sim.state,
            &Command::SetVehicleOrders(truck_id, vec![a, b])
        )
        .is_ok(),
        "SetVehicleOrders con teselas de carretera"
    );
    let truck = sim
        .state
        .vehicles
        .iter()
        .find(|v| v.id == truck_id)
        .expect("camión demo");
    assert_eq!(truck.orders.len(), 2);
}

#[test]
fn clone_vehicle_orders_copies_demo_truck_route() {
    let mut sim = SimWorld::default();
    use crate::state::bootstrap::{DEMO_ECONOMY_DELIVER_STATION, DEMO_ECONOMY_LOAD_STATION};

    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("hierba para depósito");
    };
    let exit = TileCoord::new(c.x + 1, c.y);
    assert!(apply_command(&mut sim.state, &Command::PlaceRoad(exit)).is_ok());
    assert!(apply_command(&mut sim.state, &Command::PlaceRoadDepotDir(c, 2)).is_ok());
    assert!(
        apply_command(
            &mut sim.state,
            &Command::BuildRoadVehicleAtDepot(c, openttdrs_core::VehicleKind::Truck)
        )
        .is_ok()
    );
    assert!(
        apply_command(
            &mut sim.state,
            &Command::BuildRoadVehicleAtDepot(c, openttdrs_core::VehicleKind::Truck)
        )
        .is_ok()
    );
    let ids: Vec<u32> = sim.state.vehicles.iter().map(|v| v.id).collect();
    let src = ids[ids.len() - 2];
    let dst = ids[ids.len() - 1];
    assert!(
        apply_command(
            &mut sim.state,
            &Command::SetVehicleStationOrders(
                src,
                vec![DEMO_ECONOMY_LOAD_STATION, DEMO_ECONOMY_DELIVER_STATION],
            )
        )
        .is_ok()
    );
    assert!(
        apply_command(
            &mut sim.state,
            &Command::CloneVehicleOrders {
                from_vehicle_id: src,
                to_vehicle_id: dst,
            }
        )
        .is_ok()
    );
    let dst_truck = sim
        .state
        .vehicles
        .iter()
        .find(|v| v.id == dst)
        .expect("segundo camión");
    assert_eq!(dst_truck.orders.len(), 2);
}

#[test]
fn clear_tile_after_road_restores_grass() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoad(c)).is_ok(),
        "PlaceRoad sobre hierba"
    );
    assert!(
        apply_command(&mut sim.state, &Command::ClearTile(c)).is_ok(),
        "ClearTile tras carretera"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Grass));
}
