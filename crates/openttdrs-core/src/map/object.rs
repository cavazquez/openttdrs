//! Objetos de mapa (`MP_OBJECT` / `TileType::Object` en `OpenTTD`).

use super::Tile;

/// Nibble alto de `mapt` para teselas objeto.
pub const OTTD_MP_OBJECT: u8 = 10;

/// `mapt` con tipo objeto (bits 4–7 = 10).
pub const MP_OBJECT_MAPT: u8 = OTTD_MP_OBJECT << 4;

/// `ObjectType` en saves vanilla (`object_type.h`).
pub const OBJECT_TYPE_TRANSMITTER: u8 = 0;
pub const OBJECT_TYPE_LIGHTHOUSE: u8 = 1;
pub const OBJECT_TYPE_OWNED_LAND: u8 = 2;

#[must_use]
pub const fn is_map_object_tile(mapt: u8) -> bool {
    (mapt >> 4) & 0xF == OTTD_MP_OBJECT
}

#[must_use]
pub const fn object_type_from_tile(tile: &Tile) -> Option<u8> {
    if is_map_object_tile(tile.mapt) {
        Some(tile.m5)
    } else {
        None
    }
}

#[must_use]
pub const fn is_owned_land_tile(tile: &Tile) -> bool {
    is_map_object_tile(tile.mapt) && tile.m5 == OBJECT_TYPE_OWNED_LAND
}
