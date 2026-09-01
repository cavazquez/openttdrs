//! Agrupación de teselas `MP_INDUSTRY` — OpenTTD usa `m2()` como `IndustryID`.

use crate::map::{Tile, TileKind};

/// Clave de instancia de industria en una tesela (`GetIndustryIndex` → `m2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndustryTileLink {
    /// `m2` ≠ 0: ID de instancia en el array global de industrias.
    Instance(u16),
    /// Fixture legacy: `m1 = 0x80 | id` con `m2 = 0`.
    LegacyM1(u8),
    /// Sin ID en mapa: flood solo dentro del mismo componente anónimo.
    Anonymous,
}

/// `IndustryID` de la tesela (`m2` en OpenTTD).
#[must_use]
pub fn industry_instance_id(tile: &Tile) -> u16 {
    u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8)
}

#[must_use]
pub fn industry_tile_link(tile: &Tile) -> Option<IndustryTileLink> {
    if tile.kind != TileKind::Industry {
        return None;
    }
    let instance_id = industry_instance_id(tile);
    if instance_id != 0 {
        return Some(IndustryTileLink::Instance(instance_id));
    }
    let low = tile.m1 & 0x7F;
    if low != 0 && tile.m1 & 0x80 != 0 {
        return Some(IndustryTileLink::LegacyM1(low));
    }
    Some(IndustryTileLink::Anonymous)
}

/// ¿Dos teselas de industria pertenecen a la misma planta?
///
/// Con `Instance`/`LegacyM1` basta el ID. Con `Anonymous`, el caller debe
/// pasar `anonymous_same_group == true` cuando el criterio extra (p. ej. rango
/// de `gfx`) indica que son la misma planta.
#[must_use]
pub fn industry_tiles_mergeable(a: &Tile, b: &Tile, anonymous_same_group: bool) -> bool {
    let Some(la) = industry_tile_link(a) else {
        return false;
    };
    let Some(lb) = industry_tile_link(b) else {
        return false;
    };
    match (la, lb) {
        (IndustryTileLink::Instance(ia), IndustryTileLink::Instance(ib)) => ia == ib,
        (IndustryTileLink::LegacyM1(ia), IndustryTileLink::LegacyM1(ib)) => ia == ib,
        (IndustryTileLink::Anonymous, IndustryTileLink::Anonymous) => anonymous_same_group,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Map;
    use crate::map::TileCoord;

    fn industry_tile(m1: u8, m2: u8, m5: u8) -> Tile {
        let mut t = Map::new_flat(1, 1, 0)
            .get(TileCoord::new(0, 0))
            .expect("tile");
        t.kind = TileKind::Industry;
        t.m1 = m1;
        t.m2 = m2;
        t.m5 = m5;
        t
    }

    #[test]
    fn link_uses_m2_first() {
        let t = industry_tile(0x80, 5, 0);
        assert_eq!(industry_tile_link(&t), Some(IndustryTileLink::Instance(5)));
    }

    #[test]
    fn link_preserves_the_high_map2_byte_for_large_industry_ids() {
        let mut t = industry_tile(0x80, 0x05, 0);
        t.m2_hi = 0x01;
        assert_eq!(industry_instance_id(&t), 0x0105);
        assert_eq!(
            industry_tile_link(&t),
            Some(IndustryTileLink::Instance(0x0105))
        );
    }

    #[test]
    fn link_legacy_m1_when_m2_zero() {
        let t = industry_tile(0x83, 0, 1);
        assert_eq!(industry_tile_link(&t), Some(IndustryTileLink::LegacyM1(3)));
    }

    #[test]
    fn mergeable_same_m2_different_gfx() {
        let a = industry_tile(0x80, 2, 0);
        let b = industry_tile(0x80, 2, 1);
        assert!(industry_tiles_mergeable(&a, &b, false));
    }

    #[test]
    fn not_mergeable_different_m2() {
        let a = industry_tile(0x80, 2, 0);
        let b = industry_tile(0x80, 3, 0);
        assert!(!industry_tiles_mergeable(&a, &b, true));
    }
}
