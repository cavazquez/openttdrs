//! Integración ligera: mismos comandos del core que aplicaría la toolbar sobre [`SimWorld`].

#![allow(clippy::expect_used)]

use std::collections::HashSet;

use openttdrs_core::{Command, TileCoord, TileKind, apply_command, diag_dir_offset};

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
fn place_road_bits_full_cross_matches_toolbar_road_tool() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoadBits(c, 0x0F)).is_ok(),
        "PlaceRoadBits 0x0F es la herramienta Road de la toolbar"
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
    let mut sim = SimWorld::default();
    let truck_id = 9010;
    let load = TileCoord::new(3, 6);
    let deliver = TileCoord::new(10, 6);
    assert!(
        apply_command(
            &mut sim.state,
            &Command::SetVehicleStationOrders(truck_id, vec![load, deliver])
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
