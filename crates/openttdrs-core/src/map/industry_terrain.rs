//! Terreno bajo industrias — agua costera / plataformas (`DrawWaterClassGround`).

use super::{Map, TileCoord, TileKind};

/// `GFX_OILRIG_1` … `GFX_OILRIG_5` en `industry_map.h`.
pub const GFX_OILRIG_FIRST: u16 = 24;
pub const GFX_OILRIG_LAST: u16 = 28;

/// OpenTTD `SPR_FLAT_GRASS_TILE` (suelo genérico en tabla industrial).
pub const SPR_FLAT_GRASS_TILE: u32 = 3924;

/// Plataforma petrolera: siempre suelo de agua bajo la tesela.
#[must_use]
pub fn industry_gfx_is_oil_rig(gfx: u16) -> bool {
    (GFX_OILRIG_FIRST..=GFX_OILRIG_LAST).contains(&gfx)
}

#[must_use]
pub fn tile_adjacent_to_water(map: &Map, c: TileCoord) -> bool {
    [(0, 1), (0, -1), (1, 0), (-1, 0)]
        .into_iter()
        .any(|(dx, dy)| {
            map.get(TileCoord::new(c.x + dx, c.y + dy))
                .is_some_and(|t| t.kind == TileKind::Water)
        })
}

/// Dibujar agua animada como base (`DrawWaterClassGround` simplificado).
#[must_use]
pub fn industry_uses_water_ground(
    map: &Map,
    c: TileCoord,
    gfx: u16,
    ground_sprite_id: u32,
) -> bool {
    if industry_gfx_is_oil_rig(gfx) {
        return true;
    }
    ground_sprite_id == SPR_FLAT_GRASS_TILE && tile_adjacent_to_water(map, c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    #[test]
    fn oil_rig_always_water_ground() {
        let map = Map::new_flat(3, 3, 0);
        assert!(industry_uses_water_ground(
            &map,
            TileCoord::new(1, 1),
            24,
            0
        ));
    }

    #[test]
    fn flat_grass_near_water_uses_water() {
        let mut map = Map::new_flat(3, 3, 0);
        map.set_kind(TileCoord::new(1, 0), TileKind::Water).unwrap();
        assert!(industry_uses_water_ground(
            &map,
            TileCoord::new(1, 1),
            0,
            SPR_FLAT_GRASS_TILE
        ));
    }

    #[test]
    fn inland_flat_grass_stays_dry() {
        let map = Map::new_flat(3, 3, 0);
        assert!(!industry_uses_water_ground(
            &map,
            TileCoord::new(1, 1),
            0,
            SPR_FLAT_GRASS_TILE
        ));
    }
}
