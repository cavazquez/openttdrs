//! Objetos de mapa (`MP_OBJECT` / `TileType::Object` en `OpenTTD`).

use super::{Map, Tile, TileCoord};
use crate::object_spec::{
    NEW_OBJECT_OFFSET, ObjectSpecDef, decode_object_tile_offset, encode_object_tile_offset,
    object_footprint_tile_index, object_spec_def,
};

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

/// `true` si `m5` es un id de objeto `NewGRF` (`≥` [`NEW_OBJECT_OFFSET`]).
#[must_use]
pub const fn is_newgrf_object_type(m5: u8) -> bool {
    (m5 as u16) >= NEW_OBJECT_OFFSET
}

#[must_use]
pub const fn object_type_from_tile(tile: &Tile) -> Option<u8> {
    if is_map_object_tile(tile.mapt) {
        Some(tile.m5)
    } else {
        None
    }
}

/// Id de spec `NewGRF` persistido en `m5`, si aplica.
#[must_use]
pub const fn object_spec_id_from_tile(tile: &Tile) -> Option<u16> {
    match object_type_from_tile(tile) {
        Some(m5) if is_newgrf_object_type(m5) => Some(m5 as u16),
        _ => None,
    }
}

#[must_use]
pub const fn is_owned_land_tile(tile: &Tile) -> bool {
    is_map_object_tile(tile.mapt) && tile.m5 == OBJECT_TYPE_OWNED_LAND
}

/// Dimensiones W×H del objeto (vanilla = 1×1).
#[must_use]
pub fn object_type_dims(object_type: u8, catalog: &[ObjectSpecDef]) -> (u8, u8) {
    if is_newgrf_object_type(object_type) {
        object_spec_def(catalog, u16::from(object_type))
            .map(|d| (d.size_width(), d.size_height()))
            .unwrap_or((1, 1))
    } else {
        (1, 1)
    }
}

/// Teselas del footprint con origen en `origin` y tamaño `w`×`h`.
#[must_use]
pub fn object_footprint_tiles(origin: TileCoord, w: u8, h: u8) -> Vec<TileCoord> {
    let mut out = Vec::with_capacity(usize::from(w).saturating_mul(usize::from(h)));
    for dy in 0..h {
        for dx in 0..w {
            out.push(TileCoord::new(
                origin.x.saturating_add(i32::from(dx)),
                origin.y.saturating_add(i32::from(dy)),
            ));
        }
    }
    out
}

/// Origen del objeto a partir de una tesela del footprint (`m2` = offset).
#[must_use]
pub fn object_origin_from_tile(tile: &Tile, at: TileCoord) -> Option<TileCoord> {
    let _ = object_type_from_tile(tile)?;
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    Some(TileCoord::new(
        at.x.saturating_sub(i32::from(dx)),
        at.y.saturating_sub(i32::from(dy)),
    ))
}

/// Todas las teselas del footprint del objeto que contiene `at`.
#[must_use]
pub fn object_footprint_at(
    map: &Map,
    at: TileCoord,
    catalog: &[ObjectSpecDef],
) -> Option<Vec<TileCoord>> {
    let tile = map.get(at)?;
    let object_type = object_type_from_tile(&tile)?;
    let origin = object_origin_from_tile(&tile, at)?;
    let (w, h) = object_type_dims(object_type, catalog);
    Some(object_footprint_tiles(origin, w, h))
}

/// Índice de vista Action3 para la tesela (`dy * width + dx`).
#[must_use]
pub fn object_view_index_for_tile(tile: &Tile, catalog: &[ObjectSpecDef]) -> Option<usize> {
    let object_type = object_type_from_tile(tile)?;
    let (w, _) = object_type_dims(object_type, catalog);
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    Some(object_footprint_tile_index(dx, dy, w))
}

/// Codifica offset de footprint en `m2` (reexport útil para build).
#[must_use]
pub const fn object_tile_offset_byte(dx: u8, dy: u8) -> u8 {
    encode_object_tile_offset(dx, dy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::sav;

    #[test]
    fn dual_fixture_overlays_transmitter_and_lighthouse_types() {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))
        .expect("fixture");
        let state = GameState::from_sav_game(sav::load(&raw).expect("load"));
        let tx = state
            .map
            .get(TileCoord::new(47, 33))
            .expect("transmitter tile");
        let lh = state
            .map
            .get(TileCoord::new(60, 55))
            .expect("lighthouse tile");
        assert_eq!(object_type_from_tile(&tx), Some(OBJECT_TYPE_TRANSMITTER));
        assert_eq!(object_type_from_tile(&lh), Some(OBJECT_TYPE_LIGHTHOUSE));
    }
}
