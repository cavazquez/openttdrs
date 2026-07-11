//! Clase de agua OpenTTD (`WaterClass` en `water_map.h`: bits 5–6 de `m1`).

use super::{Map, Tile, TileCoord, TileKind, inclined_slope_direction, tile_slope_and_z};

/// Clase de agua (`WaterClass` en OpenTTD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WaterClass {
    Sea = 0,
    Canal = 1,
    River = 2,
    Invalid = 3,
}

impl WaterClass {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Sea,
            1 => Self::Canal,
            2 => Self::River,
            _ => Self::Invalid,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `HasTileWaterClass`: Water / Station / Industry / Object / Forest(trees).
#[must_use]
pub fn tile_has_water_class(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Water | TileKind::Station | TileKind::Industry | TileKind::Forest
    )
}

/// Lee `WaterClass` de bits 5–6 de `m1`.
#[must_use]
pub fn water_class_from_m1(m1: u8) -> WaterClass {
    WaterClass::from_u8((m1 >> 5) & 0x03)
}

/// Escribe `WaterClass` en bits 5–6 de `m1` (conserva el resto).
#[must_use]
pub fn set_water_class_m1(m1: u8, wc: WaterClass) -> u8 {
    (m1 & !(0x03 << 5)) | (wc.as_u8() << 5)
}

#[must_use]
pub fn water_class(tile: Tile) -> Option<WaterClass> {
    if !tile_has_water_class(tile.kind) {
        return None;
    }
    Some(water_class_from_m1(tile.m1))
}

#[must_use]
pub fn is_river_tile(tile: Tile) -> bool {
    tile.kind == TileKind::Water
        && (tile.m5 >> 4).trailing_zeros() >= 4
        && water_class_from_m1(tile.m1) == WaterClass::River
}

#[must_use]
pub fn is_canal_tile(tile: Tile) -> bool {
    tile.kind == TileKind::Water
        && (tile.m5 >> 4).trailing_zeros() >= 4
        && water_class_from_m1(tile.m1) == WaterClass::Canal
}

/// Río en pendiente inclinada: no navegable (hace falta esclusa), como OpenTTD.
#[must_use]
pub fn river_tile_is_ship_navigable(map: &Map, c: TileCoord) -> bool {
    let Some(tile) = map.get(c) else {
        return false;
    };
    if !is_river_tile(tile) {
        return true;
    }
    let Some((tileh, _)) = tile_slope_and_z(map, c) else {
        return false;
    };
    tileh == 0 || inclined_slope_direction(tileh).is_none()
}

/// Convierte la tesela en agua Clear con la clase dada (conserva altura).
pub fn make_water_tile(map: &mut Map, c: TileCoord, wc: WaterClass) -> Result<(), super::MapError> {
    let mut tile = map.get(c).ok_or(super::MapError::OutOfBounds)?;
    tile.kind = TileKind::Water;
    tile.mapt = 0x60;
    tile.m5 = 0; // WaterTileType::Clear
    tile.m1 = set_water_class_m1(tile.m1, wc);
    map.set_tile(c, tile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_class_roundtrip_in_m1() {
        assert_eq!(water_class_from_m1(0), WaterClass::Sea);
        assert_eq!(
            water_class_from_m1(set_water_class_m1(0, WaterClass::Canal)),
            WaterClass::Canal
        );
        assert_eq!(
            water_class_from_m1(set_water_class_m1(0x1F, WaterClass::River)),
            WaterClass::River
        );
        assert_eq!(set_water_class_m1(0x9F, WaterClass::Sea) & 0x1F, 0x1F);
    }
}
