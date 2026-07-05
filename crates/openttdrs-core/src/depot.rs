//! Consultas sobre depósitos en el mapa (sin lógica de UI).

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::VehicleKind;

/// Tesela de depósito compatible con el tipo de vehículo.
#[must_use]
pub fn depot_tile_kind_for_vehicle(kind: VehicleKind) -> TileKind {
    match kind {
        VehicleKind::Train => TileKind::RailDepot,
        VehicleKind::Bus | VehicleKind::Truck => TileKind::RoadDepot,
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

/// Boca del depósito de vía (`m5 & 3`) si la tesela es un depósito ferroviario.
#[must_use]
pub fn rail_depot_mouth_dir(map: &Map, pos: TileCoord) -> Option<u8> {
    map.get(pos)
        .filter(|t| t.kind == TileKind::RailDepot)
        .map(|t| t.m5 & 0x03)
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
}
