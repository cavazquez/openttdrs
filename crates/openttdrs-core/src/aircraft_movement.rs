//! Movimiento mínimo de aviones en línea recta.

use crate::map::TileCoord;
use crate::vehicle::VehicleKind;

/// Ruta en línea recta (pasos Manhattan hacia el destino).
#[must_use]
pub fn straight_line_path(from: TileCoord, to: TileCoord) -> Vec<TileCoord> {
    if from == to {
        return vec![];
    }
    let mut path = Vec::new();
    let mut cur = from;
    while cur != to {
        let dx = to.x - cur.x;
        let dy = to.y - cur.y;
        cur = if dx.abs() >= dy.abs() {
            TileCoord::new(cur.x + dx.signum(), cur.y)
        } else {
            TileCoord::new(cur.x, cur.y + dy.signum())
        };
        path.push(cur);
    }
    path
}

#[must_use]
pub fn aircraft_requires_path(kind: VehicleKind) -> bool {
    kind == VehicleKind::Aircraft
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::GameState;
    use crate::engine::ENGINE_AIRCRAFT_DAKOTA;
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::{Command, TileCoord, TileKind, apply_command};

    use super::*;

    #[test]
    fn straight_line_path_is_direct() {
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(4, 2);
        let path = straight_line_path(from, to);
        assert_eq!(path.last().copied(), Some(to));
        assert_eq!(path.len(), 6);
    }

    #[test]
    fn air_pathfinder_ignores_terrain() {
        let s = GameState::new(8, 8);
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(5, 3);
        let path = find_path(&s.map, from, to, PathNetwork::Air).expect("ruta aérea");
        assert_eq!(path.last().copied(), Some(to));
    }

    #[test]
    fn aircraft_flies_straight_to_destination() {
        let mut s = GameState::new(16, 16);
        let airport = TileCoord::new(2, 2);
        s.map.set_kind(airport, TileKind::Airport).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(airport, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let dest = TileCoord::new(10, 6);
        s.vehicles[0].dest = dest;
        s.vehicles[0].running = true;
        s.vehicles[0].path = find_path(&s.map, airport, dest, PathNetwork::Air)
            .unwrap()
            .into();
        s.vehicles[0].set_cruise_speed();
        for _ in 0..800 {
            s.vehicles[0].step();
            if s.vehicles[0].pos == dest {
                break;
            }
        }
        assert_eq!(s.vehicles[0].pos, dest);
    }
}
