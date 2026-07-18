//! Terreno bajo industrias — agua costera / plataformas (`DrawWaterClassGround`).

use super::{Map, Tile, TileCoord, TileKind, WaterClass, water_class_from_m1};

/// `GFX_OILRIG_1` … `GFX_OILRIG_5` en `industry_map.h`.
pub const GFX_OILRIG_FIRST: u16 = 24;
pub const GFX_OILRIG_LAST: u16 = 28;

/// OpenTTD `SPR_FLAT_BARE_LAND` (antes mal etiquetado como hierba).
pub const SPR_FLAT_BARE_LAND: u32 = 3924;
/// OpenTTD `SPR_FLAT_GRASS_TILE`.
pub const SPR_FLAT_GRASS_TILE: u32 = 3981;
/// OpenTTD `SPR_FLAT_WATER_TILE` — único suelo que dispara `DrawWaterClassGround`.
pub const SPR_FLAT_WATER_TILE: u32 = 4061;

/// Plataforma petrolera: siempre suelo de agua bajo la tesela.
#[must_use]
pub fn industry_gfx_is_oil_rig(gfx: u16) -> bool {
    (GFX_OILRIG_FIRST..=GFX_OILRIG_LAST).contains(&gfx)
}

/// OpenTTD `IsTileOnWater`: `WaterClass != Invalid` en tipos con clase de agua.
#[must_use]
pub fn industry_tile_on_water(tile: Tile) -> bool {
    tile.kind == TileKind::Industry && water_class_from_m1(tile.m1) != WaterClass::Invalid
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

/// Dibujar agua animada como base (`DrawWaterClassGround`).
///
/// Paridad OpenTTD `DrawTile_Industry`: agua solo si el sprite de suelo es
/// `SPR_FLAT_WATER_TILE` y `IsTileOnWater` (más oil-rig).
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
    ground_sprite_id == SPR_FLAT_WATER_TILE && map.get(c).is_some_and(industry_tile_on_water)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{Map, set_water_class_m1};

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
    fn water_sprite_on_sea_industry_uses_water() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Industry).unwrap();
        map.set_m1(c, set_water_class_m1(0, WaterClass::Sea))
            .unwrap();
        assert!(industry_uses_water_ground(&map, c, 0, SPR_FLAT_WATER_TILE));
    }

    #[test]
    fn water_sprite_on_land_industry_stays_dry() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Industry).unwrap();
        map.set_m1(c, set_water_class_m1(0, WaterClass::Invalid))
            .unwrap();
        assert!(!industry_uses_water_ground(&map, c, 0, SPR_FLAT_WATER_TILE));
    }

    #[test]
    fn zero_m1_is_sea_but_needs_water_sprite() {
        // Regresión: `m1=0` ⇒ Sea; sin sprite de agua no debe pintar WaterTile.
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Industry).unwrap();
        map.set_m1(c, 0).unwrap();
        assert!(industry_tile_on_water(map.get(c).unwrap()));
        assert!(!industry_uses_water_ground(&map, c, 0, 0));
        assert!(!industry_uses_water_ground(&map, c, 0, SPR_FLAT_BARE_LAND));
    }

    #[test]
    fn inland_construction_ground_stays_dry() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Industry).unwrap();
        map.set_m1(c, set_water_class_m1(0, WaterClass::Invalid))
            .unwrap();
        assert!(!industry_uses_water_ground(&map, c, 0, 0));
    }

    #[test]
    fn industry_with_sea_water_class_and_water_sprite() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Industry).unwrap();
        map.set_m1(c, set_water_class_m1(0x80, WaterClass::Sea))
            .unwrap();
        assert!(industry_uses_water_ground(&map, c, 0, SPR_FLAT_WATER_TILE));
        assert!(industry_tile_on_water(map.get(c).unwrap()));
    }
}
