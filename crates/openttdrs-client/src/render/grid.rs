use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::iso::{iso, tile_slope_and_min_z};
use crate::render::MapTileChunk;
use crate::render::viewport::TileViewportBounds;

/// `true` si algún vecino del 8-neighborhood no es agua ni vacío (borde mar/tierra o río).
///
/// Los exports `.ottdmap` a veces dejan `m5=0` en toda el agua y se pierde
/// `WaterTileType::Coast` en bits 4–7; sin esto solo se pinta agua plana en la orilla.
fn water_tile_touches_land(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> bool {
    let is_land = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= mw as i32 || y >= mh as i32 {
            return false;
        }
        map.get(TileCoord::new(x, y)).is_some_and(|t| {
            !matches!(
                t.kind,
                TileKind::Water | TileKind::ShipDepot | TileKind::Void
            )
        })
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

fn tile_render_info_at(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> TileRenderInfo {
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
            || (mapt == 0 && water_tile_type == 0 && water_tile_touches_land(map, tx, ty, mw, mh))
    } else {
        false
    };
    TileRenderInfo {
        tileh,
        base_z,
        use_shore,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TileRenderInfo {
    pub(crate) tileh: u8,
    pub(crate) base_z: u8,
    pub(crate) use_shore: bool,
}

/// Rejilla de pendientes/orillas acotada a una región (viewport ± margen).
///
/// En mapas grandes no se materializa el mapa completo: solo la ventana pedida.
pub(crate) struct RenderGrid {
    tx0: u32,
    ty0: u32,
    /// Ancho de la región almacenada (`tx1 - tx0`).
    stride: u32,
    tiles: Vec<TileRenderInfo>,
}

impl RenderGrid {
    /// Mapa completo (tests y mapas pequeños).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_map(map: &Map, mw: u32, mh: u32) -> Self {
        Self::from_bounds(map, mw, mh, TileViewportBounds::full(mw, mh))
    }

    /// Solo la región `bounds` (expandir +1 en el caller si hace falta costa por vecinos).
    pub(crate) fn from_bounds(map: &Map, mw: u32, mh: u32, bounds: TileViewportBounds) -> Self {
        let tx0 = bounds.tx0.min(mw);
        let ty0 = bounds.ty0.min(mh);
        let tx1 = bounds.tx1.min(mw).max(tx0);
        let ty1 = bounds.ty1.min(mh).max(ty0);
        let stride = tx1.saturating_sub(tx0);
        let rows = ty1.saturating_sub(ty0);
        let len = usize::try_from(stride.saturating_mul(rows)).unwrap_or(0);
        let mut tiles = vec![
            TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
            len
        ];

        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                let idx = ((ty - ty0) * stride + (tx - tx0)) as usize;
                tiles[idx] = tile_render_info_at(map, tx, ty, mw, mh);
            }
        }

        Self {
            tx0,
            ty0,
            stride,
            tiles,
        }
    }

    fn get(&self, tx: u32, ty: u32) -> TileRenderInfo {
        let Some(ix) = tx.checked_sub(self.tx0) else {
            return TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
        };
        let Some(iy) = ty.checked_sub(self.ty0) else {
            return TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
        };
        if self.stride == 0 || ix >= self.stride {
            return TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
        }
        let idx = (iy * self.stride + ix) as usize;
        self.tiles.get(idx).copied().unwrap_or(TileRenderInfo {
            tileh: 0,
            base_z: 0,
            use_shore: false,
        })
    }
}

pub(crate) struct TileRenderContext {
    pub(crate) tx: u32,
    pub(crate) ty: u32,
    pub(crate) coord: TileCoord,
    pub(crate) tile: Option<Tile>,
    /// Tipo efectivo de `MP_OBJECT`, resuelto desde el pool importado `OBJS`
    /// cuando existe. `m5` queda disponible como byte crudo del ObjectID.
    pub(crate) object_type: Option<u16>,
    pub(crate) kind: TileKind,
    pub(crate) info: TileRenderInfo,
    pub(crate) iso_pos: Vec2,
}

impl TileRenderContext {
    pub(crate) fn new(map: &Map, grid: &RenderGrid, tx: u32, ty: u32) -> Self {
        let coord = TileCoord::new(tx as i32, ty as i32);
        let tile = map.get(coord);
        let object_type = map.object_type_at(coord);
        let kind = tile.map_or(TileKind::Grass, |t| t.kind);
        Self {
            tx,
            ty,
            coord,
            tile,
            object_type,
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

    #[test]
    fn from_bounds_only_materializes_region() {
        let map = Map::new_flat(64, 64, 0);
        let bounds = TileViewportBounds {
            tx0: 10,
            ty0: 10,
            tx1: 14,
            ty1: 14,
        };
        let grid = RenderGrid::from_bounds(&map, 64, 64, bounds);
        assert_eq!(grid.tiles.len(), 16);
        let _ = grid.get(10, 10);
        let _ = grid.get(13, 13);
    }
}
