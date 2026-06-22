use bevy::prelude::*;
use openttdrs_core::{Map, Tile, TileCoord, TileKind};

use crate::iso::{iso, tile_slope_and_min_z};
use crate::render::MapTileChunk;

/// `true` si algún vecino del 8-neighborhood no es agua ni vacío (borde mar/tierra o río).
///
/// Los exports `.ottdmap` a veces dejan `m5=0` en toda el agua y se pierde
/// `WaterTileType::Coast` en bits 4–7; sin esto solo se pinta agua plana en la orilla.
fn water_tile_touches_land(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> bool {
    let is_land = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= mw as i32 || y >= mh as i32 {
            return false;
        }
        map.get(TileCoord::new(x, y))
            .is_some_and(|t| t.kind != TileKind::Water && t.kind != TileKind::Void)
    };
    let x = tx as i32;
    let y = ty as i32;
    const NEIGH8: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    NEIGH8.iter().any(|(dx, dy)| is_land(x + dx, y + dy))
}

#[derive(Clone, Copy)]
pub(crate) struct TileRenderInfo {
    pub(crate) tileh: u8,
    pub(crate) base_z: u8,
    pub(crate) use_shore: bool,
}

pub(crate) struct RenderGrid {
    width: u32,
    tiles: Vec<TileRenderInfo>,
}

impl RenderGrid {
    pub(crate) fn from_map(map: &Map, mw: u32, mh: u32) -> Self {
        let mut tiles = vec![
            TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
            (mw * mh) as usize
        ];

        for ty in 0..mh {
            for tx in 0..mw {
                let idx = (ty * mw + tx) as usize;
                let (tileh, base_z) = tile_slope_and_min_z(map, tx, ty);
                let c = TileCoord::new(tx as i32, ty as i32);
                let tile = map.get(c);
                let kind = tile.map_or(TileKind::Grass, |t| t.kind);
                let use_shore = if kind == TileKind::Water {
                    let m5_w = tile.map_or(0u8, |t| t.m5);
                    let mapt = tile.map_or(0u8, |t| t.mapt);
                    let water_tile_type = (m5_w >> 4) & 0x0F;
                    // OpenTTD solo dibuja `DrawShoreTile` en `WATER_TILE_COAST`
                    // (`water_cmd.cpp`); el agua lisa junto a tierra es agua plana.
                    // La heurística de vecinos queda solo para mapas generados sin
                    // MAPT (demo), donde `m5` no trae el tipo de agua.
                    water_tile_type == 1
                        || (mapt == 0
                            && water_tile_type == 0
                            && water_tile_touches_land(map, tx, ty, mw, mh))
                } else {
                    false
                };
                tiles[idx] = TileRenderInfo {
                    tileh,
                    base_z,
                    use_shore,
                };
            }
        }

        Self { width: mw, tiles }
    }

    fn get(&self, tx: u32, ty: u32) -> TileRenderInfo {
        self.tiles[(ty * self.width + tx) as usize]
    }
}

pub(crate) struct TileRenderContext {
    pub(crate) tx: u32,
    pub(crate) ty: u32,
    pub(crate) coord: TileCoord,
    pub(crate) tile: Option<Tile>,
    pub(crate) kind: TileKind,
    pub(crate) info: TileRenderInfo,
    pub(crate) iso_pos: Vec2,
}

impl TileRenderContext {
    pub(crate) fn new(map: &Map, grid: &RenderGrid, tx: u32, ty: u32) -> Self {
        let coord = TileCoord::new(tx as i32, ty as i32);
        let tile = map.get(coord);
        let kind = tile.map_or(TileKind::Grass, |t| t.kind);
        Self {
            tx,
            ty,
            coord,
            tile,
            kind,
            info: grid.get(tx, ty),
            iso_pos: iso(tx as i32, ty as i32),
        }
    }

    pub(crate) fn tx_i32(&self) -> i32 {
        self.tx as i32
    }

    pub(crate) fn ty_i32(&self) -> i32 {
        self.ty as i32
    }

    #[must_use]
    pub(crate) fn map_tile_chunk(&self) -> MapTileChunk {
        MapTileChunk::from_tile(self.tx, self.ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coord(x: i32, y: i32) -> TileCoord {
        TileCoord::new(x, y)
    }

    #[test]
    fn flat_grass_tiles_keep_base_height_without_shore() {
        let map = Map::new_flat(2, 2, 0);
        let grid = RenderGrid::from_map(&map, 2, 2);

        let info = grid.get(1, 1);

        assert_eq!(info.base_z, 0);
        assert!(!info.use_shore);
    }

    #[test]
    fn water_with_default_m5_uses_shore_when_touching_land() {
        let mut map = Map::new_flat(2, 1, 0);
        assert!(map.set_kind(coord(0, 0), TileKind::Water).is_ok());

        let grid = RenderGrid::from_map(&map, 2, 1);

        assert!(grid.get(0, 0).use_shore);
    }

    #[test]
    fn water_with_default_m5_stays_flat_when_surrounded_by_water() {
        let mut map = Map::new_flat(3, 3, 0);
        for y in 0..3 {
            for x in 0..3 {
                assert!(map.set_kind(coord(x, y), TileKind::Water).is_ok());
            }
        }

        let grid = RenderGrid::from_map(&map, 3, 3);

        assert!(!grid.get(1, 1).use_shore);
    }

    #[test]
    fn water_with_coast_m5_uses_shore_without_land_neighbors() {
        let mut map = Map::new_flat(3, 3, 0);
        for y in 0..3 {
            for x in 0..3 {
                assert!(map.set_kind(coord(x, y), TileKind::Water).is_ok());
            }
        }
        assert!(map.set_mapt_m5(coord(1, 1), 0x60, 0x10).is_ok());

        let grid = RenderGrid::from_map(&map, 3, 3);

        assert!(grid.get(1, 1).use_shore);
        assert!(!grid.get(0, 0).use_shore);
    }

    #[test]
    fn water_with_default_m5_uses_shore_when_touching_land_diagonally() {
        let mut map = Map::new_flat(3, 3, 0);
        for y in 0..3 {
            for x in 0..3 {
                assert!(map.set_kind(coord(x, y), TileKind::Water).is_ok());
            }
        }
        assert!(map.set_kind(coord(2, 2), TileKind::Grass).is_ok());

        let grid = RenderGrid::from_map(&map, 3, 3);

        assert!(grid.get(1, 1).use_shore);
    }

    /// Regresión ítem 12: en saves reales (MAPT presente) el agua lisa
    /// (`WATER_TILE_CLEAR`) junto a tierra NO dibuja orilla — solo las teselas
    /// `WATER_TILE_COAST`. La heurística de vecinos es solo para mapas sin MAPT.
    #[test]
    fn sav_plain_water_near_land_does_not_use_shore() {
        let mut map = Map::new_flat(3, 3, 0);
        for y in 0..3 {
            for x in 0..3 {
                assert!(map.set_kind(coord(x, y), TileKind::Water).is_ok());
                // MAPT con nibble alto MP_WATER, m5 = WATER_TILE_CLEAR.
                assert!(map.set_mapt_m5(coord(x, y), 0x60, 0x00).is_ok());
            }
        }
        assert!(map.set_kind(coord(2, 2), TileKind::Grass).is_ok());
        assert!(map.set_mapt_m5(coord(2, 2), 0x00, 0x00).is_ok());
        // La tesela (1,2) es costa real marcada en m5.
        assert!(map.set_mapt_m5(coord(1, 2), 0x60, 0x10).is_ok());

        let grid = RenderGrid::from_map(&map, 3, 3);

        assert!(!grid.get(1, 1).use_shore, "agua lisa diagonal a tierra");
        assert!(grid.get(1, 2).use_shore, "WATER_TILE_COAST marcada");
    }
}
