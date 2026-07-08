//! Movimiento mínimo de barcos sobre teselas de agua.

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::{Vehicle, VehicleKind};

/// Tesela transitable por la red acuática (agua libre o depósito de barcos).
#[must_use]
pub fn is_water_network_tile(kind: TileKind) -> bool {
    matches!(kind, TileKind::Water | TileKind::ShipDepot)
}

/// Dos teselas de agua adyacentes están conectadas (sin road bits).
#[must_use]
pub fn water_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    map.get_kind(cur).is_some_and(is_water_network_tile)
        && map.get_kind(next).is_some_and(is_water_network_tile)
}

/// Barcos solo avanzan con ruta precalculada (como trenes).
#[must_use]
pub fn ship_requires_path(v: &Vehicle) -> bool {
    v.kind == VehicleKind::Ship
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::GameState;
    use crate::engine::ENGINE_SHIP_MPS;
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::{Command, TileCoord, Vehicle, VehicleKind, apply_command};

    use super::*;

    fn water_line(state: &mut GameState, y: i32, x0: i32, x1: i32) {
        for x in x0..=x1 {
            state
                .map
                .set_kind(TileCoord::new(x, y), TileKind::Water)
                .unwrap();
        }
    }

    #[test]
    fn water_path_follows_water_tiles() {
        let mut s = GameState::new(12, 12);
        water_line(&mut s, 5, 1, 8);
        let from = TileCoord::new(1, 5);
        let to = TileCoord::new(8, 5);
        let path = find_path(&s.map, from, to, PathNetwork::Water).expect("ruta acuática");
        assert_eq!(path.len(), 7);
        assert!(
            path.iter()
                .all(|c| { s.map.get_kind(*c) == Some(TileKind::Water) })
        );
    }

    #[test]
    fn ship_moves_along_water_path() {
        let mut s = GameState::new(12, 12);
        water_line(&mut s, 3, 0, 6);
        let depot = TileCoord::new(0, 3);
        s.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_SHIP_MPS),
        )
        .unwrap();
        let dest = TileCoord::new(6, 3);
        s.vehicles[0].dest = dest;
        s.vehicles[0].running = true;
        s.vehicles[0].path = find_path(&s.map, depot, dest, PathNetwork::Water)
            .unwrap()
            .into();
        s.vehicles[0].set_cruise_speed();
        for _ in 0..500 {
            s.vehicles[0].step();
            if s.vehicles[0].pos == dest {
                break;
            }
        }
        assert_eq!(s.vehicles[0].pos, dest);
    }

    #[test]
    fn ship_without_orders_moves_on_water() {
        let mut s = GameState::new(12, 6);
        for x in 0..6 {
            s.map
                .set_kind(TileCoord::new(x, 2), TileKind::Water)
                .unwrap();
        }
        let start = TileCoord::new(1, 2);
        let mut v = Vehicle::new(1, VehicleKind::Ship, start, start);
        v.running = true;
        v.set_cruise_speed();
        s.vehicles.push(v);
        for _ in 0..400 {
            s.step();
            if s.vehicles[0].pos != start {
                break;
            }
        }
        assert_ne!(s.vehicles[0].pos, start);
        assert_eq!(s.map.get_kind(s.vehicles[0].pos), Some(TileKind::Water));
    }

    #[test]
    fn ship_does_not_wander_off_water_without_path() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        v.running = true;
        v.set_cruise_speed();
        v.step();
        assert_eq!(v.pos, TileCoord::new(0, 0));
    }
}
