//! Movimiento mínimo de barcos sobre teselas de agua.

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::{Vehicle, VehicleKind};

/// Ticks de espera al cruzar una esclusa.
pub const LOCK_TRANSIT_TICKS: u32 = 32;

/// Tesela transitable por la red acuática (agua libre, depósito o muelle).
#[must_use]
pub fn is_water_network_tile(kind: TileKind) -> bool {
    matches!(kind, TileKind::Water | TileKind::ShipDepot)
}

/// Incluye muelles y boyas (`StationType::Dock` = 4, `Buoy` = 6 en `m6`).
#[must_use]
pub fn is_water_network_tile_at(map: &Map, c: TileCoord) -> bool {
    let Some(tile) = map.get(c) else {
        return false;
    };
    if is_water_network_tile(tile.kind) {
        return crate::map::river_tile_is_ship_navigable(map, c);
    }
    if tile.kind != TileKind::Station {
        return false;
    }
    matches!((tile.m6 >> 3) & 0x0F, 4 | 6)
}

#[must_use]
fn tile_height(map: &Map, c: TileCoord) -> u8 {
    map.get(c).map_or(0, |t| t.height)
}

#[must_use]
pub fn water_tile_is_lock(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .filter(|t| t.kind == TileKind::Water)
        .is_some_and(|t| (t.m5 >> 4) & 0x0F == 2)
}

fn lock_axis_neighbors(c: TileCoord, axis_y: bool) -> (TileCoord, TileCoord) {
    if axis_y {
        (TileCoord::new(c.x, c.y - 1), TileCoord::new(c.x, c.y + 1))
    } else {
        (TileCoord::new(c.x - 1, c.y), TileCoord::new(c.x + 1, c.y))
    }
}

/// Índice de sprite Lock: 0=lower, 1=middle, 2=upper según altura vs vecinos.
#[must_use]
pub fn lock_sprite_level(map: &Map, c: TileCoord) -> usize {
    let Some(tile) = map.get(c) else {
        return 1;
    };
    let axis_y = tile.m5 & 1 != 0;
    let (a, b) = lock_axis_neighbors(c, axis_y);
    let ha = map.get(a).map_or(tile.height, |t| t.height);
    let hb = map.get(b).map_or(tile.height, |t| t.height);
    let hmin = ha.min(hb);
    let hmax = ha.max(hb);
    if tile.height <= hmin {
        0
    } else if tile.height >= hmax {
        2
    } else {
        1
    }
}

/// Dos teselas de agua adyacentes están conectadas.
/// Misma altura: siempre (si ambas son red acuática).
/// Distinta altura: solo si alguna es esclusa (Lock) y |Δh| == 1.
#[must_use]
pub fn water_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    if !is_water_network_tile_at(map, cur) || !is_water_network_tile_at(map, next) {
        return false;
    }
    let hc = tile_height(map, cur);
    let hn = tile_height(map, next);
    if hc == hn {
        return true;
    }
    if hc.abs_diff(hn) != 1 {
        return false;
    }
    // Cruce de nivel: al menos una tesela debe ser Lock.
    water_tile_is_lock(map, cur) || water_tile_is_lock(map, next)
}

/// Barcos solo avanzan con ruta precalculada (como trenes).
#[must_use]
pub fn ship_requires_path(v: &Vehicle) -> bool {
    v.kind == VehicleKind::Ship
}

/// Si el barco acaba de entrar en una esclusa, inicia espera de tránsito.
pub fn maybe_start_lock_transit(v: &mut Vehicle, map: &Map) {
    if v.kind != VehicleKind::Ship {
        return;
    }
    if water_tile_is_lock(map, v.pos) && v.wait_counter == 0 && v.cur_speed > 0 {
        v.wait_counter = LOCK_TRANSIT_TICKS;
        v.cur_speed = 0;
    }
}

/// Durante la espera en esclusa, decrementa y reanuda al terminar.
pub fn tick_ship_lock_wait(v: &mut Vehicle) -> bool {
    if v.kind != VehicleKind::Ship || v.wait_counter == 0 {
        return false;
    }
    v.wait_counter = v.wait_counter.saturating_sub(1);
    v.cur_speed = 0;
    if v.wait_counter == 0 {
        v.set_cruise_speed();
    }
    true
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

    #[test]
    fn lock_connects_different_heights() {
        let mut s = GameState::new(10, 6);
        let low = TileCoord::new(2, 2);
        let lock = TileCoord::new(3, 2);
        let high = TileCoord::new(4, 2);
        for c in [low, lock, high] {
            s.map.set_kind(c, TileKind::Water).unwrap();
        }
        s.map.set_height(low, 0).unwrap();
        s.map.set_height(lock, 0).unwrap();
        s.map.set_height(high, 1).unwrap();
        // Sin esclusa: no conecta.
        assert!(!water_tiles_connected(&s.map, low, high));
        assert!(!water_tiles_connected(&s.map, lock, high));
        apply_command(&mut s, &Command::PlaceLock(lock, false)).unwrap();
        assert!(water_tiles_connected(&s.map, lock, high));
        let path = find_path(&s.map, low, high, PathNetwork::Water);
        assert!(path.is_some());
    }

    #[test]
    fn different_height_without_lock_no_path() {
        let mut s = GameState::new(10, 6);
        let a = TileCoord::new(1, 2);
        let b = TileCoord::new(2, 2);
        s.map.set_kind(a, TileKind::Water).unwrap();
        s.map.set_kind(b, TileKind::Water).unwrap();
        s.map.set_height(a, 0).unwrap();
        s.map.set_height(b, 1).unwrap();
        assert!(find_path(&s.map, a, b, PathNetwork::Water).is_none());
    }
}
