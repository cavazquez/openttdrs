//! Cercas de vía (`DrawTrackDetails` / `SPR_TRACK_FENCE_*`).
//!
//! La decisión no se puede inferir mirando las teselas vecinas. OpenTTD la
//! persiste en el `RailGroundType` de `m3hi` y, después de aplicar una posible
//! fundación, adapta el sprite a la pendiente efectiva de la tesela.

use super::track_fence_meta_generated::TRACK_FENCE_SPRITE_META;

/// `RailGroundType` fence values (`rail_map.h`) — nibble bajo de `m3hi`
/// (= `m4` de OpenTTD).
pub(crate) const RAIL_GROUND_FENCE_NW: u8 = 2;
pub(crate) const RAIL_GROUND_FENCE_SE: u8 = 3;
pub(crate) const RAIL_GROUND_FENCE_SENW: u8 = 4;
pub(crate) const RAIL_GROUND_FENCE_NE: u8 = 5;
pub(crate) const RAIL_GROUND_FENCE_SW: u8 = 6;
pub(crate) const RAIL_GROUND_FENCE_NESW: u8 = 7;
pub(crate) const RAIL_GROUND_FENCE_VERT1: u8 = 8;
pub(crate) const RAIL_GROUND_FENCE_VERT2: u8 = 9;
pub(crate) const RAIL_GROUND_FENCE_HORIZ1: u8 = 10;
pub(crate) const RAIL_GROUND_FENCE_HORIZ2: u8 = 11;
pub(crate) const RAIL_GROUND_HALF_TILE_WATER: u8 = 13;

const SLOPE_W: u8 = 0x01;
const SLOPE_S: u8 = 0x02;
const SLOPE_E: u8 = 0x04;
const SLOPE_N: u8 = 0x08;
const SLOPE_HALFTILE: u8 = 0x20;

/// Esquina usada como referencia vertical por `FenceOffset`.
///
/// Conserva el orden de `Corner` de OpenTTD: W, S, E, N.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FenceHeightRef {
    W,
    S,
    E,
    N,
}

/// `FenceOffset` de `rail_cmd.cpp`.
#[derive(Clone, Copy)]
struct FenceOffset {
    height_ref: Option<FenceHeightRef>,
    ox: i32,
    oy: i32,
    ex: i32,
    ey: i32,
}

/// Las dieciséis entradas de `_fence_offsets` en `rail_cmd.cpp`.
const FENCE_OFFSETS: [FenceOffset; 16] = [
    FenceOffset {
        height_ref: None,
        ox: 0,
        oy: 1,
        ex: 16,
        ey: 1,
    }, // RFO_FLAT_X_NW
    FenceOffset {
        height_ref: None,
        ox: 1,
        oy: 0,
        ex: 1,
        ey: 16,
    }, // RFO_FLAT_Y_NE
    FenceOffset {
        height_ref: Some(FenceHeightRef::W),
        ox: 8,
        oy: 8,
        ex: 1,
        ey: 1,
    }, // LEFT
    FenceOffset {
        height_ref: Some(FenceHeightRef::N),
        ox: 8,
        oy: 8,
        ex: 1,
        ey: 1,
    }, // UPPER
    FenceOffset {
        height_ref: None,
        ox: 0,
        oy: 1,
        ex: 16,
        ey: 1,
    }, // SLOPE_SW_NW
    FenceOffset {
        height_ref: None,
        ox: 1,
        oy: 0,
        ex: 1,
        ey: 16,
    }, // SLOPE_SE_NE
    FenceOffset {
        height_ref: None,
        ox: 0,
        oy: 1,
        ex: 16,
        ey: 1,
    }, // SLOPE_NE_NW
    FenceOffset {
        height_ref: None,
        ox: 1,
        oy: 0,
        ex: 1,
        ey: 16,
    }, // SLOPE_NW_NE
    FenceOffset {
        height_ref: None,
        ox: 0,
        oy: 15,
        ex: 16,
        ey: 1,
    }, // RFO_FLAT_X_SE
    FenceOffset {
        height_ref: None,
        ox: 15,
        oy: 0,
        ex: 1,
        ey: 16,
    }, // RFO_FLAT_Y_SW
    FenceOffset {
        height_ref: Some(FenceHeightRef::E),
        ox: 8,
        oy: 8,
        ex: 1,
        ey: 1,
    }, // RIGHT
    FenceOffset {
        height_ref: Some(FenceHeightRef::S),
        ox: 8,
        oy: 8,
        ex: 1,
        ey: 1,
    }, // LOWER
    FenceOffset {
        height_ref: None,
        ox: 0,
        oy: 15,
        ex: 16,
        ey: 1,
    }, // SLOPE_SW_SE
    FenceOffset {
        height_ref: None,
        ox: 15,
        oy: 0,
        ex: 1,
        ey: 16,
    }, // SLOPE_SE_SW
    FenceOffset {
        height_ref: None,
        ox: 0,
        oy: 15,
        ex: 16,
        ey: 1,
    }, // SLOPE_NE_SE
    FenceOffset {
        height_ref: None,
        ox: 15,
        oy: 0,
        ex: 1,
        ey: 16,
    }, // SLOPE_NW_SW
];

/// Selección lógica y bounding box de una cerca, antes de convertirla a PNG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TrackFenceDraw {
    /// Índice relativo a `SPR_TRACK_FENCE_FLAT_X` (`0..8`).
    pub(crate) sprite_index: usize,
    pub(crate) bounds_origin: (i32, i32, i32),
    pub(crate) bounds_extent: (i32, i32, i32),
    height_ref: Option<FenceHeightRef>,
}

/// Rectángulo NFO del PNG activo (8bpp u OpenGFX2 32bpp).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TrackFenceSpriteMeta {
    pub(crate) width: i16,
    pub(crate) height: i16,
    pub(crate) xrel: i16,
    pub(crate) yrel: i16,
}

/// Metadatos de recorte para el sprite de cerca seleccionado.
#[must_use]
pub(crate) const fn track_fence_sprite_meta(sprite_index: usize) -> Option<TrackFenceSpriteMeta> {
    if sprite_index >= TRACK_FENCE_SPRITE_META.len() {
        return None;
    }
    let (width, height, xrel, yrel) = TRACK_FENCE_SPRITE_META[sprite_index];
    Some(TrackFenceSpriteMeta {
        width,
        height,
        xrel,
        yrel,
    })
}

const fn draw_for_rfo(rfo: usize) -> TrackFenceDraw {
    let offset = FENCE_OFFSETS[rfo];
    TrackFenceDraw {
        // El baseset vanilla tiene ocho sprites y OpenTTD hace `% num_sprites`.
        sprite_index: rfo % 8,
        bounds_origin: (offset.ox, offset.oy, 0),
        bounds_extent: (offset.ex, offset.ey, 4),
        height_ref: offset.height_ref,
    }
}

const fn rfo_nw(tileh: u8) -> usize {
    if tileh & (SLOPE_N | SLOPE_W) != 0 {
        if tileh & SLOPE_W != 0 { 4 } else { 6 }
    } else {
        0
    }
}

const fn rfo_se(tileh: u8) -> usize {
    if tileh & (SLOPE_S | SLOPE_E) != 0 {
        if tileh & SLOPE_S != 0 { 12 } else { 14 }
    } else {
        8
    }
}

const fn rfo_ne(tileh: u8) -> usize {
    if tileh & (SLOPE_N | SLOPE_E) != 0 {
        if tileh & SLOPE_E != 0 { 5 } else { 7 }
    } else {
        1
    }
}

const fn rfo_sw(tileh: u8) -> usize {
    if tileh & (SLOPE_S | SLOPE_W) != 0 {
        if tileh & SLOPE_S != 0 { 13 } else { 15 }
    } else {
        9
    }
}

fn half_tile_water_rfo(tileh: u8) -> Option<usize> {
    let corner = if tileh & SLOPE_HALFTILE != 0 {
        // `GetHalftileSlopeCorner`.
        (tileh >> 6) & 0x03
    } else {
        // `OppositeCorner(GetHighestSlopeCorner(ComplementSlope(tileh)))`.
        let missing_corner = match (tileh & 0x0F) ^ 0x0F {
            SLOPE_W => 0,
            SLOPE_S => 1,
            SLOPE_E => 2,
            SLOPE_N => 3,
            _ => return None,
        };
        missing_corner ^ 2
    };
    match corner {
        0 => Some(2),  // W / LEFT
        1 => Some(11), // S / LOWER
        2 => Some(10), // E / RIGHT
        3 => Some(3),  // N / UPPER
        _ => None,
    }
}

/// Cercas desde el `RailGroundType` almacenado en `m3hi`.
///
/// `tileh` es la pendiente efectiva *después* de `DrawFoundation`, igual que
/// el `TileInfo` que recibe `DrawTrackDetails` en OpenTTD.
#[must_use]
pub(crate) fn track_fence_draws_from_ground(ground: u8, tileh: u8) -> Vec<TrackFenceDraw> {
    match ground {
        RAIL_GROUND_FENCE_NW => vec![draw_for_rfo(rfo_nw(tileh))],
        RAIL_GROUND_FENCE_SE => vec![draw_for_rfo(rfo_se(tileh))],
        RAIL_GROUND_FENCE_SENW => vec![draw_for_rfo(rfo_nw(tileh)), draw_for_rfo(rfo_se(tileh))],
        RAIL_GROUND_FENCE_NE => vec![draw_for_rfo(rfo_ne(tileh))],
        RAIL_GROUND_FENCE_SW => vec![draw_for_rfo(rfo_sw(tileh))],
        RAIL_GROUND_FENCE_NESW => vec![draw_for_rfo(rfo_ne(tileh)), draw_for_rfo(rfo_sw(tileh))],
        RAIL_GROUND_FENCE_VERT1 => vec![draw_for_rfo(2)],
        RAIL_GROUND_FENCE_VERT2 => vec![draw_for_rfo(10)],
        RAIL_GROUND_FENCE_HORIZ1 => vec![draw_for_rfo(3)],
        RAIL_GROUND_FENCE_HORIZ2 => vec![draw_for_rfo(11)],
        RAIL_GROUND_HALF_TILE_WATER => {
            half_tile_water_rfo(tileh).map_or_else(Vec::new, |rfo| vec![draw_for_rfo(rfo)])
        }
        _ => Vec::new(),
    }
}

/// Cercas para la tesela: sólo las persistidas por OpenTTD, sin inferencia.
#[must_use]
pub(crate) fn track_fence_draws_for_tile(m3hi: u8, tileh: u8) -> Vec<TrackFenceDraw> {
    track_fence_draws_from_ground(m3hi & 0x0F, tileh)
}

/// `GetSlopePixelZInCorner(RemoveHalftileSlope(...), corner)` de OpenTTD.
///
/// Las alturas de esquina son discretas (0, 8 o 16 px), no un muestreo
/// sub-tesela: usar `partial_pixel_z(15, 0, ...)` perdería un pixel en varias
/// pendientes y desplazaría las cercas verticales.
#[must_use]
pub(crate) const fn track_fence_height_px(draw: TrackFenceDraw, tileh: u8) -> i32 {
    let Some(corner) = draw.height_ref else {
        return 0;
    };
    let slope = tileh & 0x1F; // `RemoveHalftileSlope`.
    let (bit, steep) = match corner {
        FenceHeightRef::W => (SLOPE_W, 0x1B),
        FenceHeightRef::S => (SLOPE_S, 0x17),
        FenceHeightRef::E => (SLOPE_E, 0x1E),
        FenceHeightRef::N => (SLOPE_N, 0x1D),
    };
    let mut z = if slope & bit != 0 { 8 } else { 0 };
    if slope == steep {
        z += 8;
    }
    z
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite_for(ground: u8, tileh: u8) -> TrackFenceDraw {
        let draws = track_fence_draws_from_ground(ground, tileh);
        assert_eq!(draws.len(), 1);
        draws[0]
    }

    #[test]
    fn slope_variants_match_draw_track_details() {
        // NW/SE usan los sprites X; NE/SW, los Y. Los ocho índices verifican
        // también el `% num_sprites` de `DrawTrackFence`.
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_NW, 0).sprite_index, 0);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_NW, SLOPE_W).sprite_index, 4);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_NW, SLOPE_N).sprite_index, 6);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_SE, 0).sprite_index, 0);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_SE, SLOPE_S).sprite_index, 4);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_SE, SLOPE_E).sprite_index, 6);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_NE, 0).sprite_index, 1);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_NE, SLOPE_E).sprite_index, 5);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_NE, SLOPE_N).sprite_index, 7);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_SW, 0).sprite_index, 1);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_SW, SLOPE_S).sprite_index, 5);
        assert_eq!(sprite_for(RAIL_GROUND_FENCE_SW, SLOPE_W).sprite_index, 7);
    }

    #[test]
    fn offsets_and_corner_heights_match_fence_offset_table() {
        let west = sprite_for(RAIL_GROUND_FENCE_VERT1, SLOPE_W);
        assert_eq!(west.bounds_origin, (8, 8, 0));
        assert_eq!(west.bounds_extent, (1, 1, 4));
        assert_eq!(track_fence_height_px(west, SLOPE_W), 8);
        assert_eq!(track_fence_height_px(west, 0x1B), 16);

        let east = sprite_for(RAIL_GROUND_FENCE_VERT2, SLOPE_E);
        assert_eq!(east.sprite_index, 2);
        assert_eq!(track_fence_height_px(east, SLOPE_E), 8);
        assert_eq!(track_fence_height_px(east, 0x1E), 16);

        let north = sprite_for(RAIL_GROUND_FENCE_HORIZ1, SLOPE_N);
        assert_eq!(north.sprite_index, 3);
        assert_eq!(track_fence_height_px(north, SLOPE_N), 8);
        assert_eq!(track_fence_height_px(north, 0x1D), 16);

        let south = sprite_for(RAIL_GROUND_FENCE_HORIZ2, SLOPE_S);
        assert_eq!(south.sprite_index, 3);
        assert_eq!(track_fence_height_px(south, SLOPE_S), 8);
        assert_eq!(track_fence_height_px(south, 0x17), 16);
    }

    #[test]
    fn half_tile_water_uses_the_same_corner_mapping_as_openttd() {
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0x20).sprite_index,
            2
        ); // W
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0x60).sprite_index,
            3
        ); // S
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0xA0).sprite_index,
            2
        ); // E
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0xE0).sprite_index,
            3
        ); // N
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0x07).sprite_index,
            3
        ); // WSE → S
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0x0B).sprite_index,
            2
        ); // NWS → W
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0x0D).sprite_index,
            3
        ); // ENW → N
        assert_eq!(
            sprite_for(RAIL_GROUND_HALF_TILE_WATER, 0x0E).sprite_index,
            2
        ); // SEN → E
    }

    #[test]
    fn non_fence_ground_never_invents_a_fence_from_neighbours() {
        assert!(track_fence_draws_for_tile(0, 0).is_empty());
        assert!(track_fence_draws_for_tile(1, SLOPE_W).is_empty());
        assert!(track_fence_draws_for_tile(12, SLOPE_E).is_empty());
    }

    #[test]
    fn metadata_has_one_entry_for_each_vanilla_fence_sprite() {
        assert!(track_fence_sprite_meta(0).is_some());
        assert!(track_fence_sprite_meta(7).is_some());
        assert!(track_fence_sprite_meta(8).is_none());
    }
}
