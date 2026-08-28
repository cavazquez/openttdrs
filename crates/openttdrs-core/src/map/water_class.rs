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
    // `MakeWater`/`MakeRiver` de OpenTTD asigna OWNER_WATER (0x11) a
    // mares y ríos. Los canales conservan el dueño que ya haya seleccionado
    // el comando constructor.
    if matches!(wc, WaterClass::Sea | WaterClass::River) {
        tile.m1 = crate::company::OWNER_WATER_M1;
    }
    tile.m5 = 0; // WaterTileType::Clear
    tile.m1 = set_water_class_m1(tile.m1, wc);
    // `MakeWater` reinicia el estado de la tesela anterior. Conservar `m3`
    // o los planos altos de MAP2/MAP4 deja restos de suelo, estaciones o
    // industrias que no existen en el mapa de OpenTTD.
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    // `SB(m6, 2, 6, 0)`: los dos bits bajos no pertenecen a la parte que
    // reinicializa `MakeWater`.
    tile.m6 &= 0x03;
    tile.m7 = 0;
    tile.m8 = 0;
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

    #[test]
    fn make_water_reinitializes_the_raw_water_contract() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        let mut old = map.get(c).expect("tile");
        old.m1 = 0xFF;
        old.m2 = 0xAA;
        old.m2_hi = 0xBB;
        old.m3 = 0xCC;
        old.m3hi = 0xDD;
        old.m5 = 0xEE;
        old.m6 = 0xF3;
        old.m7 = 0x44;
        old.m8 = 0x5566;
        map.set_tile(c, old).expect("replace tile");

        make_water_tile(&mut map, c, WaterClass::Sea).expect("make sea");
        let water = map.get(c).expect("water");
        assert_eq!(water.kind, TileKind::Water);
        assert_eq!(water.mapt, 0x60);
        assert_eq!(water.m1, crate::company::OWNER_WATER_M1);
        assert_eq!(water.m2, 0);
        assert_eq!(water.m2_hi, 0);
        assert_eq!(water.m3, 0);
        assert_eq!(water.m3hi, 0);
        assert_eq!(water.m5, 0);
        assert_eq!(water.m6, 0x03);
        assert_eq!(water.m7, 0);
        assert_eq!(water.m8, 0);
    }
}
