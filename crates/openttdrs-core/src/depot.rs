//! Consultas sobre depósitos en el mapa (sin lógica de UI).

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::VehicleKind;

/// Bit de reserva PBS en depósitos ferroviarios (`HasDepotReservation` / `m5` bit 4).
///
/// Coincide con el bit de cruces a nivel; la interpretación depende de `TileKind`.
pub const DEPOT_RESERVATION_M5_BIT: u8 = 1 << 4;

/// `HasDepotReservation` (`rail_map.h`): bit 4 de `m5` en `RailDepot`.
#[must_use]
pub fn has_depot_reservation(map: &Map, pos: TileCoord) -> bool {
    map.get(pos)
        .is_some_and(|t| t.kind == TileKind::RailDepot && t.m5 & DEPOT_RESERVATION_M5_BIT != 0)
}

/// `SetDepotReservation` (`rail_map.h`). Devuelve `true` si el mapa cambió.
pub fn set_depot_reservation(map: &mut Map, pos: TileCoord, reserved: bool) -> bool {
    let Some(mut tile) = map.get(pos) else {
        return false;
    };
    if tile.kind != TileKind::RailDepot {
        return false;
    }
    let had = tile.m5 & DEPOT_RESERVATION_M5_BIT != 0;
    if reserved {
        tile.m5 |= DEPOT_RESERVATION_M5_BIT;
    } else {
        tile.m5 &= !DEPOT_RESERVATION_M5_BIT;
    }
    if had == reserved {
        return false;
    }
    let _ = map.set_tile(pos, tile);
    true
}

/// Limpia reservas de depósito huérfanas tras importar un `.sav` (`afterload.cpp`).
pub fn clear_all_depot_reservations(map: &mut Map) {
    let (w, h) = map.dimensions();
    for y in 0..h.cast_signed() {
        for x in 0..w.cast_signed() {
            let c = TileCoord::new(x, y);
            let _ = set_depot_reservation(map, c, false);
        }
    }
}

/// Tesela de depósito compatible con el tipo de vehículo.
#[must_use]
pub fn depot_tile_kind_for_vehicle(kind: VehicleKind) -> TileKind {
    match kind {
        VehicleKind::Train => TileKind::RailDepot,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => TileKind::RoadDepot,
        VehicleKind::Ship => TileKind::ShipDepot,
        VehicleKind::Aircraft => TileKind::Airport,
    }
}

/// Depósito más cercano en distancia Manhattan desde `from`.
#[must_use]
pub fn nearest_depot_tile(map: &Map, from: TileCoord, kind: VehicleKind) -> Option<TileCoord> {
    let target = depot_tile_kind_for_vehicle(kind);
    let (mw, mh) = map.dimensions();
    let mut best: Option<(u32, TileCoord)> = None;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if map.get_kind(c) == Some(target) {
                let dist = from.x.abs_diff(c.x) + from.y.abs_diff(c.y);
                if best.is_none_or(|(d, _)| dist < d) {
                    best = Some((dist, c));
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Depósito alcanzable más cercano por pathfinding (road/tram).
#[must_use]
pub fn nearest_reachable_depot_tile(
    map: &Map,
    from: TileCoord,
    kind: VehicleKind,
) -> Option<TileCoord> {
    use crate::pathfinder::{PathNetwork, find_path};

    let target = depot_tile_kind_for_vehicle(kind);
    let network = if kind == VehicleKind::Tram {
        PathNetwork::Tram
    } else {
        PathNetwork::Road
    };
    let (mw, mh) = map.dimensions();
    let mut best: Option<(u32, TileCoord)> = None;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if map.get_kind(c) != Some(target) {
                continue;
            }
            let path_target = road_depot_entrance_tile(map, c).unwrap_or(c);
            if find_path(map, from, path_target, network).is_none() {
                continue;
            }
            let dist = from.x.abs_diff(c.x) + from.y.abs_diff(c.y);
            if best.is_none_or(|(d, _)| dist < d) {
                best = Some((dist, c));
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Boca del depósito de vía (`m5 & 3`) si la tesela es un depósito ferroviario.
#[must_use]
pub fn rail_depot_mouth_dir(map: &Map, pos: TileCoord) -> Option<u8> {
    map.get(pos)
        .filter(|t| t.kind == TileKind::RailDepot)
        .map(|t| t.m5 & 0x03)
}

/// Tesela de vía contigua a la boca del depósito (entrada/salida).
#[must_use]
pub fn rail_depot_entrance_tile(map: &Map, depot_pos: TileCoord) -> Option<TileCoord> {
    let mouth = rail_depot_mouth_dir(map, depot_pos)?;
    let ((dx, dy), _) = match mouth & 0x03 {
        0 => ((-1_i32, 0_i32), 0x01_u8),
        1 => ((0_i32, 1_i32), 0x02_u8),
        2 => ((1_i32, 0_i32), 0x01_u8),
        _ => ((0_i32, -1_i32), 0x02_u8),
    };
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some(c)
}

/// Depósito ferroviario cuya boca es `entrance` (tesela de vía vecina).
#[must_use]
pub fn rail_depot_for_entrance_tile(map: &Map, entrance: TileCoord) -> Option<TileCoord> {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let depot = TileCoord::new(entrance.x + dx, entrance.y + dy);
        if map.get_kind(depot) == Some(TileKind::RailDepot)
            && rail_depot_entrance_tile(map, depot) == Some(entrance)
        {
            return Some(depot);
        }
    }
    None
}

/// Boca del depósito road (`m5 & 3`) si la tesela corresponde a uno.
#[must_use]
pub fn road_depot_mouth_dir(map: &Map, pos: TileCoord) -> Option<u8> {
    map.get(pos)
        .filter(|t| t.kind == TileKind::RoadDepot)
        .map(|t| t.m5 & 0x03)
}

/// Tesela road contigua a la boca del depósito.
#[must_use]
pub fn road_depot_entrance_tile(map: &Map, depot_pos: TileCoord) -> Option<TileCoord> {
    let mouth = road_depot_mouth_dir(map, depot_pos)?;
    let (dx, dy) = match mouth {
        0 => (-1_i32, 0_i32),
        1 => (0_i32, 1_i32),
        2 => (1_i32, 0_i32),
        _ => (0_i32, -1_i32),
    };
    let entrance = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    (entrance.x >= 0
        && entrance.y >= 0
        && entrance.x < mw.cast_signed()
        && entrance.y < mh.cast_signed())
    .then_some(entrance)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::map::TileKind;
    use crate::{Command, GameState, apply_command};

    use super::*;

    #[test]
    fn nearest_depot_picks_closest_manhattan() {
        let mut s = GameState::new(12, 12);
        let near = TileCoord::new(4, 4);
        let far = TileCoord::new(9, 9);
        apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(4, 3))).unwrap();
        apply_command(&mut s, &Command::PlaceRoadDepotDir(near, 3)).unwrap();
        apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(9, 8))).unwrap();
        apply_command(&mut s, &Command::PlaceRoadDepotDir(far, 3)).unwrap();
        let from = TileCoord::new(3, 4);
        assert_eq!(
            nearest_depot_tile(&s.map, from, VehicleKind::Bus),
            Some(near)
        );
    }

    #[test]
    fn train_uses_rail_depot_only() {
        let mut s = GameState::new(8, 8);
        let road = TileCoord::new(1, 1);
        let rail = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
        apply_command(&mut s, &Command::PlaceRoadDepotDir(road, 3)).unwrap();
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(5, 4))).unwrap();
        apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
        assert_eq!(
            nearest_depot_tile(&s.map, TileCoord::new(0, 0), VehicleKind::Train),
            Some(rail)
        );
        assert_eq!(s.map.get_kind(road), Some(TileKind::RoadDepot));
    }

    #[test]
    fn depot_reservation_bit_roundtrips() {
        let mut s = GameState::new(8, 8);
        let rail = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(5, 4))).unwrap();
        apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
        assert!(!has_depot_reservation(&s.map, rail));
        assert!(set_depot_reservation(&mut s.map, rail, true));
        assert!(has_depot_reservation(&s.map, rail));
        assert!(set_depot_reservation(&mut s.map, rail, false));
        assert!(!has_depot_reservation(&s.map, rail));
        // Tesela no-depósito: no-op.
        assert!(!set_depot_reservation(
            &mut s.map,
            TileCoord::new(5, 4),
            true
        ));
    }
}
