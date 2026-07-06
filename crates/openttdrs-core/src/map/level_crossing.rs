//! Cruces a nivel carretera–vía (`RoadTileType::Crossing` en `road_map.h`).

use super::{OTTD_MP_ROAD, TileKind};

/// `RoadTileType::Crossing` en bits 6–7 de `m5`.
#[must_use]
pub fn is_road_level_crossing(mapt: u8, m5: u8, kind: TileKind) -> bool {
    kind == TileKind::Road && (mapt >> 4) & 0xF == OTTD_MP_ROAD && ((m5 >> 6) & 0x3) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_road_rail_crossing() {
        assert!(is_road_level_crossing(
            OTTD_MP_ROAD << 4,
            1 << 6,
            TileKind::Road
        ));
    }

    #[test]
    fn rejects_plain_road() {
        assert!(!is_road_level_crossing(
            OTTD_MP_ROAD << 4,
            0,
            TileKind::Road
        ));
        assert!(!is_road_level_crossing(
            OTTD_MP_ROAD << 4,
            1 << 6,
            TileKind::Rail
        ));
    }
}
